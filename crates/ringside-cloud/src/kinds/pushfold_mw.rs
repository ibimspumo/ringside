//! Push/fold for one table of 2–9 players, chip EV or ICM, as solved by
//! `ringside_engine::multiway::solve`.
//!
//! # Check
//! The submitted profile is measured with `ringside_engine::multiway::evaluate`: one
//! best-response pass per seat on the engine's own tree and showdown model gives its
//! NashConv. That costs a fraction of a solve, so the app can afford to check every
//! download. The Monte Carlo showdowns are deterministic (a seed per class tuple,
//! independent of the thread count), so the same profile always measures the same.
//!
//! # Units
//! Chip EV in BB per hand; ICM in percent of the prize pool. In both the engine's
//! convergence target is 1e-3 ([`TARGET`]).
//!
//! # Model
//! What is verified is the equilibrium of the engine's model: pairwise card removal
//! and Monte Carlo showdowns with [`SAMPLES`] deals per class tuple for pots of three
//! or more (see the engine's module docs). Both are fixed by [`ENGINE_VERSION`], so
//! the reference and every client use the same estimate and its sampling error never
//! shows up as exploitability.
//!
//! [`ENGINE_VERSION`]: PushFoldMw::ENGINE_VERSION

use std::sync::{Arc, LazyLock, Mutex};

use ringside_engine::multiway::{self, Params, Solution};
use serde::{Deserialize, Serialize, Serializer};

use super::Kind;

pub struct PushFoldMw;

/// Monte Carlo deals per class tuple for showdowns of three or more players.
pub const SAMPLES: u32 = 1024;
pub const MAX_ITERATIONS: u32 = 5_000;
/// The engine's NashConv target in this kind's units.
pub const TARGET: f64 = 1e-3;
/// Prize pool of a normalized ICM input.
pub const POOL_PPM: u32 = 1_000_000;
const MAX_PLAYERS: usize = multiway::MAX_PLAYERS;
/// Pots of four or more all-in players take minutes to solve; the cloud only takes
/// what a verifier can check in seconds.
const MAX_ALL_IN: u32 = 3;
/// Most rounding error allowed in the sum of the payout shares.
const PPM_SLACK: u32 = MAX_PLAYERS as u32;

/// Amounts in thousandths of a big blind, payouts in millionths of the prize pool, so
/// the key never depends on float formatting and ICM spots of any buy-in share keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushFoldMwInput {
    /// Stack per seat before posting, in preflop action order (first after the big blind,
    /// …, small blind, big blind); the number of players is its length.
    pub stacks_mbb: Vec<u32>,
    pub sb_mbb: u32,
    /// Ante paid by every player.
    pub ante_mbb: u32,
    /// Ante paid by the big blind alone.
    pub bb_ante_mbb: u32,
    /// Extra dead money in the pot.
    pub dead_mbb: u32,
    /// Most players all in at once; later players fold.
    pub max_all_in: u32,
    /// Prize per finishing place, first place first, as parts of [`POOL_PPM`]; empty for
    /// chip EV.
    pub payouts_ppm: Vec<u32>,
}

fn mbb(x: f64) -> u32 {
    (x * 1000.0).round().clamp(0.0, u32::MAX as f64) as u32
}

