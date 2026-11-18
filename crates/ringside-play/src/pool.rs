//! The stock of postflop spots hands are steered into, so a hand never waits for a solve.
//!
//! A worker keeps up to [`TARGET`] spots ready for the tables of the current plan. For each
//! it picks one of the model's heads-up flops (weighted by how often the model plays there,
//! and with the hero's seat in it when the seat is fixed) and a flop, one that is solved
//! already when the [`Lookup`] knows one ([`Lookup::flops`]: the app's spot store, with
//! spots of the library and of other players), dealt in a random suit renaming so the
//! same solved flop does not come back in the same suits. It first asks the [`Lookup`] for it:
//! stored spots open at once but know only the first street and the first decision on
//! the turn. Otherwise it takes a [`Slot`] (the right to solve that spot now, see
//! [`Lookup::slot`]) and solves with the solver page's
//! default precision ([`TARGET_PCT`], [`MAX_ITERATIONS`]) on few threads and 16-bit
//! storage, which halves the memory, and hands the solve to [`Lookup::save`], so the
//! solver page and every other area find the same solution for the same spot. A spot serves up to
//! [`MAX_USES`] hands with different cards, then makes room for a new one.
//!
//! A hand that reaches a spot nobody prepared (another line, a stored spot that ends)
//! asks for it with [`Pool::request`]: the worker solves it before anything else, faster
//! and less exact ([`LIVE_PCT`]), looks for a stored solution first and saves the solve
//! the same way. While the hero thinks, the spot the hand most likely needs next is
//! solved ahead. Live spots count against the same memory budget; unused stock spots
//! make room for them.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::{Card, CardSet};
use ringside_solver::library::flops::canonical;
use ringside_solver::suits;
use ringside_solver::{Error, Settings, Solution, Spot, estimate, solve};
use serde::Serialize;

use crate::hand::Need;
use crate::scenario::Scenario;
use crate::table::{Deep, TableKey, deep};
use crate::tree::{Tree, TreeSource};

pub const TARGET: usize = 3;
/// Exploitability the practice solves aim for, in percent of the pot: the solver page's
/// default (`src/lib/solver/store.svelte.ts`), so both grade a spot by the same solution.
pub const TARGET_PCT: f64 = 0.5;
pub const MAX_ITERATIONS: u32 = 1000;
pub const MAX_USES: u32 = 3;
/// Seconds a solve is assumed to take before one was measured.
const FIRST_GUESS_SECS: f64 = 30.0;
/// Exploitability of a spot solved while a hand waits, in percent of the pot: fast enough
/// to keep playing, and the page says how exact it is.
pub const LIVE_PCT: f64 = 2.0;
pub const LIVE_MAX_ITERATIONS: u32 = 400;
const LIVE_FIRST_GUESS_SECS: f64 = 10.0;

/// The preflop line a spot's ranges come from: the table and the non-fold actions.
pub type Line<'a> = Option<(TableKey, &'a [ringside_engine::preflop::Step])>;

/// The right to solve one spot now: the app hands it out one solve at a time, the most
/// urgent first, and never for a spot another area is solving.
pub trait Slot: Send {
    /// Waits up to `timeout` for the turn; true once the solve may start.
    fn hold(&self, timeout: Duration) -> bool;
    /// A more urgent solve waits: stop and try again later.
    fn should_yield(&self) -> bool;
}

/// A slot that is always free, for a pool on its own.
pub struct Free;

impl Slot for Free {
    fn hold(&self, _timeout: Duration) -> bool {
        true
    }

    fn should_yield(&self) -> bool {
        false
    }
}

/// Stored solutions the worker looks at before it solves.
pub trait Lookup: Send + Sync {
    /// A stored solution of exactly this spot, at most `max_pct` exploitable.
    fn load(&self, spot: &Spot, line: Line, max_pct: f64) -> Option<Box<dyn Tree>>;
    /// Flops that are solved already for the scenario's line at `table`, one per suit
    /// class: the pool deals any suit renaming of them and [`Lookup::load`] finds it.
    fn flops(&self, _table: TableKey, _scenario: &Scenario) -> Vec<String> {
        Vec::new()
    }
    /// The right to solve `spot` now; `None` while another area solves the same spot.
    /// `urgent`: a hand waits for it.
    fn slot(&self, _spot: &Spot, _line: Line, _urgent: bool) -> Option<Box<dyn Slot>> {
        Some(Box::new(Free))
    }
    /// Keeps a fresh solve where [`Lookup::load`] and the solver page find it; `seats` are
    /// OOP and IP, `line` the model's line for a stock spot (solved to [`TARGET_PCT`], so
    /// it may be shared), `None` for a live spot (too rough to pass on).
    fn save(&self, _table: TableKey, _seats: [usize; 2], _line: Option<&[ringside_engine::preflop::Step]>, _solution: &mut Solution, _settings: &Settings) {}
}

