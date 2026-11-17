//! Rating thresholds. Losses are in big blinds per decision, the unit every source shares.
//!
//! - Below [`BEST_BB`] the difference is within what the sources resolve: tables store
//!   EVs in 0.01 BB steps and are solved to a NashConv of 0.001–0.01 BB, the postflop
//!   solver stops at about 1 % of the pot.
//! - [`GOOD_BB`]: a few hundredths of a BB are noise of the model assumptions (equal
//!   stacks, standard sizes), so these still count as good.
//! - [`INACCURACY_BB`]: half a big blind per decision is about what a typical leak costs
//!   per hand where it comes up; repeated, it shows in the results.
//! - [`MISTAKE_BB`]: from 1.5 BB on, a decision costs more than the blinds of a whole
//!   orbit at a nine-handed table.
//!
//! Mixed strategies are not mistakes: an action the source takes at least
//! [`MIXED_FREQ`] of the time with this hand is at worst "good", one it takes at least
//! [`MAIN_FREQ`] of the time is the best choice, and either counts as no loss (see
//! `score` in the crate root).

use ringside_core::db::Rating;

pub const BEST_BB: f64 = 0.05;
pub const GOOD_BB: f64 = 0.15;
pub const INACCURACY_BB: f64 = 0.5;
pub const MISTAKE_BB: f64 = 1.5;
pub const MIXED_FREQ: f64 = 0.10;
pub const MAIN_FREQ: f64 = 0.5;

/// Rating from the EV loss and how often the source plays the chosen action. Without an
/// EV the frequency alone decides.
pub fn rate(loss_bb: Option<f64>, freq: f64) -> Rating {
    let by_freq = if freq >= MAIN_FREQ {
        Rating::Best
    } else if freq >= MIXED_FREQ {
        Rating::Good
    } else if freq >= 0.03 {
        Rating::Inaccuracy
    } else if freq >= 0.005 {
        Rating::Mistake
    } else {
        Rating::Blunder
    };
    let Some(loss) = loss_bb else { return by_freq };
    let by_loss = if loss < BEST_BB {
        Rating::Best
    } else if loss < GOOD_BB {
        Rating::Good
    } else if loss < INACCURACY_BB {
        Rating::Inaccuracy
    } else if loss < MISTAKE_BB {
        Rating::Mistake
    } else {
        Rating::Blunder
    };
    if freq >= MIXED_FREQ { by_loss.min(by_freq) } else { by_loss }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds() {
        assert_eq!(rate(Some(0.0), 0.0), Rating::Best);
        assert_eq!(rate(Some(0.1), 0.0), Rating::Good);
        assert_eq!(rate(Some(0.3), 0.0), Rating::Inaccuracy);
        assert_eq!(rate(Some(1.0), 0.0), Rating::Mistake);
        assert_eq!(rate(Some(2.0), 0.0), Rating::Blunder);
        // A mixed action played often enough is no mistake, whatever the rounding left.
        assert_eq!(rate(Some(2.0), 0.2), Rating::Good);
        assert_eq!(rate(Some(2.0), 0.6), Rating::Best);
        assert_eq!(rate(None, 0.02), Rating::Mistake);
    }
}
