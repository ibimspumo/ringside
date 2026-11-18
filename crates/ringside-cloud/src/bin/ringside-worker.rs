//! Operator tool: solves open spots of Ringside Cloud's community queue on a computer
//! without the app and submits them, as "Mitrechnen" in the app does. Run with `--release`.

use std::collections::BTreeMap;
use std::io::Write;
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringside_cloud::client::{DEFAULT_BASE_URL, RetryPolicy};
use ringside_cloud::worker::{self, Done, End, Event, Outcome, Power, RunOptions};
use ringside_cloud::Client;

const HELP: &str = "\
Usage: ringside-worker [--kinds K,K] [--minutes M] [--max N] [--threads T] [--memory-gb G]
                       [--dry-run] [--base URL]

Takes open spots from Ringside Cloud's community queue, solves them here and submits the
solutions through the public API, like \"Mitrechnen\" in the app. The hourly verifier
checks them before anyone gets them. Needs no keys or .env. Build and run it with
--release, e.g.

  cargo run --release -p ringside-cloud --bin ringside-worker -- --minutes 120

  --kinds K,K      kinds to take: postflop, spot, pushfold-mw, pushfold-hu (default all)
  --minutes M      take no new spot after M minutes; the running one is finished
  --max N          take at most N spots
  --threads T      solver threads for postflop and spot (default half the cores)
  --memory-gb G    memory one postflop tree may take (default 8, what the verifier
                   allows); a larger spot fails
  --dry-run        solve, but submit nothing and give the lease back
  --base URL       another API (default https://ringside-api.bunny.run)
  --help           this text

One line per spot: kind, start of its key, spot, time, exploitability (postflop and spot in percent of the
pot, push/fold in BB or percent of the prize pool) and whether it went up; a summary at
the end. Ctrl+C stops the solve, submits nothing and gives the lease back; a second
Ctrl+C quits at once. The tool does not start while a PokerStars client runs here and
stops when one starts.

The server hands out at most 60 spots and takes at most 60 solutions per hour and IP
address (the app on the same network counts too); the tool waits when it hits a limit
and retries network errors with a growing pause.

Exit code 0 when the run ended normally (queue empty, --max or --minutes), 1 on errors,
130 after Ctrl+C.";

const GB: u64 = 1 << 30;
/// How often the process list is read during a run.
const CLIENT_CHECK_EVERY: Duration = Duration::from_secs(10);

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ringside-worker: {e}");
            ExitCode::FAILURE
        }
    }
}

fn number<T: std::str::FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String> {
    args.next().and_then(|v| v.parse().ok()).ok_or_else(|| format!("{flag} needs a number"))
}

fn kinds(list: &str) -> Result<Vec<(&'static str, u32)>, String> {
    list.split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| {
            worker::KINDS.iter().copied().find(|(name, _)| *name == k).ok_or_else(|| {
                let names: Vec<_> = worker::KINDS.iter().map(|(n, _)| *n).collect();
                format!("--kinds: unknown kind {k}; use {}", names.join(", "))
            })
        })
        .collect()
}

fn run() -> Result<ExitCode, String> {
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    let mut opts = RunOptions {
        kinds: worker::KINDS.to_vec(),
        max: None,
        until: None,
        dry_run: false,
        power: Power { threads: cores.div_ceil(2), memory: 8 * GB },
        backoff: Duration::from_secs(15),
        max_backoff: Duration::from_secs(10 * 60),
    };
    let mut base = DEFAULT_BASE_URL.to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--kinds" => opts.kinds = kinds(&args.next().ok_or("--kinds needs a list")?)?,
            "--minutes" => {
                let m: f64 = number(&mut args, "--minutes")?;
                opts.until = Some(Instant::now() + Duration::try_from_secs_f64(m * 60.0).map_err(|e| e.to_string())?);
            }
            "--max" => opts.max = Some(number(&mut args, "--max")?),
            "--threads" => opts.power.threads = number::<usize>(&mut args, "--threads")?.max(1),
            "--memory-gb" => opts.power.memory = (number::<f64>(&mut args, "--memory-gb")? * GB as f64) as u64,
            "--dry-run" => opts.dry_run = true,
            "--base" => base = args.next().ok_or("--base needs a URL")?,
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(ExitCode::SUCCESS);
            }
            other => return Err(format!("unknown argument {other}; see --help")),
        }
    }
    if opts.kinds.is_empty() {
        return Err("--kinds names no kind".into());
    }
    if poker_client_running()? {
        return Err("a PokerStars client runs on this computer; Ringside does not compute while it does".into());
    }

    let presses = Arc::new(AtomicU8::new(0));
    let handler = presses.clone();
    ctrlc::set_handler(move || {
        if handler.fetch_add(1, Ordering::SeqCst) == 0 {
            eprintln!("\nstopping: the solve ends, nothing is submitted, the lease goes back (Ctrl+C again quits at once)");
        } else {
            std::process::exit(130);
        }
    })
    .map_err(|e| format!("Ctrl+C handler: {e}"))?;

    let client_seen = AtomicBool::new(false);
    let last_check = Mutex::new(Instant::now());
    let stop = || {
        if presses.load(Ordering::SeqCst) > 0 || client_seen.load(Ordering::Relaxed) {
            return true;
        }
        let mut last = last_check.lock().unwrap();
        if last.elapsed() >= CLIENT_CHECK_EVERY {
            *last = Instant::now();
            // A failed check counts as a client: better to stop than to compute next to it.
            if poker_client_running().unwrap_or(true) {
                client_seen.store(true, Ordering::Relaxed);
                return true;
            }
        }
        false
    };

    let names: Vec<_> = opts.kinds.iter().map(|(n, _)| *n).collect();
    println!(
        "kinds {}, {} threads, {:.0} GB per tree{}{}{}",
        names.join(","),
        opts.power.threads,
        opts.power.memory as f64 / GB as f64,
        opts.max.map_or(String::new(), |m| format!(", at most {m} spots")),
        opts.until.map_or(String::new(), |t| format!(", {:.0} minutes", (t - Instant::now()).as_secs_f64() / 60.0)),
        if opts.dry_run { ", dry run" } else { "" },
    );
    let started = Instant::now();
    let mut client = Client::with_policy(&base, RetryPolicy::default());
    let mut done: Vec<Done> = Vec::new();
    let end = worker::run(&mut client, &opts, &stop, |e| match e {
        Event::Started { key, kind, name } => {
            print!("{kind:<12} {} {name:<28} ", &key[..key.len().min(8)]);
            let _ = std::io::stdout().flush();
        }
        Event::Done(d) => {
            println!("{}", line(d));
            done.push(d.clone());
        }
    });
    summary(&done, started.elapsed());
    let code = match end {
        End::Empty => {
            println!("queue empty for these kinds");
            ExitCode::SUCCESS
        }
        End::Max => ExitCode::SUCCESS,
        End::Time => {
            println!("time is up");
            ExitCode::SUCCESS
        }
        End::Stopped if client_seen.load(Ordering::Relaxed) => {
            eprintln!("stopped: a PokerStars client started");
            ExitCode::FAILURE
        }
        End::Stopped => ExitCode::from(130),
        End::Error(e) => {
            eprintln!("ringside-worker: {e}");
            ExitCode::FAILURE
        }
    };
    Ok(code)
}