pub struct NoLookup;

impl Lookup for NoLookup {
    fn load(&self, _spot: &Spot, _line: Line, _max_pct: f64) -> Option<Box<dyn Tree>> {
        None
    }
}

/// What the worker prepares for.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub tables: Vec<TableKey>,
    pub hero_seat: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Waiting,
    Running,
    PausedClient,
    Done,
    Cancelled,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PoolStatus {
    pub status: Status,
    /// What is going on, in German, e.g. „Bereite 3 Spots vor …“.
    pub title: String,
    pub ready: usize,
    pub target: usize,
    /// 0–1 of the stock.
    pub fraction: f64,
    /// Until the stock is full.
    pub eta_secs: Option<f64>,
    /// Until the spot being solved now is ready.
    pub next_secs: Option<f64>,
    /// Average solve time measured in this session.
    pub solve_secs: Option<f64>,
    pub solved: u32,
    /// Stock spots that were solved already (own, library, other players): no wait.
    pub from_store: u32,
    pub error: Option<String>,
}

pub struct Entry {
    pub id: u64,
    pub table: TableKey,
    pub scenario: Scenario,
    pub flop: [Card; 3],
    pub source: TreeSource,
    pub complete: bool,
    pub exploitability_pct: f64,
    pub memory_bytes: u64,
    pub tree: Arc<Mutex<Box<dyn Tree>>>,
    uses: u32,
}

impl Entry {
    pub fn flop_text(&self) -> String {
        self.flop.iter().map(|c| c.to_string()).collect()
    }
}

/// A solved spot a hand plays in.
#[derive(Clone)]
pub struct Ready {
    pub id: u64,
    pub source: TreeSource,
    pub complete: bool,
    pub exploitability_pct: f64,
    pub tree: Arc<Mutex<Box<dyn Tree>>>,
}

/// What a hand takes from the pool.
#[derive(Clone)]
pub struct Taken {
    pub table: TableKey,
    pub scenario: Scenario,
    pub flop: [Card; 3],
    pub ready: Ready,
}

/// How soon a hand needs a live spot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    /// The hand waits for it.
    Now,
    /// The hand will most likely need it after the hero's next move.
    Ahead,
}

type Held = Weak<Mutex<Box<dyn Tree>>>;

enum LiveState {
    Queued,
    Running,
    Ready(Ready, u64),
    Failed(String),
    Cancelled,
}

struct LiveJob {
    need: Need,
    urgency: Urgency,
    state: LiveState,
}

/// A live spot's solve, for the table and the task center.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LiveStatus {
    pub urgency: Urgency,
    /// `waiting` (queued), `running`, `paused_client`, `done`, `cancelled` or `error`.
    pub status: Status,
    /// 0–1.
    pub fraction: f64,
    pub secs_left: Option<f64>,
    /// The precision it solves to, percent of the pot.
    pub target_pct: f64,
    pub error: Option<String>,
}

struct Inner {
    plan: Option<Plan>,
    ready: VecDeque<Entry>,
    /// Plan generation the running solve belongs to.
    generation: u64,
    solving: Option<(Instant, f64)>,
    progress: f64,
    times: Vec<f64>,
    status: Status,
    error: Option<String>,
    live: Vec<LiveJob>,
    /// Changes whenever the running live solve is no longer wanted.
    live_generation: u64,
    live_solving: Option<(Instant, f64)>,
    live_progress: f64,
    live_times: Vec<f64>,
    /// Live trees handed to hands, with their size, while a hand holds them.
    held: Vec<(Held, u64)>,
    from_store: u32,
}

impl Inner {
    /// Memory of everything solved that is still held: stock, live spots in hands, live
    /// spots ready to be taken.
    fn used(&mut self) -> u64 {
        self.held.retain(|(w, _)| w.strong_count() > 0);
        let stock: u64 = self.ready.iter().map(|e| e.memory_bytes).sum();
        let held: u64 = self.held.iter().map(|(_, b)| b).sum();
        let live: u64 = self.live.iter().map(|j| if let LiveState::Ready(_, b) = j.state { b } else { 0 }).sum();
        stock + held + live
    }

    fn live_waiting(&self) -> bool {
        self.live.iter().any(|j| matches!(j.state, LiveState::Queued))
    }
}

