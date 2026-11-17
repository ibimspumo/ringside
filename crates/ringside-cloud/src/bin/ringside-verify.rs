//! Trusted verifier: checks submitted solutions with Ringside's engine and marks them
//! verified or rejected. Runs hourly in the repository's `verify` workflow, or by hand
//! with the full database token.
//!
//! Anyone can submit, so every check runs in a child process that is killed after a time
//! limit, and a run stops starting checks once its budget is used up.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use ringside_cloud::kinds::{self, KINDS, KindEntry, Verdict};
use ringside_cloud::operator::{Db, VerifyOptions, load_env, prune, verify_pending};
use serde::{Deserialize, Serialize};

const HELP: &str = "\
Usage: ringside-verify [--limit N] [--max-seconds S] [--budget-seconds S] [--dry-run] [--env-file PATH]

Checks pending solutions in Ringside Cloud, oldest first, and sets each to verified or
rejected. Reads BUNNY_DATABASE_URL and BUNNY_DATABASE_AUTH_TOKEN from .env (searched from
the working directory upwards) or the environment. Also deletes rate-limit counters older
than two days and parser reports and submission log entries older than 90 days.

  --limit N           check at most N solutions (default 50)
  --max-seconds S     reject a solution whose check takes longer (default 120)
  --budget-seconds S  start no check after S seconds; the rest waits (default 2400)
  --dry-run           only print verdicts, change and delete nothing
  --env-file PATH     use this .env file
  --help              this text

Exit code 0 when the run completed (rejections included), 1 on errors.";

/// Internal: check one row read from stdin and print the verdict; used for the child.
const CHECK_ONE: &str = "--check-one";

fn main() -> ExitCode {
    let result = if std::env::args().nth(1).as_deref() == Some(CHECK_ONE) { check_one() } else { run() };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ringside-verify: {e}");
            ExitCode::FAILURE
        }
    }
}

fn number<T: std::str::FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String> {
    args.next().and_then(|v| v.parse().ok()).ok_or_else(|| format!("{flag} needs a number"))
}

fn run() -> Result<(), String> {
    let mut limit = 50u32;
    let mut max_seconds = 120u64;
    let mut budget_seconds = 2400u64;
    let mut dry_run = false;
    let mut env_file: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--limit" => limit = number(&mut args, "--limit")?,
            "--max-seconds" => max_seconds = number(&mut args, "--max-seconds")?,
            "--budget-seconds" => budget_seconds = number(&mut args, "--budget-seconds")?,
            "--dry-run" => dry_run = true,
            "--env-file" => env_file = Some(args.next().ok_or("--env-file needs a path")?.into()),
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}; see --help")),
        }
    }
    let db = Db::from_env(&load_env(env_file.as_deref())?)?;
    let kinds: Vec<String> = KINDS.iter().map(|k| format!("{}/{}", k.name, k.engine_version)).collect();
    println!("kinds: {}", kinds.join(", "));

    let exe = std::env::current_exe().map_err(|e| format!("own executable: {e}"))?;
    let limit_per_check = Duration::from_secs(max_seconds);
    let check = |entry: &KindEntry, key: &str, input: &str, result: &str| {
        let started = Instant::now();
        let verdict = in_child(&exe, entry, key, input, result, limit_per_check);
        println!("  checked {} {key} in {:.1}s", entry.name, started.elapsed().as_secs_f64());
        verdict
    };
    let opts = VerifyOptions {
        keys: None,
        limit,
        deadline: Some(Instant::now() + Duration::from_secs(budget_seconds)),
        dry_run,
    };
    let checked = verify_pending(&db, &opts, &check)?;
    let (mut verified, mut rejected, mut skipped) = (0, 0, 0);
    for c in &checked {
        match &c.verdict {
            Some(Verdict::Verified { exploitability }) => {
                verified += 1;
                println!("verified {} {} exploitability {exploitability:.2e}", c.kind, c.key);
            }
            Some(Verdict::Rejected { reason, .. }) => {
                rejected += 1;
                println!("rejected {} {} {reason}", c.kind, c.key);
            }
            None => {
                skipped += 1;
                println!("skipped  {} {} (kind or engine version unknown here)", c.kind, c.key);
            }
        }
    }
    println!(
        "{verified} verified, {rejected} rejected, {skipped} skipped{}",
        if dry_run { " (dry run, nothing stored)" } else { "" }
    );
    if !dry_run {
        let p = prune(&db)?;
        println!(
            "deleted {} rate-limit counters, {} reports and {} submission log entries",
            p.rate_limits, p.reports, p.submissions
        );
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Job {
    kind: String,
    engine_version: u32,
    key: String,
    input: String,
    result: String,
}

#[derive(Serialize, Deserialize)]
struct Outcome {
    verified: bool,
    exploitability: Option<f64>,
    reason: Option<String>,
}

fn rejected(reason: String) -> Verdict {
    Verdict::Rejected { exploitability: None, reason }
}

/// Runs the check in a child process; a check that is too slow or crashes is a rejection.
fn in_child(exe: &PathBuf, entry: &KindEntry, key: &str, input: &str, result: &str, limit: Duration) -> Verdict {
    let job = Job {
        kind: entry.name.into(),
        engine_version: entry.engine_version,
        key: key.into(),
        input: input.into(),
        result: result.into(),
    };
    let mut child = match Command::new(exe)
        .arg(CHECK_ONE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return rejected(format!("could not start the check: {e}")),
    };
    let body = serde_json::to_vec(&job).unwrap_or_default();
    let mut stdin = child.stdin.take();
    // The child reads all of stdin before it computes, so writing from a thread can't
    // deadlock against a child that is blocked on output.
    let writer = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(&body);
        }
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() >= limit => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return rejected(format!("check failed: {e}")),
        }
    };
    let _ = writer.join();
    let Some(status) = status else {
        return rejected(format!("check took longer than {}s", limit.as_secs()));
    };
    let mut out = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut out);
    }
    match serde_json::from_str::<Outcome>(&out) {
        Ok(o) if status.success() && o.verified => match o.exploitability {
            Some(exploitability) => Verdict::Verified { exploitability },
            None => rejected("check reported no exploitability".into()),
        },
        Ok(o) if status.success() => Verdict::Rejected {
            exploitability: o.exploitability,
            reason: o.reason.unwrap_or_else(|| "rejected".into()),
        },
        _ => rejected(format!("check crashed ({status})")),
    }
}

fn check_one() -> Result<(), String> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
    let job: Job = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let entry = kinds::find(&job.kind, job.engine_version).ok_or("unknown kind")?;
    let outcome = match (entry.check)(&job.key, &job.input, &job.result) {
        Verdict::Verified { exploitability } => Outcome { verified: true, exploitability: Some(exploitability), reason: None },
        Verdict::Rejected { exploitability, reason } => Outcome { verified: false, exploitability, reason: Some(reason) },
    };
    println!("{}", serde_json::to_string(&outcome).map_err(|e| e.to_string())?);
    Ok(())
}
