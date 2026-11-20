//! A call against an all-in: whatever the stacks, the call costs at most what it puts in
//! and wins at most the pot, and a short all-in is rated at its own depth, not at the
//! depth of the deepest stack at the table.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_analysis::{SpotType, Status, analyze_hand, chart_targets};
use ringside_core::model::Hand;
use ringside_core::parse_hand;
use ringside_engine::preflop_table::STACKS;

const BB: i64 = 600;
const ANTE: i64 = 75;
const OPEN_TO: i64 = 1260;

fn nearest(grid: &[f64], x: f64) -> f64 {
    grid.iter().copied().min_by(|a, b| (a / x).ln().abs().total_cmp(&(b / x).ln().abs())).unwrap()
}

/// Eight players, the hero opens from UTG+1, the small blind with `sb` chips goes all-in,
/// the big blind folds and the hero calls or folds with `hero` chips.
fn hand(hero: i64, sb: i64, cards: &str, call: bool) -> (Hand, f64, f64) {
    let to = sb - ANTE;
    let behind = hero - ANTE - OPEN_TO;
    let to_call = to - OPEN_TO;
    let paid = to_call.min(behind);
    let back = to_call - paid;
    let dead = 8 * ANTE + BB;
    let mut t = format!(
        "PokerStars Hand #260000000001: Tournament #3600000001, $1+$0.10 USD Hold'em No Limit - Level XI (300/600) - 2026/11/20 15:31:55 ET\n\
         Table '3600000001 66' 8-max Seat #2 is the button\n\
         Seat 1: Early (33377 in chips)\nSeat 2: Button (19472 in chips)\nSeat 3: Small ({sb} in chips)\nSeat 4: Big (11628 in chips)\n\
         Seat 5: Under (41644 in chips)\nSeat 6: Hero ({hero} in chips)\nSeat 7: Lojack (55302 in chips)\nSeat 8: Hijack (11595 in chips)\n"
    );
    for p in ["Early", "Button", "Small", "Big", "Under", "Hero", "Lojack", "Hijack"] {
        t.push_str(&format!("{p}: posts the ante {ANTE}\n"));
    }
    t.push_str(&format!(
        "Small: posts small blind 300\nBig: posts big blind {BB}\n*** HOLE CARDS ***\nDealt to Hero [{cards}]\n\
         Under: folds\nHero: raises {} to {OPEN_TO}\nLojack: folds\nHijack: folds\nEarly: folds\nButton: folds\n\
         Small: raises {} to {to} and is all-in\nBig: folds\n",
        OPEN_TO - BB,
        to - OPEN_TO
    ));
    if call {
        let all_in = if paid == behind { " and is all-in" } else { "" };
        t.push_str(&format!("Hero: calls {paid}{all_in}\n"));
        if back > 0 {
            t.push_str(&format!("Uncalled bet ({back}) returned to Small\n"));
        }
        let pot = dead + OPEN_TO + paid + to - back;
        t.push_str(&format!(
            "*** FLOP *** [2d 8c Th]\n*** TURN *** [2d 8c Th] [3s]\n*** RIVER *** [2d 8c Th 3s] [4c]\n*** SHOW DOWN ***\n\
             Small collected {pot} from pot\n*** SUMMARY ***\nTotal pot {pot} | Rake 0\n"
        ));
    } else {
        t.push_str(&format!("Hero: folds\nUncalled bet ({to_call}) returned to Small\n"));
        let pot = dead + OPEN_TO + OPEN_TO;
        t.push_str(&format!("Small collected {pot} from pot\n*** SUMMARY ***\nTotal pot {pot} | Rake 0\n"));
    }
    let pot_final = (dead + OPEN_TO + to + paid - back) as f64 / BB as f64;
    (parse_hand(&t).unwrap_or_else(|e| panic!("{e:?}\n{t}")), paid as f64 / BB as f64, pot_final)
}

#[test]
fn a_call_against_an_all_in_risks_only_the_call() {
    let mut rng = ChaCha8Rng::seed_from_u64(11);
    let classes = ["As 7s", "Ah Ad", "9s 9h", "Kh Qc", "7d 2c", "As 2s", "Th 9h", "Jc Jd", "Ac 5d"];
    let mut short = 0;
    for _ in 0..300 {
        let hero = rng.random_range(12..130) * BB / 2 + 1500;
        let sb = rng.random_range(5..90) * BB / 2 + 300;
        if sb - ANTE <= OPEN_TO + (OPEN_TO - BB) {
            continue;
        }
        let cards = classes[rng.random_range(0..classes.len())];
        let call = rng.random_bool(0.5);
        let (h, paid, pot_final) = hand(hero, sb, cards, call);
        let a = analyze_hand(&h);
        let Some(d) = a.decisions.iter().find(|d| d.spot == SpotType::CallShove && d.status == Status::Rated) else { continue };
        let ev = |k: &str| d.strategy.iter().find(|c| c.kind == k).and_then(|c| c.ev_bb);
        let (Some(fold), Some(called)) = (ev("fold"), ev("call")) else { continue };
        let what = format!("hero {hero}, small blind {sb}, {cards}: fold {fold:.2}, call {called:.2}, call {paid:.2} BB, pot {pot_final:.2} BB");
        let depth = (hero.min(sb)) as f64 / BB as f64;
        // The model's own all-in may be a little deeper than the real one.
        let slack = 0.15 * paid + 0.5;
        assert!(called - fold >= -paid - slack, "loses more than the call, {what}");
        assert!(called - fold <= pot_final - paid + slack, "wins more than the pot, {what}");
        let exact = d.note.contains("Gewinnchance");
        if let Some(t) = chart_targets(&h).into_iter().find(|t| t.action_index == d.action_index) {
            if exact {
                assert_eq!(t.stack_bb, nearest(&STACKS, depth), "chart depth, {what}");
            } else {
                assert!(t.stack_bb <= nearest(&STACKS, depth / 0.8) + 1e-9, "chart deeper than the all-in, {what}");
            }
        }
        if exact {
            short += 1;
            assert!(called - fold >= -paid - 1e-9, "the exact call loses more than it puts in, {what}");
            assert!(called - fold <= pot_final - paid + 1e-9, "the exact call wins more than the pot, {what}");
        }
    }
    assert!(short > 50, "only {short} short all-ins rated by equity");
}
