//! Throughput of the preflop analysis on a large history:
//! `RINGSIDE_SCALE_HANDS=1000000 cargo test --release -p ringside-analysis --test scale -- --ignored --nocapture`.

use std::time::Instant;

use ringside_analysis::{ENGINE, analyze_raw};
use ringside_core::db::{Db, DecisionFilter, HandQuery, Rating};
use ringside_core::parse_hand;

const RANKS: &[u8] = b"23456789TJQKA";
const SUITS: &[u8] = b"cdhs";

/// Two different cards for hand `i`, not on the boards below.
fn hole(i: usize) -> String {
    let mut x = (i as u64).wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    let mut card = || {
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        // Ranks 2..8 only: the boards use higher cards.
        let r = RANKS[(x >> 33) as usize % 7] as char;
        let s = SUITS[(x >> 40) as usize % 4] as char;
        format!("{r}{s}")
    };
    let a = card();
    let mut b = card();
    while b == a {
        b = card();
    }
    format!("{a} {b}")
}

fn tournament(id: u64, stack: i64, cards: &str, body: &str) -> String {
    let names = ["P1", "P2", "P3", "Hero", "P5", "P6"];
    let mut out = format!(
        "PokerStars Hand #{id}: Tournament #3500000001, $10+$1 USD Hold'em No Limit - Level IV (100/200) - 2024/01/15 21:00:00 CET [2024/01/15 15:00:00 ET]\nTable '3500000001 12' 9-max Seat #3 is the button\n"
    );
    for (i, n) in names.iter().enumerate() {
        out.push_str(&format!("Seat {}: {n} ({stack} in chips)\n", i + 1));
    }
    for n in names {
        out.push_str(&format!("{n}: posts the ante 25\n"));
    }
    out.push_str(&format!("Hero: posts small blind 100\nP5: posts big blind 200\n*** HOLE CARDS ***\nDealt to Hero [{cards}]\n{body}"));
    out
}

/// Every third hand a short-stack push/fold spot, a deeper open, and a c-bet that waits
/// for the solver.
fn raw(i: usize) -> String {
    let id = 4_000_000_000 + i as u64;
    let cards = hole(i);
    match i % 3 {
        0 => tournament(id, 1600, &cards, "P6: folds\nP1: folds\nP2: folds\nP3: folds\nHero: folds\nUncalled bet (100) returned to P5\nP5 collected 350 from pot\n*** SUMMARY ***\nTotal pot 350 | Rake 0\n"),
        1 => tournament(id, 7400, &cards, "P6: folds\nP1: folds\nP2: folds\nP3: folds\nHero: raises 400 to 600\nP5: folds\nUncalled bet (400) returned to Hero\nHero collected 750 from pot\n*** SUMMARY ***\nTotal pot 750 | Rake 0\n"),
        _ => tournament(
            id,
            8000,
            &cards,
            "P6: folds\nP1: folds\nP2: folds\nP3: folds\nHero: raises 400 to 600\nP5: calls 400\n*** FLOP *** [Kd Qs Jc]\nHero: bets 400\nP5: folds\nUncalled bet (400) returned to Hero\nHero collected 1350 from pot\n*** SUMMARY ***\nTotal pot 1350 | Rake 0\nBoard [Kd Qs Jc]\n",
        ),
    }
}

#[test]
#[ignore = "slow; run on demand"]
fn preflop_throughput() {
    let n: usize = std::env::var("RINGSIDE_SCALE_HANDS").ok().and_then(|v| v.parse().ok()).unwrap_or(100_000);
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open(&dir.path().join("scale.sqlite")).unwrap();
    let started = Instant::now();
    const CHUNK: usize = 20_000;
    for start in (0..n).step_by(CHUNK) {
        let hands: Vec<_> = (start..(start + CHUNK).min(n))
            .map(|i| {
                let r = raw(i);
                (parse_hand(&r).unwrap(), r)
            })
            .collect();
        db.insert_hands(&hands).unwrap();
    }
    println!("{n} Hände importiert in {:.1} s", started.elapsed().as_secs_f64());

    let (mut fetch, mut analyse, mut save) = (0.0, 0.0, 0.0);
    let mut after = 0;
    let threads = rayon::current_num_threads();
    loop {
        let t = Instant::now();
        let rows = db.analysis_candidates(ENGINE, after, 2000).unwrap();
        fetch += t.elapsed().as_secs_f64();
        let Some(last) = rows.last() else { break };
        after = last.0;
        let t = Instant::now();
        let results = analyze_raw(&rows);
        analyse += t.elapsed().as_secs_f64();
        let t = Instant::now();
        db.save_analysis(&results).unwrap();
        save += t.elapsed().as_secs_f64();
    }
    let total = fetch + analyse + save;
    let counts = db.analysis_counts(ENGINE).unwrap();
    println!(
        "Analyse ({threads} Threads): lesen {fetch:.1} s, bewerten {analyse:.1} s, speichern {save:.1} s = {total:.1} s; {:.0} Hände/s; hochgerechnet auf 1 Mio.: {:.0} s",
        n as f64 / total,
        total * 1e6 / n as f64
    );
    println!("{counts:?}");
    assert_eq!(counts.analyzed as usize, n);

    let t = Instant::now();
    let q = HandQuery {
        decisions: Some(DecisionFilter { min_rating: Some(Rating::Inaccuracy), ..Default::default() }),
        sort_by_loss: true,
        limit: 100,
        ..Default::default()
    };
    let list = db.hands(&q).unwrap();
    println!("Liste „teuerste Fehler“: {} Zeilen in {:.2} s", list.len(), t.elapsed().as_secs_f64());
    let t = Instant::now();
    let summary = db.analysis_summary(&Default::default(), ENGINE, 5).unwrap();
    println!("Zusammenfassung in {:.2} s, Verlust {:.1} BB", t.elapsed().as_secs_f64(), summary.loss_bb);
}