pub struct Pool {
    inner: Mutex<Inner>,
    wake: Condvar,
    cancel: AtomicBool,
    next_id: AtomicU64,
    lookup: Box<dyn Lookup>,
    threads: usize,
    /// Threads of a live solve, which a hand waits for.
    live_threads: usize,
    /// For all ready spots together (live ones included), and the largest single one.
    budget: u64,
}

impl Pool {
    pub fn new(lookup: Box<dyn Lookup>, threads: usize, budget: u64) -> Pool {
        Pool {
            inner: Mutex::new(Inner {
                plan: None,
                ready: VecDeque::new(),
                generation: 0,
                solving: None,
                progress: 0.0,
                times: Vec::new(),
                status: Status::Waiting,
                error: None,
                live: Vec::new(),
                live_generation: 0,
                live_solving: None,
                live_progress: 0.0,
                live_times: Vec::new(),
                held: Vec::new(),
                from_store: 0,
            }),
            wake: Condvar::new(),
            cancel: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            lookup,
            threads: threads.max(1),
            live_threads: threads.max(1),
            budget,
        }
    }

    pub fn with_live_threads(mut self, threads: usize) -> Pool {
        self.live_threads = threads.max(1);
        self
    }

    // -----------------------------------------------------------------------------
    // Live spots.

    /// Asks for `need`. `Now` drops whatever else was asked for: the hand has moved on.
    pub fn request(&self, need: Need, urgency: Urgency) {
        let mut inner = self.inner.lock().unwrap();
        let running = |j: &LiveJob| matches!(j.state, LiveState::Running);
        let mut stopped = false;
        if let Some(j) = inner.live.iter_mut().find(|j| j.need == need) {
            if urgency == Urgency::Now {
                j.urgency = Urgency::Now;
            }
        } else {
            stopped |= inner.live.iter().any(|j| j.urgency == urgency && running(j));
            inner.live.retain(|j| j.urgency != urgency);
            inner.live.push(LiveJob { need: need.clone(), urgency, state: LiveState::Queued });
        }
        if urgency == Urgency::Now {
            stopped |= inner.live.iter().any(|j| j.need != need && running(j));
            inner.live.retain(|j| j.need == need);
        }
        if stopped {
            inner.live_generation += 1;
        }
        self.wake.notify_all();
    }

    /// The solve of `need` once it is done: the spot, or why there is none.
    pub fn take_live(&self, need: &Need) -> Option<Result<Ready, String>> {
        let mut inner = self.inner.lock().unwrap();
        let i = inner.live.iter().position(|j| &j.need == need)?;
        if matches!(inner.live[i].state, LiveState::Queued | LiveState::Running) {
            return None;
        }
        match inner.live.remove(i).state {
            LiveState::Ready(r, bytes) => {
                inner.held.push((Arc::downgrade(&r.tree), bytes));
                Some(Ok(r))
            }
            LiveState::Failed(e) => Some(Err(e)),
            _ => Some(Err(Error::Cancelled.to_string())),
        }
    }

    /// Stops the solve the hand waits for (the hand then plays on without it) and drops
    /// the one solved ahead.
    pub fn cancel_live(&self) {
        let mut inner = self.inner.lock().unwrap();
        let open = |j: &LiveJob| matches!(j.state, LiveState::Queued | LiveState::Running);
        let stopped = inner.live.iter().any(|j| matches!(j.state, LiveState::Running));
        inner.live.retain(|j| j.urgency == Urgency::Now || !open(j));
        for j in inner.live.iter_mut().filter(|j| open(j)) {
            j.state = LiveState::Cancelled;
        }
        if stopped {
            inner.live_generation += 1;
        }
        inner.live_solving = None;
        self.wake.notify_all();
    }

    /// Drops every live spot asked for (a new hand starts).
    pub fn forget_live(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.live.iter().any(|j| matches!(j.state, LiveState::Running)) {
            inner.live_generation += 1;
        }
        inner.live.clear();
        inner.live_solving = None;
        self.wake.notify_all();
    }

    /// The spot the hand waits for, if any.
    pub fn live_status(&self) -> Option<LiveStatus> {
        let inner = self.inner.lock().unwrap();
        inner.live.iter().find(|j| j.urgency == Urgency::Now).map(|j| live_status_of(&inner, j))
    }

