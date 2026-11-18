//! Endless practice with whole random hands, like a hand at a real table: the opponents act
//! by Ringside's preflop model (push/fold equilibrium up to 20 BB), the hero decides and
//! every decision is graded against the same solution. Heads-up flops continue in a spot
//! that a background worker prepared ([`pool`]); hands are steered there, so nobody waits.
//! Every other heads-up flop is solved live, and multiway pots play on without a grade
//! ([`free`], [`assess`]), so every hand ends in a fold or a showdown.

pub mod assess;
pub mod free;
pub mod grade;
pub mod hand;
pub mod model;
pub mod pool;
pub mod scenario;
pub mod table;
pub mod tree;

pub use grade::{Grade, Judgement};
pub use hand::{Deal, Hand, HandView, Lesson, Need, Summary, Verdict};
pub use pool::{LiveStatus, Lookup, NoLookup, Plan, Pool, PoolStatus, Ready, Slot, Status, Urgency};
pub use table::{Config, Mode, TableKey};
