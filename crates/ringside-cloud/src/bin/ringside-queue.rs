//! Operator tool for the community queue ("Mitrechnen"): shows what is open and seeds it
//! with catalog spots and push/fold gaps. Needs the full database token in `.env`.

use std::path::PathBuf;
use std::process::ExitCode;

use ringside_cloud::operator::{Db, catalog_seeds, load_env, preflop_seeds, queue_counts, seed_work};

const HELP: &str = "\
Usage: ringside-queue [--seed catalog|preflop|all] [--dry-run] [--env-file PATH]

Shows the open spots of Ringside Cloud's community queue by kind and source. With
--seed, first adds the library catalog's spots, the push/fold ICM gaps, or both; spots
already queued or verified are left alone. Reads BUNNY_DATABASE_URL and
BUNNY_DATABASE_AUTH_TOKEN from .env (searched from the working directory upwards) or the
environment.

  --seed WHICH     catalog, preflop or all
  --dry-run        only count the seeds, touch nothing
  --env-file PATH  use this .env file
  --help           this text";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ringside-queue: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut which: Option<String> = None;
    let mut dry_run = false;
    let mut env_file: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => which = Some(args.next().ok_or("--seed needs catalog, preflop or all")?),
            "--dry-run" => dry_run = true,
            "--env-file" => env_file = Some(args.next().ok_or("--env-file needs a path")?.into()),
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}; see --help")),
        }
    }
    let mut seeds = Vec::new();
    match which.as_deref() {
        None => {}
        Some("catalog") => seeds = catalog_seeds()?,
        Some("preflop") => seeds = preflop_seeds()?,
        Some("all") => {
            seeds = catalog_seeds()?;
            seeds.extend(preflop_seeds()?);
        }
        Some(other) => return Err(format!("--seed {other}: use catalog, preflop or all")),
    }
    if dry_run {
        println!("{} seeds (dry run, nothing stored)", seeds.len());
        return Ok(());
    }
    let db = Db::from_env(&load_env(env_file.as_deref())?)?;
    if !seeds.is_empty() {
        println!("added {} of {} seeds", seed_work(&db, &seeds)?, seeds.len());
    }
    for (kind, source, n, leased) in queue_counts(&db)? {
        println!("{kind:<12} {source:<8} {n:>6} open, {leased} leased");
    }
    Ok(())
}
