//! How fast the stock fills: `cargo test --release -p ringside-play --test measure -- --ignored --nocapture`
//! with `THREADS` (default 2) and `SPOTS` (default 8).

use std::time::Instant;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::pushfold_table::Ante;
use ringside_play::pool::{NoLookup, Plan, Pool};
use ringside_play::table::TableKey;

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
        pool.step(&mut rng, &|| false, &|_| {});
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
