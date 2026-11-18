//! End-to-end check against the deployed API: health, submit, hidden until verified,
//! verify, served, manipulated solution rejected (for every kind, postflop included),
//! shared spots (9-max with limps, plain heads-up) through the community queue, rate
//! limit, report. Deletes its test rows afterwards, also when a step fails.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ringside_cloud::client::{DEFAULT_BASE_URL, RetryPolicy};
use ringside_cloud::kinds::pushfold_mw::reference;
use ringside_cloud::kinds::{
    Kind, ModelLine, Postflop, PostflopInput, PostflopSolution, PushFoldHu, PushFoldHuInput, PushFoldHuSolution,
    PushFoldMw, PushFoldMwInput, PushFoldMwSolution, SpotInput, SpotKind, SpotSolution, Verdict,
};
use ringside_cloud::operator::{Db, SqlValue, VerifyOptions, check_here, load_env, need, now, verify_pending};
use ringside_cloud::{Client, Error, KeyStatus, Report, SubmitStatus};
use ringside_engine::preflop_table::{self, Ante, Family};
use ringside_engine::range::HandClass;
use ringside_engine::solve_push_fold;
use ringside_solver::library::format::encode;
use ringside_solver::library::{SpotId, catalog, solve_spot};
use ringside_solver::{Settings, solve};

const HELP: &str = "\
Usage: ringside-smoke [--base URL] [--env-file PATH]

Runs the end-to-end check against the deployed Ringside Cloud API (default
https://ringside-api.bunny.run) and the database in .env, then deletes its test data and
purges its URLs from the CDN (needs BUNNY_DATABASE_URL, BUNNY_DATABASE_AUTH_TOKEN and
BUNNY_API_KEY).
It resets the current hour's rate-limit counters, so run it only for maintenance.";

const SMOKE_VERSION: &str = "0.0.0-smoke";

fn main() -> ExitCode {
    let mut base = DEFAULT_BASE_URL.to_string();
    let mut env_file: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--base" => base = args.next().unwrap_or_default(),
            "--env-file" => env_file = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!("{HELP}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument {other}; see --help");
                return ExitCode::FAILURE;
            }
        }
    }
    let setup = load_env(env_file.as_deref())
        .and_then(|env| Ok((Db::from_env(&env)?, need(&env, "BUNNY_API_KEY")?.to_string())));
    let (db, api_key) = match setup {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ringside-smoke: {e}");
            return ExitCode::FAILURE;
        }
    };
    let started = now();
    let mut keys: Vec<(&'static str, String)> = Vec::new();
    let outcome = run(&base, &db, &mut keys);
    let plain: Vec<String> = keys.iter().map(|(_, k)| k.clone()).collect();
    let cleanup = clean(&db, &plain, started).and(purge(&base, &api_key, &keys));
    match (&outcome, &cleanup) {
        (Ok(()), Ok(())) => {
            println!("smoke test passed, test data deleted");
            ExitCode::SUCCESS
        }
        _ => {
            if let Err(e) = outcome {
                eprintln!("FAILED: {e}");
            }
            if let Err(e) = cleanup {
                eprintln!("CLEANUP FAILED: {e}");
            }
            ExitCode::FAILURE
        }
    }
}

fn step(name: &str, ok: bool, detail: impl std::fmt::Display) -> Result<(), String> {
    if ok {
        println!("ok   {name}: {detail}");
        Ok(())
    } else {
        Err(format!("{name}: {detail}"))
    }
}