    /// Prepares for `plan` from now on; ready spots of other tables are dropped. Restarts
    /// a cancelled worker.
    pub fn set_plan(&self, plan: Option<Plan>) {
        let mut inner = self.inner.lock().unwrap();
        self.cancel.store(false, Ordering::Relaxed);
        if inner.plan != plan {
            if let Some(p) = &plan {
                inner.ready.retain(|e| p.tables.contains(&e.table));
            } else {
                inner.ready.clear();
            }
            inner.plan = plan;
            inner.generation += 1;
        }
        if inner.status == Status::Cancelled {
            inner.status = Status::Waiting;
        }
        self.wake.notify_all();
    }

    /// Stops the running solve and keeps the worker idle until the next plan.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let mut inner = self.inner.lock().unwrap();
        inner.status = Status::Cancelled;
        inner.solving = None;
        self.wake.notify_all();
    }

    pub fn status(&self) -> PoolStatus {
        let inner = self.inner.lock().unwrap();
        status_of(&inner)
    }

    /// Frees every ready spot (e.g. when the practice page is left for long).
    pub fn clear(&self) {
        self.inner.lock().unwrap().ready.clear();
        self.wake.notify_all();
    }

    /// Memory of the solved spots held now, live ones included.
    pub fn memory_bytes(&self) -> u64 {
        self.inner.lock().unwrap().used()
    }

    /// A ready spot of `table`s with `hero_seat` in it (if fixed), complete trees and
    /// fresh spots first.
    pub fn take(&self, tables: &[TableKey], hero_seat: Option<usize>, rng: &mut impl Rng) -> Option<Taken> {
        let mut inner = self.inner.lock().unwrap();
        let fits = |e: &Entry| tables.contains(&e.table) && hero_seat.is_none_or(|s| e.scenario.seats().contains(&s));
        let best = inner.ready.iter().filter(|e| fits(e)).map(|e| (!e.complete, e.uses)).min()?;
        let candidates: Vec<usize> =
            (0..inner.ready.len()).filter(|&i| fits(&inner.ready[i]) && (!inner.ready[i].complete, inner.ready[i].uses) == best).collect();
        let i = candidates[rng.random_range(0..candidates.len())];
        let e = &mut inner.ready[i];
        e.uses += 1;
        let taken = Taken {
            table: e.table,
            scenario: e.scenario.clone(),
            flop: e.flop,
            ready: Ready { id: e.id, source: e.source, complete: e.complete, exploitability_pct: e.exploitability_pct, tree: e.tree.clone() },
        };
        if e.uses >= MAX_USES {
            inner.ready.remove(i);
            self.wake.notify_all();
        }
        Some(taken)
    }

    /// Adds a spot prepared elsewhere.
    pub fn insert(&self, table: TableKey, scenario: Scenario, flop: [Card; 3], tree: Box<dyn Tree>) {
        let generation = self.inner.lock().unwrap().generation;
        self.add(generation, table, scenario, flop, tree, 0, &|_| {});
    }

    /// Removes a spot so its tree can move to the solver page.
    pub fn release(&self, id: u64) {
        self.inner.lock().unwrap().ready.retain(|e| e.id != id);
        self.wake.notify_all();
    }

    /// Runs the worker until `quit` answers true. `paused` (PokerStars runs) stops a solve
    /// and waits; `report` hears every change of the stock's status, `live` of a live spot's.
    pub fn run(&self, quit: impl Fn() -> bool, paused: impl Fn() -> bool, report: impl Fn(&PoolStatus), live: impl Fn(&LiveStatus)) {
        let mut rng = ChaCha8Rng::from_os_rng_or_time();
        while !quit() {
            self.step(&mut rng, &paused, &report, &live);
        }
    }

    /// One round of the worker: solves a live spot, adds at most one spot to the stock or
    /// waits a little.
    pub fn step(&self, rng: &mut ChaCha8Rng, paused: &dyn Fn() -> bool, report: &dyn Fn(&PoolStatus), live: &dyn Fn(&LiveStatus)) {
        let set = |status: Status, inner: &mut Inner| {
            if inner.status != status {
                inner.status = status;
                report(&status_of(inner));
            }
        };
        let mut inner = self.inner.lock().unwrap();
        if paused() {
            set(Status::PausedClient, &mut inner);
            if let Some(j) = inner.live.iter().find(|j| j.urgency == Urgency::Now && matches!(j.state, LiveState::Queued)) {
                let mut s = live_status_of(&inner, j);
                s.status = Status::PausedClient;
                live(&s);
            }
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        }
        if inner.live_waiting() {
            drop(inner);
            return self.solve_live(paused, live);
        }
        if self.cancel.load(Ordering::Relaxed) {
            set(Status::Cancelled, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        }
        let Some(plan) = inner.plan.clone() else {
            set(Status::Waiting, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        };
        let used = inner.used();
        let complete = inner.ready.iter().filter(|e| e.complete).count();
        if complete >= TARGET || inner.ready.len() >= 2 * TARGET {
            set(Status::Done, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        }
        let generation = inner.generation;
        set(Status::Running, &mut inner);
        drop(inner);

        let Some((table, scenario, flop)) = self.pick(&plan, rng) else {
            let mut inner = self.inner.lock().unwrap();
            inner.error = Some("Für diese Auswahl kennt das Preflop-Modell keinen Heads-up-Flop.".into());
            set(Status::Error, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(2));
            return;
        };
        let spot = scenario.spot(&flop.iter().map(|c| c.to_string()).collect::<String>());
        let line = Some((table, scenario.line.as_slice()));
        if let Some(tree) = self.lookup.load(&spot, line, TARGET_PCT) {
            self.inner.lock().unwrap().from_store += 1;
            self.add(generation, table, scenario, flop, tree, 0, report);
            return;
        }
        // Another area solves this spot: the next round finds it stored or picks another.
        let Some(slot) = self.lookup.slot(&spot, line, false) else { return };
        if !slot.hold(Duration::from_secs(1)) {
            return;
        }
        let fits = |bytes: u64| used + bytes <= self.budget;
        let full = match estimate(&spot, u64::MAX, self.threads) {
            Ok(e) => e.memory_bytes,
            Err(e) => return self.fail(e, report),
        };
        // Forces 16-bit storage: the solver compresses whenever the full size is over the limit.
        let limit = full.saturating_sub(1);
        if !fits(full / 2) {
            let mut inner = self.inner.lock().unwrap();
            if inner.ready.is_empty() {
                // Nothing to free: this spot is too large for the budget, try another one.
                return;
            }
            set(Status::Done, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(2));
            return;
        }
        let guess = self.average().unwrap_or(FIRST_GUESS_SECS);
        {
            let mut inner = self.inner.lock().unwrap();
            inner.solving = Some((Instant::now(), guess));
            inner.progress = 0.0;
            report(&status_of(&inner));
        }
        let start = Instant::now();
        let mut first: Option<f64> = None;
        let mut last_report = Instant::now();
        let settings = Settings { target_pct: TARGET_PCT, max_iterations: MAX_ITERATIONS, threads: self.threads };
        let result = solve(
            &spot,
            &settings,
            limit,
            |p| {
                let f = progress(&mut first, p, TARGET_PCT, MAX_ITERATIONS);
                let mut inner = self.inner.lock().unwrap();
                inner.progress = f;
                if f > 0.05 {
                    let elapsed = start.elapsed().as_secs_f64();
                    inner.solving = Some((start, elapsed / f));
                }
                if last_report.elapsed() >= Duration::from_millis(400) {
                    last_report = Instant::now();
                    report(&status_of(&inner));
                }
            },
            || {
                let inner = self.inner.lock().unwrap();
                self.cancel.load(Ordering::Relaxed)
                    || paused()
                    || inner.generation != generation
                    || inner.live_waiting()
                    || slot.should_yield()
            },
        );
        let secs = start.elapsed().as_secs_f64();
        match result {
            Ok(mut solution) => {
                self.lookup.save(table, scenario.seats(), Some(&scenario.line), &mut solution, &settings);
                drop(slot);
                let memory = solution.summary.memory_bytes;
                self.inner.lock().unwrap().times.push(secs);
                self.add(generation, table, scenario, flop, Box::new(solution), memory, report);
            }
            Err(Error::Cancelled) => {
                let mut inner = self.inner.lock().unwrap();
                inner.solving = None;
                report(&status_of(&inner));
            }
            Err(e) => self.fail(e, report),
        }
    }

    /// Solves the live spot asked for most urgently: from a stored solution if one fits,
    /// else with the live precision.
    fn solve_live(&self, paused: &dyn Fn() -> bool, live: &dyn Fn(&LiveStatus)) {
        let (need, generation) = {
            let mut inner = self.inner.lock().unwrap();
            let pick = |u: Urgency| inner.live.iter().position(|j| j.urgency == u && matches!(j.state, LiveState::Queued));
            let Some(i) = pick(Urgency::Now).or_else(|| pick(Urgency::Ahead)) else { return };
            inner.live[i].state = LiveState::Running;
            let guess = live_average(&inner.live_times).unwrap_or(LIVE_FIRST_GUESS_SECS);
            inner.live_solving = Some((Instant::now(), guess));
            inner.live_progress = 0.0;
            live(&live_status_of(&inner, &inner.live[i]));
            (inner.live[i].need.clone(), inner.live_generation)
        };
        let finish = |state: LiveState| {
            let mut inner = self.inner.lock().unwrap();
            inner.live_solving = None;
            if let Some(i) = inner.live.iter().position(|j| j.need == need && matches!(j.state, LiveState::Running)) {
                inner.live[i].state = state;
                live(&live_status_of(&inner, &inner.live[i]));
            }
        };
        let requeue = || {
            let mut inner = self.inner.lock().unwrap();
            inner.live_solving = None;
            if let Some(j) = inner.live.iter_mut().find(|j| j.need == need && matches!(j.state, LiveState::Running)) {
                j.state = LiveState::Queued;
            }
            if let Some(j) = inner.live.iter().find(|j| j.need == need) {
                live(&live_status_of(&inner, j));
            }
        };
        if !need.complete
            && let Some(tree) = self.lookup.load(&need.spot, None, LIVE_PCT)
        {
            let ready = Ready {
                id: self.next_id.fetch_add(1, Ordering::Relaxed),
                source: tree.source(),
                complete: tree.complete(),
                exploitability_pct: tree.exploitability_pct(),
                tree: Arc::new(Mutex::new(tree)),
            };
            return finish(LiveState::Ready(ready, 0));
        }
        let urgent = self.inner.lock().unwrap().live.iter().any(|j| j.need == need && j.urgency == Urgency::Now);
        // Solved elsewhere right now, or a more urgent solve runs: asked again next round.
        let Some(slot) = self.lookup.slot(&need.spot, None, urgent) else {
            requeue();
            let inner = self.inner.lock().unwrap();
            let _ = self.wake.wait_timeout(inner, Duration::from_millis(300));
            return;
        };
        if !slot.hold(Duration::from_secs(1)) {
            return requeue();
        }
        let full = match estimate(&need.spot, u64::MAX, self.live_threads) {
            Ok(e) => e.memory_bytes,
            Err(e) => return finish(LiveState::Failed(e.to_string())),
        };
        if !self.make_room(full / 2, &need) {
            let needed = Error::TooLarge { needed: full / 2, limit: self.budget };
            return finish(LiveState::Failed(needed.to_string()));
        }
        let settings = Settings { target_pct: LIVE_PCT, max_iterations: LIVE_MAX_ITERATIONS, threads: self.live_threads };
        let start = Instant::now();
        let mut first: Option<f64> = None;
        let mut last_report = Instant::now();
        let result = solve(
            &need.spot,
            &settings,
            full.saturating_sub(1),
            |p| {
                let f = progress(&mut first, p, LIVE_PCT, LIVE_MAX_ITERATIONS);
                let mut inner = self.inner.lock().unwrap();
                inner.live_progress = f;
                if f > 0.05 {
                    inner.live_solving = Some((start, start.elapsed().as_secs_f64() / f));
                }
                if last_report.elapsed() >= Duration::from_millis(250)
                    && let Some(j) = inner.live.iter().find(|j| j.need == need)
                {
                    last_report = Instant::now();
                    live(&live_status_of(&inner, j));
                }
            },
            || paused() || self.inner.lock().unwrap().live_generation != generation || slot.should_yield(),
        );
        match result {
            Ok(mut solution) => {
                self.lookup.save(need.table, need.seats, None, &mut solution, &settings);
                drop(slot);
                self.inner.lock().unwrap().live_times.push(start.elapsed().as_secs_f64());
                let bytes = solution.summary.memory_bytes;
                let ready = Ready {
                    id: self.next_id.fetch_add(1, Ordering::Relaxed),
                    source: TreeSource::Live,
                    complete: true,
                    exploitability_pct: solution.summary.exploitability_pct,
                    tree: Arc::new(Mutex::new(Box::new(solution) as Box<dyn Tree>)),
                };
                finish(LiveState::Ready(ready, bytes));
            }
            // Stopped for PokerStars or a more urgent solve: it starts again afterwards.
            Err(Error::Cancelled) => requeue(),
            Err(e) => finish(LiveState::Failed(e.to_string())),
        }
    }

    /// Frees unused stock spots, oldest first, until `bytes` more fit the budget.
    fn make_room(&self, bytes: u64, need: &Need) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let urgent = inner.live.iter().any(|j| &j.need == need && j.urgency == Urgency::Now);
        while inner.used() + bytes > self.budget {
            // Only a hand that waits pushes stock spots out; one asked for ahead does not.
            let free = urgent.then(|| inner.ready.iter().position(|e| Arc::strong_count(&e.tree) == 1)).flatten();
            match free {
                Some(i) => {
                    inner.ready.remove(i);
                }
                None => return false,
            }
        }
        true
    }

    fn average(&self) -> Option<f64> {
        let inner = self.inner.lock().unwrap();
        (!inner.times.is_empty()).then(|| inner.times.iter().sum::<f64>() / inner.times.len() as f64)
    }

    fn fail(&self, e: Error, report: &dyn Fn(&PoolStatus)) {
        let mut inner = self.inner.lock().unwrap();
        inner.solving = None;
        inner.error = Some(e.to_string());
        inner.status = Status::Error;
        report(&status_of(&inner));
        let _ = self.wake.wait_timeout(inner, Duration::from_secs(5));
    }

    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        generation: u64,
        table: TableKey,
        scenario: Scenario,
        flop: [Card; 3],
        tree: Box<dyn Tree>,
        memory_bytes: u64,
        report: &dyn Fn(&PoolStatus),
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner.solving = None;
        inner.progress = 0.0;
        if inner.generation != generation {
            return;
        }
        inner.error = None;
        inner.ready.push_back(Entry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            table,
            scenario,
            flop,
            source: tree.source(),
            complete: tree.complete(),
            exploitability_pct: tree.exploitability_pct(),
            memory_bytes,
            tree: Arc::new(Mutex::new(tree)),
            uses: 0,
        });
        report(&status_of(&inner));
    }

    /// A table, one of its heads-up flops and a flop: stored flops first, else random.
    fn pick(&self, plan: &Plan, rng: &mut ChaCha8Rng) -> Option<(TableKey, Scenario, [Card; 3])> {
        let tables: Vec<Arc<Deep>> = plan.tables.iter().filter_map(|&k| deep(k)).collect();
        let options: Vec<(&Arc<Deep>, &Scenario)> = tables
            .iter()
            .flat_map(|t| t.scenarios.iter().map(move |s| (t, s)))
            .filter(|(_, s)| plan.hero_seat.is_none_or(|h| s.seats().contains(&h)))
            .collect();
        if options.is_empty() {
            return None;
        }
        let total: f64 = options.iter().map(|(_, s)| s.reach).sum();
        let mut r = rng.random::<f64>() * total;
        let (table, scenario) = *options.iter().find(|(_, s)| {
            r -= s.reach;
            r < 0.0
        })?;
        let flop = self.stored_flop(table.key, scenario, rng).unwrap_or_else(|| random_flop(rng));
        Some((table.key, scenario.clone(), flop))
    }

    /// A solved flop of the line in one of its suit renamings, each renaming of each flop
    /// alike, so a flop class comes up as often as it is dealt; none whose class the stock
    /// holds already.
    fn stored_flop(&self, table: TableKey, scenario: &Scenario, rng: &mut ChaCha8Rng) -> Option<[Card; 3]> {
        let taken: Vec<[Card; 3]> = self.inner.lock().unwrap().ready.iter().map(|e| canonical(e.flop)).collect();
        let flops: Vec<[Card; 3]> = self
            .lookup
            .flops(table, scenario)
            .iter()
            .filter_map(|f| parse_flop(f))
            .filter(|f| !taken.contains(&canonical(*f)))
            .flat_map(renamings)
            .collect();
        (!flops.is_empty()).then(|| flops[rng.random_range(0..flops.len())])
    }
}

