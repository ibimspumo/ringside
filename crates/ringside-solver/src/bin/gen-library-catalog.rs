//! Writes `crates/ringside-solver/data/library-v1.json`, the spot catalog of the library:
//! ranges from the preflop model's table (9 players, big-blind ante), the flop subset and
//! the bet sizes. Run with `--release` from the repository root; the result is committed
//! and only regenerated together with a new catalog version.

use ringside_engine::preflop::{Kind, Solution, Step};
use ringside_engine::preflop_table;
use ringside_engine::pushfold_table::Ante;
use ringside_engine::range::{HandClass, Range};
use ringside_solver::StreetSizes;
use ringside_solver::library::catalog::{CATALOG_VERSION, Catalog, FLOPS, PotType, Scenario, subset_flops};

const PLAYERS: usize = 9;
const STACKS: [f64; 3] = [20.0, 40.0, 75.0];
const BTN: usize = 6;
const SB: usize = 7;
const BB: usize = 8;
/// Range weights are rounded to this step; smaller weights are dropped.
const WEIGHT_STEP: f64 = 0.05;

fn name(seat: usize) -> &'static str {
    ringside_engine::multiway::seat_name(PLAYERS, seat)
}

fn sizes(pot_type: PotType) -> [StreetSizes; 3] {
    let s = |bet: &[f64], raise: &[f64]| StreetSizes { bet: bet.to_vec(), raise: raise.to_vec() };
    match pot_type {
        PotType::Srp => [s(&[33.0], &[60.0]), s(&[75.0], &[]), s(&[75.0], &[])],
        PotType::ThreeBet => [s(&[33.0], &[]), s(&[66.0], &[]), s(&[75.0], &[])],
    }
}

/// The action of `kind` at `node` and its frequency per class.
fn action(sol: &Solution, node: usize, kind: Kind) -> (Step, Vec<f64>) {
    let n = &sol.nodes[node];
    let a = n.actions.iter().position(|a| a.kind == kind).unwrap_or_else(|| panic!("no {kind:?} at node {node}"));
    (Step { seat: n.seat, action: n.actions[a] }, n.freq[a].clone())
}

fn range(weights: &[f64]) -> String {
    let mut r = Range::empty();
    for c in HandClass::all() {
        let w = (weights[c.index()] / WEIGHT_STEP).round() * WEIGHT_STEP;
        if w > 0.0 {
            r.set_class(c, w.min(1.0) as f32);
        }
    }
    r.to_string()
}

fn bb(x: f64) -> String {
    format!("{}", (x * 100.0).round() / 100.0).replace('.', ",")
}

struct Built {
    oop: usize,
    ip: usize,
    weights: [Vec<f64>; 2],
    /// Chips each player has in front at the flop, blinds included.
    put: [f64; 2],
    notes: [String; 2],
}

/// `seat` sits after the flop in this order: blinds first, the button last.
fn postflop_order(seat: usize) -> usize {
    if seat >= SB { seat - SB } else { seat + 2 }
}

fn srp(sol: &Solution, opener: usize, caller: usize) -> Built {
    let (open, open_freq) = action(sol, sol.open_node(opener).expect("open node"), Kind::Raise);
    let defend = sol.node(caller, &[open]).expect("defender node");
    let (_, call_freq) = action(sol, defend, Kind::Call);
    let to = open.action.to_bb;
    let opener_note = format!("Open-Raise aus {} auf {} BB", name(opener), bb(to));
    let caller_note = format!("Call aus {} gegen das Open", name(caller));
    order(opener, caller, [open_freq, call_freq], [to, to], [opener_note, caller_note])
}

fn three_bet(sol: &Solution, opener: usize, three_bettor: usize) -> Built {
    let (open, open_freq) = action(sol, sol.open_node(opener).expect("open node"), Kind::Raise);
    let facing = sol.node(three_bettor, &[open]).expect("3-bet node");
    let (three, three_freq) = action(sol, facing, Kind::Raise);
    let back = sol.node(opener, &[open, three]).expect("opener facing the 3-bet");
    let (_, call_freq) = action(sol, back, Kind::Call);
    let opener_weights: Vec<f64> = open_freq.iter().zip(&call_freq).map(|(a, b)| a * b).collect();
    let to = three.action.to_bb;
    let opener_note = format!("Open-Raise aus {}, Call gegen die 3-Bet", name(opener));
    let three_note = format!("3-Bet aus {} auf {} BB", name(three_bettor), bb(to));
    order(opener, three_bettor, [opener_weights, three_freq], [to, to], [opener_note, three_note])
}

