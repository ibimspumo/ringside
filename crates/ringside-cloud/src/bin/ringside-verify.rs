//! Trusted verifier: checks submitted solutions with Ringside's engine and marks them
//! verified or rejected. Runs on the maintainer's machine with the full database token.

use std::path::PathBuf;
use std::process::ExitCode;

use ringside_cloud::kinds::{KINDS, Verdict};
use ringside_cloud::operator::{Db, load_env, prune_rate_limits, verify_pending};

const HELP: &str = "\
Usage: ringside-verify [--limit N] [--dry-run] [--env-file PATH]

Checks pending solutions in Ringside Cloud, oldest first, and sets each to verified or
rejected. Reads BUNNY_DATABASE_URL and BUNNY_DATABASE_AUTH_TOKEN from .env (searched from
the working directory upwards) or the environment. Also deletes old rate-limit counters.

  --limit N        check at most N solutions (default 500)
  --dry-run        only print verdicts, change nothing
  --env-file PATH  use this .env file
  --help           this text

Exit code 0 when the run completed (rejections included), 1 on errors.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ringside-verify: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut limit = 500u32;
    let mut dry_run = false;
    let mut env_file: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--limit" => limit = args.next().and_then(|v| v.parse().ok()).ok_or("--limit needs a number")?,
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

    let checked = verify_pending(&db, None, limit, dry_run)?;
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
        println!("pruned {} old rate-limit counters", prune_rate_limits(&db)?);
    }
    Ok(())
}
