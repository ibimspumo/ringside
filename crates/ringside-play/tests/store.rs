//! The stock takes spots that are solved already before it solves, deals them in a random
//! suit renaming, and never solves a spot another area is solving.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::pushfold_table::Ante;
use ringside_engine::CardSet;
use ringside_play::Config;
use ringside_play::pool::{Line, Lookup, Plan, Pool, Slot, parse_flop};
use ringside_play::scenario::Scenario;
use ringside_play::tree::{Tree, TreeSource};
use ringside_play::TableKey;
use ringside_solver::library::flops::canonical;
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

/// A store with one ready flop for every line, or none; `free` says whether a solve may
/// start.
struct Store {
    ready: bool,
    free: bool,
    slots: AtomicU32,
    /// Whether each lookup knew the preflop line, so it can find the spot by its cloud key.
    lines: Mutex<Vec<bool>>,
    /// The boards looked up.
    boards: Mutex<Vec<String>>,
}

fn store() -> Store {
    Store { ready: true, free: false, slots: AtomicU32::new(0), lines: Mutex::default(), boards: Mutex::default() }
}

/// Like the app's store: a solved flop serves every suit renaming of it.
fn same_class(board: &str, flop: &str) -> bool {
    parse_flop(board).zip(parse_flop(flop)).is_some_and(|(a, b)| canonical(a) == canonical(b))
}

impl Lookup for &'static Store {
    fn load(&self, spot: &Spot, line: Line, _max_pct: f64) -> Option<Box<dyn Tree>> {
        self.lines.lock().unwrap().push(line.is_some());
        let board: String = spot.board.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        self.boards.lock().unwrap().push(board.clone());
        (self.ready && same_class(&board, READY)).then(|| Box::new(Stored) as Box<dyn Tree>)
    }
    fn flops(&self, _table: TableKey, _scenario: &Scenario) -> Vec<String> {
        if self.ready { vec![READY.to_string()] } else { Vec::new() }
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

/// The ready flop comes in many suit renamings, and the cards dealt into it never repeat
/// a board card.
#[test]
fn a_ready_flop_is_dealt_in_a_random_suit_renaming_with_cards_that_fit() {
    let store: &'static Store = Box::leak(Box::new(store()));
    let mut seen = Vec::new();
    for seed in 0..40 {
        let p = pool(store);
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        p.step(&mut rng, &|| false, &|_| {}, &|_| {});
        assert_eq!(p.status().from_store, 1);
        let config = Config { mode: ringside_play::Mode::Postflop, ..Config::default() };
        let deal = ringside_play::hand::deal(&config, seed, Some(&p)).unwrap();
        let flop: String = deal.board[..3].iter().map(|c| c.to_string()).collect();
        assert!(same_class(&flop, READY), "{flop} is a renaming of {READY}");
        let mut cards: Vec<_> = deal.board.to_vec();
        cards.extend(deal.holes.iter().flatten());
        assert_eq!(CardSet::from_cards(&cards).len() as usize, cards.len(), "a card twice in {cards:?}");
        if !seen.contains(&flop) {
            seen.push(flop);
        }
    }
    assert!(seen.len() >= 6, "only {seen:?}");
    assert!(store.slots.load(Ordering::SeqCst) == 0, "nothing solved");
}

/// Nothing ready: a random flop, solved here.
#[test]
fn without_a_ready_flop_the_stock_deals_at_random_and_solves() {
    let store: &'static Store = Box::leak(Box::new(Store { ready: false, ..store() }));
    let p = pool(store);
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    for _ in 0..3 {
        p.step(&mut rng, &|| false, &|_| {}, &|_| {});
    }
    assert_eq!(store.slots.load(Ordering::SeqCst), 3, "every pick goes on to solve");
    let boards = store.boards.lock().unwrap();
    assert_eq!(boards.len(), 3);
    assert!(boards.iter().any(|b| !same_class(b, READY)), "{boards:?}");
}

/// While PokerStars runs the stock neither looks anything up nor solves.
#[test]
fn with_pokerstars_running_the_stock_does_nothing() {
    let store: &'static Store = Box::leak(Box::new(store()));
    let p = pool(store);
    let mut rng = ChaCha8Rng::seed_from_u64(4);
    p.step(&mut rng, &|| true, &|_| {}, &|_| {});
    assert_eq!((store.boards.lock().unwrap().len(), store.slots.load(Ordering::SeqCst)), (0, 0));
    assert_eq!(p.status().ready, 0);
}
