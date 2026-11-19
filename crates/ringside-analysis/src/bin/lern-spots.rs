//! Builds the learning spots pack `data/lern-spots.json` from `data/lern-spots-spec.json`,
//! and the browser preview's sample `src/lib/learn/demo-spots.json`:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin lern-spots -- [--spec FILE] [--threads T] [--memory-gb G] [--only ID,ID] [--cache DIR] [--minutes M]
//! ```
//!
//! Every spot is rated by the analysis (push/fold equilibrium, preflop model, postflop
//! solver from the flop, ICM for bubble spots) and kept in `--cache` (default
//! `target/lern-spots`), so a run that stops after `--minutes` goes on where it stopped.
//! The pack is written once every spot of the spec is in the cache. Spots with the same
//! flop, ranges and sizes share one solve when they follow each other in the spec.
//!
//! `--scan` rates every hand class of each preflop spec instead and prints one JSON line
//! per class with the grade of every choice, to pick hands whose answer is clear (bubble
//! spots are skipped).

use std::path::{Path, PathBuf};
use std::time::Instant;

use ringside_analysis::ENGINE;
use ringside_analysis::drill::answer;
use ringside_analysis::spots::{LernSpot, Pack, PackFile, PackTopic, PackedSpot, Specs, TOPICS, demo_sample, build, hand_id, hand_of, rate_preflop};
use ringside_engine::HandClass;

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let threads: usize = arg(&args, "--threads").map_or(10, |t| t.parse().expect("--threads"));
    let memory: u64 = arg(&args, "--memory-gb").map_or(24, |g| g.parse().expect("--memory-gb")) << 30;
    let minutes: f64 = arg(&args, "--minutes").map_or(25.0, |m| m.parse().expect("--minutes"));
    let only: Option<Vec<&str>> = arg(&args, "--only").map(|o| o.split(',').collect());
    let cache: PathBuf = arg(&args, "--cache").map_or_else(|| root.join("../../target/lern-spots"), PathBuf::from);
    std::fs::create_dir_all(&cache).expect("cache directory");

    let spec_file: PathBuf = arg(&args, "--spec").map_or_else(|| root.join("data/lern-spots-spec.json"), PathBuf::from);
    let specs: Specs = serde_json::from_str(&std::fs::read_to_string(&spec_file).expect("spec")).expect("spec parses");
    if args.iter().any(|a| a == "--scan") {
        scan(&specs);
        return;
    }
    let mut solved = None;
    let started = Instant::now();
    let mut failed = Vec::new();
    for spec in &specs.spots {
        if only.as_ref().is_some_and(|o| !o.contains(&spec.id.as_str())) {
            continue;
        }
        let file = cache.join(format!("{}.json", spec.id));
        let fresh = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<(String, ringside_analysis::spots::Spec, LernSpot)>(&t).ok())
            .is_some_and(|(engine, s, _)| engine == ENGINE && s == *spec);
        if fresh {
            continue;
        }
        if started.elapsed().as_secs_f64() > minutes * 60.0 {
            println!("Zeit um – der nächste Lauf macht bei {} weiter.", spec.id);
            break;
        }
        let t = Instant::now();
        match build(spec, threads, memory, &mut solved) {
            Ok(spot) => {
                let d = &spot.decision;
                let mix: Vec<String> = (0..d.strategy.len())
                    .map(|i| {
                        let c = &d.strategy[i];
                        let a = answer(d, i).expect("every choice grades");
                        format!("{} {:.0} % ({:+.2}) {:?} −{:.2}", c.label, c.freq * 100.0, c.ev_bb.unwrap_or(f64::NAN), a.decision.rating.unwrap(), a.decision.ev_loss_bb.unwrap_or(0.0))
                    })
                    .collect();
                println!("{:<28} {:>6.1} s  {:?}  {}", spec.id, t.elapsed().as_secs_f64(), d.source.unwrap(), mix.join(" · "));
                std::fs::write(&file, serde_json::to_string(&(ENGINE, spec, &spot)).unwrap()).expect("cache write");
            }
            Err(e) => {
                println!("{:<28} FEHLER {e}", spec.id);
                failed.push(spec.id.clone());
            }
        }
    }
    println!("Rechenzeit dieses Laufs: {:.1} min", started.elapsed().as_secs_f64() / 60.0);
    if !failed.is_empty() {
        println!("Fehlgeschlagen: {}", failed.join(", "));
        std::process::exit(1);
    }
    let mut spots = Vec::new();
    let mut packed = Vec::new();
    for spec in &specs.spots {
        let Ok(text) = std::fs::read_to_string(cache.join(format!("{}.json", spec.id))) else {
            println!("Noch nicht gerechnet: {} – Paket bleibt, wie es ist.", spec.id);
            return;
        };
        let (_, _, spot): (String, ringside_analysis::spots::Spec, LernSpot) = serde_json::from_str(&text).expect("cache entry");
        packed.push(PackedSpot::new(spec, &spot).expect("the spec builds its hand history"));
        spots.push(spot);
    }
    if spec_file != root.join("data/lern-spots-spec.json") {
        println!("Alle {} Spots von {} gerechnet (Paket nur aus der Spezifikation im Repo).", spots.len(), spec_file.display());
        return;
    }
    let topics: Vec<PackTopic> = TOPICS.iter().map(PackTopic::from).collect();
    let file = PackFile { engine: ENGINE.into(), topics: topics.clone(), spots: packed };
    let out = root.join("data/lern-spots.json");
    let text = serde_json::to_string(&file).unwrap();
    std::fs::write(&out, &text).expect("pack write");
    println!("Paket geschrieben: {} Spots, {} KB → {}", file.spots.len(), text.len() / 1024, out.display());
    let demo = demo_sample(&Pack { engine: ENGINE.into(), topics, spots });
    let out = root.join("../../src/lib/learn/demo-spots.json");
    let text = serde_json::to_string(&demo).unwrap();
    std::fs::write(&out, &text).expect("demo write");
    println!("Vorschau-Auswahl geschrieben: {} Spots, {} KB → {}", demo.spots.len(), text.len() / 1024, out.display());
}

fn scan(specs: &Specs) {
    // Bubble spots solve with ICM per hand, too slow for every class: their grid shows it.
    for spec in specs.spots.iter().filter(|s| s.payouts.is_empty()) {
        for class in HandClass::all() {
            let (a, b) = class.combos()[0].cards();
            let mut s = spec.clone();
            s.cards = format!("{a}{b}");
            let Ok((hand, index)) = hand_of(&s, hand_id(&s.id)) else { continue };
            let Ok(d) = rate_preflop(&s, &hand, index) else { continue };
            let choices: Vec<serde_json::Value> = (0..d.strategy.len())
                .map(|i| {
                    let c = &d.strategy[i];
                    let r = answer(&d, i).expect("every choice grades").decision;
                    serde_json::json!({ "label": c.label, "kind": c.kind, "freq": c.freq, "ev": c.ev_bb, "rating": r.rating, "loss": r.ev_loss_bb })
                })
                .collect();
            println!("{}", serde_json::json!({ "id": spec.id, "class": class.to_string(), "cards": s.cards, "choices": choices }));
        }
    }
}
