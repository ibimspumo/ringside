//! Large-database behaviour. The quick test runs everywhere; the full one is
//! `RINGSIDE_SCALE_HANDS=1000000 cargo test --release -p ringside-core --test scale -- --ignored --nocapture`.

use std::time::Instant;

use ringside_core::db::{Db, Filter, HandQuery};
use ringside_core::model::Hand;
use ringside_core::{parse_hand, split_hands};

const CASH: &str = include_str!("fixtures/cash.txt");
const NAMES: [&str; 5] = ["Alice", "Bob", "Carol", "Dave", "Eve: The Great"];

/// `n` distinct hands built from the fixtures, with players drawn from a pool of `pool` names.
fn synthetic(n: usize, pool: usize, start: usize) -> Vec<(Hand, String)> {
    let templates: Vec<&str> = split_hands(CASH);
    (start..start + n)
        .map(|i| {
            let mut raw = templates[i % 2]
                .replace("#250000000001:", &format!("#{}:", 3_000_000_000u64 + i as u64))
                .replace("#250000000002:", &format!("#{}:", 3_000_000_000u64 + i as u64))
                .replace("2024/01/15", &format!("2025/{:02}/{:02}", 1 + (i / 28 / 24) % 12, 1 + (i / 24) % 28));
            for (k, name) in NAMES.iter().enumerate() {
                raw = raw.replace(name, &format!("P{}", (i * 7 + k * 13) % pool));
            }
            (parse_hand(&raw).expect("synthetic hand parses"), raw)
        })
        .collect()
}

fn fill(db: &mut Db, total: usize, pool: usize) {
    let batch = 10_000;
    let mut done = 0;
    while done < total {
        let n = batch.min(total - done);
        db.insert_hands(&synthetic(n, pool, done)).unwrap();
        done += n;
    }
}

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    println!("{label:<34} {:>9.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    out
}

/// Running totals must always equal the per-hand rows they summarise.
fn assert_totals_consistent(db: &Db) {
    let fast = db.players(&Filter::default(), "", usize::MAX >> 1).unwrap();
    let slow = db.players(&Filter { since: Some("0000".into()), ..Default::default() }, "", usize::MAX >> 1).unwrap();
    assert_eq!(fast.len(), slow.len());
    for (a, b) in fast.iter().zip(&slow) {
        assert_eq!((&a.name, a.net, a.counters), (&b.name, b.net, b.counters));
    }
}

#[test]
fn totals_match_rows_and_results_stay_small() {
    let mut db = Db::open_in_memory().unwrap();
    fill(&mut db, 3_000, 200);
    assert_totals_consistent(&db);
    let r = db.hero_results(&Filter::default(), 500).unwrap();
    assert_eq!(r.hands, 3_000);
    assert!(r.points.len() <= 500);
    assert_eq!(r.points.last().unwrap().n, 3_000);
}

#[test]
#[ignore = "slow; run on demand"]
fn million_hands() {
    let total: usize = std::env::var("RINGSIDE_SCALE_HANDS").ok().and_then(|v| v.parse().ok()).unwrap_or(1_000_000);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.sqlite");
    let mut db = Db::open(&path).unwrap();
    timed(&format!("import {total} hands"), || fill(&mut db, total, 20_000));

    let all = Filter::default();
    timed("players (search, totals)", || db.players(&all, "P12", 200).unwrap());
    timed("players (top 200, totals)", || db.players(&all, "", 200).unwrap());
    let names: Vec<String> = (0..8).map(|i| format!("P{i}")).collect();
    timed("overlay counters, 8 players", || db.counters_for(&names, None).unwrap());
    let r = timed("results graph", || db.hero_results(&all, 1500).unwrap());
    assert!(r.points.len() <= 1500 && r.hands == total as i64);
    timed("report by position", || db.hero_groups(&all, false).unwrap());
    timed("report by starting hand", || db.hero_groups(&all, true).unwrap());
    timed("sessions", || db.sessions(&all, 30.0).unwrap());
    timed("hands page (newest 100)", || db.hands(&HandQuery { limit: 100, ..Default::default() }).unwrap());
    timed("hands page (one player)", || {
        db.hands(&HandQuery { player: Some("P42".into()), limit: 100, ..Default::default() }).unwrap()
    });
    timed("players since a date (scan)", || {
        db.players(&Filter { since: Some("2025/06/01".into()), ..Default::default() }, "", 200).unwrap()
    });

    // An upgrade rebuilds everything in bounded batches.
    drop(db);
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA user_version = 1").unwrap();
    }
    let mut db = Db::open(&path).unwrap();
    timed("upgrade rebuild", || db.rebuild_all().unwrap());
    timed("totals consistency check", || assert_totals_consistent(&db));
    println!("database size {:>26.0} MB", std::fs::metadata(&path).unwrap().len() as f64 / 1e6);
}
