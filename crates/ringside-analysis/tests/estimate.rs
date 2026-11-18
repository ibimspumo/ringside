//! What the estimate promises against the solver, on solved heads-up spots of the
//! preflop model's tables (`fixtures/solved_nodes.json`, every 40th node of the tested
//! half, written by `analysis-calibrate --fixture`).

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_analysis::calibration::{SolvedNode, stored};
use ringside_analysis::estimate::{Opt, estimate, grades, strength};
use ringside_play::grade::{Grade, MIX_MIN_FREQUENCY};

fn nodes() -> Vec<SolvedNode> {
    serde_json::from_str(include_str!("fixtures/solved_nodes.json")).expect("fixture")
}

/// When the solver mixes ways to go on (check or bet, call or raise) with a hand the
/// estimate sees enough equity to go on with, none of them is a mistake.
#[test]
fn a_mix_of_the_solver_is_never_a_mistake() {
    ringside_engine::eval::init();
    let mut mixed = 0;
    for n in nodes() {
        let opp = n.opponent_range(|board| strength(board, &mut ChaCha8Rng::seed_from_u64(1)));
        let options = n.options();
        let t = stored().street_tunings[n.board.len() - 3];
        for (i, h) in n.hands.iter().enumerate() {
            let on: Vec<usize> = (0..options.len()).filter(|&k| options[k] != Opt::Fold && h.strategy[k] >= MIX_MIN_FREQUENCY).collect();
            if on.len() < 2 {
                continue;
            }
            let est = estimate(&n.spot(h.combo.parse().unwrap(), &opp), &options, i as u64, &t);
            if est.equity < est.pot_odds.unwrap_or(0.0) - t.line_margin || est.equity >= t.line_top {
                continue;
            }
            let mine = grades(&est, &options, n.pot, &t);
            for k in on {
                mixed += 1;
                assert!(
                    mine[k] < Grade::Mistake,
                    "{} {:?} {}: {} played {:.0} % by the solver, estimate {:?} ({:?})",
                    n.label,
                    n.board,
                    h.combo,
                    options[k].key(),
                    h.strategy[k] * 100.0,
                    mine[k],
                    est.evs
                );
            }
        }
    }
    assert!(mixed >= 50, "only {mixed} mixed options");
}

/// What the hand analysis stores for an estimated decision: folding brings 0,00 BB, as the
/// legend under the table says, and the played move costs its gap to the best one.
#[test]
fn an_estimated_fold_brings_nothing() {
    use ringside_analysis::{Source, Status, analyze_hand, estimate_rest};
    use ringside_core::{parse_hand, split_hands};
    let mut folds = 0;
    for raw in split_hands(include_str!("fixtures/hands.txt")) {
        let Ok(hand) = parse_hand(raw) else { continue };
        let decisions = analyze_hand(&hand).decisions;
        let pending = decisions.iter().filter(|d| d.status == Status::Pending).cloned().collect();
        for d in decisions.iter().chain(&estimate_rest(&hand, pending, "Für den Test geschätzt.")) {
            if d.source != Some(Source::Estimate) || d.status != Status::Rated {
                continue;
            }
            let top = d.strategy.iter().filter_map(|c| c.ev_bb).fold(f64::NEG_INFINITY, f64::max);
            if let Some(fold) = d.strategy.iter().find(|c| c.kind == "fold") {
                assert_eq!(fold.ev_bb, Some(0.0), "#{} @{}: {:?}", d.hand_id, d.action_index, d.strategy);
                folds += 1;
            }
            let played = d.strategy.iter().find(|c| c.played).and_then(|c| c.ev_bb).unwrap();
            let loss = d.ev_loss_bb.unwrap();
            assert!(loss == 0.0 || (loss - (top - played)).abs() < 1e-9, "#{} @{}: {loss} vs {}", d.hand_id, d.action_index, top - played);
        }
    }
    assert!(folds > 0, "no estimated decision with a fold in the fixtures");
}
