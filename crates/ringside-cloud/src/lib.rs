//! Ringside Cloud: look up solutions other clients computed, submit own ones for
//! verification, and send parser failure reports.
//!
//! The server (`server/edge` in the repository) only serves solutions a trusted verifier
//! (`ringside-verify`, see [`operator`]) has checked, and the client checks them again
//! before using them, so neither a broken client nor the server can spread wrong results.

pub mod canonical;
pub mod client;
pub mod kinds;
pub mod operator;
pub mod packs;
pub mod privacy;
pub mod worker;

pub use client::{Client, Error, KeyStatus, Report, SubmitStatus, Work};
pub use kinds::{Kind, Postflop, PushFoldHu, PushFoldMw, SpotKind};
