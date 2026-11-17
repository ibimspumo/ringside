//! Calculation core for Ringside's training and analysis tools: hand evaluation,
//! ranges, equity, the 169×169 preflop matrix, ICM and heads-up push/fold Nash.

pub mod cards;
pub mod equity;
pub mod eval;
pub mod icm;
pub mod multiway;
pub mod outs;
pub mod preflop_matrix;
pub mod pushfold;
pub mod pushfold_table;
pub mod range;

use std::fmt;

pub use cards::{Card, CardSet};
pub use equity::{EquityResult, Method, PlayerEquity, equity};
pub use eval::{Hand, HandCategory, HandRank, evaluate};
pub use icm::{IcmResult, icm};
pub use outs::{OutsResult, outs, outs_to};
pub use preflop_matrix::PreflopMatrix;
pub use pushfold::{ClassEv, PushFoldParams, PushFoldResult, solve_push_fold};
pub use range::{Combo, HandClass, Range};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Parse(String),
    InvalidInput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(m) => write!(f, "parse error: {m}"),
            Error::InvalidInput(m) => write!(f, "invalid input: {m}"),
        }
    }
}

impl std::error::Error for Error {}
