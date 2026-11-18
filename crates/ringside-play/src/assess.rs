//! Where a decision gets its grade when no solution covers it (multiway after the flop, a
//! line the preflop model does not play, a stopped live solve). Without an [`Assess`] such
//! a move stays "nicht bewertet" with its reason; nothing invents a verdict.
//!
//! The calibrated estimate (equity against likely ranges against pot odds, measured
//! against the solver) docks here: it implements [`Assess`] and its answers grade the move
//! with [`Source::Estimate`](crate::hand::Source::Estimate).

use ringside_engine::Card;

use crate::hand::{Choice, Street, Unrated};

/// What an assessment sees of a decision.
#[derive(Clone, Debug)]
pub struct Situation<'a> {
    pub street: Street,
    pub board: &'a [Card],
    pub hero: [Card; 2],
    /// Seat of the hero and of every other player still in, in preflop action order.
    pub hero_seat: usize,
    pub opponents: Vec<usize>,
    pub players: usize,
    /// Everything in the middle, bets in front included, BB.
    pub pot: f64,
    pub to_call: f64,
    /// Chips behind of each seat, BB.
    pub stacks: &'a [f64],
    /// Chips in front of each seat on this street, BB.
    pub bets: &'a [f64],
    /// For each of `opponents`: its range from the preflop model's line, weight per hand
    /// class; `None` once the line left the model.
    pub ranges: Vec<Option<Vec<f64>>>,
    /// Every move after the flop so far, in order.
    pub moves: Vec<Move>,
    /// The seat that bet or raised last.
    pub aggressor: Option<usize>,
    pub choices: &'a [Choice],
    /// Why no solution covers the decision.
    pub why: Unrated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub street: Street,
    pub seat: usize,
    /// As [`Choice::kind`].
    pub kind: &'static str,
}

/// A grade source's answer: the frequency of each choice and, if known, its EV in BB.
#[derive(Clone, Debug, PartialEq)]
pub struct Assessment {
    pub strategy: Vec<f64>,
    pub evs: Option<Vec<f64>>,
    pub equity: Option<f64>,
    /// Share of the calibration spots where the source's grade was within one step of the
    /// solver's, shown beside the verdict.
    pub reliability: Option<f64>,
    /// Why the best move is best, in plain words.
    pub why: Option<String>,
}

pub trait Assess: Send + Sync {
    /// `None`: the source has nothing it would stand by here, and the move stays ungraded.
    fn assess(&self, situation: &Situation) -> Option<Assessment>;
}
