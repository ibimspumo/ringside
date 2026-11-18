//! How much of a hand history the analysis rates, by situation and source:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin analysis-coverage -- <dir or file>... [--solve THREADS] [--profile]
//! ```
//!
//! With `--solve` every postflop spot is solved as the app's queue would, so nothing
//! waits any more. `--profile` adds the playing profile's costliest kinds of spot. Hand histories stay where they are; nothing is written.

use std::path::{Path, PathBuf};
use std::time::Instant;

use ringside_analysis::coverage::Coverage;
use ringside_analysis::drill;
use ringside_core::profile::profile;
use ringside_analysis::{PostflopJob, analyze_hand, settle_job};
use ringside_core::model::Hand;
use ringside_core::{parse_hand, split_hands};

fn files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for e in std::fs::read_dir(path).expect("read dir").flatten() {
            files(&e.path(), out);
        }
    } else if path.extension().is_some_and(|x| x == "txt") {
        out.push(path.to_path_buf());
    }
}

fn main() {
    let mut paths = Vec::new();
    let mut threads: Option<usize> = None;
    let mut with_profile = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--solve" => threads = Some(args.next().and_then(|v| v.parse().ok()).expect("--solve THREADS")),
            "--profile" => with_profile = true,
            _ => files(Path::new(&a), &mut paths),
        }
    }
    let mut hands: Vec<Hand> = Vec::new();
    for p in &paths {
        let text = String::from_utf8_lossy(&std::fs::read(p).expect("read file")).into_owned();
        hands.extend(split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok()));
    }
    let t = Instant::now();
    let mut results: Vec<_> = hands.iter().map(analyze_hand).collect();
    println!("{} Hände analysiert in {:.2} s", hands.len(), t.elapsed().as_secs_f64());
    if let Some(threads) = threads {
        let t = Instant::now();
        let mut solved = 0;
        for (hand, r) in hands.iter().zip(results.iter_mut()) {
            let Some((_, json)) = &r.job else { continue };
            let job: PostflopJob = serde_json::from_str(json).expect("job");
            let decisions = settle_job(hand, &job, threads, 8 << 30);
            for d in decisions {
                if let Some(slot) = r.decisions.iter_mut().find(|x| x.action_index == d.action_index) {
                    *slot = d;
                }
            }
            solved += 1;
        }
        println!("{solved} Postflop-Spots gelöst in {:.0} s mit {threads} Threads", t.elapsed().as_secs_f64());
    }
    let mut cov = Coverage::default();
    for (hand, r) in hands.iter().zip(&results) {
        for d in &r.decisions {
            cov.add(hand, d);
        }
    }
    print!("{cov}");
    if with_profile {
        let rows: Vec<_> = hands.iter().zip(&results).flat_map(|(h, r)| r.decisions.iter().map(|d| drill::row(h, d))).collect();
        let p = profile(&rows, &[]);
        println!("\nSpielprofil: {} bewertete Entscheidungen, {:.1} BB Verlust", p.decisions, p.loss_bb);
        for g in p.groups.iter().filter(|g| g.rank.is_some()).take(10) {
            println!(
                "{:>2}. {}, {}: {}× gespielt, Fehlerquote {:.0} %, {:.1} BB gesamt, {:.2} BB pro Mal, berechnet {:.0} %, Einschätzung {:.0} % des Verlusts{}",
                g.rank.unwrap_or(0),
                g.title,
                g.detail,
                g.decisions,
                g.error_pct,
                g.loss_bb,
                g.mean_loss_bb,
                g.computed_pct,
                g.estimate_loss_pct,
                if g.unsure { " (unsicher)" } else { "" }
            );
        }
        let ranked = p.groups.iter().filter(|g| g.rank.is_some()).count();
        println!("{} Spot-Arten, {ranked} mit mindestens {} Entscheidungen", p.groups.len(), p.min_decisions);
    }
}
