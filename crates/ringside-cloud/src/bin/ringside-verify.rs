//! Trusted verifier. Solutions are served as soon as the API finds their form right; this
//! re-solves a sample of them, clients new to it first, and bans a client whose sample
//! fails, withdrawing all its accepted solutions. Runs hourly in the repository's
//! `verify` workflow, or by hand with the full database token.
//!
//! Anyone can submit, so every check runs in a child process that is killed after a time
//! limit, and a run stops starting checks once its budget is used up. A check that runs
//! out of time or crashes gives no verdict: the solution stays as it is.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use ringside_cloud::kinds::{self, KINDS, KindEntry, Verdict};
use ringside_cloud::operator::{
    Attempt, Db, Form, SampleOptions, accept_submitted, ban_client, is_client_id, load_env, now, prune, purge_public,
    sample, unban_client, withdraw_client,
};
use serde::{Deserialize, Serialize};

const HELP: &str = "\
Usage: ringside-verify [--samples N] [--minutes M] [--max-seconds S] [--budget-seconds S]
                       [--dry-run] [--env-file PATH]
       ringside-verify --ban-client ID | --withdraw-client ID | --unban-client ID
                       [--env-file PATH]

Ringside Cloud serves every solution whose form checks out at once. A run of this tool
  1. checks the form of rows still waiting from before that and accepts or rejects them,
  2. re-solves one random accepted solution of each of N clients, the ones it has
     verified least first; a client whose sample fails is banned and all its accepted
     solutions are withdrawn (verified ones stay),
  3. deletes rate-limit counters older than two days, parser reports and submission log
     entries older than 90 days and queue entries whose spot is served.
Reads BUNNY_DATABASE_URL and BUNNY_DATABASE_AUTH_TOKEN from .env (searched from the
working directory upwards) or the environment; with BUNNY_API_KEY it also drops
withdrawn solutions from the CDN, else they leave it within a day.

  --samples N          clients to sample (default 5)
  --minutes M          start no check that could end after M minutes (same as
                       --budget-seconds M*60)
  --max-seconds S      a check that takes longer gives no verdict (default: the kind's
                       own limit, 120 s for push/fold, 900 s for postflop)
  --budget-seconds S   start no check that could end after S seconds (default 2400)
  --dry-run            only print what would happen, change and delete nothing
  --ban-client ID      withdraw all accepted solutions of a client and refuse its
                       submissions; ID is the 32-digit hash in solutions.submitter
  --withdraw-client ID only withdraw its accepted solutions
  --unban-client ID    accept its submissions again (withdrawn ones stay withdrawn)
  --env-file PATH      use this .env file
  --help               this text

Prints solution keys, client hashes and verdicts only, never the database's address or a
key. Exit code 0 when the run completed (failed samples included), 1 on errors.";

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

/// What a client command does.
#[derive(Clone, Copy, PartialEq)]
enum ClientAction {
    Ban,
    Withdraw,
    Unban,
}