/// The verified solution, once the read replicas have the verdict (they can lag a minute).
fn served<K: Kind>(client: &Client, input: &K::Input) -> Result<Option<K::Solution>, String> {
    for _ in 0..45 {
        if let Some(s) = client.get_solution::<K>(input).map_err(|e| e.to_string())? {
            return Ok(Some(s));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Ok(None)
}

/// Checks only this run's keys, in this process.
fn verify(db: &Db, keys: &[(&str, String)]) -> Result<Vec<(String, Option<Verdict>)>, String> {
    let plain: Vec<String> = keys.iter().map(|(_, k)| k.clone()).collect();
    let opts = VerifyOptions { keys: Some(&plain), limit: 10, offset: 0, tried_since: None, deadline: None, dry_run: false };
    Ok(verify_pending(db, &opts, &check_here)?.into_iter().map(|c| (c.key, c.attempt.verdict())).collect())
}

fn run(base: &str, db: &Db, keys: &mut Vec<(&'static str, String)>) -> Result<(), String> {
    let client = &Client::new(base);

    let health = ureq::get(&format!("{base}/v1/health")).call().map_err(|e| e.to_string())?;
    let status = health.status();
    let body = health.into_string().map_err(|e| e.to_string())?;
    step("health", status == 200 && body.contains("\"status\":\"ok\""), format!("{status} {body}"))?;

    // A stack nobody else asks for, so this run's keys are its own.
    let stack_mbb = 5_000 + (now() % 20_000) as u32;
    let good_input = PushFoldHuInput { stack_mbb, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 };
    let bad_input = PushFoldHuInput { stack_mbb: stack_mbb + 1, ..good_input };
    let good = PushFoldHuSolution::from(&solve_push_fold(&good_input.params()).map_err(|e| e.to_string())?);
    let mut bad = PushFoldHuSolution::from(&solve_push_fold(&bad_input.params()).map_err(|e| e.to_string())?);
    bad.shove["AA".parse::<HandClass>().map_err(|e| e.to_string())?.index()] = 0.0;
    let good_key = PushFoldHu::key(&good_input).map_err(|e| e.to_string())?;
    let bad_key = PushFoldHu::key(&bad_input).map_err(|e| e.to_string())?;
    keys.extend([(PushFoldHu::NAME, good_key.clone()), (PushFoldHu::NAME, bad_key.clone())]);

    let r = client.submit_solution::<PushFoldHu>(&good_input, &good).map_err(|e| e.to_string())?;
    step("submit good", r == SubmitStatus::Submitted, format!("{r:?} key {good_key}"))?;
    let r = client.get_solution::<PushFoldHu>(&good_input).map_err(|e| e.to_string())?;
    step("unverified is hidden", r.is_none(), "404")?;
    let r = client.submit_solution::<PushFoldHu>(&bad_input, &bad).map_err(|e| e.to_string())?;
    step("submit manipulated (AA folds)", r == SubmitStatus::Submitted, format!("{r:?} key {bad_key}"))?;

    let checked = verify(db, keys)?;
    let verdict = |k: &str| checked.iter().find(|c| c.0 == k).and_then(|c| c.1.clone());
    let v = verdict(&good_key);
    step("verifier accepts good", matches!(v, Some(Verdict::Verified { .. })), format!("{v:?}"))?;
    let v = verdict(&bad_key);
    step("verifier rejects manipulated", matches!(v, Some(Verdict::Rejected { .. })), format!("{v:?}"))?;

    // Reads may hit a database replica that has not applied the verdict yet.
    let served = served::<PushFoldHu>(client, &good_input)?;
    step("verified is served", served.as_ref() == Some(&good), "200, locally re-checked")?;
    let again = ureq::get(&format!("{base}/v1/solutions/{}/{good_key}", PushFoldHu::NAME))
        .call()
        .map_err(|e| e.to_string())?;
    let cache = again.header("Cache-Control").unwrap_or("").to_string();
    step("served with immutable caching", cache.contains("immutable"), cache)?;
    let r = client.get_solution::<PushFoldHu>(&bad_input).map_err(|e| e.to_string())?;
    step("rejected stays hidden", r.is_none(), "404")?;
    let r = client.submit_solution::<PushFoldHu>(&good_input, &good).map_err(|e| e.to_string())?;
    step("resubmit verified", r == SubmitStatus::AlreadyVerified, format!("{r:?}"))?;
    let unknown = "0".repeat(64);
    let r = client.statuses(&[good_key.clone(), bad_key.clone(), unknown.clone()]).map_err(|e| e.to_string())?;
    let ok = r[0].1 == KeyStatus::Verified && matches!(r[1].1, KeyStatus::Rejected(Some(_))) && r[2].1 == KeyStatus::Unknown;
    step("status of many keys at once", ok, format!("{r:?}"))?;

    multiway(client, db, keys, stack_mbb)?;
    postflop(client, db, keys)?;
    // The hand analysis's tables with limps from every seat, and heads-up practice.
    shared_spot(client, db, keys, Family::Limps, 9)?;
    // The queue takes six requests a minute per client, fewer than two rounds make.
    db.execute(&[("DELETE FROM rate_limits WHERE bucket LIKE 'work-min:%' AND window_start >= ?", vec![(now() / 60 * 60).into()])])?;
    shared_spot(client, db, keys, Family::Plain, 2)?;

    let report = Report {
        app_version: SMOKE_VERSION.into(),
        error: "smoke test: unknown line".into(),
        hand: "PokerStars Hand #0: smoke test, no real data".into(),
    };
    client.report(&report).map_err(|e| e.to_string())?;
    step("report accepted", true, "202")?;
    let impatient = Client::with_policy(base, RetryPolicy { attempts: 1, ..RetryPolicy::default() });
    let mut limited = None;
    for i in 2..=20 {
        match impatient.report(&report) {
            Ok(()) => {}
            Err(Error::RateLimited { retry_after }) => {
                limited = Some((i, retry_after));
                break;
            }
            Err(e) => return Err(format!("report {i}: {e}")),
        }
    }
    step(
        "rate limit",
        matches!(limited, Some((_, Some(_)))),
        format!("{limited:?} (request number, Retry-After)"),
    )?;
    Ok(())
}

/// The same round for `pushfold-mw`: three players, ICM, heads-up pots, so the solve
/// takes about a second.
fn multiway(client: &Client, db: &Db, keys: &mut Vec<(&'static str, String)>, stack_mbb: u32) -> Result<(), String> {
    let good_input = PushFoldMwInput {
        stacks_mbb: vec![stack_mbb, 12_000, 9_000],
        sb_mbb: 500,
        ante_mbb: 0,
        bb_ante_mbb: 0,
        dead_mbb: 0,
        max_all_in: 2,
        payouts_ppm: vec![650_000, 350_000],
    };
    let bad_input = PushFoldMwInput { stacks_mbb: vec![stack_mbb + 1, 12_000, 9_000], ..good_input.clone() };
    let good = PushFoldMwSolution::from(&*reference(&good_input)?);
    let mut bad = PushFoldMwSolution::from(&*reference(&bad_input)?);
    bad.act[0]["AA".parse::<HandClass>().map_err(|e| e.to_string())?.index()] = 0.0;
    let good_key = PushFoldMw::key(&good_input).map_err(|e| e.to_string())?;
    let bad_key = PushFoldMw::key(&bad_input).map_err(|e| e.to_string())?;
    keys.extend([(PushFoldMw::NAME, good_key.clone()), (PushFoldMw::NAME, bad_key.clone())]);

    let r = client.submit_solution::<PushFoldMw>(&good_input, &good).map_err(|e| e.to_string())?;
    step("mw: submit good", r == SubmitStatus::Submitted, format!("{r:?} key {good_key}"))?;
    let r = client.submit_solution::<PushFoldMw>(&bad_input, &bad).map_err(|e| e.to_string())?;
    step("mw: submit manipulated (first seat folds AA)", r == SubmitStatus::Submitted, format!("{r:?}"))?;
    let checked = verify(db, &keys[keys.len() - 2..])?;
    let verdict = |k: &str| checked.iter().find(|c| c.0 == k).and_then(|c| c.1.clone());
    let v = verdict(&good_key);
    step("mw: verifier accepts good", matches!(v, Some(Verdict::Verified { .. })), format!("{v:?}"))?;
    let v = verdict(&bad_key);
    step("mw: verifier rejects manipulated", matches!(v, Some(Verdict::Rejected { .. })), format!("{v:?}"))?;
    let served = served::<PushFoldMw>(client, &good_input)?;
    step("mw: verified is served", served.as_ref() == Some(&good), "200, locally re-checked")?;
    let r = client.get_solution::<PushFoldMw>(&bad_input).map_err(|e| e.to_string())?;
    step("mw: rejected stays hidden", r.is_none(), "404")
}

/// The same round for `postflop`, on a catalog spot nobody has submitted yet. Inputs are
/// catalog spots only, so the manipulated solution goes first under the same key and is
/// then replaced by the good one, as the API allows for rejected rows.
fn postflop(client: &Client, db: &Db, keys: &mut Vec<(&'static str, String)>) -> Result<(), String> {
    let c = catalog();
    let scenario = c.scenario("3bp-btn-bb-40").ok_or("smoke scenario missing from the catalog")?;
    let offset = (now() % c.flops.len() as i64) as usize;
    let mut chosen = None;
    for k in 0..c.flops.len() {
        let flop = (offset + k) % c.flops.len();
        let input = PostflopInput::new(&c.scenarios[scenario].id, &c.flops[flop]);
        let key = Postflop::key(&input).map_err(|e| e.to_string())?;
        let rows = db.execute(&[("SELECT count(*) AS n FROM solutions WHERE key = ?", vec![key.as_str().into()])])?;
        if rows[0].col(0, "n").as_i64() == Some(0) {
            chosen = Some((SpotId { scenario, flop }, input, key));
            break;
        }
    }
    let (id, input, key) = chosen.ok_or("every smoke spot is taken")?;
    keys.push((Postflop::NAME, key.clone()));
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let memory = 8u64 << 30;
    let s = &c.scenarios[scenario];
    let flop = &c.flops[id.flop];

    // One iteration: a well-formed spot whose flop strategy is far from solved.
    let rough = Settings { target_pct: c.target_pct, max_iterations: 1, threads };
    let mut early = solve(&s.spot(flop), &rough, memory, |_| {}, || false).map_err(|e| e.to_string())?;
    let bad = PostflopSolution::from_stored(&encode(&mut early, c, s, flop).map_err(|e| e.to_string())?);
    let good_stored = solve_spot(c, id, threads, memory, |_| {}, || false).map_err(|e| e.to_string())?;
    let good = PostflopSolution::from_stored(&good_stored);

    let r = client.submit_solution::<Postflop>(&input, &bad).map_err(|e| e.to_string())?;
    step("pf: submit unsolved flop strategy", r == SubmitStatus::Submitted, format!("{r:?} {} {flop} key {key}", s.id))?;
    let checked = verify(db, &keys[keys.len() - 1..])?;
    let v = checked.first().and_then(|c| c.1.clone());
    step("pf: verifier rejects it", matches!(v, Some(Verdict::Rejected { exploitability: Some(e), .. }) if e > Postflop::TOLERANCE), format!("{v:?}"))?;
    let r = client.get_solution::<Postflop>(&input).map_err(|e| e.to_string())?;
    step("pf: rejected stays hidden", r.is_none(), "404")?;

    let r = client.submit_solution::<Postflop>(&input, &good).map_err(|e| e.to_string())?;
    step("pf: submit solved spot (replaces the rejected row)", r == SubmitStatus::Submitted, format!("{r:?}"))?;
    let r = client.get_solution::<Postflop>(&input).map_err(|e| e.to_string())?;
    step("pf: unverified is hidden", r.is_none(), "404")?;
    let checked = verify(db, &keys[keys.len() - 1..])?;
    let v = checked.first().and_then(|c| c.1.clone());
    let rebuilt = matches!(&v, Some(Verdict::Verified { result: Some(_), .. }));
    step("pf: verifier accepts it and stores its own rebuild", rebuilt, format!("{:?}", v.as_ref().map(|v| match v {
        Verdict::Verified { exploitability, .. } => format!("verified {exploitability:.3} %"),
        Verdict::Rejected { reason, .. } => format!("rejected: {reason}"),
    })))?;
    let served = served::<Postflop>(client, &input)?;
    let served = served.ok_or("pf: verified spot is not served")?;
    let stored = served.stored(&input)?;
    let same_flop = stored.nodes.iter().zip(&good_stored.nodes).all(|(a, b)| a.strategy == b.strategy)
        && stored.nodes.len() == good_stored.nodes.len();
    step("pf: served spot reads and keeps the submitted flop strategy", same_flop, format!("{} bytes", served.data.len()))
}

/// A spot off the catalog: requested, handed out by the queue (and again once its lease
/// ran out), solved on a suit renaming of its board, submitted, verified and served.
fn shared_spot(client: &Client, db: &Db, keys: &mut Vec<(&'static str, String)>, family: Family, players: usize) -> Result<(), String> {
    let (ante, stack) = (Ante::BigBlind, 40.0);
    let params = family.params(ante, players, stack);
    let sol = preflop_table::lookup(&params).ok_or("model table missing")?;
    // The 3-bet pot with the fewest combos solves in seconds.
    let sc = ringside_play::scenario::scenarios(&sol, &params)
        .into_iter()
        .filter(|s| s.pot_type == ringside_play::scenario::PotType::ThreeBet)
        .min_by(|a, b| a.reach.total_cmp(&b.reach))
        .ok_or("no 3-bet pot in the model")?;
    let model = ModelLine { family, ante, players, stack_bb: stack, line: sc.line.clone() };
    let c = catalog();
    let offset = (now() % c.flops.len() as i64) as usize;
    let mut chosen = None;
    for k in 0..c.flops.len() {
        // Spades and diamonds swapped: the app solves the board as dealt.
        let local: String = c.flops[(offset + k) % c.flops.len()].chars().map(|ch| match ch {
            's' => 'd',
            'd' => 's',
            x => x,
        }).collect();
        let input = SpotInput::from_model(&model, &sc.spot(&local))?;
        let key = SpotKind::key(&input).map_err(|e| e.to_string())?;
        let rows = db.execute(&[(
            "SELECT (SELECT count(*) FROM solutions WHERE key = ?) + (SELECT count(*) FROM work WHERE key = ?) AS n",
            vec![key.as_str().into(), key.as_str().into()],
        )])?;
        if rows[0].col(0, "n").as_i64() == Some(0) {
            chosen = Some((local, input, key));
            break;
        }
    }
    let (local, input, key) = chosen.ok_or("every smoke spot is taken")?;
    keys.push((SpotKind::NAME, key.clone()));
    step("spot: local board and canonical key", input.board != local, format!("{family:?} {players} players, {local} -> {} key {key}", input.board))?;

    let status = client.request_work::<SpotKind>(&input).map_err(|e| e.to_string())?;
    step("queue: request", status == "queued", &status)?;
    // Ahead of everything real, so the next hand-out is this one.
    db.execute(&[("UPDATE work SET priority = 1000000000 WHERE key = ?", vec![key.as_str().into()])])?;
    let kinds = [(SpotKind::NAME, SpotKind::ENGINE_VERSION)];
    let mut handed: Vec<String> = Vec::new();
    let first = client.next_work(&kinds).map_err(|e| e.to_string())?;
    handed.extend(first.iter().map(|w| w.key.clone()));
    let leased = first.as_ref().is_some_and(|w| w.key == key && w.lease_until > now() as u64);
    step("queue: hands out the requested spot with a lease", leased, format!("{:?}", first.as_ref().map(|w| (&w.kind, w.lease_until))))?;
    let second = client.next_work(&kinds).map_err(|e| e.to_string())?;
    handed.extend(second.iter().map(|w| w.key.clone()));
    let result = (|| {
        step("queue: a leased spot goes to nobody else", second.as_ref().is_none_or(|w| w.key != key), "other or none")?;
        db.execute(&[("UPDATE work SET lease_until = ? WHERE key = ?", vec![(now() - 1).into(), key.as_str().into()])])?;
        let third = client.next_work(&kinds).map_err(|e| e.to_string())?;
        step("queue: after the lease ran out it is handed out again", third.as_ref().is_some_and(|w| w.key == key), "same key")?;
        client.release_work(&key).map_err(|e| e.to_string())?;
        let again = client.next_work(&kinds).map_err(|e| e.to_string())?;
        step("queue: a given-back lease is handed out again at once", again.as_ref().is_some_and(|w| w.key == key), "same key")?;

        let catalog_of = input.catalog(Some(&local))?;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let local_id = SpotId { scenario: 0, flop: catalog_of.flop(&local).ok_or("local board")? };
        let stored = solve_spot(&catalog_of, local_id, threads, 8 << 30, |_| {}, || false).map_err(|e| e.to_string())?;
        let shared = stored.renamed(&catalog_of.scenarios[0], &input.board).map_err(|e| e.to_string())?;
        let r = client.submit_solution::<SpotKind>(&input, &SpotSolution::from_stored(&shared)).map_err(|e| e.to_string())?;
        step("spot: submit", r == SubmitStatus::Submitted, format!("{r:?}"))?;
        db.execute(&[("UPDATE work SET lease_until = NULL WHERE key = ?", vec![key.as_str().into()])])?;
        let next = client.next_work(&kinds).map_err(|e| e.to_string())?;
        step("queue: a submitted spot is not handed out", next.as_ref().is_none_or(|w| w.key != key), "skipped")?;
        let r = client.get_solution::<SpotKind>(&input).map_err(|e| e.to_string())?;
        step("spot: unverified is hidden", r.is_none(), "404")?;
        let checked = verify(db, &keys[keys.len() - 1..])?;
        let v = checked.first().and_then(|c| c.1.clone());
        step("spot: verifier rebuilds the ranges and accepts it", matches!(v, Some(Verdict::Verified { .. })), format!("{v:?}").chars().take(160).collect::<String>())?;
        let served = served::<SpotKind>(client, &input)?;
        let served = served.ok_or("spot: verified spot is not served")?;
        let back = served.stored(&input)?.renamed(&catalog_of.scenarios[0], &local).map_err(|e| e.to_string())?;
        let same = back.nodes.len() == stored.nodes.len() && back.nodes.iter().zip(&stored.nodes).all(|(a, b)| a.strategy == b.strategy);
        step("spot: served spot renamed to the local board keeps the flop strategy", same, format!("{} bytes", served.data.len()))
    })();
    // Leases this run took from real queue entries go back.
    for k in handed.iter().filter(|k| **k != key) {
        db.execute(&[(
            "UPDATE work SET lease_until = NULL, leased_to = NULL, leases = max(leases - 1, 0) WHERE key = ?",
            vec![k.as_str().into()],
        )])?;
    }
    result
}

/// Drops this run's solution URLs from the CDN cache; only for a ringside-* host.
fn purge(base: &str, api_key: &str, keys: &[(&str, String)]) -> Result<(), String> {
    let host = base.trim_start_matches("https://");
    if !host.starts_with("ringside-") || host.contains('/') {
        return Err(format!("refusing to purge {base}: not a ringside-* host"));
    }
    for (kind, key) in keys {
        let url = format!("{base}/v1/solutions/{kind}/{key}");
        ureq::post("https://api.bunny.net/purge")
            .set("AccessKey", api_key)
            .query("url", &url)
            .call()
            .map_err(|e| format!("purge {url}: {}", e.kind()))?;
    }
    println!("cdn: purged {} solution URL(s)", keys.len());
    Ok(())
}

fn clean(db: &Db, keys: &[String], started: i64) -> Result<(), String> {
    let mut stmts: Vec<(&str, Vec<SqlValue>)> = Vec::new();
    for k in keys {
        stmts.push(("DELETE FROM solutions WHERE key = ?", vec![k.as_str().into()]));
        stmts.push(("DELETE FROM submissions WHERE key = ?", vec![k.as_str().into()]));
        // Only the queue entry this run requested; a catalog spot it used stays queued.
        stmts.push(("DELETE FROM work WHERE key = ? AND source = 'request'", vec![k.as_str().into()]));
    }
    stmts.push(("DELETE FROM reports WHERE app_version = ?", vec![SMOKE_VERSION.into()]));
    stmts.push(("DELETE FROM rate_limits WHERE window_start >= ?", vec![(started / 3600 * 3600).into()]));
    let r = db.execute(&stmts)?;
    let deleted: u64 = r.iter().map(|r| r.affected).sum();
    println!("cleanup: {deleted} rows deleted");
    let marks = vec!["?"; keys.len().max(1)].join(",");
    let sql = format!(
        "SELECT (SELECT count(*) FROM solutions WHERE key IN ({marks})) + \
         (SELECT count(*) FROM submissions WHERE key IN ({marks})) + \
         (SELECT count(*) FROM work WHERE key IN ({marks}) AND source = 'request') + \
         (SELECT count(*) FROM reports WHERE app_version = ?) AS n"
    );
    let mut args: Vec<SqlValue> = Vec::new();
    for _ in 0..3 {
        if keys.is_empty() {
            args.push(SqlValue::Null);
        }
        args.extend(keys.iter().map(|k| SqlValue::from(k.as_str())));
    }
    args.push(SMOKE_VERSION.into());
    let left = db.execute(&[(sql.as_str(), args)])?;
    let n = left[0].col(0, "n").as_i64().unwrap_or(-1);
    if n == 0 { Ok(()) } else { Err(format!("{n} rows left in solutions/submissions/work/reports")) }
}
