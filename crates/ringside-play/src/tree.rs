//! Where a postflop spot's strategies come from: a full solve held in memory, or a stored
//! solution (spot library, saved solve) that knows only the first street and the first
//! decision on the next card.

use ringside_solver::{Error, Node, Solution, Step};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeSource {
    /// Solved by the background solver for this practice.
    Solved,
    /// Solved while a hand waited, less exact ([`crate::pool::LIVE_PCT`]).
    Live,
    /// From the spot library (downloaded or solved in the background).
    Library,
    /// A solve the user saved.
    Saved,
}

pub trait Tree: Send {
    fn node(&mut self, path: &[Step]) -> Result<Node, Error>;
    /// Every node down to the river is there.
    fn complete(&self) -> bool;
    fn source(&self) -> TreeSource;
    /// Exploitability in percent of the pot.
    fn exploitability_pct(&self) -> f64;
    /// Hands the solve over (for the solver page); stored trees have none.
    fn into_solution(self: Box<Self>) -> Option<Solution> {
        None
    }
}

impl Tree for Solution {
    fn node(&mut self, path: &[Step]) -> Result<Node, Error> {
        Solution::node(self, path)
    }

    fn complete(&self) -> bool {
        true
    }

    fn source(&self) -> TreeSource {
        TreeSource::Solved
    }

    fn exploitability_pct(&self) -> f64 {
        self.summary.exploitability_pct
    }

    fn into_solution(self: Box<Self>) -> Option<Solution> {
        Some(*self)
    }
}

impl Tree for ringside_solver::saved::Snapshot {
    fn node(&mut self, path: &[Step]) -> Result<Node, Error> {
        ringside_solver::saved::Snapshot::node(self, path)
    }

    fn complete(&self) -> bool {
        ringside_solver::saved::Snapshot::complete(self)
    }

    fn source(&self) -> TreeSource {
        TreeSource::Saved
    }

    fn exploitability_pct(&self) -> f64 {
        self.meta.summary.exploitability_pct
    }
}

/// A library spot with its catalog entry.
pub struct LibraryTree(pub ringside_solver::library::Stored);

impl Tree for LibraryTree {
    fn node(&mut self, path: &[Step]) -> Result<Node, Error> {
        let catalog = ringside_solver::library::catalog();
        ringside_solver::library::format::View::new(&self.0, catalog)?.node(path)
    }

    fn complete(&self) -> bool {
        false
    }

    fn source(&self) -> TreeSource {
        TreeSource::Library
    }

    fn exploitability_pct(&self) -> f64 {
        self.0.exploitability_pct as f64
    }
}
