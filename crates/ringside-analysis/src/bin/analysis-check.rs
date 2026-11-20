//! Checks every rated decision of a hand history against the hard bounds of
//! [`ringside_analysis::check`], analysed as the app does (postflop estimated instead of
//! solved):
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin analysis-check -- <dir or file>... [--examples N] [--dump FILE]
//! ```
//!
//! Prints the violations by rule, spot and source with the worst examples. `--dump`
//! writes every rated decision (hand, action, spot, source, loss, played, best, cards, note) as TSV,
//! for comparing two engine versions. Hand histories stay where they are.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use ringside_analysis::check::{Violation, check, chips};
use ringside_analysis::{Status, analyze_hand, estimate_rest};
use ringside_core::model::{Hand, hand_label};
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
    let mut examples = 4usize;
    let mut dump: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--examples" => examples = args.next().and_then(|v| v.parse().ok()).expect("--examples N"),
            "--dump" => dump = Some(PathBuf::from(args.next().expect("--dump FILE"))),
            _ => files(Path::new(&a), &mut paths),
        }
    }
    paths.sort();
    let mut hands: Vec<Hand> = Vec::new();
    for p in &paths {
        let text = String::from_utf8_lossy(&std::fs::read(p).expect("read file")).into_owned();
        hands.extend(split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok()));
    }
    hands.sort_by_key(|h| h.id);
    hands.dedup_by_key(|h| h.id);

    let mut out = dump.map(|p| std::io::BufWriter::new(std::fs::File::create(p).expect("dump file")));
    let mut groups: BTreeMap<(&'static str, String, String), (usize, f64)> = BTreeMap::new();
    let mut worst: BTreeMap<&'static str, Vec<(f64, String)>> = BTreeMap::new();
    let (mut rated, mut flagged) = (0usize, 0usize);
    for hand in &hands {
        let a = analyze_hand(hand);
        let decisions = if a.decisions.iter().any(|d| d.status == Status::Pending) { estimate_rest(hand, a.decisions, "geschätzt") } else { a.decisions };
        let hero = hand.hero.clone().unwrap_or_default();
        for d in decisions.iter().filter(|d| d.status == Status::Rated) {
            rated += 1;
            let spot = format!("{:?}", d.spot);
            let source = d.source.map_or("-".into(), |s| format!("{s:?}"));
            if let Some(w) = out.as_mut() {
                writeln!(
                    w,
                    "{}\t{}\t{spot}\t{source}\t{:.3}\t{}\t{}\t{}\t{}",
                    hand_label(hand.id),
                    d.action_index,
                    d.ev_loss_bb.unwrap_or(0.0),
                    d.played,
                    d.best.as_deref().unwrap_or("-"),
                    hand.hero_cards.join(""),
                    d.note.replace(['\t', '\n'], " ")
                )
                .expect("dump");
            }
            let found: Vec<Violation> = check(hand, d);
            if found.is_empty() {
                continue;
            }
            flagged += 1;
            let c = chips(hand, &hero, d.action_index);
            for f in found {
                let g = groups.entry((f.rule, spot.clone(), source.clone())).or_default();
                g.0 += 1;
                g.1 = g.1.max(f.by);
                let strategy: Vec<String> =
                    d.strategy.iter().map(|x| format!("{} {:.0}% {}", x.label, x.freq * 100.0, x.ev_bb.map_or("-".into(), |e| format!("{e:+.2}")))).collect();
                let line = format!(
                    "{} #{} {} {spot}/{source}: {} | Stack {:.1}, im Spiel {:.1}, Pot {:.2}, Call {:.2} | gespielt {} | {} | Verlust {:.2}",
                    hand_label(hand.id),
                    d.action_index,
                    hand.hero_cards.join(""),
                    f.detail,
                    c.map_or(0.0, |c| c.hero_in + c.behind),
                    c.map_or(0.0, |c| c.depth),
                    c.map_or(0.0, |c| c.pot),
                    c.map_or(0.0, |c| c.to_call),
                    d.played,
                    strategy.join(", "),
                    d.ev_loss_bb.unwrap_or(0.0)
                );
                worst.entry(f.rule).or_default().push((f.by, line));
            }
        }
    }
    println!("{} Hände, {rated} bewertete Entscheidungen, {flagged} mit Verstoß", hands.len());
    println!("\nRegel        Spot            Quelle        Anzahl   größter Abstand");
    for ((rule, spot, source), (n, by)) in &groups {
        println!("{rule:<12} {spot:<15} {source:<13} {n:>6}   {by:>8.2}");
    }
    for (rule, mut list) in worst {
        list.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("\n{rule}: {} Verstöße, die größten:", list.len());
        for (_, line) in list.iter().take(examples) {
            println!("  {line}");
        }
    }
}