impl PushFoldMwInput {
    /// The input for `p`, amounts rounded to the nearest milli-BB and payouts normalized
    /// to the prize pool. Fails when `p` asks for solver settings other than this kind's
    /// (see [`Self::params`]), because the cloud's solutions would not answer that.
    pub fn from_params(p: &Params) -> Result<PushFoldMwInput, String> {
        let pool: f64 = p.payouts.iter().sum();
        let expected_target = if p.payouts.is_empty() { TARGET } else { 1e-5 * pool };
        let same_settings = p.card_removal
            && p.multiway_samples == SAMPLES
            && p.max_iterations == MAX_ITERATIONS
            && (p.target_exploitability - expected_target).abs() <= 1e-9 * expected_target.abs().max(1.0);
        if !same_settings {
            return Err("solver settings differ from the cloud's".into());
        }
        if !p.payouts.is_empty() && !(pool.is_finite() && pool > 0.0) {
            return Err("the prize pool must be positive".into());
        }
        Ok(PushFoldMwInput {
            stacks_mbb: p.stacks_bb.iter().map(|&s| mbb(s)).collect(),
            sb_mbb: mbb(p.small_blind_bb),
            ante_mbb: mbb(p.ante_bb),
            bb_ante_mbb: mbb(p.bb_ante_bb),
            dead_mbb: mbb(p.dead_bb),
            max_all_in: p.max_all_in.min(u32::MAX as usize) as u32,
            payouts_ppm: p
                .payouts
                .iter()
                .map(|x| (x / pool * POOL_PPM as f64).round().clamp(0.0, POOL_PPM as f64) as u32)
                .collect(),
        })
    }

    /// Engine parameters with this kind's fixed solver settings; ICM payouts in parts of
    /// [`POOL_PPM`].
    pub fn params(&self) -> Params {
        let p = Params {
            stacks_bb: self.stacks_mbb.iter().map(|&s| s as f64 / 1000.0).collect(),
            small_blind_bb: self.sb_mbb as f64 / 1000.0,
            ante_bb: self.ante_mbb as f64 / 1000.0,
            bb_ante_bb: self.bb_ante_mbb as f64 / 1000.0,
            dead_bb: self.dead_mbb as f64 / 1000.0,
            card_removal: true,
            target_exploitability: TARGET,
            max_iterations: MAX_ITERATIONS,
            multiway_samples: SAMPLES,
            max_all_in: self.max_all_in as usize,
            ..Params::new(Vec::new())
        };
        if self.payouts_ppm.is_empty() {
            p
        } else {
            p.with_payouts(self.payouts_ppm.iter().map(|&x| x as f64).collect())
        }
    }

    pub fn players(&self) -> usize {
        self.stacks_mbb.len()
    }

    fn icm(&self) -> bool {
        !self.payouts_ppm.is_empty()
    }

    /// Engine result units (BB, or parts of [`POOL_PPM`]) → this kind's units.
    fn unit(&self) -> f64 {
        if self.icm() { 100.0 / self.payouts_ppm.iter().map(|&x| x as f64).sum::<f64>() } else { 1.0 }
    }
}

/// Decision nodes of the engine's tree: one open node per seat but the big blind, then
/// one per (shover, later seat, set of callers in between) while fewer than `cap`
/// players are all in. Lets the shape of a submission be checked before solving.
pub fn node_count(players: usize, cap: usize) -> usize {
    let choose = |n: usize, k: usize| -> usize { (0..k).fold(1, |acc, i| acc * (n - i) / (i + 1)) };
    let mut count = players.saturating_sub(1);
    for shover in 0..players {
        for seat in shover + 1..players {
            let between = seat - shover - 1;
            count += (0..=between.min(cap.saturating_sub(2))).map(|k| choose(between, k)).sum::<usize>();
        }
    }
    count
}

/// Shove or call frequency per class at every decision node, in the engine's node order
/// (`Solution::nodes`), indexed by `HandClass::index`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PushFoldMwSolution {
    #[serde(serialize_with = "compact")]
    pub act: Vec<Vec<f64>>,
}

/// Mixed frequencies to six decimals, pure ones as `0` and `1`: a 9-player profile has
/// 21 000 entries and must fit in one request.
fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn compact<S: Serializer>(act: &[Vec<f64>], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut outer = s.serialize_seq(Some(act.len()))?;
    for node in act {
        let row: Vec<serde_json::Value> = node
            .iter()
            .map(|&x| {
                if x == 0.0 || x == 1.0 {
                    serde_json::Value::from(x as u8)
                } else {
                    serde_json::Number::from_f64(x).map_or(serde_json::Value::Null, serde_json::Value::Number)
                }
            })
            .collect();
        outer.serialize_element(&row)?;
    }
    outer.end()
}

