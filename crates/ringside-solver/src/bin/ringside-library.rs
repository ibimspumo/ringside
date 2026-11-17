//! Operator tool for the spot library: solves catalog scenarios into stored spots, bundles
//! them into a pack for `deno task packs upload`, and measures the cloud check. Run with
//! `--release`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use ringside_solver::library::format::{Stored, pack, unpack};
use ringside_solver::library::{SpotId, catalog, check, solve_spot};

const HELP: &str = "\
Usage: ringside-library <command>

  solve <scenario> <dir> [--flops N] [--threads T] [--minutes M]
        solves the scenario on the first N catalog flops (default all) into <dir>/<flop>.rsls,
        skipping flops already there, and stops starting new ones after M minutes
  pack <scenario> <dir> <file>
        bundles <dir>/*.rsls of the scenario, in catalog order, into one pack file
  check <file.rsls> [--threads T] [--max-iterations I]
        runs the cloud check on a stored spot and prints its exploitability and time
  list  prints the catalog's scenarios";

const GB: u64 = 1 << 30;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ringside-library: {e}");
            ExitCode::FAILURE
        }
    }
}

fn flag<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, String> {
    match args.iter().position(|a| a == name) {
        Some(i) => args.get(i + 1).and_then(|v| v.parse().ok()).ok_or_else(|| format!("{name} needs a number")),
        None => Ok(default),
    }
}

fn threads_default() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

fn run(args: Vec<String>) -> Result<(), String> {
    let c = catalog();
    match args.first().map(String::as_str) {
        Some("list") => {
            for s in &c.scenarios {
                println!("{:<16} {} gegen {}, Pot {} BB, Stack {} BB", s.id, s.oop, s.ip, s.pot, s.stack);
            }
            Ok(())
        }
        Some("solve") if args.len() >= 3 => {
            let scenario = c.scenario(&args[1]).ok_or("unknown scenario; see list")?;
            let dir = PathBuf::from(&args[2]);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let n = flag(&args, "--flops", c.flops.len())?.min(c.flops.len());
            let threads = flag(&args, "--threads", threads_default())?;
            let minutes = flag(&args, "--minutes", f64::INFINITY)?;
            let started = Instant::now();
            let (mut solved, mut bytes, mut seconds) = (0, 0usize, 0.0);
            for flop in 0..n {
                let path = dir.join(format!("{}.rsls", c.flops[flop]));
                if path.exists() {
                    continue;
                }
                if started.elapsed().as_secs_f64() > minutes * 60.0 {
                    println!("time budget used up, stopping");
                    break;
                }
                let t = Instant::now();
                let stored = solve_spot(c, SpotId { scenario, flop }, threads, 64 * GB, |_| {}, || false)
                    .map_err(|e| format!("{}: {e}", c.flops[flop]))?;
                let data = stored.to_bytes();
                std::fs::write(&path, &data).map_err(|e| e.to_string())?;
                let s = t.elapsed().as_secs_f64();
                println!(
                    "{:>3}/{n} {}  {:5.1}s  {:3} it  {:.3} %  {:6} bytes",
                    flop + 1,
                    c.flops[flop],
                    s,
                    stored.iterations,
                    stored.exploitability_pct,
                    data.len()
                );
                solved += 1;
                bytes += data.len();
                seconds += s;
            }
            if solved > 0 {
                println!(
                    "{solved} spots in {:.1} min, {:.1} s and {:.0} KB per spot, {:.1} MB in all",
                    seconds / 60.0,
                    seconds / solved as f64,
                    bytes as f64 / solved as f64 / 1024.0,
                    bytes as f64 / 1e6
                );
            }
            Ok(())
        }
        Some("pack") if args.len() == 4 => {
            let scenario = c.scenario(&args[1]).ok_or("unknown scenario; see list")?;
            let dir = Path::new(&args[2]);
            let mut spots = Vec::new();
            for flop in &c.flops {
                let Ok(data) = std::fs::read(dir.join(format!("{flop}.rsls"))) else { continue };
                let stored = Stored::from_bytes(&data, c).map_err(|e| format!("{flop}: {e}"))?;
                if stored.scenario != c.scenarios[scenario].id || &stored.flop != flop {
                    return Err(format!("{flop}: belongs to {} {}", stored.scenario, stored.flop));
                }
                spots.push(data);
            }
            let bytes = pack(&spots);
            unpack(&bytes).map_err(|e| e.to_string())?;
            std::fs::write(&args[3], &bytes).map_err(|e| e.to_string())?;
            println!("{} spots, {} bytes -> {}", spots.len(), bytes.len(), args[3]);
            Ok(())
        }
        Some("check") if args.len() >= 2 => {
            let data = std::fs::read(&args[1]).map_err(|e| e.to_string())?;
            let stored = Stored::from_bytes(&data, c).map_err(|e| e.to_string())?;
            let threads = flag(&args, "--threads", threads_default())?;
            let max = flag(&args, "--max-iterations", 300u32)?;
            let t = Instant::now();
            let r = check(&stored, c, threads, 64 * GB, max, || false).map_err(|e| e.to_string())?;
            println!(
                "{} {}: stored {:.3} %, checked {:.3} % in {:.1}s ({} iterations, {threads} threads)",
                stored.scenario,
                stored.flop,
                stored.exploitability_pct,
                r.exploitability_pct,
                t.elapsed().as_secs_f64(),
                r.iterations
            );
            Ok(())
        }
        _ => {
            println!("{HELP}");
            Ok(())
        }
    }
}
