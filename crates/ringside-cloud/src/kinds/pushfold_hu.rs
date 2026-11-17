//! Heads-up push/fold in chip EV, as solved by `ringside_engine::solve_push_fold`.
//!
//! The exploitability check is a single best response per player over the 169×169 class
//! matrix, a few hundred microseconds against the full solve. It is written here from the
//! engine's public pieces (equity matrix, card-removal counts) rather than taken from the
//! solver, so a solver bug cannot make its own output pass; the tests pin both to the
//! same numbers.

use ringside_engine::preflop_matrix::PreflopMatrix;
use ringside_engine::range::{HandClass, NUM_CLASSES, class_pair_count};
use ringside_engine::{PushFoldParams, PushFoldResult};
use serde::{Deserialize, Serialize};

use super::Kind;

pub struct PushFoldHu;

/// Amounts in thousandths of a big blind, so the key never depends on float formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushFoldHuInput {
    /// Effective stack including blinds and ante.
    pub stack_mbb: u32,
    pub sb_mbb: u32,
    /// Ante paid by each of the two players.
    pub ante_mbb: u32,
    /// Dead money in the pot that goes to the winner.
    pub dead_mbb: u32,
}

impl PushFoldHuInput {
    /// Rounds to the nearest milli-BB; solve [`Self::params`] so the submitted solution
    /// matches the key exactly.
    pub fn from_params(p: &PushFoldParams) -> PushFoldHuInput {
        let mbb = |x: f64| (x * 1000.0).round().clamp(0.0, u32::MAX as f64) as u32;
        PushFoldHuInput {
            stack_mbb: mbb(p.stack_bb),
            sb_mbb: mbb(p.small_blind_bb),
            ante_mbb: mbb(p.ante_bb),
            dead_mbb: mbb(p.dead_bb),
        }
    }

    pub fn params(&self) -> PushFoldParams {
        PushFoldParams {
            stack_bb: self.stack_mbb as f64 / 1000.0,
            small_blind_bb: self.sb_mbb as f64 / 1000.0,
            ante_bb: self.ante_mbb as f64 / 1000.0,
            dead_bb: self.dead_mbb as f64 / 1000.0,
            ..PushFoldParams::new(1.0)
        }
    }
}

/// Shove and call frequency per class, indexed by `HandClass::index`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushFoldHuSolution {
    pub shove: Vec<f64>,
    pub call: Vec<f64>,
}

impl From<&PushFoldResult> for PushFoldHuSolution {
    fn from(r: &PushFoldResult) -> PushFoldHuSolution {
        PushFoldHuSolution { shove: r.shove.clone(), call: r.call.clone() }
    }
}

impl Kind for PushFoldHu {
    const NAME: &'static str = "pushfold-hu";
    const ENGINE_VERSION: u32 = 1;
    /// BB per hand, summed over both players: far above float noise between platforms
    /// (~1e-12), far below anything that changes a decision at the table.
    const TOLERANCE: f64 = 1e-4;

    type Input = PushFoldHuInput;
    type Solution = PushFoldHuSolution;

    fn check_input(i: &PushFoldHuInput) -> Result<(), String> {
        let ok = (1..=1000).contains(&i.sb_mbb)
            && i.ante_mbb <= 1000
            && i.dead_mbb <= 50_000
            && (1000 + i.ante_mbb..=200_000).contains(&i.stack_mbb);
        if ok {
            Ok(())
        } else {
            Err(format!("input out of range: {i:?}"))
        }
    }

    fn exploitability(input: &PushFoldHuInput, s: &PushFoldHuSolution) -> Result<f64, String> {
        for (name, f) in [("shove", &s.shove), ("call", &s.call)] {
            if f.len() != NUM_CLASSES {
                return Err(format!("{name} has {} entries, not {NUM_CLASSES}", f.len()));
            }
            if !f.iter().all(|x| (0.0..=1.0).contains(x)) {
                return Err(format!("{name} frequencies must be in [0, 1]"));
            }
        }
        Ok(nash_conv(&input.params(), PreflopMatrix::embedded(), &s.shove, &s.call))
    }
}