impl From<&Solution> for PushFoldMwSolution {
    fn from(s: &Solution) -> PushFoldMwSolution {
        PushFoldMwSolution { act: s.nodes.iter().map(|n| n.act.iter().map(|&x| round6(x)).collect()).collect() }
    }
}

impl Kind for PushFoldMw {
    const NAME: &'static str = "pushfold-mw";
    const ENGINE_VERSION: u32 = 1;
    /// The engine's convergence target (1e-3, what its own solves reach) plus half of it
    /// for rounding to six decimals and float noise. At 6 players and 10 BB, folding
    /// aces under the gun or calling everything at the last node lands far above it.
    const TOLERANCE: f64 = 1.5e-3;

    type Input = PushFoldMwInput;
    type Solution = PushFoldMwSolution;

    fn check_input(i: &PushFoldMwInput) -> Result<(), String> {
        let n = i.players();
        let min_stack = 1000 + i.ante_mbb + i.bb_ante_mbb;
        let ppm_sum: u32 = i.payouts_ppm.iter().sum();
        let ok = (2..=MAX_PLAYERS).contains(&n)
            && i.stacks_mbb.iter().all(|s| (min_stack..=100_000).contains(s))
            && (1..=1000).contains(&i.sb_mbb)
            && i.ante_mbb <= 1000
            && i.bb_ante_mbb <= 2000
            && i.dead_mbb <= 50_000
            && (2..=MAX_ALL_IN).contains(&i.max_all_in)
            && i.payouts_ppm.len() <= n
            && i.payouts_ppm.iter().all(|&x| x <= POOL_PPM)
            && (i.payouts_ppm.is_empty() || ppm_sum.abs_diff(POOL_PPM) <= PPM_SLACK);
        if ok { Ok(()) } else { Err(format!("input out of range: {i:?}")) }
    }

    fn exploitability(input: &PushFoldMwInput, s: &PushFoldMwSolution) -> Result<f64, String> {
        Ok(evaluated(input, s)?.exploitability * input.unit())
    }
}

/// `solution` measured on `input`: the full engine [`Solution`] of that profile (EVs and
/// best-response gains per node), in [`PushFoldMwInput::params`] units.
pub fn evaluated(input: &PushFoldMwInput, solution: &PushFoldMwSolution) -> Result<Solution, String> {
    PushFoldMw::check_input(input)?;
    multiway::evaluate(&input.params(), &solution.act).map_err(|e| e.to_string())
}

/// Reference solves of the last few inputs, so a download checked for a caller and the
/// result handed to it come from one solve.
type Solved = (PushFoldMwInput, Arc<Solution>);
static REFERENCES: LazyLock<Mutex<Vec<Solved>>> = LazyLock::new(Mutex::default);
const KEEP_REFERENCES: usize = 4;

/// The engine's solution of `input` (in [`PushFoldMwInput::params`] units).
pub fn reference(input: &PushFoldMwInput) -> Result<Arc<Solution>, String> {
    PushFoldMw::check_input(input)?;
    if let Some((_, s)) = REFERENCES.lock().unwrap().iter().find(|(i, _)| i == input) {
        return Ok(s.clone());
    }
    let solved = Arc::new(multiway::solve(&input.params()).map_err(|e| e.to_string())?);
    let mut cache = REFERENCES.lock().unwrap();
    if cache.len() >= KEEP_REFERENCES {
        cache.remove(0);
    }
    cache.push((input.clone(), solved.clone()));
    Ok(solved)
}

