//! Operator tool for the community queue ("Mitrechnen"): shows what is open and seeds it
//! with catalog spots, push/fold gaps and the lines practice deals most. Needs the full
//! database token in `.env`.

use std::path::PathBuf;
use std::process::ExitCode;

use ringside_cloud::operator::{
    Db, Seed, catalog_seeds, load_env, practice_flops, practice_lines, practice_seeds, preflop_seeds, queue_counts,
    seed_work, shipped_scenarios, unseed_shipped,
};
use ringside_solver::library::catalog::FLOPS;

const HELP: &str = "\
Usage: ringside-queue [--seed catalog|preflop|all|practice] [--dry-run] [--env-file PATH]
       ringside-queue --seed practice [--players 6] [--stacks 40] [--lines 5] [--flops N]

Shows the open spots of Ringside Cloud's community queue by kind and source. With
--seed, first adds the library catalog's spots, the push/fold ICM gaps, or both; spots
already queued or served are left alone (a withdrawn one goes back in), and catalog
scenarios a signed library pack ships are taken out. Reads BUNNY_DATABASE_URL and
BUNNY_DATABASE_AUTH_TOKEN from .env (searched from the working directory upwards) or the
environment.

--seed practice adds model-line spots (kind spot) for the lines \"Hände spielen\" deals
most at each table size and stack (big-blind ante), on N of the catalog's flops. Practice
deals them in every suit renaming. Up to 20 BB practice plays push/fold, so those stacks
are refused. Lines that are catalog scenarios are skipped (use --seed catalog).

  --seed WHICH     catalog, preflop, all (catalog and preflop) or practice
  --players LIST   practice: table sizes, comma-separated (default 6)
  --stacks LIST    practice: stacks in BB, comma-separated (default 40)
  --lines N        practice: most dealt lines per table (default 5)
  --flops N        practice: catalog flops per line (default and at most 184)
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
    let mut practice = Practice { players: vec![6], stacks: vec![40.0], lines: 5, flops: FLOPS };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => which = Some(args.next().ok_or("--seed needs catalog, preflop, all or practice")?),
            "--players" => practice.players = list(&arg, args.next())?,
            "--stacks" => practice.stacks = list(&arg, args.next())?,
            "--lines" => practice.lines = list(&arg, args.next())?.into_iter().next().unwrap_or(0),
            "--flops" => practice.flops = list(&arg, args.next())?.into_iter().next().unwrap_or(0),
            "--dry-run" => dry_run = true,
            "--env-file" => env_file = Some(args.next().ok_or("--env-file needs a path")?.into()),
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}; see --help")),
        }
    }
    let catalog = matches!(which.as_deref(), Some("catalog" | "all"));
    let shipped = if catalog { shipped_scenarios()? } else { Vec::new() };
    let mut seeds = Vec::new();
    match which.as_deref() {
        None => {}
        Some("catalog") => seeds = catalog_seeds(&shipped)?,
        Some("preflop") => seeds = preflop_seeds()?,
        Some("all") => {
            seeds = catalog_seeds(&shipped)?;
            seeds.extend(preflop_seeds()?);
        }
        Some("practice") => seeds = practice.seeds()?,
        Some(other) => return Err(format!("--seed {other}: use catalog, preflop, all or practice")),
    }
    if catalog {
        println!("shipped in packs, not queued: {}", if shipped.is_empty() { "none".into() } else { shipped.join(", ") });
    }
    if dry_run {
        println!("{} seeds (dry run, nothing stored)", seeds.len());
        return Ok(());
    }
    let db = Db::from_env(&load_env(env_file.as_deref())?)?;
    if catalog {
        println!("took {} shipped catalog spots out", unseed_shipped(&db, &shipped)?);
    }
    if !seeds.is_empty() {
        println!("added {} of {} seeds", seed_work(&db, &seeds)?, seeds.len());
    }
    for (kind, source, n, leased) in queue_counts(&db)? {
        println!("{kind:<12} {source:<8} {n:>6} open, {leased} leased");
    }
    Ok(())
}

/// What `--seed practice` adds.
struct Practice {
    players: Vec<usize>,
    stacks: Vec<f64>,
    lines: usize,
    flops: usize,
}

impl Practice {
    /// The seeds of every table's most dealt lines; prints one row per line.
    fn seeds(&self) -> Result<Vec<Seed>, String> {
        if self.lines == 0 || !(1..=FLOPS).contains(&self.flops) {
            return Err(format!("--lines must be at least 1 and --flops 1 to {FLOPS}"));
        }
        let flops = practice_flops(self.flops);
        let mut out = Vec::new();
        for &players in &self.players {
            for &stack_bb in &self.stacks {
                for (i, line) in practice_lines(players, stack_bb)?.iter().take(self.lines).enumerate() {
                    let seeds = practice_seeds(line, &flops)?;
                    let what = if line.catalog { "catalog line, see --seed catalog".to_string() } else { format!("{} spots", seeds.len()) };
                    println!("{players}p {stack_bb} BB #{}: {} ({:.1} % of flops): {what}", i + 1, line.label(), 100.0 * line.share);
                    out.extend(seeds);
                }
            }
        }
        Ok(out)
    }
}

/// A comma-separated list of numbers after `flag`.
fn list<T: std::str::FromStr>(flag: &str, value: Option<String>) -> Result<Vec<T>, String> {
    let value = value.ok_or(format!("{flag} needs a value"))?;
    value.split(',').map(|v| v.trim().parse().map_err(|_| format!("{flag}: {v} is not a number"))).collect()
}
