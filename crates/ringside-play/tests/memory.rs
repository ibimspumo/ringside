//! Resident memory of the heavy parts, step by step:
//! `cargo test --release -p ringside-play --test memory -- --ignored --nocapture --test-threads 1`
//! with `THREADS` (default 4), `SPOTS` (stock spots, default 3, 0 skips the stock) and
//! `BUDGET_MB` (stock budget, default 1536). Reads the RSS with `ps` and the footprint
//! with macOS' `footprint`, so it measures nothing on Windows. macOS keeps freed blocks of
//! hundreds of megabytes in its allocator's large cache; `MallocLargeCache=0` returns them
//! at once, as Windows does.

use std::time::{Duration, Instant};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::multiway;
use ringside_engine::pushfold_table::{self, Ante};
use ringside_engine::{preflop_table, release_idle_tables};
use ringside_play::pool::{NoLookup, Plan, Pool, Status};
use ringside_play::table::TableKey;
use ringside_solver::library::catalog;
use ringside_solver::saved::Snapshot;
use ringside_solver::{Settings, StreetSizes, solve};

fn rss_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.trim().parse::<f64>().ok()).map_or(f64::NAN, |kb| kb / 1024.0)
}

/// The physical footprint on macOS (what Activity Monitor shows, and the closest to
/// Windows' private bytes): unlike the RSS it leaves out pages `free` returned with
/// `MADV_FREE_REUSABLE`, which stay resident until the system needs them.
fn footprint_mb() -> f64 {
    let pid = std::process::id().to_string();
    let out = std::process::Command::new("footprint").args(["-f", "bytes", "--noCategories", "-p", &pid]).output();
    let text = out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let bytes = text.split("Footprint: ").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|n| n.parse::<f64>().ok());
    bytes.map_or(f64::NAN, |b| b / 1048576.0)
}

fn line(what: &str, t: Instant) {
    let secs = t.elapsed().as_secs_f64();
    std::thread::sleep(Duration::from_millis(300));
    println!("{what:<48} {:>7.0} MB {:>7.0} MB {:>7.2} s", rss_mb(), footprint_mb(), secs);
}

#[test]
#[ignore]
fn resident_memory_by_step() {
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|t| t.parse().ok()).unwrap_or(d);
    let (threads, spots, budget) = (env("THREADS", 4), env("SPOTS", 3), env("BUDGET_MB", 1536) as u64);
    println!("{:<48} {:>10} {:>10} {:>9}", "step", "RSS", "footprint", "time");
    line("start", Instant::now());

    let t = Instant::now();
    for ante in Ante::ALL {
        pushfold_table::chart(ante, 6, pushfold_table::stacks(ante)[0]);
    }
    line("push/fold charts, all 3 tables inflated", t);
    let t = Instant::now();
    for ante in Ante::ALL {
        for &players in &preflop_table::PLAYERS {
            preflop_table::chart(ante, players, preflop_table::STACKS[0]);
        }
    }
    line("preflop charts, all 9 tables inflated", t);
    let t = Instant::now();
    let mut params = pushfold_table::params(Ante::BigBlind, 6, 13.3);
    params.max_iterations = params.max_iterations.min(200);
    multiway::solve(&params).unwrap();
    line("live push/fold solve, 6 players (3-way tables)", t);
    let t = Instant::now();
    let released = release_idle_tables(Duration::ZERO);
    line(&format!("{released} tables released"), t);

    let c = catalog();
    let s = &c.scenarios[c.scenario("srp-btn-bb-40").unwrap_or(0)];
    let mut spot = s.spot("Ks7d2c");
    spot.sizes = [
        StreetSizes { bet: vec![33.0, 75.0], raise: vec![60.0] },
        StreetSizes { bet: vec![66.0], raise: vec![100.0] },
        StreetSizes { bet: vec![75.0], raise: vec![100.0] },
    ];
    spot.allin_threshold = 150.0;
    let settings = Settings { target_pct: 50.0, max_iterations: 2, threads };
    let t = Instant::now();
    let mut solution = solve(&spot, &settings, 32 << 30, |_| {}, || false).unwrap();
    line(&format!("flop solve {} ({:.0} MB tree)", s.id, solution.summary.memory_bytes as f64 / 1048576.0), t);
    let t = Instant::now();
    let snapshot = Snapshot::take(&mut solution, &settings).unwrap();
    line(&format!("snapshot taken ({:.1} MB on disk)", snapshot.to_bytes().len() as f64 / 1048576.0), t);
    let t = Instant::now();
    drop(solution);
    line("solution dropped, snapshot kept", t);
    drop(snapshot);
    line("snapshot dropped", Instant::now());

    if spots == 0 {
        return;
    }
    let key = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    let pool = Pool::new(Box::new(NoLookup), threads, budget << 20);
    pool.set_plan(Some(Plan { tables: vec![key], hero_seat: None }));
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let t = Instant::now();
    while pool.status().ready < spots && pool.status().status != Status::Done {
        pool.step(&mut rng, &|| false, &|_| {}, &|_| {});
    }
    let mb = pool.memory_bytes() as f64 / 1048576.0;
    line(&format!("stock: {} spots ready ({mb:.0} MB), budget {budget} MB", pool.status().ready), t);
    let t = Instant::now();
    pool.set_plan(None);
    ringside_play::table::release();
    line("stock and tables released", t);
}
