//! End-to-end check against the deployed API: health, submit, hidden until verified,
//! verify, served, manipulated solution rejected, rate limit, report. Deletes its test
//! rows afterwards, also when a step fails.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use ringside_cloud::client::{DEFAULT_BASE_URL, RetryPolicy};
use ringside_cloud::kinds::{Kind, PushFoldHu, PushFoldHuInput, PushFoldHuSolution, Verdict};
use ringside_cloud::operator::{Db, SqlValue, load_env, need, now, verify_pending};
use ringside_cloud::{Client, Error, Report, SubmitStatus};
use ringside_engine::range::HandClass;
use ringside_engine::solve_push_fold;

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
    let mut keys = Vec::new();
    let outcome = run(&base, &db, &mut keys);
    let cleanup = clean(&db, &keys, started).and(purge(&base, &api_key, &keys));
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

fn run(base: &str, db: &Db, keys: &mut Vec<String>) -> Result<(), String> {
    let client = Client::new(base);

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
    keys.extend([good_key.clone(), bad_key.clone()]);

    let r = client.submit_solution::<PushFoldHu>(&good_input, &good).map_err(|e| e.to_string())?;
    step("submit good", r == SubmitStatus::Submitted, format!("{r:?} key {good_key}"))?;
    let r = client.get_solution::<PushFoldHu>(&good_input).map_err(|e| e.to_string())?;
    step("unverified is hidden", r.is_none(), "404")?;
    let r = client.submit_solution::<PushFoldHu>(&bad_input, &bad).map_err(|e| e.to_string())?;
    step("submit manipulated (AA folds)", r == SubmitStatus::Submitted, format!("{r:?} key {bad_key}"))?;

    let checked = verify_pending(db, Some(keys.as_slice()), 10, false)?;
    let verdict = |k: &str| checked.iter().find(|c| c.key == k).and_then(|c| c.verdict.clone());
    let v = verdict(&good_key);
    step("verifier accepts good", matches!(v, Some(Verdict::Verified { .. })), format!("{v:?}"))?;
    let v = verdict(&bad_key);
    step("verifier rejects manipulated", matches!(v, Some(Verdict::Rejected { .. })), format!("{v:?}"))?;

    // Reads may hit a database replica that has not applied the verdict yet.
    let mut served = None;
    for _ in 0..15 {
        served = client.get_solution::<PushFoldHu>(&good_input).map_err(|e| e.to_string())?;
        if served.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
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

/// Drops this run's solution URLs from the CDN cache; only for a ringside-* host.
fn purge(base: &str, api_key: &str, keys: &[String]) -> Result<(), String> {
    let host = base.trim_start_matches("https://");
    if !host.starts_with("ringside-") || host.contains('/') {
        return Err(format!("refusing to purge {base}: not a ringside-* host"));
    }
    for key in keys {
        let url = format!("{base}/v1/solutions/{}/{key}", PushFoldHu::NAME);
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
         (SELECT count(*) FROM reports WHERE app_version = ?) AS n"
    );
    let mut args: Vec<SqlValue> = Vec::new();
    for _ in 0..2 {
        if keys.is_empty() {
            args.push(SqlValue::Null);
        }
        args.extend(keys.iter().map(|k| SqlValue::from(k.as_str())));
    }
    args.push(SMOKE_VERSION.into());
    let left = db.execute(&[(sql.as_str(), args)])?;
    let n = left[0].col(0, "n").as_i64().unwrap_or(-1);
    if n == 0 { Ok(()) } else { Err(format!("{n} rows left in solutions/submissions/reports")) }
}