fn outcome(o: &Outcome) -> String {
    match o {
        Outcome::Submitted => "submitted".into(),
        Outcome::AlreadyVerified => "not submitted (already verified)".into(),
        Outcome::DryRun => "not submitted (dry run)".into(),
        Outcome::Stopped => "not submitted (stopped)".into(),
        Outcome::Failed(e) => format!("not submitted: {e}"),
    }
}

/// The rest of a spot's line, after kind and name.
fn line(d: &Done) -> String {
    let e = d.exploitability.map_or("-".to_string(), |(v, unit)| format!("{v:.2e} {unit}"));
    format!("{:>8.1}s  {e:<16} {}", d.seconds, outcome(&d.outcome))
}

fn summary(done: &[Done], elapsed: Duration) {
    let count = |f: fn(&Outcome) -> bool| done.iter().filter(|d| f(&d.outcome)).count();
    println!(
        "{} spots in {:.1} min: {} submitted, {} already verified, {} dry run, {} stopped, {} failed",
        done.len(),
        elapsed.as_secs_f64() / 60.0,
        count(|o| *o == Outcome::Submitted),
        count(|o| *o == Outcome::AlreadyVerified),
        count(|o| *o == Outcome::DryRun),
        count(|o| *o == Outcome::Stopped),
        count(|o| matches!(o, Outcome::Failed(_))),
    );
    let mut by_kind: BTreeMap<&str, (u32, f64)> = BTreeMap::new();
    for d in done.iter().filter(|d| matches!(d.outcome, Outcome::Submitted | Outcome::AlreadyVerified | Outcome::DryRun)) {
        let e = by_kind.entry(&d.kind).or_default();
        *e = (e.0 + 1, e.1 + d.seconds);
    }
    for (kind, (n, seconds)) in by_kind {
        println!("  {kind:<12} {n} solved, {:.1} s each", seconds / n as f64);
    }
}

/// Whether a PokerStars client runs here: like the app, any process whose executable
/// name starts with "pokerstars", in any case.
fn poker_client_running() -> Result<bool, String> {
    let names = process_names()?;
    Ok(names.iter().any(|n| is_poker_client(n)))
}

fn is_poker_client(name: &str) -> bool {
    let exe = name.trim().rsplit(['/', '\\']).next().unwrap_or_default();
    exe.to_lowercase().starts_with("pokerstars")
}

#[cfg(windows)]
fn process_names() -> Result<Vec<String>, String> {
    let out = Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output().map_err(|e| format!("tasklist: {e}"))?;
    if !out.status.success() {
        return Err(format!("tasklist failed ({})", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split("\",\"").next().map(|n| n.trim_matches('"').to_string()))
        .collect())
}

#[cfg(not(windows))]
fn process_names() -> Result<Vec<String>, String> {
    let out = Command::new("ps").args(["-A", "-o", "comm="]).output().map_err(|e| format!("ps: {e}"))?;
    if !out.status.success() {
        return Err(format!("ps failed ({})", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_client_by_its_executable() {
        for name in [
            "PokerStars.exe",
            "C:\\Program Files\\PokerStars.DE\\PokerStarsDE.exe",
            "/Applications/PokerStars.app/Contents/MacOS/PokerStars",
            "pokerstarsupdate",
        ] {
            assert!(is_poker_client(name), "{name}");
        }
        for name in ["/Applications/PokerStars.app/Contents/MacOS/Helper", "ringside", "stars"] {
            assert!(!is_poker_client(name), "{name}");
        }
    }

    #[test]
    fn reads_the_process_list() {
        assert!(!process_names().unwrap().is_empty());
    }

    #[test]
    fn kinds_are_checked() {
        assert_eq!(kinds("postflop, pushfold-mw").unwrap().iter().map(|k| k.0).collect::<Vec<_>>(), ["postflop", "pushfold-mw"]);
        assert!(kinds("postflop,nope").is_err());
    }
}
