//! The tables a practice session can deal: preflop model or push/fold equilibrium by player
//! count, ante and stack, with the model's heads-up flops. Decoded once per process.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use ringside_engine::multiway;
use ringside_engine::preflop;
use ringside_engine::preflop_table;
use ringside_engine::pushfold_table::{self, Ante};
use serde::{Deserialize, Serialize};

use crate::scenario::{Scenario, scenarios};

/// From here down every decision is shove or fold.
pub const PUSH_FOLD_MAX_BB: f64 = 20.0;
pub const PLAYERS: [usize; 7] = preflop_table::PLAYERS;
/// Stacks of the push/fold mode when they are mixed.
pub const SHORT_STACKS: [f64; 8] = [6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Preflop decisions, then the flop, turn and river where a solve is ready.
    Full,
    /// Shove or fold with up to 20 BB.
    PushFold,
    /// The hand starts on the flop of a prepared spot.
    Postflop,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub mode: Mode,
    pub players: usize,
    pub ante: Ante,
    pub stack_bb: f64,
    /// Every hand on another stack depth of the mode.
    pub mixed_stacks: bool,
    /// Seat in preflop action order; `None` deals the seat at random.
    pub hero_seat: Option<usize>,
}

impl Default for Config {
    fn default() -> Config {
        Config { mode: Mode::Full, players: 6, ante: Ante::BigBlind, stack_bb: 40.0, mixed_stacks: false, hero_seat: None }
    }
}

impl Config {
    /// The stack depths hands are dealt on.
    pub fn stacks(&self) -> Vec<f64> {
        match (self.mode, self.mixed_stacks) {
            (Mode::PushFold, true) => SHORT_STACKS.iter().copied().filter(|s| pushfold_table::stacks(self.ante).contains(s)).collect(),
            (_, true) => preflop_table::STACKS.iter().copied().filter(|&s| s > PUSH_FOLD_MAX_BB).collect(),
            _ => vec![self.stack_bb],
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if !PLAYERS.contains(&self.players) {
            let (last, rest) = PLAYERS.split_last().expect("table sizes");
            let rest: Vec<String> = rest.iter().map(|n| n.to_string()).collect();
            return Err(format!("Gespielt wird mit {} oder {last} Spielern.", rest.join(", ")));
        }
        if self.hero_seat.is_some_and(|s| s >= self.players) {
            return Err("Diesen Platz gibt es am Tisch nicht.".into());
        }
        if self.mixed_stacks {
            return Ok(());
        }
        let s = self.stack_bb;
        let ok = match self.mode {
            Mode::PushFold => s <= PUSH_FOLD_MAX_BB && pushfold_table::stacks(self.ante).contains(&s),
            Mode::Postflop => s > PUSH_FOLD_MAX_BB && preflop_table::STACKS.contains(&s),
            Mode::Full => preflop_table::STACKS.contains(&s) || (s <= PUSH_FOLD_MAX_BB && pushfold_table::stacks(self.ante).contains(&s)),
        };
        if ok { Ok(()) } else { Err(format!("Für {s} BB gibt es in diesem Modus keine Lösung.")) }
    }

    /// The tables of this configuration that can lead to a flop.
    pub fn deep_keys(&self) -> Vec<TableKey> {
        if self.mode == Mode::PushFold {
            return Vec::new();
        }
        self.stacks()
            .into_iter()
            .filter(|&s| s > PUSH_FOLD_MAX_BB)
            .map(|stack_bb| TableKey { players: self.players, ante: self.ante, stack_bb })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableKey {
    pub players: usize,
    pub ante: Ante,
    pub stack_bb: f64,
}

impl TableKey {
    fn bits(&self) -> (usize, Ante, u64) {
        (self.players, self.ante, self.stack_bb.to_bits())
    }
}

pub struct Deep {
    pub key: TableKey,
    pub params: preflop::Params,
    pub solution: preflop::Solution,
    pub scenarios: Vec<Scenario>,
}

pub struct Short {
    pub key: TableKey,
    pub params: multiway::Params,
    pub solution: multiway::Solution,
}

type DeepCache = HashMap<(usize, Ante, u64), Arc<Deep>>;
type ShortCache = HashMap<(usize, Ante, u64), Arc<Short>>;
static DEEP: LazyLock<Mutex<DeepCache>> = LazyLock::new(Default::default);
static SHORT: LazyLock<Mutex<ShortCache>> = LazyLock::new(Default::default);

pub fn deep(key: TableKey) -> Option<Arc<Deep>> {
    if let Some(t) = DEEP.lock().unwrap().get(&key.bits()) {
        return Some(t.clone());
    }
    let params = preflop_table::params(key.ante, key.players, key.stack_bb);
    let solution = preflop_table::lookup(&params)?;
    let scenarios = scenarios(&solution, &params);
    let table = Arc::new(Deep { key, params, solution, scenarios });
    DEEP.lock().unwrap().insert(key.bits(), table.clone());
    Some(table)
}

/// Drops the decoded tables (e.g. when the practice page is left); hands still holding
/// one keep it.
pub fn release() {
    DEEP.lock().unwrap().clear();
    SHORT.lock().unwrap().clear();
}

pub fn short(key: TableKey) -> Option<Arc<Short>> {
    if let Some(t) = SHORT.lock().unwrap().get(&key.bits()) {
        return Some(t.clone());
    }
    let params = pushfold_table::params(key.ante, key.players, key.stack_bb);
    let solution = pushfold_table::lookup(&params)?;
    let table = Arc::new(Short { key, params, solution });
    SHORT.lock().unwrap().insert(key.bits(), table.clone());
    Some(table)
}
