//! Solves the tables of [`ringside_engine::preflop_table`] and writes
//! `data/preflop_<ante>_<n>max.bin`; rebuild the crate afterwards to embed them.
//!
//! ```text
//! gen-preflop-tables [--ante none|classic|big_blind]... [--players 6]... [--stacks 15,20]
//!                    [--jobs 2] [--cache DIR] [--no-write]
//! ```
//!
//! Every solved entry is cached as its own file first (default
//! `target/preflop-cache`), so an interrupted run continues where it stopped. A table
//! file is only written when all its stacks are in the cache, unless `--no-write`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ringside_engine::preflop::{Kind, solve};
use ringside_engine::preflop_table::{self, Ante, PLAYERS, STACKS, encode_entry, params};

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
    let mut cache = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/preflop-cache"));
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{a} needs a value"));
        match a.as_str() {
            "--ante" => {
                let key = value();
                antes.push(*Ante::ALL.iter().find(|a| a.key() == key).unwrap_or_else(|| panic!("unknown ante {key}")));
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
        players = PLAYERS.to_vec();
    }
    std::fs::create_dir_all(&cache).expect("create cache dir");
    ringside_engine::eval::init();

    // Deep stacks first: they take longest, so the run ends with short jobs on all workers.
    let mut todo: Vec<Job> = Vec::new();
    for &stack in STACKS.iter().rev() {
        for &ante in &antes {
            for &n in &players {
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
                            let opens: Vec<String> = (0..job.players - 1)
                                .map(|seat| {
                                    let v = sol.open_node(seat).expect("every seat but the big blind opens");
                                    format!("{:.0}/{:.0}", sol.play_pct(v), sol.kind_pct(v, Kind::AllIn))
                                })
                                .collect();
                            println!(
                                "{:>3}/{total} {} n={} {:>5} BB  {:>7.1} s  it={:>4} conv={} expl={:.5} evsum={:+.4} {} nodes {} bytes  open%/jam% {}  [{:.0} s]",
                                i + 1,
                                job.ante.key(),
                                job.players,
                                job.stack,
                                t.elapsed().as_secs_f64(),
                                sol.iterations,
                                sol.converged,
                                sol.exploitability,
                                sol.ev_sum_error,
                                sol.nodes.len(),
                                bytes.len(),
                                opens.join(" "),
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
        for &n in &players {
            let mut entries = Vec::new();
            let mut missing = 0;
            for &stack in &STACKS {
                match std::fs::read(cache_file(&cache, &Job { ante, players: n, stack })) {
                    Ok(b) => entries.push(b),
                    Err(_) => missing += 1,
                }
            }
            if missing > 0 {
                println!("{} {n} players: {missing} entries missing, table not written", ante.key());
                continue;
            }
            let raw: usize = entries.iter().map(|e| e.len() + 4).sum();
            let file = preflop_table::write_file(ante, n, &entries);
            let path = format!("{data}/preflop_{}_{n}max.bin", ante.key());
            std::fs::write(&path, &file).expect("write table");
            let info = preflop_table::file_info(&file).expect("fresh table");
            println!("{path}: {} entries, {raw} bytes raw, {} bytes compressed, checksum {:016x}", info.entries, file.len(), info.checksum);
        }
    }
}
