//! The app's whole way headless, for checking it on real hand histories:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin app-pipeline -- --db <file> [--import <dir>...] [--solve THREADS --minutes N] [--out <file.json>]
//! ```
//!
//! Imports like the importer (`import_file`, `import_summary`), analyses like the preflop
//! batches (`analyze_raw`, `save_analysis`), solves the postflop queue like the app
//! (`next_postflop_job`, `estimate_rest`, `finish_postflop`) until `--minutes` are used up,
//! and writes what the pages read: sessions with their review, tournaments with their
//! report, the profile, leaks, „Was als Nächstes?“ and the Spielwertung. The database is
//! the checkpoint: a second run with the same `--db` goes on where the last one stopped.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ringside_analysis::{ENGINE, PostflopJob, analyze_raw, estimate_rest, solve_job};
use ringside_core::db::{Db, Filter};
use ringside_core::import::{import_file, import_summary};
use ringside_core::leaks;
use ringside_core::next::{Inputs, rank};
use ringside_core::profile::profile;
use ringside_core::session::LISTED_SESSIONS;
use ringside_core::skill;

fn files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for e in std::fs::read_dir(path).expect("read dir").flatten() {
            files(&e.path(), out);
        }
    } else if path.extension().is_some_and(|x| x == "txt") {
        out.push(path.to_path_buf());
    }
}

fn main() {
    let mut db_path = None;
    let mut imports = Vec::new();
    let mut summaries = Vec::new();
    let mut threads = None;
    let mut minutes = 25.0;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--db" => db_path = args.next().map(PathBuf::from),
            "--import" => files(Path::new(&args.next().expect("--import DIR")), &mut imports),
            "--summaries" => files(Path::new(&args.next().expect("--summaries DIR")), &mut summaries),
            "--solve" => threads = args.next().and_then(|v| v.parse().ok()),
            "--minutes" => minutes = args.next().and_then(|v| v.parse().ok()).expect("--minutes N"),
            "--out" => out = args.next().map(PathBuf::from),
            other => panic!("unbekannt: {other}"),
        }
    }
    let mut db = Db::open(&db_path.expect("--db FILE")).expect("open db");
    imports.sort();
    let (mut new, mut malformed) = (0, 0);
    for p in &imports {
        let r = import_file(&mut db, p).expect("import");
        new += r.new_hands.len();
        malformed += r.malformed;
        for (raw, why) in &r.malformed_hands {
            eprintln!("kaputt in {}: {why}\n{}", p.display(), raw.lines().next().unwrap_or_default());
        }
    }
    for p in &summaries {
        import_summary(&mut db, p).expect("summary");
    }
    println!("Import: {new} neue Hände, {malformed} kaputt, {} insgesamt", db.hand_count().unwrap());

    let t = Instant::now();
    let mut analysed = 0;
    loop {
        let rows = db.analysis_candidates(ENGINE, 0, 500).expect("candidates");
        if rows.is_empty() {
            break;
        }
        analysed += rows.len();
        db.save_analysis(&analyze_raw(&rows)).expect("save");
    }
    println!("Analyse: {analysed} Hände in {:.1} s", t.elapsed().as_secs_f64());

    if let Some(threads) = threads {
        let deadline = Instant::now() + Duration::from_secs_f64(minutes * 60.0);
        let mut solved = 0;
        while Instant::now() < deadline {
            let Some((hand_id, json)) = db.next_postflop_job(0).expect("next job") else { break };
            let started = Instant::now();
            let job: PostflopJob = serde_json::from_str(&json).expect("job");
            let result = solve_job(&job, threads, 8 << 30, |_| {}, || Instant::now() >= deadline);
            let (decisions, why) = match result {
                Ok(d) => (d, String::new()),
                Err(ringside_analysis::postflop::JobError::Cancelled) => break,
                Err(e) => {
                    let pending = db.hand_decisions(hand_id).unwrap().into_iter().filter(|d| d.status == ringside_analysis::Status::Pending);
                    (pending.collect(), format!("{e:?}"))
                }
            };
            let hand = db.hand(hand_id).unwrap().expect("hand");
            let decisions = estimate_rest(&hand, decisions, &format!("Postflop-Solve nicht möglich: {why}"));
            db.finish_postflop(hand_id, &decisions).expect("finish");
            solved += 1;
            eprintln!("Hand {hand_id} gelöst in {:.0} s{}", started.elapsed().as_secs_f64(), if why.is_empty() { String::new() } else { format!(" ({why})") });
        }
        let left = db.analysis_counts(ENGINE).expect("counts");
        println!("Solver: {solved} Spots in diesem Lauf, Stand {left:?}");
    }

    let all = Filter::default();
    let spans = db.session_spans(LISTED_SESSIONS).expect("spans");
    let sessions: Vec<_> = spans
        .iter()
        .map(|s| {
            let mut moves = db.skill_moves().expect("moves");
            moves.retain(|m| m.played_at <= s.end);
            let change = skill::change(&moves, |m| m.played_at < s.start);
            serde_json::json!({ "review": db.session_review(s, ENGINE).expect("review"), "skill": change })
        })
        .collect();
    let tournaments: Vec<_> = db
        .tournaments(&all)
        .expect("tournaments")
        .into_iter()
        .map(|t| {
            let report = db.tournament_report(t.id, &t.started_at, ENGINE).expect("report");
            serde_json::json!({ "tournament": t, "report": report })
        })
        .collect();
    let rows = db.profile_rows(&all).expect("rows");
    let practiced = db.spot_practice().expect("practice");
    let prof = profile(&rows, &practiced);
    let leak = leaks::find(&db.hero_cells(&all).expect("cells"));
    let summary = db.analysis_summary(&all, ENGINE, 5).expect("summary");
    let next = rank(&Inputs { profile: &prof, leaks: &leak, costliest: &summary.costliest, concepts: &[], reviews: &[] });
    let skill = skill::profile(&db.skill_moves().expect("moves"));
    println!(
        "Sessions {}, Turniere {}, nächste Schritte {}, Wertung {:?} ± {:.0} nach {} Turnieren",
        sessions.len(),
        tournaments.len(),
        next.len(),
        skill.rating,
        skill.sigma,
        skill.entries.len()
    );
    if let Some(out) = out {
        let all = serde_json::json!({
            "sessions": sessions,
            "tournaments": tournaments,
            "profile": prof,
            "leaks": leak,
            "summary": summary,
            "next": next,
            "skill": skill,
        });
        std::fs::write(out, serde_json::to_string_pretty(&all).expect("json")).expect("write");
    }
}
