//! Grades for a decision, by what it gives up against the best action with the same hand,
//! in percent of the pot at the decision, so they mean the same in small and big pots.
//! The first three thresholds are those of the solver page's practice mode
//! (`src/lib/solver/practice.ts`); the blunder line is new.
//!
//! A mixed strategy is no mistake: "best" is the main move (the one the solution plays
//! most often with the hand, or at least half the time), a rarer move worth the same
//! that the solution still plays at least [`MIX_MIN_FREQUENCY`] of the time is "mixed",
//! with full points.

use serde::{Deserialize, Serialize};

/// Up to this loss two actions are equally good within what a solve to 0.5–1 % of the pot
/// can tell apart.
pub const BEST_PCT: f64 = 0.5;
pub const GOOD_PCT: f64 = 2.0;
pub const INACCURATE_PCT: f64 = 6.0;
/// From here on the move costs as much as giving away about every seventh pot.
pub const MISTAKE_PCT: f64 = 15.0;
/// Played this often, a move worth the same as the best one is a main move.
pub const MAIN_FREQUENCY: f64 = 0.5;
/// Played this often, a move worth the same as the best one is part of the mix.
pub const MIX_MIN_FREQUENCY: f64 = 0.1;
/// A decision taken after looking at the hint counts this much.
pub const HINT_FACTOR: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Best,
    /// As good as the main move, which the solution plays more often.
    Mix,
    Good,
    Inaccurate,
    Mistake,
    Blunder,
}

impl Grade {
    pub const ALL: [Grade; 6] = [Grade::Best, Grade::Mix, Grade::Good, Grade::Inaccurate, Grade::Mistake, Grade::Blunder];

    /// Points out of 100.
    pub fn points(self) -> f64 {
        match self {
            Grade::Best | Grade::Mix => 100.0,
            Grade::Good => 70.0,
            Grade::Inaccurate => 30.0,
            Grade::Mistake | Grade::Blunder => 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Judgement {
    pub grade: Grade,
    /// Index of the action with the highest EV (or frequency without EVs).
    pub best: usize,
    /// Index of the action the solution plays most often with this hand.
    pub main: usize,
    /// How often the solution takes the chosen action with this hand.
    pub frequency: f64,
    /// EV given up against the best action, BB and percent of the pot; `None` without EVs.
    pub loss_bb: Option<f64>,
    pub loss_pct: Option<f64>,
}

/// Grades action `chosen` of a hand with `strategy` and, if known, the EV of every action.
pub fn judge(strategy: &[f64], evs: Option<&[f64]>, chosen: usize, pot: f64) -> Judgement {
    let frequency = strategy.get(chosen).copied().unwrap_or(0.0);
    let argmax = |v: &[f64]| (0..v.len()).max_by(|&a, &b| v[a].total_cmp(&v[b])).unwrap_or(0);
    let main = argmax(strategy);
    let is_main = chosen == main || frequency >= MAIN_FREQUENCY;
    match evs.filter(|e| e.len() == strategy.len() && !e.is_empty()) {
        Some(evs) => {
            let best = argmax(evs);
            let loss = (evs[best] - evs[chosen]).max(0.0);
            let pct = loss / pot.max(1e-9) * 100.0;
            let grade = if pct <= BEST_PCT && is_main && frequency > 0.0 {
                Grade::Best
            } else if pct <= BEST_PCT && frequency >= MIX_MIN_FREQUENCY {
                Grade::Mix
            } else if pct <= GOOD_PCT {
                Grade::Good
            } else if pct <= INACCURATE_PCT {
                Grade::Inaccurate
            } else if pct <= MISTAKE_PCT {
                Grade::Mistake
            } else {
                Grade::Blunder
            };
            Judgement { grade, best, main, frequency, loss_bb: Some(loss), loss_pct: Some(pct) }
        }
        None => {
            // Without EVs per action only the frequencies tell how good a choice was.
            let grade = if is_main {
                Grade::Best
            } else if frequency >= MIX_MIN_FREQUENCY {
                Grade::Mix
            } else if frequency >= 0.02 {
                Grade::Inaccurate
            } else {
                Grade::Mistake
            };
            Judgement { grade, best: main, main, frequency, loss_bb: None, loss_pct: None }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_in_percent_of_the_pot() {
        let s = [0.5, 0.5];
        let g = |loss: f64| judge(&s, Some(&[1.0, 1.0 - loss]), 1, 10.0).grade;
        assert_eq!(g(0.04), Grade::Best);
        assert_eq!(g(0.15), Grade::Good);
        assert_eq!(g(0.5), Grade::Inaccurate);
        assert_eq!(g(1.2), Grade::Mistake);
        assert_eq!(g(2.0), Grade::Blunder);
        // Equal EV but never played is not the best choice.
        assert_eq!(judge(&[1.0, 0.0], Some(&[1.0, 1.0]), 1, 10.0).grade, Grade::Good);
        assert_eq!(judge(&[0.95, 0.05], None, 1, 10.0).grade, Grade::Inaccurate);
    }

    #[test]
    fn a_mix_is_no_mistake_but_only_the_main_move_is_best() {
        // KJo on the button against an open: call 14 %, raise 86 %, both about +0.11 BB.
        let strategy = [0.0, 0.14, 0.86, 0.0];
        let evs = [0.0, 0.110, 0.112, -0.42];
        let call = judge(&strategy, Some(&evs), 1, 5.0);
        assert_eq!(call.grade, Grade::Mix);
        assert_eq!(call.main, 2);
        assert_eq!(call.grade.points(), 100.0);
        assert_eq!(judge(&strategy, Some(&evs), 2, 5.0).grade, Grade::Best);
        // The main move is best even when rounding gives the rarer one the higher EV.
        assert_eq!(judge(&[0.3, 0.7], Some(&[1.001, 1.0]), 1, 10.0).grade, Grade::Best);
        assert_eq!(judge(&[0.3, 0.7], Some(&[1.001, 1.0]), 0, 10.0).grade, Grade::Mix);
        // Rarer than the mix threshold: graded by the loss alone.
        assert_eq!(judge(&[0.05, 0.95], Some(&[1.0, 1.0]), 0, 10.0).grade, Grade::Good);
        assert_eq!(judge(&[0.8, 0.2], None, 1, 10.0).grade, Grade::Mix);
    }
}
