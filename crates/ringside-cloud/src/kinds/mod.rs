//! Solution kinds. A kind fixes the input and solution types and how to measure a
//! submitted solution's exploitability; adding one means implementing [`Kind`] and listing
//! it in [`KINDS`] (and in the server whitelist, `server/edge/kinds.ts`).

pub mod postflop;
mod pushfold_hu;
pub mod pushfold_mw;
pub mod spot;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::canonical::{CanonicalError, canonicalize, solution_key};

pub use postflop::{Postflop, PostflopInput, PostflopSolution};
pub use pushfold_hu::{PushFoldHu, PushFoldHuInput, PushFoldHuSolution};
pub use pushfold_mw::{PushFoldMw, PushFoldMwInput, PushFoldMwSolution};
pub use spot::{ModelLine, SpotInput, SpotKind, SpotSolution};

pub trait Kind: Sized {
    /// Name in URLs and the database, e.g. `pushfold-hu`.
    const NAME: &'static str;
    /// Version of the game model; bump it whenever the same input would give a different
    /// equilibrium (other equity data, other rules), so old and new solutions never mix.
    const ENGINE_VERSION: u32;
    /// Largest exploitability that still counts as solved, in the kind's unit.
    const TOLERANCE: f64;
    /// Longest the verifier lets one check run, in seconds.
    const CHECK_SECONDS: u64 = 120;

    /// Serialized only with integers and short ASCII strings, see [`crate::canonical`].
    type Input: Serialize + DeserializeOwned;
    type Solution: Serialize + DeserializeOwned;

    /// The ranges the server accepts; mirrors `checkInput` in `server/edge/kinds.ts`.
    fn check_input(input: &Self::Input) -> Result<(), String>;

    /// What best responses gain against `solution` (NashConv), or why it is malformed.
    /// Runs on every verification and download, so it should be far cheaper than solving;
    /// `pushfold-mw` is the exception, see its module docs.
    fn exploitability(input: &Self::Input, solution: &Self::Solution) -> Result<f64, String>;

    /// The verifier's check: the exploitability, and for a kind whose stored result the
    /// verifier derives itself, the result to store instead of the submitted one.
    fn assess(input: &Self::Input, solution: &Self::Solution) -> Result<(f64, Option<Self::Solution>), String> {
        Ok((Self::exploitability(input, solution)?, None))
    }

    /// What the app checks before using a download: by default the full check. A kind
    /// whose check costs a solve only checks the form; the verifier re-solves samples.
    fn check_download(input: &Self::Input, solution: &Self::Solution) -> Result<(), String> {
        match verdict::<Self>(input, solution) {
            Verdict::Verified { .. } => Ok(()),
            Verdict::Rejected { reason, .. } => Err(reason),
        }
    }

    /// The exploitability a solution reports about itself, if its kind stores one.
    fn reported(_solution: &Self::Solution) -> Result<Option<f64>, String> {
        Ok(None)
    }

    fn key(input: &Self::Input) -> Result<String, CanonicalError> {
        let value = serde_json::to_value(input).map_err(|e| CanonicalError(e.to_string()))?;
        Ok(solution_key(Self::NAME, Self::ENGINE_VERSION, &canonicalize(&value)?))
    }
}

/// Outcome of checking a stored submission.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// `result`: JSON to store in place of the submitted result.
    Verified { exploitability: f64, result: Option<String> },
    Rejected { exploitability: Option<f64>, reason: String },
}

/// A kind the verifier knows, with its check over raw stored JSON.
pub struct KindEntry {
    pub name: &'static str,
    pub engine_version: u32,
    pub check_seconds: u64,
    /// (key, canonical input JSON, result JSON) → verdict.
    pub check: fn(&str, &str, &str) -> Verdict,
    /// (key, canonical input JSON, result JSON) → [`accept`]'s answer, without solving.
    pub accept: fn(&str, &str, &str) -> Result<Option<f64>, String>,
}

pub const KINDS: &[KindEntry] =
    &[entry::<PushFoldHu>(), entry::<PushFoldMw>(), entry::<Postflop>(), entry::<SpotKind>()];

pub fn find(name: &str, engine_version: u32) -> Option<&'static KindEntry> {
    KINDS
        .iter()
        .find(|k| k.name == name && k.engine_version == engine_version)
}

const fn entry<K: Kind>() -> KindEntry {
    KindEntry {
        name: K::NAME,
        engine_version: K::ENGINE_VERSION,
        check_seconds: K::CHECK_SECONDS,
        check: check_stored::<K>,
        accept: accept_stored::<K>,
    }
}

fn read_stored<K: Kind>(key: &str, input_json: &str, result_json: &str) -> Result<(K::Input, K::Solution), String> {
    let input: K::Input = serde_json::from_str(input_json).map_err(|e| format!("input: {e}"))?;
    // The server derived the key from this input; recomputing it here means a row whose
    // key and input disagree can never be served.
    if K::key(&input).map_err(|e| e.to_string())? != key {
        return Err("key does not match input".into());
    }
    let solution = serde_json::from_str(result_json).map_err(|e| format!("result: {e}"))?;
    Ok((input, solution))
}

fn accept_stored<K: Kind>(key: &str, input_json: &str, result_json: &str) -> Result<Option<f64>, String> {
    let (input, solution) = read_stored::<K>(key, input_json, result_json)?;
    accept::<K>(&input, &solution)
}

fn check_stored<K: Kind>(key: &str, input_json: &str, result_json: &str) -> Verdict {
    match read_stored::<K>(key, input_json, result_json) {
        Ok((input, solution)) => verdict::<K>(&input, &solution),
        Err(reason) => Verdict::Rejected { exploitability: None, reason },
    }
}

/// Whether `solution` may be served without re-solving it: its form and what the app
/// checks on a download, and a reported exploitability within the kind's tolerance.
/// Returns that exploitability.
pub fn accept<K: Kind>(input: &K::Input, solution: &K::Solution) -> Result<Option<f64>, String> {
    K::check_download(input, solution)?;
    let reported = K::reported(solution)?;
    if let Some(e) = reported
        && !(e.is_finite() && (0.0..=K::TOLERANCE).contains(&e))
    {
        return Err(format!("reported exploitability {e:.3e} outside 0 to {:.1e}", K::TOLERANCE));
    }
    Ok(reported)
}

/// Whether `solution` passes for `input`: well-formed and within the kind's tolerance.
pub fn verdict<K: Kind>(input: &K::Input, solution: &K::Solution) -> Verdict {
    let reject = |reason: String| Verdict::Rejected { exploitability: None, reason };
    if let Err(e) = K::check_input(input) {
        return reject(e);
    }
    match K::assess(input, solution) {
        Ok((e, replacement)) if e.is_finite() && e <= K::TOLERANCE => {
            let result = match replacement.map(|r| serde_json::to_string(&r)).transpose() {
                Ok(r) => r,
                Err(e) => return reject(format!("rebuilt result: {e}")),
            };
            Verdict::Verified { exploitability: e, result }
        }
        Ok((e, _)) => Verdict::Rejected {
            exploitability: e.is_finite().then_some(e),
            reason: format!("exploitability {e:.3e} above tolerance {:.1e}", K::TOLERANCE),
        },
        Err(e) => reject(e),
    }
}

/// The JSON body a client submits for `K`.
pub fn submission<K: Kind>(input: &K::Input, solution: &K::Solution) -> Value {
    serde_json::json!({
        "kind": K::NAME,
        "engine_version": K::ENGINE_VERSION,
        "input": input,
        "result": solution,
    })
}