/// Summed best-response gains of both players against (shove, call), in BB per hand.
fn nash_conv(p: &PushFoldParams, matrix: &PreflopMatrix, shove: &[f64], call: &[f64]) -> f64 {
    const N: usize = NUM_CLASSES;
    const DEALS: f64 = 1326.0 * 1225.0;
    let steal = 1.0 + p.ante_bb + p.dead_bb;
    let fold_loss = p.small_blind_bb + p.ante_bb;
    // SB values per class: (shove, fold). BB values per class once the SB may have
    // shoved: (call, fold). Both relative to the same baseline, from each player's view.
    let mut sb = [(0.0f64, 0.0f64); N];
    let mut bb = [(0.0f64, 0.0f64); N];
    for a in HandClass::all() {
        let ai = a.index();
        let pa = a.combo_count() as f64 / 1326.0;
        let mut shove_value = steal * pa;
        for b in HandClass::all() {
            let bi = b.index();
            let joint = class_pair_count(a, b) as f64 / DEALS;
            let eq = matrix.equity(a, b);
            let showdown = (2.0 * eq - 1.0) * p.stack_bb + eq * p.dead_bb;
            // What a BB call changes for the SB compared with a fold.
            let call_delta = joint * (showdown - steal);
            shove_value += call_delta * call[bi];
            let folded = -shove[ai] * steal * joint;
            bb[bi].0 += folded - shove[ai] * call_delta;
            bb[bi].1 += folded;
        }
        sb[ai] = (shove_value, -fold_loss * pa);
    }
    let gain = |values: &[(f64, f64); N], freq: &[f64]| -> f64 {
        let best: f64 = values.iter().map(|v| v.0.max(v.1)).sum();
        let played: f64 = values.iter().zip(freq).map(|(v, f)| f * v.0 + (1.0 - f) * v.1).sum();
        (best - played).max(0.0)
    };
    gain(&sb, shove) + gain(&bb, call)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::{KINDS, Verdict, verdict};
    use ringside_engine::solve_push_fold;

    fn solved(input: PushFoldHuInput) -> PushFoldHuSolution {
        PushFoldHuSolution::from(&solve_push_fold(&input.params()).unwrap())
    }

    const TEN_BB: PushFoldHuInput = PushFoldHuInput { stack_mbb: 10_000, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 };

    #[test]
    fn agrees_with_the_solver() {
        for input in [
            TEN_BB,
            PushFoldHuInput { stack_mbb: 4_250, sb_mbb: 500, ante_mbb: 125, dead_mbb: 1_000 },
            PushFoldHuInput { stack_mbb: 25_000, sb_mbb: 400, ante_mbb: 100, dead_mbb: 0 },
        ] {
            let r = solve_push_fold(&input.params()).unwrap();
            let mine = PushFoldHu::exploitability(&input, &PushFoldHuSolution::from(&r)).unwrap();
            assert!((mine - r.exploitability_bb()).abs() < 1e-12, "{input:?}: {mine} vs {}", r.exploitability_bb());
            // A profile with real gains, so the comparison is not just 0 = 0.
            let mut off = r.clone();
            off.shove.iter_mut().for_each(|f| *f = 1.0 - *f);
            assert!(PushFoldHu::exploitability(&input, &PushFoldHuSolution::from(&off)).unwrap() > 0.1);
        }
    }

    #[test]
    fn good_solution_passes() {
        let s = solved(TEN_BB);
        match verdict::<PushFoldHu>(&TEN_BB, &s) {
            Verdict::Verified { exploitability } => assert!(exploitability < 1e-9, "{exploitability}"),
            v => panic!("{v:?}"),
        }
    }

    #[test]
    fn manipulated_solutions_fail() {
        let good = solved(TEN_BB);
        let aa = "AA".parse::<HandClass>().unwrap().index();
        let seven_two = "72o".parse::<HandClass>().unwrap().index();
        let mut folds_aces = good.clone();
        folds_aces.shove[aa] = 0.0;
        let mut calls_trash = good.clone();
        calls_trash.call[seven_two] = 1.0;
        let wrong_stack = solved(PushFoldHuInput { stack_mbb: 30_000, ..TEN_BB });
        for (what, bad) in [("folds AA", folds_aces), ("calls 72o", calls_trash), ("other stack", wrong_stack)] {
            assert!(
                matches!(verdict::<PushFoldHu>(&TEN_BB, &bad), Verdict::Rejected { exploitability: Some(e), .. } if e > PushFoldHu::TOLERANCE),
                "{what}"
            );
        }
    }

    #[test]
    fn malformed_solutions_fail() {
        let good = solved(TEN_BB);
        let mut short = good.clone();
        short.call.pop();
        let mut above_one = good.clone();
        above_one.shove[0] = 1.5;
        let mut nan = good;
        nan.call[3] = f64::NAN;
        for bad in [short, above_one, nan] {
            assert!(matches!(verdict::<PushFoldHu>(&TEN_BB, &bad), Verdict::Rejected { exploitability: None, .. }));
        }
        let off_range = PushFoldHuInput { stack_mbb: 999, ..TEN_BB };
        assert!(PushFoldHu::check_input(&off_range).is_err());
    }

    #[test]
    fn stored_rows_are_checked_against_their_key() {
        let entry = &KINDS[0];
        let input = serde_json::to_string(&TEN_BB).unwrap();
        let result = serde_json::to_string(&solved(TEN_BB)).unwrap();
        let key = PushFoldHu::key(&TEN_BB).unwrap();
        assert!(matches!((entry.check)(&key, &input, &result), Verdict::Verified { .. }));
        let other_key = PushFoldHu::key(&PushFoldHuInput { stack_mbb: 10_001, ..TEN_BB }).unwrap();
        assert!(matches!((entry.check)(&other_key, &input, &result), Verdict::Rejected { .. }));
        let extra_field = input.replace('}', r#","rake":0}"#);
        assert!(matches!((entry.check)(&key, &extra_field, &result), Verdict::Rejected { .. }));
    }

    #[test]
    fn key_matches_the_shared_vector() {
        let key = PushFoldHu::key(&TEN_BB).unwrap();
        assert_eq!(key, "d0bc63798dd92babad764962a8ca4da527d50757d05a175e1689ebebfb8a1ff9");
    }

    #[test]
    fn params_round_trip_in_milli_bb() {
        let p = PushFoldParams { stack_bb: 12.3456, small_blind_bb: 0.5, ante_bb: 0.125, dead_bb: 0.0, ..PushFoldParams::new(1.0) };
        let i = PushFoldHuInput::from_params(&p);
        assert_eq!(i, PushFoldHuInput { stack_mbb: 12_346, sb_mbb: 500, ante_mbb: 125, dead_mbb: 0 });
        assert_eq!(PushFoldHuInput::from_params(&i.params()), i);
    }
}
