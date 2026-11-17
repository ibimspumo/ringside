//! Ringside's AI coach. A language model reviews the hero's decisions in a finished
//! tournament through OpenRouter; every number it uses comes from Ringside's engine,
//! either as a fact computed in advance or through a calculator tool.
//!
//! Flow: [`prepare`] picks the hands and estimates the cost, [`Prepared::confirm`] takes
//! the token of that estimate, [`run`] talks to the model and returns a checked
//! [`Report`]. Nothing is sent before the confirmation.

pub mod concepts;
pub mod openrouter;
pub mod render;
pub mod report;
pub mod select;
pub mod session;
pub mod tools;

pub use concepts::{CONCEPTS, Concept};
pub use openrouter::{ApiError, Client, KeyInfo, ModelInfo};
pub use report::{Report, ReportInfo};
pub use session::{
    CoachError, ConceptCount, Confirmed, Effort, Estimate, Prepared, Progress, concept_counts, prepare, run,
};
