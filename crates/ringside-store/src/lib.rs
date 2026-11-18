//! The spot store's rules, without files or network: which key a solved spot has
//! ([`key`]), in which order every area looks for it ([`order`]) and how two areas that
//! need the same spot at once share one solve ([`flight`]).
//!
//! The app (`src-tauri/src/store.rs`) holds the files and the index and is the only place
//! that reads and writes them; the hand analysis, practice, the solver page and the
//! library all go through it.

pub mod flight;
pub mod key;
pub mod order;

pub use flight::{Claim, InFlight};
pub use key::{Cloud, Keyed, key_of};
pub use order::{Origin, Policy, Step, resolve, steps};
