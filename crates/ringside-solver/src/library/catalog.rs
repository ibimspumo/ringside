//! The spot catalog: a fixed, versioned list of common tournament postflop spots that the
//! library solves on every flop of a representative subset ([`super::flops`]).
//!
//! A scenario is a heads-up pot after the preflop action (single-raised or 3-bet pot,
//! position against position, starting stack). Its ranges come from Ringside's preflop
//! model (9 players, big-blind ante, chip EV; see `ringside_engine::preflop`) and are
//! frozen in `data/library-v1.json`, written by `gen-library-catalog`; a new preflop
//! model changes the catalog only with a new catalog version.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::flops;
use crate::spot::{Spot, StreetSizes};

pub const CATALOG_VERSION: u32 = 1;
/// Flops per scenario.
pub const FLOPS: usize = 184;

static JSON: &str = include_str!("../../data/library-v1.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PotType {
    /// Open and call.
    Srp,
    /// Open, 3-bet, call.
    ThreeBet,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    /// E.g. "srp-btn-bb-40".
    pub id: String,
    pub pot_type: PotType,
    /// Positions of the player out of and in position after the flop.
    pub oop: String,
    pub ip: String,
    /// Stack of every player before the hand, BB.
    pub stack_bb: f64,
    /// Pot at the flop and the smaller stack behind, BB.
    pub pot: f64,
    pub stack: f64,
    pub oop_range: String,
    pub ip_range: String,
    pub sizes: [StreetSizes; 3],
    pub allin_threshold: f64,
    /// How the ranges came about, shown with the solution.
    pub oop_note: String,
    pub ip_note: String,
}

impl Scenario {
    pub fn spot(&self, flop: &str) -> Spot {
        Spot {
            oop_range: self.oop_range.clone(),
            ip_range: self.ip_range.clone(),
            board: flop.to_string(),
            pot: self.pot,
            stack: self.stack,
            sizes: self.sizes.clone(),
            allin_threshold: self.allin_threshold,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u32,
    /// Where the ranges come from.
    pub source: String,
    /// Canonical flops ([`flops::canonical`]) and how many of the 22 100 flops each stands for.
    pub flops: Vec<String>,
    pub flop_weights: Vec<f64>,
    pub scenarios: Vec<Scenario>,
    /// Exploitability the library solves to, in percent of the pot.
    pub target_pct: f64,
    pub max_iterations: u32,
}

/// One entry: a scenario on a flop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpotId {
    pub scenario: usize,
    pub flop: usize,
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| serde_json::from_str(JSON).expect("library catalog parses"));

pub fn catalog() -> &'static Catalog {
    &CATALOG
}

impl Catalog {
    pub fn scenario(&self, id: &str) -> Option<usize> {
        self.scenarios.iter().position(|s| s.id == id)
    }

    pub fn flop(&self, text: &str) -> Option<usize> {
        self.flops.iter().position(|f| f == text)
    }

    pub fn len(&self) -> usize {
        self.scenarios.len() * self.flops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every spot, scenario by scenario in catalog order, flops in subset order.
    pub fn ids(&self) -> impl Iterator<Item = SpotId> + '_ {
        (0..self.scenarios.len()).flat_map(move |scenario| (0..self.flops.len()).map(move |flop| SpotId { scenario, flop }))
    }

    pub fn spot(&self, id: SpotId) -> Spot {
        self.scenarios[id.scenario].spot(&self.flops[id.flop])
    }

    /// The scenario with exactly the ranges, pot, stack and sizes of `spot`, on any board.
    pub fn scenario_of(&self, spot: &Spot) -> Option<usize> {
        self.scenarios.iter().position(|s| {
            s.oop_range == spot.oop_range
                && s.ip_range == spot.ip_range
                && (s.pot - spot.pot).abs() < 0.011
                && (s.stack - spot.stack).abs() < 0.011
                && s.sizes == spot.sizes
                && s.allin_threshold == spot.allin_threshold
        })
    }
}

/// The flops of a catalog with `n` flops per scenario, as stored in it.
pub fn subset_flops(n: usize) -> (Vec<String>, Vec<f64>) {
    flops::subset(n).into_iter().map(|p| (p.flop.text(), p.weight)).unzip()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::estimate;

    #[test]
    fn catalog_matches_the_flop_procedure() {
        let c = catalog();
        assert_eq!(c.version, CATALOG_VERSION);
        let (flops, weights) = subset_flops(FLOPS);
        assert_eq!(c.flops, flops);
        // JSON floats come back within an ulp, not always bit for bit.
        assert_eq!(c.flop_weights.len(), weights.len());
        for (a, b) in c.flop_weights.iter().zip(&weights) {
            assert!((a - b).abs() <= 1e-9 * b, "{a} vs {b}");
        }
        let mut ids: Vec<&str> = c.scenarios.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), c.scenarios.len(), "scenario ids are unique");
    }

    #[test]
    fn every_scenario_builds_within_the_verifier_limits() {
        let c = catalog();
        for s in &c.scenarios {
            // A two-tone ace-high board: the largest trees come from the widest ranges,
            // which an unpaired board leaves all their combos.
            let e = estimate(&s.spot("AsKs2h"), 6 << 30, 4).unwrap_or_else(|e| panic!("{}: {e}", s.id));
            assert!(e.fits && !e.compressed, "{}: {} bytes", s.id, e.memory_bytes);
        }
    }
}
