//! How well the estimate agrees with the solver (after the flop) and the preflop model
//! (before it), measured by `analysis-calibrate` and stored in
//! `data/estimate_calibration.json`: for every option of every hand at the checked
//! nodes, whether the estimate's grade equals the reference's or is one step off; and
//! the curves the opponents' ranges narrow by after the flop.
//!
//! The reliability the analysis shows for a street is the lowest agreement within one
//! step of every check of that street with at least [`MIN_SAMPLES`] comparisons, plain
//! or balanced over the reference grades; smaller checks are named, not counted.

use std::sync::LazyLock;

use ringside_core::model::Street;
use ringside_engine::{Card, CardSet, Combo};
use serde::{Deserialize, Serialize};

use crate::estimate::{Curve, Opponent, Opt, Range, Signal, Spot, Strength, Tuning, narrow_by};

/// Fewer comparisons than this say too little to stand for the estimate's reliability.
pub const MIN_SAMPLES: usize = 100;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Agreement {
    /// Graded options compared.
    pub samples: usize,
    /// Share with the same grade (best and mix count as one).
    pub same: f64,
    /// Share at most one grade apart.
    pub within_one: f64,
    /// The same per grade of the reference, averaged over the grades: how well mistakes
    /// are found, not only good moves confirmed.
    #[serde(default)]
    pub balanced: f64,
    /// Spots (solved flops, model tables or real hands) the samples come from.
    pub spots: usize,
    /// Comparisons where the reference's grade is below "mix": what the balanced share
    /// of the mistakes rests on.
    #[serde(default)]
    pub off_best: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Fitted on half of the checked spots; the agreements are those of the other half.
    pub preflop_tuning: Tuning,
    /// Flop, turn and river.
    pub street_tunings: [Tuning; 3],
    pub preflop: Agreement,
    pub postflop: Agreement,
    /// The played decisions of real hands at solved nodes, as a check on the sample.
    pub real: Agreement,
    /// `postflop` and `real` on the flop, the turn and the river.
    #[serde(default)]
    pub postflop_streets: [Agreement; 3],
    #[serde(default)]
    pub real_streets: [Agreement; 3],
    /// How the opponents' ranges narrow after the flop.
    pub moves: Vec<Curve>,
}

static STORED: LazyLock<Calibration> =
    LazyLock::new(|| serde_json::from_str(include_str!("../data/estimate_calibration.json")).expect("calibration file"));

pub fn stored() -> &'static Calibration {
    &STORED
}

impl Agreement {
    /// The lower of plain and balanced agreement within one step; the balanced one only
    /// with enough of the reference's mistakes to tell how well they are found.
    pub fn reliability(&self) -> f64 {
        if self.off_best >= MIN_SAMPLES { self.within_one.min(self.balanced) } else { self.within_one }
    }
}

impl Calibration {
    /// Every check of the estimate at `street`, by name.
    pub fn checks(&self, street: Street) -> Vec<(&'static str, &Agreement)> {
        match street {
            Street::Preflop => vec![("Preflop-Modell", &self.preflop)],
            s => {
                let k = match s {
                    Street::Flop => 0,
                    Street::Turn => 1,
                    _ => 2,
                };
                vec![
                    ("Solver-Spots", &self.postflop_streets[k]),
                    ("echte Entscheidungen", &self.real_streets[k]),
                    ("echte Entscheidungen aller Straßen", &self.real),
                ]
            }
        }
    }

    /// What the analysis shows as the estimate's reliability at `street`.
    /// Without a check large enough the smaller ones stand in.
    pub fn reliability(&self, street: Street) -> f64 {
        let checks = self.checks(street);
        let large = checks.iter().any(|(_, a)| a.samples >= MIN_SAMPLES);
        checks
            .iter()
            .filter(|(_, a)| a.samples > 0 && (!large || a.samples >= MIN_SAMPLES))
            .map(|(_, a)| a.reliability())
            .reduce(f64::min)
            .unwrap_or(0.0)
    }