/// How far a solve is, 0–1. Exploitability falls about geometrically, so its logarithm
/// measures progress.
fn progress(first: &mut Option<f64>, p: &ringside_solver::Progress, target_pct: f64, max_iterations: u32) -> f64 {
    let e0 = *first.get_or_insert(p.exploitability_pct.max(target_pct * 1.01));
    let f = ((e0 / p.exploitability_pct.max(1e-9)).ln() / (e0 / target_pct).ln()).clamp(0.0, 1.0);
    f.max(p.iteration as f64 / max_iterations as f64)
}

fn live_average(times: &[f64]) -> Option<f64> {
    (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64)
}

fn live_status_of(inner: &Inner, job: &LiveJob) -> LiveStatus {
    let (status, error) = match &job.state {
        LiveState::Queued => (Status::Waiting, None),
        LiveState::Running => (Status::Running, None),
        LiveState::Ready(..) => (Status::Done, None),
        LiveState::Failed(e) => (Status::Error, Some(e.clone())),
        LiveState::Cancelled => (Status::Cancelled, None),
    };
    let running = matches!(job.state, LiveState::Running);
    let secs_left = match (running, inner.live_solving) {
        (true, Some((start, total))) => Some((total - start.elapsed().as_secs_f64()).max(1.0)),
        (false, _) if matches!(job.state, LiveState::Queued) => live_average(&inner.live_times),
        _ => None,
    };
    LiveStatus {
        urgency: job.urgency,
        status,
        fraction: match job.state {
            LiveState::Running => inner.live_progress,
            LiveState::Ready(..) => 1.0,
            _ => 0.0,
        },
        secs_left,
        target_pct: LIVE_PCT,
        error,
    }
}