fn run() -> Result<(), String> {
    let mut samples = 5usize;
    let mut max_seconds: Option<u64> = None;
    let mut budget_seconds = 2400u64;
    let mut dry_run = false;
    let mut env_file: Option<PathBuf> = None;
    let mut client: Option<(ClientAction, String)> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let action = match arg.as_str() {
            "--ban-client" => Some(ClientAction::Ban),
            "--withdraw-client" => Some(ClientAction::Withdraw),
            "--unban-client" => Some(ClientAction::Unban),
            _ => None,
        };
        if let Some(action) = action {
            let id = args.next().filter(|id| is_client_id(id)).ok_or(format!("{arg} needs a 32-digit client hash"))?;
            if client.replace((action, id)).is_some() {
                return Err("only one client command at a time".into());
            }
            continue;
        }
        match arg.as_str() {
            "--samples" => samples = number(&mut args, "--samples")?,
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
    let env = load_env(env_file.as_deref())?;
    let db = Db::from_env(&env)?;
    let api_key = env.get("BUNNY_API_KEY").filter(|k| !k.is_empty()).cloned();
    let purge = |withdrawn: &[(String, String)]| {
        if withdrawn.is_empty() {
            return Ok(());
        }
        match &api_key {
            Some(key) => purge_public(key, withdrawn).map(|()| println!("cdn: purged {} URL(s)", withdrawn.len())),
            None => {
                println!("cdn: BUNNY_API_KEY not set, withdrawn solutions leave the CDN within a day");
                Ok(())
            }
        }
    };

    if let Some((action, id)) = client {
        if action == ClientAction::Unban {
            let lifted = unban_client(&db, &id)?;
            println!("{}", if lifted { format!("client {id} may submit again") } else { format!("client {id} was not banned") });
            return Ok(());
        }
        let reason = "withdrawn by the operator";
        if action == ClientAction::Ban {
            ban_client(&db, &id, reason)?;
            println!("client {id} banned");
        }
        let withdrawn = withdraw_client(&db, &id, reason)?;
        println!("{} solutions of client {id} withdrawn", withdrawn.len());
        return purge(&withdrawn);
    }

    let kinds: Vec<String> = KINDS.iter().map(|k| format!("{}/{}", k.name, k.engine_version)).collect();
    println!("kinds: {}", kinds.join(", "));
    let started_run = Instant::now();
    let started_unix = now();

    let formed = accept_submitted(&db, dry_run)?;
    let mut counts = (0, 0, 0);
    for f in &formed {
        match &f.form {
            Form::Accepted(_) => counts.0 += 1,
            Form::Rejected(reason) => {
                counts.1 += 1;
                println!("rejected {} {} form: {reason}", f.kind, f.key);
            }
            Form::Unknown => counts.2 += 1,
        }
    }
    if !formed.is_empty() {
        println!("waiting rows: {} accepted, {} rejected, {} of unknown kind or version left", counts.0, counts.1, counts.2);
    }

    let exe = std::env::current_exe().map_err(|e| format!("own executable: {e}"))?;
    let deadline = started_run + Duration::from_secs(budget_seconds);
    let check = |entry: &KindEntry, key: &str, input: &str, result: &str| {
        let limit = Duration::from_secs(max_seconds.unwrap_or(entry.check_seconds));
        // A check starts only if it can end within the budget; the row waits for the next run.
        if Instant::now() + limit > deadline + Duration::from_secs(budget_slack(budget_seconds)) {
            return Attempt::PutOff;
        }
        let started = Instant::now();
        let attempt = in_child(&exe, entry, key, input, result, limit);
        println!("  checked {} {key} in {:.1}s", entry.name, started.elapsed().as_secs_f64());
        attempt
    };
    let opts = SampleOptions { clients: samples, tried_since: started_unix, deadline: Some(deadline), dry_run };
    let sampled = sample(&db, &opts, &check)?;
    let mut withdrawn = Vec::new();
    for s in &sampled {
        match &s.attempt {
            Attempt::Verdict(Verdict::Verified { exploitability, .. }) => {
                println!("verified {} {} of client {} exploitability {exploitability:.2e}", s.kind, s.key, s.submitter);
            }
            Attempt::Verdict(Verdict::Rejected { reason, .. }) => {
                println!("FAILED   {} {} of client {}: {reason}", s.kind, s.key, s.submitter);
                if !dry_run {
                    println!("         client {} banned, {} solutions withdrawn", s.submitter, s.withdrawn.len());
                }
            }
            Attempt::NoVerdict(reason) => println!("no verdict {} {} of client {}: {reason}", s.kind, s.key, s.submitter),
            Attempt::PutOff => println!("skipped  {} {} (unknown kind or engine version, or no time left)", s.kind, s.key),
        }
        withdrawn.extend(s.withdrawn.iter().cloned());
    }
    println!(
        "{} samples in {:.1} min{}",
        sampled.len(),
        started_run.elapsed().as_secs_f64() / 60.0,
        if dry_run { " (dry run, nothing stored)" } else { "" }
    );
    if !dry_run {
        purge(&withdrawn)?;
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
