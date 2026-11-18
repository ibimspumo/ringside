//! A practice seed of `ringside-queue --seed practice` from the queue to "Hände spielen":
//! the worker solves it, and the key it is filed under is the one the practice stock looks
//! the line's flop up with, on every suit renaming the stock deals.

use std::time::Instant;

use ringside_cloud::client::Work;
use ringside_cloud::kinds::{Kind, SpotKind, SpotSolution};
use ringside_cloud::operator::{Seed, practice_flops, practice_lines, practice_seeds};
use ringside_cloud::worker::{Power, Solved, Task, solve};
use ringside_play::pool::{parse_flop, renamings};
use ringside_solver::library::catalog;
use ringside_store::Cloud;
use ringside_store::key::{key_of, line_flops};

#[test]
fn a_practice_seed_is_solved_and_found_by_the_stock() {
    let line = practice_lines(6, 40.0).unwrap().remove(0);
    // Trips solve fastest; the way through the queue is the same on every flop.
    let flop = catalog().flops.last().unwrap().clone();
    let seed = practice_seeds(&line, std::slice::from_ref(&flop)).unwrap().remove(0);
    let key = format!("spot:{}", seed.key);

    // The stock asks for the line's flops by these keys.
    let flops = line_flops(&line.scenario.spot(&catalog().flops[0]), Some(&line.model));
    assert!(flops.contains(&(key.clone(), flop.clone())), "{key} on {flop} is not among the stock's keys");

    // The worker reads the leased row and solves it.
    let (solved, seconds) = solved(&seed);
    println!("{} on {flop}: {seconds:.0} s", line.label());
    let Solved::Spot { input, stored } = solved else { panic!("a spot task") };
    assert_eq!(SpotKind::key(&input).unwrap(), seed.key, "submitted under the queued key");
    let solution = SpotSolution::from_stored(&stored);
    SpotKind::check_download(&input, &solution).unwrap();
    assert!(SpotKind::reported(&solution).unwrap().is_some_and(|e| e <= SpotKind::TOLERANCE));

    // Practice deals the flop in a suit renaming; the store finds the solution by the
    // dealt board's key and renames it there, as for a cloud download.
    let dealt = renamings(parse_flop(&flop).unwrap()).into_iter().map(|f| f.map(|c| c.to_string()).concat()).find(|b| *b != flop).unwrap();
    let keyed = key_of(&line.scenario.spot(&dealt), Some(&line.model)).unwrap();
    assert_eq!(keyed.key, key);
    let Some(Cloud::Spot(asked)) = keyed.cloud else { panic!("a model flop") };
    assert_eq!(asked, input);
    let catalog = asked.catalog(Some(&dealt)).unwrap();
    let renamed = solution.stored(&asked).unwrap().renamed(&catalog.scenarios[0], &dealt).unwrap();
    assert_eq!(renamed.flop, dealt);
}

/// `seed` as the worker leases and solves it, with the seconds it took.
fn solved(seed: &Seed) -> (Solved, f64) {
    let work = Work {
        key: seed.key.clone(),
        kind: seed.kind.into(),
        engine_version: seed.engine_version,
        input: serde_json::from_str(&seed.input).unwrap(),
        lease_until: 0,
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let start = Instant::now();
    let solved = solve(&Task::parse(&work).unwrap(), Power { threads, memory: 8 << 30 }, |_| {}, || false).unwrap().unwrap();
    (solved, start.elapsed().as_secs_f64())
}

/// Seconds per spot of each line `ringside-queue --seed practice` seeds by default, on
/// flops spread over the catalog, to size `--flops` for a time budget per line:
/// `cargo test --release -p ringside-store --test practice_seed -- --ignored --nocapture`.
#[test]
#[ignore = "measurement, minutes in release"]
fn practice_solve_times() {
    let n: usize = std::env::var("FLOPS").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
    let flops = practice_flops(n);
    for line in practice_lines(6, 40.0).unwrap().iter().take(5) {
        let times: Vec<f64> = practice_seeds(line, &flops).unwrap().iter().map(|s| solved(s).1).collect();
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let max = times.iter().copied().fold(0.0, f64::max);
        println!("{}: {:.1} s per spot on average, at most {:.1} s ({:?})", line.label(), mean, max, flops);
    }
}
