//! How well the estimate agrees with the solver (after the flop) and the preflop model
//! (before it), measured by `analysis-calibrate` and stored in
//! `data/estimate_calibration.json`: for every option of every hand at the checked
//! nodes, whether the estimate's grade equals the reference's or is one step off.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::estimate::Tuning;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Agreement {
    /// Graded options compared.
    pub samples: usize,
    /// Share with the same grade (best and mix count as one).
    pub same: f64,
    /// Share at most one grade apart.
    pub within_one: f64,
    /// The same per grade of the reference, averaged over the grades: how well mistakes
    /// are found, not only good moves confirmed. The reliability shown is the lower of
    /// the two.
    #[serde(default)]
    pub balanced: f64,
    /// Spots (solved flops or model tables) the samples come from.
    pub spots: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Fitted on half of the checked spots; the agreements are those of the other half.
    pub preflop_tuning: Tuning,
    pub postflop_tuning: Tuning,
    pub preflop: Agreement,
    pub postflop: Agreement,
    /// The played decisions of real hands at solved nodes, as a check on the sample.
    pub real: Agreement,
}

static STORED: LazyLock<Calibration> =
    LazyLock::new(|| serde_json::from_str(include_str!("../data/estimate_calibration.json")).expect("calibration file"));

pub fn stored() -> &'static Calibration {
    &STORED
}

impl Agreement {
    /// What the analysis shows as the estimate's reliability.
    pub fn reliability(&self) -> f64 {
        self.within_one.min(self.balanced)
    }
}
