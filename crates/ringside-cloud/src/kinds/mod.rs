//! Solution kinds. A kind fixes the input and solution types and how to measure a
//! submitted solution's exploitability; adding one means implementing [`Kind`] and listing
//! it in [`KINDS`] (and in the server whitelist, `server/edge/kinds.ts`).

mod pushfold_hu;
pub mod pushfold_mw;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::canonical::{CanonicalError, canonicalize, solution_key};

pub use pushfold_hu::{PushFoldHu, PushFoldHuInput, PushFoldHuSolution};
pub use pushfold_mw::{PushFoldMw, PushFoldMwInput, PushFoldMwSolution};

pub trait Kind {
    /// Name in URLs and the database, e.g. `pushfold-hu`.
    const NAME: &'static str;
    /// Version of the game model; bump it whenever the same input would give a different
    /// equilibrium (other equity data, other rules), so old and new solutions never mix.
    const ENGINE_VERSION: u32;
    /// Largest exploitability that still counts as solved, in the kind's unit.
    const TOLERANCE: f64;

    /// Serialized only with integers and short ASCII strings, see [`crate::canonical`].
    type Input: Serialize + DeserializeOwned;
    type Solution: Serialize + DeserializeOwned;

    /// The ranges the server accepts; mirrors `checkInput` in `server/edge/kinds.ts`.
    fn check_input(input: &Self::Input) -> Result<(), String>;

    /// What best responses gain against `solution` (NashConv), or why it is malformed.
    /// Runs on every verification and download, so it should be far cheaper than solving;
    /// `pushfold-mw` is the exception, see its module docs.
    fn exploitability(input: &Self::Input, solution: &Self::Solution) -> Result<f64, String>;

    fn key(input: &Self::Input) -> Result<String, CanonicalError> {
        let value = serde_json::to_value(input).map_err(|e| CanonicalError(e.to_string()))?;
        Ok(solution_key(Self::NAME, Self::ENGINE_VERSION, &canonicalize(&value)?))
    }
}

/// Outcome of checking a stored submission.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Verified { exploitability: f64 },
    Rejected { exploitability: Option<f64>, reason: String },
}

/// A kind the verifier knows, with its check over raw stored JSON.
pub struct KindEntry {
    pub name: &'static str,
    pub engine_version: u32,
    /// (key, canonical input JSON, result JSON) → verdict.
    pub check: fn(&str, &str, &str) -> Verdict,
}

pub const KINDS: &[KindEntry] = &[entry::<PushFoldHu>(), entry::<PushFoldMw>()];

pub fn find(name: &str, engine_version: u32) -> Option<&'static KindEntry> {
    KINDS
        .iter()
        .find(|k| k.name == name && k.engine_version == engine_version)
}

const fn entry<K: Kind>() -> KindEntry {
    KindEntry {
        name: K::NAME,
        engine_version: K::ENGINE_VERSION,
        check: check_stored::<K>,
    }
}

fn check_stored<K: Kind>(key: &str, input_json: &str, result_json: &str) -> Verdict {
    let reject = |reason: String| Verdict::Rejected { exploitability: None, reason };
    let input: K::Input = match serde_json::from_str(input_json) {
        Ok(i) => i,
        Err(e) => return reject(format!("input: {e}")),
    };
    // The server derived the key from this input; recomputing it here means a row whose
    // key and input disagree can never be served.
    match K::key(&input) {
        Ok(k) if k == key => {}
        Ok(_) => return reject("key does not match input".into()),
        Err(e) => return reject(e.to_string()),
    }
    let solution: K::Solution = match serde_json::from_str(result_json) {
        Ok(s) => s,
        Err(e) => return reject(format!("result: {e}")),
    };
    verdict::<K>(&input, &solution)
}

/// Whether `solution` passes for `input`: well-formed and within the kind's tolerance.
pub fn verdict<K: Kind>(input: &K::Input, solution: &K::Solution) -> Verdict {
    let reject = |reason: String| Verdict::Rejected { exploitability: None, reason };
    if let Err(e) = K::check_input(input) {
        return reject(e);
    }
    match K::exploitability(input, solution) {
        Ok(e) if e.is_finite() && e <= K::TOLERANCE => Verdict::Verified { exploitability: e },
        Ok(e) => Verdict::Rejected {
            exploitability: e.is_finite().then_some(e),
            reason: format!("exploitability {e:.3e} above tolerance {:.1e}", K::TOLERANCE),
        },
        Err(e) => reject(e),
    }
}

/// The JSON body a client submits for `K`.
pub(crate) fn submission<K: Kind>(input: &K::Input, solution: &K::Solution) -> Value {
    serde_json::json!({
        "kind": K::NAME,
        "engine_version": K::ENGINE_VERSION,
        "input": input,
        "result": solution,
    })
}
