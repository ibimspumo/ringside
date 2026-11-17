//! Large-database behaviour. The quick test runs everywhere; the full one is
//! `RINGSIDE_SCALE_HANDS=1000000 cargo test --release -p ringside-core --test scale -- --ignored --nocapture`.

use std::time::Instant;

use ringside_core::db::{Db, Filter, HandQuery, SpotFilter};
use ringside_core::leaks;
use ringside_core::model::{GameKind, Hand};
use ringside_core::stats::StatId;
use ringside_core::{parse_hand, split_hands};

const CASH: &str = include_str!("fixtures/cash.txt");
const NAMES: [&str; 5] = ["Alice", "Bob", "Carol", "Dave", "Eve: The Great"];
/// Every this many hands one is a preflop all-in with both hands shown, so imports and
/// rebuilds pay for the exact equity of each.
const ALL_IN_EVERY: usize = 40;
const ALL_IN: &str = "PokerStars Hand #250000000003:  Hold'em No Limit ($0.05/$0.10 USD) - 2024/01/15 20:32:00 CET [2024/01/15 14:32:00 ET]
Table 'Aquila II' 6-max Seat #5 is the button
Seat 1: Alice ($10 in chips)
Seat 2: Bob ($10 in chips)
Seat 5: Hero ($10 in chips)
Alice: posts small blind $0.05
Bob: posts big blind $0.10
*** HOLE CARDS ***
Dealt to Hero [H1 H2]
Hero: raises $9.90 to $10 and is all-in
Alice: folds
Bob: calls $9.90 and is all-in
*** FLOP *** [B1 B2 B3]
*** TURN *** [B1 B2 B3] [B4]
*** RIVER *** [B1 B2 B3 B4] [B5]
*** SHOW DOWN ***
Bob: shows [V1 V2] (high card)
Hero: shows [H1 H2] (high card)
Hero collected $19.55 from pot
*** SUMMARY ***
Total pot $20.05 | Rake $0.50
Board [B1 B2 B3 B4 B5]
Seat 1: Alice (small blind) folded before Flop
Seat 2: Bob (big blind) showed [V1 V2] and lost with high card
Seat 5: Hero (button) showed [H1 H2] and won ($19.55) with high card";

/// Nine distinct cards for hand `i`, different from hand to hand.
fn deal(i: usize) -> Vec<String> {
    let mut deck: Vec<String> =
        "23456789TJQKA".chars().flat_map(|r| "cdhs".chars().map(move |s| format!("{r}{s}"))).collect();
    let mut x = i as u64;
    (0..9)
        .map(|k| {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            deck.swap_remove((x >> 33) as usize % (52 - k))
        })
        .collect()
}

fn all_in_hand(i: usize) -> String {
    let c = deal(i);
    let mut raw = ALL_IN.to_string();
    for (k, key) in ["H1", "H2", "V1", "V2", "B1", "B2", "B3", "B4", "B5"].iter().enumerate() {
        raw = raw.replace(key, &c[k]);
    }
    raw
}

/// `n` distinct hands built from the fixtures, with players drawn from a pool of `pool` names.
fn synthetic(n: usize, pool: usize, start: usize) -> Vec<(Hand, String)> {
    let templates: Vec<&str> = split_hands(CASH);
    (start..start + n)
        .map(|i| {
            let template = if i % ALL_IN_EVERY == ALL_IN_EVERY - 1 { all_in_hand(i) } else { templates[i % 2].to_string() };
            let mut raw = template
                .replace("#250000000001:", &format!("#{}:", 3_000_000_000u64 + i as u64))
                .replace("#250000000002:", &format!("#{}:", 3_000_000_000u64 + i as u64))
                .replace("#250000000003:", &format!("#{}:", 3_000_000_000u64 + i as u64))
                // Like real hand ids, dates only move forward: about 3,000 hands a day.
                .replace("2024/01/15", &{
                    let day = i / 3_000;
                    format!("{}/{:02}/{:02}", 2025 + day / 336, 1 + (day / 28) % 12, 1 + day % 28)
                });
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
    assert_eq!(r.all_ins, (3_000 / ALL_IN_EVERY) as i64);
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
    println!("all-ins with EV {:>25}", r.all_ins);
    timed("report by position", || db.hero_groups(&all, false).unwrap());
    let cells = timed("leak cells (query)", || db.hero_cells(&all).unwrap());
    timed("leak finder (references)", || leaks::find(&cells));
    let since = Filter { since: Some("2025/06/01".into()), ..Default::default() };
    timed("leak cells since a date", || db.hero_cells(&since).unwrap());
    timed("results graph since a date", || db.hero_results(&since, 1500).unwrap());
    for (label, made) in [("hands page (leak spot, common)", None), ("hands page (leak spot, rare)", Some(false))] {
        let spot = SpotFilter {
            stat: StatId::ThreeBet,
            made,
            positions: vec!["BB".into()],
            band: Some(3),
            game: Some(GameKind::Cash),
        };
        timed(label, || db.hands(&HandQuery { spot: Some(spot), limit: 100, ..Default::default() }).unwrap());
    }
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
