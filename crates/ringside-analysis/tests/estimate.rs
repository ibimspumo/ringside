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
        let opp = n.opponent_range(|board| strength(board, &mut ChaCha8Rng::seed_from_u64(1)), &stored().moves);
        let options = n.options();
        let t = stored().street_tunings[n.board.len() - 3];
        for (i, h) in n.hands.iter().enumerate() {
            let on: Vec<usize> = (0..options.len()).filter(|&k| options[k] != Opt::Fold && h.strategy[k] >= MIX_MIN_FREQUENCY).collect();
            if on.len() < 2 {
                continue;
            }
            let est = estimate(&n.spot(h.combo.parse().unwrap(), &opp), &options, i as u64, &t, &stored().moves);
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

/// The ranges answer the size of a bet: after a river bet of the pot or more the
/// strongest fifth of all hands makes up more of the bettor's range than after a third
/// of the pot.
#[test]
fn a_bigger_river_bet_is_stronger() {
    use ringside_analysis::estimate::{Range, Signal, narrow, top_share};
    use ringside_engine::{Card, CardSet};
    ringside_engine::eval::init();
    let board: Vec<Card> = ["Kh", "9d", "5c", "2s", "7h"].iter().map(|c| c.parse().unwrap()).collect();
    let dead = CardSet::from_cards(&board);
    let s = strength(&board, &mut ChaCha8Rng::seed_from_u64(1));
    let top = top_share(&Range::all(), &s.equity, dead);
    let strongest = |size: u16| {
        let mut r = Range::all();
        narrow(&mut r, Signal::Bet { size, aggressor: true }, &s, dead);
        let share = |keep: &dyn Fn(usize) -> bool| (0..r.0.len()).filter(|&i| keep(i)).map(|i| r.0[i] as f64).sum::<f64>();
        share(&|i| top[i] <= 0.2) / r.live(dead)
    };
    let (third, pot) = (strongest(33), strongest(120));
    assert!(pot > third + 0.03, "strongest fifth: {third:.3} after a third of the pot, {pot:.3} after the pot");
}

/// The classes of the strongest `share` of all combos, by all-in equity against a random
/// hand.
fn strongest_classes(share: f64) -> Vec<ringside_engine::HandClass> {
    use ringside_engine::{HandClass, PreflopMatrix};
    let m = PreflopMatrix::embedded();
    let mut order: Vec<HandClass> = HandClass::all().collect();
    order.sort_by(|a, b| m.equity_vs_random(*b).total_cmp(&m.equity_vs_random(*a)));
    let mut combos = 0.0;
    order
        .into_iter()
        .take_while(|c| {
            combos += c.combo_count() as f64;
            combos <= share * 1326.0
        })
        .collect()
}

fn spot_bets(opponents: &[ringside_analysis::estimate::Opponent]) -> f64 {
    opponents.iter().map(|o| o.street_in).sum()
}

/// Before the flop with at least 40 BB effective, going all in with a hand outside the
/// strongest 5 % is never the estimate's best move: whoever calls a deep shove holds a
/// strong hand, and a raise wins the same folds for far less.
#[test]
fn a_deep_shove_needs_a_premium_hand() {
    use ringside_analysis::estimate::{Opponent, Range, Signal, Spot, narrow, strategy};
    use ringside_analysis::replay;
    use ringside_core::model::{Action, ActionKind, Street};
    use ringside_engine::{CardSet, Combo};
    use rand::Rng;
    ringside_engine::eval::init();
    let premium = strongest_classes(0.05);
    let others: Vec<Combo> = Combo::all().filter(|c| !premium.contains(&c.class())).collect();
    let t = stored().preflop_tuning;
    let preflop = strength(&[], &mut ChaCha8Rng::seed_from_u64(1));
    let mut rng = ChaCha8Rng::seed_from_u64(40);
    let fold = Action { street: Street::Preflop, player: "hero".into(), kind: ActionKind::Fold, amount: 0, to: None, all_in: false };
    let mut failed = Vec::new();
    for case in 0..240 {
        let hero = others[rng.random_range(0..others.len())];
        let ante = if rng.random_bool(0.5) { 0.0 } else { 0.125 * 8.0 };
        let deep = rng.random_range(40.0..150.0);
        // The hero's place, and what happened before: nothing, limps, an open with or
        // without callers, or a 3-bet of the hero's open.
        let line = rng.random_range(0..4);
        let n = rng.random_range(1..=3usize);
        let mut opponents = Vec::new();
        let mut hero_in = [0.0, 0.5, 1.0][rng.random_range(0..3)];
        for k in 0..n {
            let mut range = Range::all();
            let total = if k == 0 { rng.random_range(40.0..200.0) } else { rng.random_range(5.0..200.0) };
            let street_in = match (line, k) {
                (1, _) => {
                    narrow(&mut range, Signal::Limp, &preflop, CardSet(0));
                    1.0
                }
                (2, 0) => {
                    narrow(&mut range, Signal::Open { behind: rng.random_range(1..6) }, &preflop, CardSet(0));
                    rng.random_range(2.0..3.5)
                }
                (2, _) => {
                    narrow(&mut range, Signal::CallRaise, &preflop, CardSet(0));
                    opponents.first().map_or(2.5, |o: &Opponent| o.street_in)
                }
                (3, 0) => {
                    hero_in = 2.5;
                    narrow(&mut range, Signal::ThreeBet, &preflop, CardSet(0));
                    rng.random_range(7.0..11.0)
                }
                _ if k == n - 1 && hero_in < 1.0 => 1.0,
                _ => 0.0,
            };
            opponents.push(Opponent { range, behind: (total - street_in).max(0.0), street_in });
        }
        // Blinds and antes as dead money on top of the bets: more to win by a shove.
        let pot = 1.5 + ante + hero_in + spot_bets(&opponents);
        let spot = Spot {
            preflop: true,
            river: false,
            board: vec![],
            hero,
            opponents,
            pot,
            hero_in,
            hero_behind: deep - hero_in,
            in_position: rng.random_bool(0.3),
            aggressor: line == 3,
        };
        let biggest = spot.opponents.iter().map(|o| o.behind + o.street_in).fold(0.0, f64::max);
        if deep.min(biggest) < 40.0 || (spot.to_call() <= 0.0 && line != 1) {
            continue;
        }
        let (options, _) = replay::options(&spot, &fold, 1.0);
        let Some(shove) = options.iter().position(|o| *o == Opt::AllIn) else { continue };
        let est = estimate(&spot, &options, case, &t, &stored().moves);
        let freq = strategy(&est, spot.pot, &t);
        if freq[shove] > 0.0 {
            failed.push(format!(
                "case {case}: {hero} {deep:.0} BB, line {line}, pot {:.1}, to call {:.1}, opponents {:?}: {:?} {:?}",
                spot.pot,
                spot.to_call(),
                spot.opponents.iter().map(|o| (o.street_in, o.behind.round())).collect::<Vec<_>>(),
                options,
                est.evs.iter().map(|e| (e * 100.0).round() / 100.0).collect::<Vec<_>>()
            ));
        }
    }
    assert!(failed.is_empty(), "{} deep shoves best:\n{}", failed.len(), failed.join("\n"));
}

/// A preflop spot of opponents `(range, put in, behind)`.
fn preflop_spot(hero: &str, hero_in: f64, hero_behind: f64, antes: f64, opponents: Vec<(ringside_analysis::estimate::Range, f64, f64)>) -> ringside_analysis::estimate::Spot {
    use ringside_analysis::estimate::{Opponent, Spot};
    let opponents: Vec<Opponent> = opponents.into_iter().map(|(range, street_in, behind)| Opponent { range, behind, street_in }).collect();
    Spot {
        preflop: true,
        river: false,
        board: vec![],
        hero: hero.parse().unwrap(),
        pot: antes + hero_in + spot_bets(&opponents),
        opponents,
        hero_in,
        hero_behind,
        in_position: false,
        aggressor: false,
    }
}

/// From a real tournament hand: a limp and a raise to 3 BB on the button, the hero in the
/// small blind with KTs and 99 BB, the limper (58 BB) and the big blind still to act. The
/// limper and the big blind don't call 98 BB with their weak hands, and a raise wins the
/// same folds: going all in is a mistake, not the best move.
#[test]
fn kings_ten_suited_does_not_shove_99_bb_over_a_raise() {
    use ringside_analysis::estimate::{Range, Signal, narrow, strategy};
    use ringside_analysis::replay;
    use ringside_core::model::{Action, ActionKind, Street};
    use ringside_engine::CardSet;
    ringside_engine::eval::init();
    let preflop = strength(&[], &mut ChaCha8Rng::seed_from_u64(1));
    let mut limper = Range::all();
    narrow(&mut limper, Signal::Limp, &preflop, CardSet(0));
    let mut raiser = Range::all();
    narrow(&mut raiser, Signal::Open { behind: 2 }, &preflop, CardSet(0));
    let spot = preflop_spot("KdTd", 0.5, 98.1, 0.84, vec![(limper, 1.0, 57.9), (raiser, 3.0, 99.4), (Range::all(), 1.0, 101.0)]);
    let fold = Action { street: Street::Preflop, player: "hero".into(), kind: ActionKind::Fold, amount: 0, to: None, all_in: false };
    let (options, _) = replay::options(&spot, &fold, 1.0);
    let t = stored().preflop_tuning;
    let est = estimate(&spot, &options, 7, &t, &stored().moves);
    let shove = options.iter().position(|o| *o == Opt::AllIn).unwrap();
    let mine = grades(&est, &options, spot.pot, &t);
    assert_eq!(strategy(&est, spot.pot, &t)[shove], 0.0, "{options:?} {:?}", est.evs);
    assert!(mine[shove] >= Grade::Mistake, "{options:?} {:?} {mine:?}", est.evs);
}

/// Against one opponent with 4,4 BB behind, going all in with 66 BB risks what going all
/// in with 10 BB does: what they can't match comes back (from a real hand: Q8s with 66 BB
/// against stacks of 22 BB at most).
#[test]
fn a_shove_against_a_short_stack_risks_only_the_short_stack() {
    use ringside_analysis::estimate::{Range, Tuning};
    ringside_engine::eval::init();
    let options = [Opt::Fold, Opt::Call, Opt::AllIn];
    let raw = Tuning { shrink: 1.0, line_margin: -1.0, ..stored().preflop_tuning };
    let shove = |behind: f64| {
        let spot = preflop_spot("Qd8d", 0.0, behind, 0.88, vec![(Range::all(), 0.5, 0.0), (Range::all(), 1.0, 4.4)]);
        estimate(&spot, &options, 3, &raw, &stored().moves).evs[2]
    };
    let (deep, short) = (shove(65.9), shove(9.9));
    assert!((deep - short).abs() < 1e-9, "all in with 66 BB {deep:.2}, with 10 BB {short:.2}");
}