fn status_of(inner: &Inner) -> PoolStatus {
    let complete = inner.ready.iter().filter(|e| e.complete).count().min(TARGET);
    let ready = inner.ready.len();
    let solving = inner.solving.map(|(start, total)| (total - start.elapsed().as_secs_f64()).max(1.0));
    let avg = (!inner.times.is_empty()).then(|| inner.times.iter().sum::<f64>() / inner.times.len() as f64);
    let missing = TARGET - complete;
    let fraction = ((complete as f64 + if inner.solving.is_some() { inner.progress } else { 0.0 }) / TARGET as f64).min(1.0);
    let eta = match (missing, solving) {
        (0, _) => None,
        (m, Some(next)) => Some(next + (m - 1) as f64 * avg.unwrap_or(FIRST_GUESS_SECS)),
        (m, None) => Some(m as f64 * avg.unwrap_or(FIRST_GUESS_SECS)),
    };
    let title = match inner.status {
        Status::Waiting => "Vorrat wartet auf eine Auswahl".to_string(),
        Status::Running if missing == 1 => "Bereite 1 Spot vor …".to_string(),
        Status::Running => format!("Bereite {missing} Spots vor …"),
        Status::PausedClient => "Vorrat pausiert, solange PokerStars läuft".to_string(),
        Status::Done => format!("{ready} Spots bereit"),
        Status::Cancelled => "Vorbereitung abgebrochen".to_string(),
        Status::Error => "Vorbereitung gestoppt".to_string(),
    };
    PoolStatus {
        status: inner.status,
        title,
        ready,
        target: TARGET,
        fraction,
        eta_secs: if inner.status == Status::Running { eta } else { None },
        next_secs: solving,
        solve_secs: avg,
        solved: inner.times.len() as u32,
        from_store: inner.from_store,
        error: inner.error.clone(),
    }
}

