//! The stock takes spots that are solved already before it solves, and never solves a
//! spot another area is solving.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::pushfold_table::Ante;
use ringside_play::pool::{Line, Lookup, Plan, Pool, Slot};
use ringside_play::scenario::Scenario;
use ringside_play::tree::{Tree, TreeSource};
use ringside_play::TableKey;
use ringside_solver::{Error, Node, Spot, Step};

const READY: &str = "Jd9h6h";

struct Stored;

impl Tree for Stored {
    fn node(&mut self, _path: &[Step]) -> Result<Node, Error> {
        Err(Error::Invalid("not read in this test".into()))
    }
    fn complete(&self) -> bool {
        false
    }
    fn source(&self) -> TreeSource {
        TreeSource::Library
    }
    fn exploitability_pct(&self) -> f64 {
        0.4
    }
}

/// A store with one ready flop for every line; `free` says whether a solve may start.
struct Store {
    free: bool,
    slots: AtomicU32,
    /// Whether each lookup knew the preflop line, so it can find the spot by its cloud key.
    lines: Mutex<Vec<bool>>,
}

fn store() -> Store {
    Store { free: false, slots: AtomicU32::new(0), lines: Mutex::default() }
}

impl Lookup for &'static Store {
    fn load(&self, spot: &Spot, line: Line, _max_pct: f64) -> Option<Box<dyn Tree>> {
        self.lines.lock().unwrap().push(line.is_some());
        let board: String = spot.board.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        (board == READY).then(|| Box::new(Stored) as Box<dyn Tree>)
    }
    fn flops(&self, _table: TableKey, _scenario: &Scenario) -> Vec<String> {
        vec![READY.to_string()]
    }
    fn slot(&self, _spot: &Spot, _line: Line, _urgent: bool) -> Option<Box<dyn Slot>> {
        self.slots.fetch_add(1, Ordering::SeqCst);
        if self.free { Some(Box::new(ringside_play::pool::Free)) } else { None }
    }
}

fn pool(store: &'static Store) -> Pool {
    let pool = Pool::new(Box::new(store), 1, 4 << 30);
    let table = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    pool.set_plan(Some(Plan { tables: vec![table], hero_seat: None }));
    pool
}

#[test]
fn the_stock_takes_a_ready_spot_first() {
    let store: &'static Store = Box::leak(Box::new(store()));
    let p = pool(store);
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    p.step(&mut rng, &|| false, &|_| {}, &|_| {});
    let s = p.status();
    assert_eq!((s.ready, s.from_store, s.solved), (1, 1, 0), "{s:?}");
    assert_eq!(store.slots.load(Ordering::SeqCst), 0, "nothing to solve");
    assert_eq!(*store.lines.lock().unwrap(), [true], "the stock names the line");
}

#[test]
fn a_spot_another_area_solves_is_not_solved_again() {
    let store: &'static Store = Box::leak(Box::new(store()));
    let p = pool(store);
    // The ready flop is in the stock now; the next pick has no ready flop left.
    let mut rng = ChaCha8Rng::seed_from_u64(2);
    p.step(&mut rng, &|| false, &|_| {}, &|_| {});
    p.step(&mut rng, &|| false, &|_| {}, &|_| {});
    let s = p.status();
    assert_eq!((s.ready, s.solved), (1, 0), "the slot was refused, nothing solved: {s:?}");
    assert_eq!(store.slots.load(Ordering::SeqCst), 1);
}
