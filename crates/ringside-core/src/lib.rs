//! Ringside core: everything that does not depend on Windows or the UI.
//!
//! Data flow: hand history text → [`parser`] → [`model::Hand`] → [`stats::analyze`]
//! → per-player counters stored by [`db::Db`] → aggregated [`stats::StatId`] values.

pub mod db;
pub mod import;
pub mod model;
pub mod parser;
pub mod stats;
pub mod table;

pub use db::Db;
pub use model::Hand;
pub use parser::{parse_hand, split_hands};