pub fn parse_flop(text: &str) -> Option<[Card; 3]> {
    let cards: Vec<Card> = (0..3).map(|i| text.get(2 * i..2 * i + 2)?.parse().ok()).collect::<Option<_>>()?;
    let set = CardSet::from_cards(&cards);
    (set.len() == 3).then(|| [cards[0], cards[1], cards[2]])
}

/// Every suit renaming of `flop`, each once.
pub fn renamings(flop: [Card; 3]) -> Vec<[Card; 3]> {
    let mut out: Vec<[Card; 3]> = Vec::new();
    for r in suits::all() {
        let renamed = flop.map(|c| suits::rename(c, &r));
        let set = CardSet::from_cards(&renamed);
        if !out.iter().any(|f| CardSet::from_cards(f) == set) {
            out.push(renamed);
        }
    }
    out
}

pub fn random_flop(rng: &mut impl Rng) -> [Card; 3] {
    let mut used = CardSet::default();
    let mut card = || loop {
        let c = Card::from_index(rng.random_range(0..52));
        if !used.contains(c) {
            used.insert(c);
            return c;
        }
    };
    [card(), card(), card()]
}

trait SeedFromTime {
    fn from_os_rng_or_time() -> Self;
}

impl SeedFromTime for ChaCha8Rng {
    fn from_os_rng_or_time() -> ChaCha8Rng {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7);
        ChaCha8Rng::seed_from_u64(nanos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flop_has_one_renaming_per_suit_pattern_without_a_card_twice() {
        let count = |text: &str| renamings(parse_flop(text).unwrap()).len();
        // Rainbow, two-tone, monotone; a pair; trips.
        assert_eq!([count("AsKh2d"), count("AsKs2d"), count("AsKs2s"), count("AsAh2d"), count("AsAhAd")], [24, 12, 4, 12, 4]);
        for f in renamings(parse_flop("Js9s6h").unwrap()) {
            assert_eq!(CardSet::from_cards(&f).len(), 3);
            assert_eq!(canonical(f), canonical(parse_flop("Js9s6h").unwrap()));
        }
    }
}
