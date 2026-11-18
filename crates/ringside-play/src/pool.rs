//! The stock of postflop spots hands are steered into, so a hand never waits for a solve.
//!
//! A worker keeps up to [`TARGET`] spots ready for the tables of the current plan. For each
//! it picks one of the model's heads-up flops (weighted by how often the model plays there,
//! and with the hero's seat in it when the seat is fixed) and a flop. It first asks the
//! [`Lookup`] (spot library, saved solves): those open at once but know only the first
//! street and the first decision on the turn. Otherwise it solves with the solver page's
//! default precision ([`TARGET_PCT`], [`MAX_ITERATIONS`]) on few threads and 16-bit
//! storage, which halves the memory, and hands the solve to [`Lookup::save`], so the
//! solver page opens the same solution for the same spot. A spot serves up to
//! [`MAX_USES`] hands with different cards, then makes room for a new one.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::{Card, CardSet};
use ringside_solver::{Error, Settings, Solution, Spot, estimate, solve};
use serde::Serialize;

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

/// Stored solutions the worker looks at before it solves.
pub trait Lookup: Send + Sync {
    /// A stored solution of exactly this spot.
    fn load(&self, spot: &Spot) -> Option<Box<dyn Tree>>;
    /// Flops stored for the spot's ranges, pot, stack and sizes (its board is ignored).
    fn flops(&self, _spot: &Spot) -> Vec<String> {
        Vec::new()
    }
    /// Keeps a fresh solve where [`Lookup::load`] and the solver page find it.
    fn save(&self, _table: TableKey, _scenario: &Scenario, _solution: &mut Solution, _settings: &Settings) {}
}

pub struct NoLookup;

impl Lookup for NoLookup {
    fn load(&self, _spot: &Spot) -> Option<Box<dyn Tree>> {
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

/// What a hand takes from the pool.
#[derive(Clone)]
pub struct Taken {
    pub id: u64,
    pub table: TableKey,
    pub scenario: Scenario,
    pub flop: [Card; 3],
    pub source: TreeSource,
    pub complete: bool,
    pub exploitability_pct: f64,
    pub tree: Arc<Mutex<Box<dyn Tree>>>,
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
}

pub struct Pool {
    inner: Mutex<Inner>,
    wake: Condvar,
    cancel: AtomicBool,
    next_id: AtomicU64,
    lookup: Box<dyn Lookup>,
    threads: usize,
    /// For all ready spots together, and the largest single one.
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
            }),
            wake: Condvar::new(),
            cancel: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            lookup,
            threads: threads.max(1),
            budget,
        }
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

    /// Memory of the solved spots ready now.
    pub fn memory_bytes(&self) -> u64 {
        self.inner.lock().unwrap().ready.iter().map(|e| e.memory_bytes).sum()
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
            id: e.id,
            table: e.table,
            scenario: e.scenario.clone(),
            flop: e.flop,
            source: e.source,
            complete: e.complete,
            exploitability_pct: e.exploitability_pct,
            tree: e.tree.clone(),
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
    /// and waits; `report` hears every change of the status.
    pub fn run(&self, quit: impl Fn() -> bool, paused: impl Fn() -> bool, report: impl Fn(&PoolStatus)) {
        let mut rng = ChaCha8Rng::from_os_rng_or_time();
        while !quit() {
            self.step(&mut rng, &paused, &report);
        }
    }

    /// One round of the worker: adds at most one spot or waits a little.
    pub fn step(&self, rng: &mut ChaCha8Rng, paused: &dyn Fn() -> bool, report: &dyn Fn(&PoolStatus)) {
        let set = |status: Status, inner: &mut Inner| {
            if inner.status != status {
                inner.status = status;
                report(&status_of(inner));
            }
        };
        let mut inner = self.inner.lock().unwrap();
        if self.cancel.load(Ordering::Relaxed) {
            set(Status::Cancelled, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        }
        if paused() {
            set(Status::PausedClient, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        }
        let Some(plan) = inner.plan.clone() else {
            set(Status::Waiting, &mut inner);
            let _ = self.wake.wait_timeout(inner, Duration::from_secs(1));
            return;
        };
        let used: u64 = inner.ready.iter().map(|e| e.memory_bytes).sum();
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
        if let Some(tree) = self.lookup.load(&spot) {
            self.add(generation, table, scenario, flop, tree, 0, report);
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
                // Exploitability falls about geometrically, so its logarithm measures progress.
                let e0 = *first.get_or_insert(p.exploitability_pct.max(TARGET_PCT * 1.01));
                let f = ((e0 / p.exploitability_pct.max(1e-9)).ln() / (e0 / TARGET_PCT).ln()).clamp(0.0, 1.0);
                let f = f.max(p.iteration as f64 / MAX_ITERATIONS as f64);
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
            || self.cancel.load(Ordering::Relaxed) || paused() || self.inner.lock().unwrap().generation != generation,
        );
        let secs = start.elapsed().as_secs_f64();
        match result {
            Ok(mut solution) => {
                self.lookup.save(table, &scenario, &mut solution, &settings);
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
        let flop = self.stored_flop(scenario, rng).unwrap_or_else(|| random_flop(rng));
        Some((table.key, scenario.clone(), flop))
    }

    fn stored_flop(&self, scenario: &Scenario, rng: &mut ChaCha8Rng) -> Option<[Card; 3]> {
        let taken: Vec<[Card; 3]> = self.inner.lock().unwrap().ready.iter().map(|e| e.flop).collect();
        let flops: Vec<[Card; 3]> =
            self.lookup.flops(&scenario.spot("")).iter().filter_map(|f| parse_flop(f)).filter(|f| !taken.contains(f)).collect();
        (!flops.is_empty()).then(|| flops[rng.random_range(0..flops.len())])
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
        error: inner.error.clone(),
    }
}

pub fn parse_flop(text: &str) -> Option<[Card; 3]> {
    let cards: Vec<Card> = (0..3).map(|i| text.get(2 * i..2 * i + 2)?.parse().ok()).collect::<Option<_>>()?;
    let set = CardSet::from_cards(&cards);
    (set.len() == 3).then(|| [cards[0], cards[1], cards[2]])
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