/// `solution` of the normalized input in the units of `p`: ICM values back from parts of
/// [`POOL_PPM`] to `p`'s prize pool. Chip EV is unchanged.
pub fn to_caller_units(mut solution: Solution, p: &Params) -> Solution {
    if p.payouts.is_empty() {
        return solution;
    }
    let f = p.payouts.iter().sum::<f64>() / POOL_PPM as f64;
    let scale = |v: &mut Vec<f64>| v.iter_mut().for_each(|x| *x *= f);
    scale(&mut solution.ev);
    scale(&mut solution.gain);
    for node in &mut solution.nodes {
        scale(&mut node.ev_act);
        scale(&mut node.ev_fold);
    }
    solution.exploitability *= f;
    solution.approximation.ev_sum_error *= f;
    solution
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringside_engine::range::{HandClass, NUM_CLASSES};
    use crate::kinds::{KINDS, Verdict, verdict};

    fn six(stack_mbb: u32) -> PushFoldMwInput {
        PushFoldMwInput {
            stacks_mbb: vec![stack_mbb; 6],
            sb_mbb: 500,
            ante_mbb: 125,
            bb_ante_mbb: 0,
            dead_mbb: 0,
            max_all_in: 3,
            payouts_ppm: Vec::new(),
        }
    }

    fn icm3() -> PushFoldMwInput {
        PushFoldMwInput {
            stacks_mbb: vec![8_000, 15_000, 12_000],
            max_all_in: 2,
            payouts_ppm: vec![650_000, 350_000],
            ..six(0)
        }
    }

    fn solved(input: &PushFoldMwInput) -> PushFoldMwSolution {
        PushFoldMwSolution::from(&*reference(input).unwrap())
    }

    #[test]
    fn node_count_matches_the_engine() {
        // Deep stacks keep the ranges, and with them the showdown tables, small.
        for (n, cap) in [(2, 2), (3, 3), (4, 3), (4, 2), (6, 3), (9, 3)] {
            let s = multiway::solve(&Params { max_iterations: 1, max_all_in: cap, ..Params::equal(n, 40.0) }).unwrap();
            assert_eq!(node_count(n, cap), s.nodes.len(), "{n} players, cap {cap}");
        }
    }

    #[test]
    fn engine_solutions_pass_with_the_reference_nashconv() {
        for input in [six(10_000), icm3()] {
            let s = solved(&input);
            match verdict::<PushFoldMw>(&input, &s) {
                Verdict::Verified { exploitability, .. } => assert!(exploitability <= TARGET * 1.001, "{exploitability}"),
                v => panic!("{input:?}: {v:?}"),
            }
        }
    }

    #[test]
    fn manipulated_solutions_fail() {
        let input = six(10_000);
        let good = solved(&input);
        let aa = "AA".parse::<HandClass>().unwrap().index();
        let mut folds_aces = good.clone();
        folds_aces.act[0][aa] = 0.0;
        let mut everyone_shoves = good.clone();
        everyone_shoves.act[0].iter_mut().for_each(|f| *f = 1.0);
        let other_stack = solved(&six(4_000));
        for (what, bad) in [("UTG folds AA", folds_aces), ("UTG shoves all", everyone_shoves), ("other stack", other_stack)] {
            assert!(
                matches!(verdict::<PushFoldMw>(&input, &bad), Verdict::Rejected { exploitability: Some(e), .. } if e > PushFoldMw::TOLERANCE),
                "{what}"
            );
        }
    }

    #[test]
    fn malformed_solutions_and_inputs_fail() {
        let input = six(10_000);
        let good = solved(&input);
        let mut short = good.clone();
        short.act.pop();
        let mut narrow = good.clone();
        narrow.act[3].pop();
        let mut nan = good;
        nan.act[1][5] = f64::NAN;
        for bad in [short, narrow, nan] {
            assert!(matches!(verdict::<PushFoldMw>(&input, &bad), Verdict::Rejected { exploitability: None, .. }));
        }
        let expensive = PushFoldMwInput { max_all_in: 4, ..six(10_000) };
        let ten = PushFoldMwInput { stacks_mbb: vec![10_000; 10], ..six(10_000) };
        let deep = PushFoldMwInput { stacks_mbb: vec![100_001, 10_000], ..six(10_000) };
        let bad_pool = PushFoldMwInput { payouts_ppm: vec![500_000, 400_000], ..icm3() };
        for bad in [expensive, ten, deep, bad_pool] {
            assert!(PushFoldMw::check_input(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn compact_json_round_trips_and_fits_a_request() {
        let s = solved(&six(10_000));
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("[0,") || json.contains(",0,"), "pure entries are written as integers");
        assert_eq!(serde_json::from_str::<PushFoldMwSolution>(&json).unwrap(), s);
        let entries = node_count(9, 3) * NUM_CLASSES;
        assert!(entries * 2 + 4 * 1024 < 128 * 1024, "a pure 9-player profile fits in the server's limit");
    }

    #[test]
    fn stored_rows_are_checked_against_their_key() {
        let entry = KINDS.iter().find(|k| k.name == PushFoldMw::NAME).unwrap();
        let input = six(10_000);
        let (json_in, json_out) = (serde_json::to_string(&input).unwrap(), serde_json::to_string(&solved(&input)).unwrap());
        let key = PushFoldMw::key(&input).unwrap();
        assert!(matches!((entry.check)(&key, &json_in, &json_out), Verdict::Verified { .. }));
        let other = PushFoldMw::key(&six(10_001)).unwrap();
        assert!(matches!((entry.check)(&other, &json_in, &json_out), Verdict::Rejected { .. }));
    }

    #[test]
    fn keys_match_the_shared_vectors() {
        let v: serde_json::Value = serde_json::from_str(include_str!("../../testdata/canonical.json")).unwrap();
        let mut seen = 0;
        for case in v["valid"].as_array().unwrap().iter().filter(|c| c["kind"] == PushFoldMw::NAME) {
            let input: PushFoldMwInput = serde_json::from_value(case["input"].clone()).unwrap();
            assert!(PushFoldMw::check_input(&input).is_ok(), "{input:?}");
            assert_eq!(PushFoldMw::key(&input).unwrap(), case["key"].as_str().unwrap());
            seen += 1;
        }
        assert!(seen >= 2, "chip EV and ICM vectors");
    }

    #[test]
    fn params_round_trip_and_icm_is_normalized() {
        let p = Params { ante_bb: 0.1254, ..Params::new(vec![10.0, 12.3456, 8.0]) }.with_payouts(vec![50.0, 30.0, 20.0]);
        let i = PushFoldMwInput::from_params(&p).unwrap();
        assert_eq!(i.stacks_mbb, [10_000, 12_346, 8_000]);
        assert_eq!((i.ante_mbb, i.max_all_in), (125, 3));
        assert_eq!(i.payouts_ppm, [500_000, 300_000, 200_000]);
        assert_eq!(PushFoldMwInput::from_params(&i.params()).unwrap(), i);
        let dollars = Params::new(vec![10.0, 12.3456, 8.0]).with_payouts(vec![5000.0, 3000.0, 2000.0]);
        assert_eq!(PushFoldMwInput::from_params(&dollars).unwrap().payouts_ppm, i.payouts_ppm, "any buy-in shares a key");
        let custom = Params { card_removal: false, ..Params::equal(3, 10.0) };
        assert!(PushFoldMwInput::from_params(&custom).is_err());
        let stricter = Params { target_exploitability: 1e-4, ..Params::equal(3, 10.0) };
        assert!(PushFoldMwInput::from_params(&stricter).is_err());
    }

    #[test]
    fn icm_values_return_to_the_callers_pool() {
        let p = Params::new(vec![8.0, 15.0, 12.0]).with_payouts(vec![65.0, 35.0]);
        let p = Params { max_all_in: 2, ante_bb: 0.125, ..p };
        let input = PushFoldMwInput::from_params(&p).unwrap();
        assert_eq!(input, icm3());
        let s = to_caller_units((*reference(&input).unwrap()).clone(), &p);
        let total: f64 = s.ev.iter().sum();
        assert!((total - 100.0).abs() < 0.5, "EVs add up to the caller's pool: {total}");
    }
}
