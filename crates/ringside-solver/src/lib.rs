//! Heads-up postflop solving from the flop, turn or river. A small API around the vendored
//! `postflop-solver`: Ringside's range notation, amounts in big blinds, a memory check before
//! anything is allocated, and a solve that reports progress and can be stopped.

pub mod gate;
pub mod library;
mod node;
pub mod saved;
mod solve;
mod spot;
pub mod suits;

use std::fmt;

pub use node::{ActionInfo, ActionKind, ClassInfo, HandInfo, Node, NodeKind, Player, PlayerSummary, Step, Street};
pub use solve::{Job, Progress, Settings, Solution, Summary, solve, spawn};
pub use spot::{Estimate, MAX_SIZES, Spot, StreetSizes, estimate};

#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// Input that does not form a valid spot; the message is written for the user.
    Invalid(String),
    /// The game tree needs more memory than allowed, in bytes.
    TooLarge { needed: u64, limit: u64 },
    Cancelled,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Invalid(m) => f.write_str(m),
            Error::TooLarge { needed, limit } => write!(
                f,
                "Der Spot braucht etwa {} Arbeitsspeicher, erlaubt sind {}. Weniger Einsatzgrößen, engere Ranges oder ein späterer Startpunkt machen ihn kleiner.",
                gib(*needed),
                gib(*limit)
            ),
            Error::Cancelled => f.write_str("Abgebrochen"),
        }
    }
}

impl std::error::Error for Error {}

fn gib(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64).replace('.', ",")
}