    /// The numbers the estimate uses at `street`.
    pub fn tuning(&self, street: Street) -> Tuning {
        match street {
            Street::Preflop => self.preflop_tuning,
            Street::Flop => self.street_tunings[0],
            Street::Turn => self.street_tunings[1],
            Street::River => self.street_tunings[2],
        }
    }

    /// What the reliability at `street` leaves out for too few cases, in words.
    pub fn small(&self, street: Street) -> Vec<String> {
        self.checks(street)
            .into_iter()
            .filter_map(|(name, a)| {
                if a.samples < MIN_SAMPLES {
                    Some(format!("{name} ({} Vergleiche)", a.samples))
                } else if a.off_best < MIN_SAMPLES {
                    Some(format!("{name} beim Finden von Fehlern (nur {} Fehler)", a.off_best))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// A decision node of a solved heads-up spot as the calibration keeps it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolvedNode {
    pub spot: usize,
    /// Table, stack and pot type of the spot.
    pub label: String,
    pub stack: f64,
    pub pot_type: String,
    pub board: Vec<String>,
    /// 0 out of position, 1 in position.
    pub me: usize,
    pub pot: f64,
    pub street_bets: [f64; 2],
    pub stacks: [f64; 2],
    pub aggressor: bool,
    /// The opponent's range at the flop and what they did since, with the board then.
    pub opp_range: String,
    pub line: Vec<(usize, Signal)>,
    /// Kind (as [`Opt::key`]) and amount of every action.
    pub options: Vec<(String, f64)>,
    pub hands: Vec<SolvedHand>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolvedHand {
    pub combo: String,
    pub strategy: Vec<f64>,
    pub evs: Vec<f64>,
    /// The solver's equity of the hand against the opponent's range at the node.
    pub equity: f64,
}

impl SolvedNode {
    pub fn cards(&self) -> Vec<Card> {
        self.board.iter().filter_map(|c| c.parse().ok()).collect()
    }

    pub fn options(&self) -> Vec<Opt> {
        self.options
            .iter()
            .map(|(k, a)| match k.as_str() {
                "fold" => Opt::Fold,
                "check" => Opt::Check,
                "call" => Opt::Call,
                "bet" => Opt::Bet(*a),
                "raise" => Opt::Raise(*a),
                _ => Opt::AllIn,
            })
            .collect()
    }

    /// The opponent's range narrowed along the line by `curves`; `strength` gives
    /// [`crate::estimate::strength`] of a board.
    pub fn opponent_range(&self, strength: impl Fn(&[Card]) -> Strength, curves: &[Curve]) -> Range {
        narrowed(&self.opp_range, &self.line, &self.cards(), strength, curves)
    }

    /// The decision with `hero` against `opp`, as the estimate sees it.
    pub fn spot(&self, hero: Combo, opp: &Range) -> Spot {
        let other = 1 - self.me;
        let board = self.cards();
        Spot {
            preflop: false,
            river: board.len() == 5,
            board,
            hero,
            opponents: vec![Opponent { range: opp.clone(), behind: self.stacks[other], street_in: self.street_bets[other] }],
            pot: self.pot,
            hero_in: self.street_bets[self.me],
            hero_behind: self.stacks[self.me],
            in_position: self.me == 1,
            aggressor: self.aggressor,
        }
    }
}

/// The range of `text` narrowed along `line` on `cards` by `curves`.
pub fn narrowed(text: &str, line: &[(usize, Signal)], cards: &[Card], strength: impl Fn(&[Card]) -> Strength, curves: &[Curve]) -> Range {
    let mut range = Range::parse(text).unwrap_or_else(Range::all);
    for (len, sig) in line {
        let board = &cards[..(*len).min(cards.len())];
        narrow_by(&mut range, *sig, &strength(board), CardSet::from_cards(board), curves);
    }
    range
}