fn order(a: usize, b: usize, weights: [Vec<f64>; 2], put: [f64; 2], notes: [String; 2]) -> Built {
    let [wa, wb] = weights;
    let [na, nb] = notes;
    if postflop_order(a) < postflop_order(b) {
        Built { oop: a, ip: b, weights: [wa, wb], put, notes: [na, nb] }
    } else {
        Built { oop: b, ip: a, weights: [wb, wa], put: [put[1], put[0]], notes: [nb, na] }
    }
}

fn scenario(sol: &Solution, stack_bb: f64, pot_type: PotType, opener: usize, other: usize) -> Scenario {
    let built = match pot_type {
        PotType::Srp => srp(sol, opener, other),
        PotType::ThreeBet => three_bet(sol, opener, other),
    };
    let (_, bb_ante) = Ante::BigBlind.amounts();
    let seats = [built.oop, built.ip];
    // Blinds of players who folded stay in the pot; the big blind's ante is always there.
    let dead_blinds: f64 = [(SB, 0.5), (BB, 1.0)].iter().filter(|(s, _)| !seats.contains(s)).map(|(_, x)| x).sum();
    let pot = built.put[0] + built.put[1] + dead_blinds + bb_ante;
    let behind = |i: usize| stack_bb - built.put[i] - if seats[i] == BB { bb_ante } else { 0.0 };
    let prefix = match pot_type {
        PotType::Srp => "srp",
        PotType::ThreeBet => "3bp",
    };
    Scenario {
        id: format!("{prefix}-{}-{}-{stack_bb}", name(opener).to_lowercase(), name(other).to_lowercase()),
        pot_type,
        oop: name(built.oop).into(),
        ip: name(built.ip).into(),
        stack_bb,
        pot: (pot * 100.0).round() / 100.0,
        stack: (behind(0).min(behind(1)) * 100.0).round() / 100.0,
        oop_range: range(&built.weights[0]),
        ip_range: range(&built.weights[1]),
        sizes: sizes(pot_type),
        allin_threshold: 150.0,
        oop_note: built.notes[0].clone(),
        ip_note: built.notes[1].clone(),
    }
}

fn main() {
    let mut scenarios = Vec::new();
    for stack in STACKS {
        let sol = preflop_table::chart(Ante::BigBlind, PLAYERS, stack).expect("table entry");
        // At 20 BB a 3-bet is almost always all in, so 3-bet pots rarely see a flop.
        let three_bets = stack > 20.0;
        for (pot_type, opener, other) in [
            (PotType::Srp, BTN, BB),
            (PotType::Srp, BTN - 1, BB),
            (PotType::Srp, 0, BB),
            (PotType::Srp, SB, BB),
            (PotType::ThreeBet, BTN, BB),
            (PotType::ThreeBet, BTN, SB),
            (PotType::ThreeBet, BTN - 1, BTN),
        ] {
            if pot_type == PotType::ThreeBet && !three_bets {
                continue;
            }
            scenarios.push(scenario(&sol, stack, pot_type, opener, other));
        }
    }
    let (flops, flop_weights) = subset_flops(FLOPS);
    let catalog = Catalog {
        version: CATALOG_VERSION,
        source: format!(
            "Preflop-Modell von Ringside (ringside-engine {}), 9 Spieler, Big-Blind-Ante, Chip-EV",
            env!("CARGO_PKG_VERSION")
        ),
        flops,
        flop_weights,
        scenarios,
        target_pct: 0.5,
        max_iterations: 1000,
    };
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/library-v1.json");
    std::fs::write(path, serde_json::to_string_pretty(&catalog).unwrap() + "\n").unwrap();
    println!("wrote {path}: {} scenarios × {} flops", catalog.scenarios.len(), catalog.flops.len());
}
