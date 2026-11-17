//! Endless practice with whole random hands, like a hand at a real table: the opponents act
//! by Ringside's preflop model (push/fold equilibrium up to 20 BB), the hero decides and
//! every decision is graded against the same solution. Heads-up flops continue in a spot
//! that a background worker prepared ([`pool`]); hands are steered there, so nobody waits.

pub mod grade;
pub mod hand;
pub mod model;
pub mod pool;
pub mod scenario;
pub mod table;
pub mod tree;

pub use grade::{Grade, Judgement};
pub use hand::{Deal, Hand, HandView, Lesson, Summary, Verdict};
pub use pool::{Lookup, NoLookup, Plan, Pool, PoolStatus, Status};
pub use table::{Config, Mode, TableKey};
