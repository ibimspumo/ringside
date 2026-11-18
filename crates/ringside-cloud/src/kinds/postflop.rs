//! Heads-up postflop spots of the library catalog (`ringside_solver::library`), stored in
//! its compact format: every flop decision per combo, the first turn decision per class.
//!
//! # Check
//! The verifier measures the stored flop strategies with `ringside_solver::library::check`:
//! for each player a solve with that player's flop locked, the opponent free, gives what
//! the flop strategy concedes to a best response (three solves in all, a minute or two on
//! a 4-core runner). Everything else in the file is derived, so the verifier stores the
//! spot it rebuilt from its own solves instead of the submitted one ([`Kind::assess`]).
//!
//! Only catalog spots are accepted, because the check costs a solve and the catalog keeps
//! every tree within the verifier's time and memory. The app does not repeat the check on
//! a download ([`Kind::check_download`]): it reads the file, which checks its form against
//! the catalog, and relies on the verifier for the strategy.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ringside_solver::library::catalog::{CATALOG_VERSION, catalog};
use ringside_solver::library::format::{Stored, header, reported_exploitability};
use ringside_solver::library::check;
use serde::{Deserialize, Serialize};

use super::Kind;

pub struct Postflop;

/// A spot of the catalog: its version, a scenario id and a canonical flop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostflopInput {
    pub catalog: u32,
    pub scenario: String,
    pub flop: String,
}

/// The stored spot, base64.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostflopSolution {
    pub data: String,
}

/// Largest stored spot the cloud takes; the biggest catalog spots are about 150 KB, and
/// the API takes bodies up to 256 KB, which base64 fills at 192 KB.
pub const MAX_BYTES: usize = 180_000;
/// The verifier's limits per solve: a GitHub runner has 4 cores and 16 GB.
const CHECK_MEMORY: u64 = 8 << 30;
const CHECK_ITERATIONS: u32 = 300;

impl PostflopInput {
    pub fn new(scenario: &str, flop: &str) -> PostflopInput {
        PostflopInput { catalog: CATALOG_VERSION, scenario: scenario.into(), flop: flop.into() }
    }
}

impl PostflopSolution {
    pub fn from_stored(stored: &Stored) -> PostflopSolution {
        PostflopSolution { data: STANDARD.encode(stored.to_bytes()) }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        if self.data.len() > MAX_BYTES.div_ceil(3) * 4 {
            return Err("stored spot too large".into());
        }
        STANDARD.decode(&self.data).map_err(|e| format!("data: {e}"))
    }

    /// The stored spot, read and checked against `input` and the catalog.
    pub fn stored(&self, input: &PostflopInput) -> Result<Stored, String> {
        let bytes = self.bytes()?;
        let (version, scenario, flop) = header(&bytes).map_err(|e| e.to_string())?;
        if version != input.catalog || scenario != input.scenario || flop != input.flop {
            return Err("stored spot is for another input".into());
        }
        Stored::from_bytes(&bytes, catalog()).map_err(|e| e.to_string())
    }
}

impl Kind for Postflop {
    const NAME: &'static str = "postflop";
    const ENGINE_VERSION: u32 = 1;
    /// Percent of the pot. The library solves to 0.5 %; a correct spot measures about
    /// that (0.4–0.6 % in tests), a strategy that was not solved measures tens of percent.
    const TOLERANCE: f64 = 1.0;
    const CHECK_SECONDS: u64 = 900;

    type Input = PostflopInput;
    type Solution = PostflopSolution;

    fn check_input(i: &PostflopInput) -> Result<(), String> {
        let c = catalog();
        if i.catalog != c.version || c.scenario(&i.scenario).is_none() || c.flop(&i.flop).is_none() {
            return Err(format!("not a catalog spot: {i:?}"));
        }
        Ok(())
    }

    fn exploitability(input: &PostflopInput, solution: &PostflopSolution) -> Result<f64, String> {
        Ok(Self::assess(input, solution)?.0)
    }

    fn assess(input: &PostflopInput, solution: &PostflopSolution) -> Result<(f64, Option<PostflopSolution>), String> {
        let stored = solution.stored(input)?;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let checked =
            check(&stored, catalog(), threads, CHECK_MEMORY, CHECK_ITERATIONS, || false).map_err(|e| e.to_string())?;
        Ok((checked.exploitability_pct, Some(PostflopSolution::from_stored(&checked.rebuilt))))
    }

    fn check_download(input: &PostflopInput, solution: &PostflopSolution) -> Result<(), String> {
        Self::check_input(input)?;
        solution.stored(input).map(|_| ())
    }

    fn reported(solution: &PostflopSolution) -> Result<Option<f64>, String> {
        let bytes = solution.bytes()?;
        Ok(Some(reported_exploitability(&bytes).map_err(|e| e.to_string())? as f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::canonicalize;

    fn shared() -> serde_json::Value {
        serde_json::from_str(include_str!("../../testdata/postflop.json")).unwrap()
    }

    #[test]
    fn catalog_matches_the_shared_vectors() {
        let v = shared();
        let c = catalog();
        assert_eq!(v["catalog"], c.version);
        let ids: Vec<&str> = c.scenarios.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(v["scenarios"], serde_json::json!(ids), "scenarios in server/edge/kinds.ts");
        assert_eq!(v["flops"], serde_json::json!(c.flops), "flops in server/edge/kinds.ts");
        for case in v["hashes"].as_array().unwrap() {
            let input: PostflopInput = serde_json::from_value(case["input"].clone()).unwrap();
            assert_eq!(canonicalize(&case["input"]).unwrap(), case["canonical"].as_str().unwrap());
            assert_eq!(Postflop::key(&input).unwrap(), case["solution_hash"].as_str().unwrap());
        }
    }

    #[test]
    fn only_catalog_spots_are_inputs() {
        assert!(Postflop::check_input(&PostflopInput::new("srp-btn-bb-40", &catalog().flops[0])).is_ok());
        assert!(Postflop::check_input(&PostflopInput::new("srp-btn-bb-41", &catalog().flops[0])).is_err());
        assert!(Postflop::check_input(&PostflopInput::new("srp-btn-bb-40", "AhKdTc")).is_err());
        let old = PostflopInput { catalog: 0, ..PostflopInput::new("srp-btn-bb-40", &catalog().flops[0]) };
        assert!(Postflop::check_input(&old).is_err());
    }

    #[test]
    fn headers_must_match_the_input() {
        let v = shared();
        for case in v["headers"].as_array().unwrap() {
            let input: PostflopInput = serde_json::from_value(case["input"].clone()).unwrap();
            let data = PostflopSolution { data: case["data"].as_str().unwrap().into() };
            let ok = data.bytes().and_then(|b| {
                let (version, scenario, flop) = header(&b).map_err(|e| e.to_string())?;
                (version == input.catalog && scenario == input.scenario && flop == input.flop)
                    .then_some(())
                    .ok_or_else(|| "other input".to_string())
            });
            assert_eq!(ok.is_ok(), case["valid"].as_bool().unwrap(), "{}", case["what"]);
        }
    }
}
