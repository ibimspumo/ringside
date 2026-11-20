//! The Spielwertung of a hand history, for calibrating its scale:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin spielwertung -- <dir or file>... [--hero NAME] [--brief]
//! ```
//!
//! Every hand is analysed as the app does; postflop spots that would wait for the solver
//! are estimated instead, so a run takes seconds. Prints the rating after every
//! tournament, the areas and what moved the last one; `--brief` only the final line.
//! Hand histories stay where they are; nothing is written.

use std::path::{Path, PathBuf};

use ringside_analysis::{Status, analyze_hand, estimate_rest};
use ringside_core::db::{effective_stack_bb, own_stack_bb};
use ringside_core::model::Street;
use ringside_core::skill::{Move, clear_fold, profile, rate_at};
use ringside_core::stats::position_labels;
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
    let mut brief = false;
    let mut hero_name: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--brief" => brief = true,
            "--hero" => hero_name = args.next(),
            _ => files(Path::new(&a), &mut paths),
        }
    }
    paths.sort();
    let mut moves: Vec<Move> = Vec::new();
    for p in &paths {
        let text = String::from_utf8_lossy(&std::fs::read(p).expect("read file")).into_owned();
        for raw in split_hands(&text) {
            let Ok(hand) = parse_hand(raw) else { continue };
            let Some(hero) = hand.hero.clone() else { continue };
            if hero_name.as_ref().is_some_and(|n| *n != hero) {
                continue;
            }
            let a = analyze_hand(&hand);
            let decisions = if a.decisions.iter().any(|d| d.status == Status::Pending) {
                estimate_rest(&hand, a.decisions, "Ohne Solver geschätzt.")
            } else {
                a.decisions
            };
            let bb = hand.big_blind.max(1) as f64;
            let position = position_labels(&hand).into_iter().find(|(n, _)| *n == hero).map(|(_, p)| p.to_string());
            for d in decisions {
                moves.push(Move {
                    hand_id: hand.id,
                    action_index: d.action_index,
                    tournament_id: hand.tournament_id,
                    played_at: hand.played_at.clone(),
                    spot: d.spot,
                    status: d.status,
                    rating: d.rating,
                    source: d.source,
                    reliability: d.reliability,
                    ev_loss_bb: d.ev_loss_bb,
                    pot_bb: d.pot_bb,
                    chips_bb: own_stack_bb(&hand, &hero),
                    stack_bb: effective_stack_bb(&hand, &hero),
                    position: position.clone(),
                    hero_net_bb: Some(hand.net(&hero) as f64 / bb),
                    hero_cards: hand.hero_cards.join(""),
                    clear_fold: d.street == Street::Preflop && clear_fold(&d.strategy, d.pot_bb),
                    played: d.played,
                    best: d.best,
                });
            }
        }
    }
    let p = profile(&moves);
    let counted: Vec<_> = p.entries.iter().filter(|e| e.performance.is_some()).collect();
    let steps: Vec<f64> = counted.iter().skip(10).map(|e| e.delta.abs()).collect();
    let mut sorted = steps.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted.get(sorted.len() / 2).copied().unwrap_or(0.0);
    let burn: f64 = p.entries.iter().map(|e| e.burned_pct).sum::<f64>() / 100.0;
    let real: f64 = p.entries.iter().map(|e| e.real).sum();
    println!(
        "Wertung {:.0} ± {:.0} ({}) nach {} Turnieren ({} gezählt), {} bewertete Entscheidungen, Fehlerquote {:.4} (Wertung {:.0} entspricht {:.4}), Schritt ab dem 11. Turnier: Median {:.0}",
        p.rating.unwrap_or(p.start),
        p.sigma,
        p.tier.unwrap_or("-"),
        p.entries.len(),
        counted.len(),
        p.grades.total(),
        if real > 0.0 { burn / real } else { 0.0 },
        p.rating.unwrap_or(p.start),
        rate_at(p.rating.unwrap_or(p.start)),
        median
    );
    if brief {
        return;
    }
    for (i, e) in p.entries.iter().enumerate() {
        println!(
            "{:>3}. {} {:>5.0} ± {:>3.0} {:>+5.0}  Leistung {:>5}  {:>4} Entsch., {:>5.1} echte, {:>5.1} % Stack verloren, grob {}",
            i + 1,
            e.started_at,
            e.rating,
            e.sigma,
            e.delta,
            e.performance.map_or("-".to_string(), |x| format!("{x:.0}")),
            e.decisions,
            e.real,
            e.burned_pct,
            e.grades.blunder
        );
    }
    println!("\nBereiche (letzte Turniere):");
    for a in &p.areas {
        println!("  {:<22} {:>5.0} {:?}  {} Entsch., {:.1} echte, {:.0} % des Verlusts", a.name, a.rating, a.verdict, a.decisions, a.real, a.burn_share_pct);
    }
    println!("\nWas zuletzt bewegt hat:");
    for m in &p.moments {
        println!(
            "  {:>+4.0}  {:?} {:?} {} → {} (besser {}), {:.1} % Stack verloren, Pot {:.0} % des Stacks{}",
            m.points,
            m.area,
            m.rating,
            m.hero_cards,
            m.played,
            m.best.as_deref().unwrap_or("-"),
            m.lost_pct,
            m.pot_pct,
            if m.bad_luck { ", Pech gehabt" } else { "" }
        );
    }
}
