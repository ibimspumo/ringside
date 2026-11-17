//! Solves the push/fold tables of [`ringside_engine::pushfold_table`] and writes
//! `data/pushfold_<ante>.bin`; rebuild the crate afterwards to embed them.
//!
//! ```text
//! gen-pushfold-tables [--ante none|classic|big_blind]... [--players 6]... [--stacks 3,10,20]
//!                     [--jobs 2] [--cache DIR] [--no-write]
//! ```
//!
//! Every solved entry is cached as its own file first (default
//! `target/pushfold-cache`), so an interrupted run continues where it stopped. A table
//! file is only written when its whole grid is in the cache, unless `--no-write`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ringside_engine::multiway::solve;
use ringside_engine::pushfold_table::{self, Ante, MAX_PLAYERS, MIN_PLAYERS, encode_entry, params, stacks};

struct Job {
    ante: Ante,
    players: usize,
    stack: f64,
}

fn cache_file(dir: &Path, job: &Job) -> PathBuf {
    dir.join(format!("{}-{}-{}.bin", job.ante.key(), job.players, job.stack))
}

fn main() {
    let mut antes: Vec<Ante> = Vec::new();
    let mut players: Vec<usize> = Vec::new();
    let mut only_stacks: Option<Vec<f64>> = None;
    let mut jobs = 1usize;
    let mut write = true;
    let mut cache = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/pushfold-cache"));
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--ante" => {
                let v = value();
                antes.push(*Ante::ALL.iter().find(|x| x.key() == v).unwrap_or_else(|| panic!("unknown ante {v}")));
            }
            "--players" => players.push(value().parse().expect("player count")),
            "--stacks" => only_stacks = Some(value().split(',').map(|s| s.parse().expect("stack")).collect()),
            "--jobs" => jobs = value().parse().expect("job count"),
            "--cache" => cache = PathBuf::from(value()),
            "--no-write" => write = false,
            _ => panic!("unknown argument {a}"),
        }
    }
    if antes.is_empty() {
        antes = Ante::ALL.to_vec();
    }
    if players.is_empty() {
        players = (MIN_PLAYERS..=MAX_PLAYERS).collect();
    }
    std::fs::create_dir_all(&cache).expect("create cache dir");
    ringside_engine::eval::init();

    // Big tables first: they take longest, so the run ends with short jobs on all workers.
    let mut todo: Vec<Job> = Vec::new();
    for &ante in &antes {
        for &n in players.iter().rev() {
            for &stack in stacks(ante).iter().rev() {
                if only_stacks.as_ref().is_some_and(|s| !s.contains(&stack)) {
                    continue;
                }
                let job = Job { ante, players: n, stack };
                if !cache_file(&cache, &job).exists() {
                    todo.push(job);
                }
            }
        }
    }
    let total = todo.len();
    println!("{total} entries to solve on {jobs} workers, {} rayon threads", rayon::current_num_threads());
    let start = Instant::now();
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = todo.get(i) else { break };
                    let p = params(job.ante, job.players, job.stack);
                    let t = Instant::now();
                    let result = solve(&p).map_err(|e| e.to_string()).and_then(|sol| {
                        let bytes = encode_entry(&p, &sol)?;
                        Ok((sol, bytes))
                    });
                    match result {
                        Ok((sol, bytes)) => {
                            let file = cache_file(&cache, job);
                            let tmp = file.with_extension("tmp");
                            std::fs::write(&tmp, &bytes).expect("write cache entry");
                            std::fs::rename(&tmp, &file).expect("move cache entry");
                            println!(
                                "{:>4}/{total} {:>9} n={} {:>4} BB  {:>6.2} s  it={:>4} conv={} expl={:.5} multiway={:.4} capped={:.4} evsum={:+.4}  [{:.0} s]",
                                i + 1,
                                job.ante.key(),
                                job.players,
                                job.stack,
                                t.elapsed().as_secs_f64(),
                                sol.iterations,
                                sol.converged,
                                sol.exploitability,
                                sol.approximation.multiway_probability,
                                sol.approximation.capped_probability,
                                sol.approximation.ev_sum_error,
                                start.elapsed().as_secs_f64(),
                            );
                        }
                        Err(e) => {
                            println!("FAILED {} n={} {} BB: {e}", job.ante.key(), job.players, job.stack);
                            failed.lock().unwrap().push(e);
                        }
                    }
                }
            });
        }
    });
    println!("solved {total} entries in {:.1} s", start.elapsed().as_secs_f64());
    if !failed.lock().unwrap().is_empty() {
        eprintln!("some entries failed; no table written");
        std::process::exit(1);
    }
    if !write {
        return;
    }
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/data");
    for &ante in &antes {
        let mut entries = Vec::new();
        let mut missing = 0;
        for n in MIN_PLAYERS..=MAX_PLAYERS {
            for &stack in stacks(ante) {
                match std::fs::read(cache_file(&cache, &Job { ante, players: n, stack })) {
                    Ok(b) => entries.push(b),
                    Err(_) => missing += 1,
                }
            }
        }
        if missing > 0 {
            println!("{}: {missing} entries missing, table not written", ante.key());
            continue;
        }
        let raw: usize = entries.iter().map(|e| e.len() + 4).sum();
        let file = pushfold_table::write_file(ante, &entries);
        let path = format!("{data}/pushfold_{}.bin", ante.key());
        std::fs::write(&path, &file).expect("write table");
        let info = pushfold_table::file_info(&file).expect("fresh table");
        println!(
            "{path}: {} entries, {raw} bytes raw, {} bytes compressed, checksum {:016x}",
            info.entries,
            file.len(),
            info.checksum
        );
    }
}
