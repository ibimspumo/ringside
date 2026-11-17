//! Throughput numbers for the engine; run with `--release`.

use std::hint::black_box;
use std::time::Instant;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use ringside_engine::{
    CardSet, Hand, Method, PreflopMatrix, PushFoldParams, Range, equity, solve_push_fold,
};

fn main() {
    let start = Instant::now();
    ringside_engine::eval::init();
    println!("evaluator tables built in {:.1} ms", ms(start));
    let _ = PreflopMatrix::embedded();

    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let hands: Vec<Hand> = (0..20_000_000)
        .map(|_| {
            let mut set = 0u64;
            while set.count_ones() < 7 {
                set |= 1 << rng.random_range(0..52);
            }
            Hand::from_set(CardSet(set))
        })
        .collect();

    let start = Instant::now();
    let sum: u64 = hands.iter().map(|h| black_box(*h).rank().0 as u64).sum();
    black_box(sum);
    let secs = start.elapsed().as_secs_f64();
    println!(
        "7-card evals, random hands, 1 thread:  {:>7.1} M/s",
        hands.len() as f64 / secs / 1e6
    );

    let rounds = 10;
    let start = Instant::now();
    for _ in 0..rounds {
        let sum: u64 = hands
            .par_chunks(1 << 16)
            .map(|c| c.iter().map(|h| black_box(*h).rank().0 as u64).sum::<u64>())
            .sum();
        black_box(sum);
    }
    let secs = start.elapsed().as_secs_f64();
    println!(
        "7-card evals, random hands, {} threads: {:>7.1} M/s",
        rayon::current_num_threads(),
        (rounds * hands.len()) as f64 / secs / 1e6
    );

    let players = [Range::parse("AhAs").unwrap(), Range::parse("KdKc").unwrap()];
    let start = Instant::now();
    let r = equity(&players, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).unwrap();
    println!(
        "exact AhAs vs KdKc preflop: {:.2} ms ({} boards, AA {:.4} %)",
        ms(start),
        r.showdowns,
        r.players[0].equity_pct
    );

    let players = [Range::parse("AA, KK").unwrap(), Range::parse("KK").unwrap()];
    let start = Instant::now();
    let r = equity(&players, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).unwrap();
    println!(
        "exact AA,KK vs KK preflop: {:.2} ms ({} showdowns)",
        ms(start),
        r.showdowns
    );

    let players = [
        Range::parse("QQ+, AKs, AKo").unwrap(),
        Range::parse("22+, A2s+, KTs+, QTs+, JTs, ATo+, KJo+").unwrap(),
        Range::all(),
    ];
    let samples = 20_000_000;
    let start = Instant::now();
    let r = equity(
        &players,
        CardSet::EMPTY,
        CardSet::EMPTY,
        Method::MonteCarlo { samples, seed: 7 },
    )
    .unwrap();
    let secs = start.elapsed().as_secs_f64();
    println!(
        "Monte Carlo 3-way ranges preflop: {:.1} M samples/s ({:.2} / {:.2} / {:.2} %)",
        samples as f64 / secs / 1e6,
        r.players[0].equity_pct,
        r.players[1].equity_pct,
        r.players[2].equity_pct
    );

    let start = Instant::now();
    let r = solve_push_fold(&PushFoldParams::new(10.0)).unwrap();
    println!(
        "Nash push/fold 10 BB: {:.1} ms ({} CFR+ iterations, exact {}, shove {:.2} %, call {:.2} %)",
        ms(start),
        r.iterations,
        r.exact,
        r.shove_pct,
        r.call_pct
    );
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e3
}
