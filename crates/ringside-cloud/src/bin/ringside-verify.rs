//! Trusted verifier: checks submitted solutions with Ringside's engine and marks them
//! verified or rejected. Runs hourly in the repository's `verify` workflow, or by hand
//! with the full database token.
//!
//! Anyone can submit, so every check runs in a child process that is killed after a time
//! limit, and a run stops starting checks once its budget is used up. A check that runs
//! out of time or crashes gives no verdict: the solution stays pending for the next run.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use ringside_cloud::kinds::{self, KINDS, KindEntry, Verdict};
use ringside_cloud::operator::{Attempt, Db, VerifyOptions, load_env, now, prune, verify_pending};
use serde::{Deserialize, Serialize};

const HELP: &str = "\
Usage: ringside-verify [--limit N] [--max-slow N] [--minutes M] [--max-seconds S]
                       [--budget-seconds S] [--dry-run] [--env-file PATH]

Checks pending solutions in Ringside Cloud, oldest first, and sets each to verified or
rejected. Reads BUNNY_DATABASE_URL and BUNNY_DATABASE_AUTH_TOKEN from .env (searched from
the working directory upwards) or the environment. Also deletes rate-limit counters older
than two days and parser reports and submission log entries older than 90 days.

To check many by hand, run it locally with the operator's .env, e.g.

  cargo run --release -p ringside-cloud --bin ringside-verify -- \\
    --env-file path/to/.env --limit 1000 --max-slow 1000 --minutes 120

  --limit N           check at most N solutions, read 50 at a time (default 50)
  --max-slow N        check at most N postflop spots (catalog or shared, each several
                      solves); the rest waits for the next run (default 4)
  --minutes M         start no check that could end after M minutes (same as
                      --budget-seconds M*60)
  --max-seconds S     reject a solution whose check takes longer (default: the kind's
                      own limit, 120 s for push/fold, 900 s for postflop)
  --budget-seconds S  start no check that could end after S seconds; the rest waits
                      (default 2400)
  --dry-run           only print verdicts, change and delete nothing
  --env-file PATH     use this .env file
  --help              this text

Prints solution keys and verdicts only, never the database's address or token.
Exit code 0 when the run completed (rejections included), 1 on errors.";

/// Rows read per query: a postflop result is up to 180 KB.
const BATCH: u32 = 50;

/// Kinds whose check may take longer than this count against `--max-slow`.
const SLOW_SECONDS: u64 = 120;

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
    let mut max_slow = 4u32;
    let mut max_seconds: Option<u64> = None;
    let mut budget_seconds = 2400u64;
    let mut dry_run = false;
    let mut env_file: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--limit" => limit = number(&mut args, "--limit")?,
            "--max-slow" => max_slow = number(&mut args, "--max-slow")?,
            "--minutes" => budget_seconds = number::<u64>(&mut args, "--minutes")? * 60,
            "--max-seconds" => max_seconds = Some(number(&mut args, "--max-seconds")?),
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
    let started_run = Instant::now();
    let deadline = started_run + Duration::from_secs(budget_seconds);
    let slow_started = std::cell::Cell::new(0u32);
    let check = |entry: &KindEntry, key: &str, input: &str, result: &str| {
        let slow = entry.check_seconds > SLOW_SECONDS;
        if slow && slow_started.get() >= max_slow {
            return Attempt::PutOff;
        }
        let limit = Duration::from_secs(max_seconds.unwrap_or(entry.check_seconds));
        // A slow kind starts only if its check can end within the budget; the row stays
        // pending for the next run.
        if Instant::now() + limit > deadline + Duration::from_secs(budget_slack(budget_seconds)) {
            return Attempt::PutOff;
        }
        if slow {
            slow_started.set(slow_started.get() + 1);
        }
        let started = Instant::now();
        let attempt = in_child(&exe, entry, key, input, result, limit);
        println!("  checked {} {key} in {:.1}s", entry.name, started.elapsed().as_secs_f64());
        attempt
    };
    let started_unix = now();
    let (mut verified, mut rejected, mut no_verdict, mut skipped) = (0, 0, 0, 0);
    let mut read = 0u32;
    // Rows a batch leaves pending stay first in line, so the next batch passes over them.
    let mut pending_ahead = 0u32;
    while read < limit && Instant::now() < deadline {
        let batch = BATCH.min(limit - read);
        let opts = VerifyOptions {
            keys: None,
            limit: batch,
            offset: pending_ahead,
            tried_since: Some(started_unix),
            deadline: Some(deadline),
            dry_run,
        };
        let checked = verify_pending(&db, &opts, &check)?;
        read += checked.len() as u32;
        for c in &checked {
            match &c.attempt {
                Attempt::Verdict(Verdict::Verified { exploitability, .. }) => {
                    verified += 1;
                    println!("verified {} {} exploitability {exploitability:.2e}", c.kind, c.key);
                }
                Attempt::Verdict(Verdict::Rejected { reason, .. }) => {
                    rejected += 1;
                    println!("rejected {} {} {reason}", c.kind, c.key);
                }
                Attempt::NoVerdict(reason) => {
                    no_verdict += 1;
                    println!("pending  {} {} no verdict: {reason}", c.kind, c.key);
                }
                Attempt::PutOff => {
                    skipped += 1;
                    println!(
                        "skipped  {} {} (unknown kind or engine version, no time left or slow checks used up)",
                        c.kind, c.key
                    );
                }
            }
            // Rows tried without a verdict leave the line (`tried_since`); put-off ones and,
            // in a dry run, all stay in place.
            if dry_run || c.attempt == Attempt::PutOff {
                pending_ahead += 1;
            }
        }
        if checked.len() < batch as usize {
            break;
        }
    }
    println!(
        "{verified} verified, {rejected} rejected, {no_verdict} without verdict (still pending), {skipped} skipped in {:.1} min{}",
        started_run.elapsed().as_secs_f64() / 60.0,
        if dry_run { " (dry run, nothing stored)" } else { "" }
    );
    if !dry_run {
        let p = prune(&db)?;
        println!(
            "deleted {} rate-limit counters, {} reports, {} submission log entries and {} solved queue entries",
            p.rate_limits, p.reports, p.submissions, p.work
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
    /// The result to store instead of the submitted one.
    result: Option<String>,
}

/// How far past the budget a check may run: a tenth of it, so a run of the default
/// budget still ends well inside the workflow's timeout.
fn budget_slack(budget_seconds: u64) -> u64 {
    budget_seconds / 10
}

/// Why a child gave no output to judge.
#[derive(Debug, PartialEq)]
enum ChildError {
    TimedOut,
    Failed(String),
}

/// Runs `cmd` with `stdin` as its input until it exits or `limit` passes, and returns its
/// exit status and output. Output is read while the child runs: a stored spot is larger
/// than a pipe's buffer, and a child blocked on a full pipe never exits.
fn run_child(mut cmd: Command, stdin: Vec<u8>, limit: Duration) -> Result<(ExitStatus, String), ChildError> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| ChildError::Failed(format!("could not start the check: {e}")))?;
    let mut input = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(s) = input.as_mut() {
            let _ = s.write_all(&stdin);
        }
    });
    let mut output = child.stdout.take();
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        if let Some(s) = output.as_mut() {
            let _ = s.read_to_string(&mut out);
        }
        out
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() >= limit => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(ChildError::TimedOut);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(ChildError::Failed(format!("check failed: {e}")));
            }
        }
    };
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    status.map(|s| (s, out))
}

