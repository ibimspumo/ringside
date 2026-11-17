//! The spot library: a catalog of common spots, solved once and stored compactly, so the
//! app can show them without solving and share them through Ringside Cloud.

pub mod catalog;
pub mod flops;
pub mod format;
pub mod solve;

pub use catalog::{Catalog, PotType, Scenario, SpotId, catalog};
pub use format::Stored;
pub use solve::{Checked, check, solve_spot};
