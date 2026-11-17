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

pub use client::{Client, Error, Report, SubmitStatus};
pub use kinds::{Kind, PushFoldHu, PushFoldMw};