/// Runs the check in a child process. Only the check's own answer is a verdict; a check
/// that is too slow or crashes leaves the solution pending.
fn in_child(exe: &PathBuf, entry: &KindEntry, key: &str, input: &str, result: &str, limit: Duration) -> Attempt {
    let job = Job {
        kind: entry.name.into(),
        engine_version: entry.engine_version,
        key: key.into(),
        input: input.into(),
        result: result.into(),
    };
    let mut cmd = Command::new(exe);
    cmd.arg(CHECK_ONE);
    let (status, out) = match run_child(cmd, serde_json::to_vec(&job).unwrap_or_default(), limit) {
        Ok(r) => r,
        Err(ChildError::TimedOut) => return Attempt::NoVerdict(format!("check took longer than {}s", limit.as_secs())),
        Err(ChildError::Failed(e)) => return Attempt::NoVerdict(e),
    };
    match serde_json::from_str::<Outcome>(&out) {
        Ok(o) if status.success() && o.verified => match o.exploitability {
            Some(exploitability) => Attempt::Verdict(Verdict::Verified { exploitability, result: o.result }),
            None => Attempt::NoVerdict("check reported no exploitability".into()),
        },
        Ok(o) if status.success() => Attempt::Verdict(Verdict::Rejected {
            exploitability: o.exploitability,
            reason: o.reason.unwrap_or_else(|| "rejected".into()),
        }),
        _ => Attempt::NoVerdict(format!("check crashed ({status})")),
    }
}

fn check_one() -> Result<(), String> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
    let job: Job = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let entry = kinds::find(&job.kind, job.engine_version).ok_or("unknown kind")?;
    let outcome = match (entry.check)(&job.key, &job.input, &job.result) {
        Verdict::Verified { exploitability, result } => {
            Outcome { verified: true, exploitability: Some(exploitability), reason: None, result }
        }
        Verdict::Rejected { exploitability, reason } => {
            Outcome { verified: false, exploitability, reason: Some(reason), result: None }
        }
    };
    println!("{}", serde_json::to_string(&outcome).map_err(|e| e.to_string())?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set for a child the tests start: what `child_helper` does in it.
    const CHILD: &str = "RINGSIDE_VERIFY_TEST_CHILD";

    /// The test binary itself as the child, so the test runs the same on every platform.
    fn helper(what: &str) -> Command {
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["tests::child_helper", "--exact", "--nocapture", "--test-threads=1"]).env(CHILD, what);
        cmd
    }

    #[test]
    fn child_helper() {
        match std::env::var(CHILD).as_deref() {
            Ok("big") => print!("{}", "x".repeat(1 << 20)),
            Ok("slow") => std::thread::sleep(Duration::from_secs(30)),
            _ => {}
        }
    }

    #[test]
    fn output_larger_than_a_pipe_does_not_block_the_child() {
        let started = Instant::now();
        let (status, out) = run_child(helper("big"), vec![b'x'; 300_000], Duration::from_secs(60)).unwrap();
        assert!(status.success());
        assert!(out.len() >= 1 << 20, "{}", out.len());
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn running_out_of_time_is_no_verdict() {
        let started = Instant::now();
        assert_eq!(run_child(helper("slow"), Vec::new(), Duration::from_millis(300)).unwrap_err(), ChildError::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));

        let entry = kinds::find("pushfold-hu", 1).unwrap();
        let attempt = in_child(&std::env::current_exe().unwrap(), entry, "k", "{}", "{}", Duration::from_millis(300));
        // The test binary does not understand --check-one: no verdict either way.
        assert!(matches!(attempt, Attempt::NoVerdict(_)), "{attempt:?}");
    }
}
