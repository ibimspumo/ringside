//! How fast the stock fills and how long a hand waits for a live spot:
//! `cargo test --release -p ringside-play --test measure -- --ignored --nocapture --test-threads 1`
//! with `THREADS` (default 2 for the stock, 8 live) and `SPOTS` (default 8).

use std::time::Instant;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::pushfold_table::Ante;
use rand::Rng;
use ringside_play::Hand;
use ringside_play::pool::{LIVE_MAX_ITERATIONS, LIVE_PCT, NoLookup, Plan, Pool};
use ringside_play::table::{Config, TableKey};
use ringside_solver::{Settings, solve};

#[test]
#[ignore]
fn stock_fill_rate() {
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|t| t.parse().ok()).unwrap_or(d);
    let (threads, spots) = (env("THREADS", 2), env("SPOTS", 8));
    let key = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    let pool = Pool::new(Box::new(NoLookup), threads, 16 << 30);
    pool.set_plan(Some(Plan { tables: vec![key], hero_seat: None }));
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut times = Vec::new();
    let start = Instant::now();
    while times.len() < spots {
        let t = Instant::now();
        let before = pool.status().solved;
        pool.step(&mut rng, &|| false, &|_| {}, &|_| {});
        if pool.status().solved > before {
            // A hand takes each spot as often as allowed, so the next round solves again.
            let taken = pool.take(&[key], None, &mut rng).unwrap();
            for _ in 1..ringside_play::pool::MAX_USES {
                pool.take(&[key], None, &mut rng);
            }
            let s = &taken.scenario;
            println!("{:?} {}-{} {}: {:.1} s", s.pot_type, s.oop, s.ip, taken.flop.iter().map(|c| c.to_string()).collect::<String>(), t.elapsed().as_secs_f64());
            times.push(t.elapsed().as_secs_f64());
        }
    }
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    println!("threads {threads}: {} spots in {:.0} s, mean {mean:.1} s, median {:.1} s, max {:.1} s", times.len(), start.elapsed().as_secs_f64(), sorted[sorted.len() / 2], sorted[sorted.len() - 1]);
}

/// Live spots as hands reach them: the hero plays the model's frequencies, the flops the
/// hands see are solved with the live settings and 16-bit storage, as the stock does.
#[test]
#[ignore]
fn live_spot_times() {
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|t| t.parse().ok()).unwrap_or(d);
    let (threads, spots) = (env("THREADS", 8), env("SPOTS", 12));
    let settings = Settings { target_pct: LIVE_PCT, max_iterations: LIVE_MAX_ITERATIONS, threads };
    let mut rng = ChaCha8Rng::seed_from_u64(5);
    let mut times = Vec::new();
    let mut seed = 0;
    while times.len() < spots {
        seed += 1;
        let mut hand = Hand::start(&Config::default(), seed, None).unwrap();
        while hand.need().is_none() {
            let Some(l) = hand.lesson() else { break };
            let s = l.strategy.unwrap_or_else(|| vec![1.0; l.choices.len()]);
            let a = ringside_play::hand::pick(&s, &mut rng).unwrap_or(rng.random_range(0..l.choices.len()));
            hand.act(a).unwrap();
        }
        let Some(need) = hand.need().cloned() else { continue };
        let t = Instant::now();
        let full = ringside_solver::estimate(&need.spot, u64::MAX, threads).unwrap().memory_bytes;
        let solution = solve(&need.spot, &settings, full - 1, |_| {}, || false).unwrap();
        let secs = t.elapsed().as_secs_f64();
        let s = &solution.summary;
        println!(
            "{:?} {}-{} pot {:.1} stack {:.1} {}: {secs:.1} s, {} iterations, {:.2} %, {:.0} MB",
            need.why,
            need.seats[0],
            need.seats[1],
            need.spot.pot,
            need.spot.stack,
            need.spot.board,
            s.iterations,
            s.exploitability_pct,
            s.memory_bytes as f64 / 1048576.0
        );
        times.push(secs);
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    println!("threads {threads}: {} live spots, median {:.1} s, max {:.1} s", times.len(), sorted[sorted.len() / 2], sorted[sorted.len() - 1]);
}
