use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Instant;

use postflop_solver::{PostFlopGame, compute_exploitability, finalize, solve_step};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::spot::{Spot, bb, build};

/// How often the exploitability is measured; a measurement costs about one iteration.
const MEASURE_EVERY: u32 = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Stop once neither player can gain more than this, in percent of the starting pot.
    pub target_pct: f64,
    pub max_iterations: u32,
    pub threads: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Progress {
    pub iteration: u32,
    pub max_iterations: u32,
    /// Average gain of a best response over the current strategies, in BB and in percent of the pot.
    pub exploitability: f64,
    pub exploitability_pct: f64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub iterations: u32,
    pub exploitability: f64,
    pub exploitability_pct: f64,
    /// `false`: stopped at `max_iterations` before reaching the target.
    pub converged: bool,
    pub elapsed_ms: u64,
    pub memory_bytes: u64,
    pub compressed: bool,
    pub threads: usize,
}

/// A solved spot. Strategies, equities and EVs are read node by node, see `Solution::node`.
pub struct Solution {
    pub(crate) game: PostFlopGame,
    pub spot: Spot,
    pub summary: Summary,
}

fn check(settings: &Settings) -> Result<(), Error> {
    if !(settings.target_pct.is_finite() && (0.01..=100.0).contains(&settings.target_pct)) {
        return Err(Error::Invalid("Die Ziel-Ausbeutbarkeit muss zwischen 0,01 und 100 % liegen.".into()));
    }
    if settings.max_iterations == 0 {
        return Err(Error::Invalid("Mindestens eine Iteration.".into()));
    }
    if !(1..=256).contains(&settings.threads) {
        return Err(Error::Invalid("Zwischen 1 und 256 Threads.".into()));
    }
    Ok(())
}

/// Solves `spot` on the calling thread with `settings.threads` workers. `stop` is asked before
/// every iteration; returning `true` ends the solve with `Error::Cancelled`.
pub fn solve(
    spot: &Spot,
    settings: &Settings,
    memory_limit: u64,
    mut on_progress: impl FnMut(&Progress),
    stop: impl Fn() -> bool,
) -> Result<Solution, Error> {
    run(spot, settings, memory_limit, &mut on_progress, &stop)
}

// Not generic: the solver's hot loops are instantiated here, in an always optimised crate,
// instead of in whichever crate calls `solve`.
fn run(
    spot: &Spot,
    settings: &Settings,
    memory_limit: u64,
    on_progress: &mut dyn FnMut(&Progress),
    stop: &dyn Fn() -> bool,
) -> Result<Solution, Error> {
    check(settings)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(settings.threads)
        .build()
        .map_err(|e| Error::Invalid(format!("Threads lassen sich nicht starten: {e}")))?;
    // Only the number crunching runs on the pool, so the callbacks need not be `Send`.
    let built = pool.install(|| build(spot, memory_limit, settings.threads))?;
    let estimate = built.estimate;
    if !estimate.fits {
        return Err(Error::TooLarge { needed: estimate.memory_bytes, limit: memory_limit });
    }
    let mut game = built.game;
    pool.install(|| game.allocate_memory(estimate.compressed));

    let start = Instant::now();
    let pot = game.tree_config().starting_pot as f64;
    let target = settings.target_pct / 100.0 * pot;
    let progress = |iteration: u32, expl: f64| Progress {
        iteration,
        max_iterations: settings.max_iterations,
        exploitability: bb(expl),
        exploitability_pct: expl / pot * 100.0,
        elapsed_ms: start.elapsed().as_millis() as u64,
    };

    let mut expl = pool.install(|| compute_exploitability(&game)) as f64;
    on_progress(&progress(0, expl));
    let mut iteration = 0;
    while iteration < settings.max_iterations && expl > target {
        if stop() {
            return Err(Error::Cancelled);
        }
        pool.install(|| solve_step(&game, iteration));
        iteration += 1;
        if iteration.is_multiple_of(MEASURE_EVERY) || iteration == settings.max_iterations {
            expl = pool.install(|| compute_exploitability(&game)) as f64;
            on_progress(&progress(iteration, expl));
        }
    }
    pool.install(|| finalize(&mut game));
    let last = progress(iteration, expl);
    Ok(Solution {
        game,
        spot: spot.clone(),
        summary: Summary {
            iterations: iteration,
            exploitability: last.exploitability,
            exploitability_pct: last.exploitability_pct,
            converged: expl <= target,
            elapsed_ms: last.elapsed_ms,
            memory_bytes: estimate.memory_bytes,
            compressed: estimate.compressed,
            threads: settings.threads,
        },
    })
}

/// A solve running on its own thread.
pub struct Job {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Job {
    /// Asks the solve to stop; it ends with `Error::Cancelled` after the current iteration.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_finished(&self) -> bool {
        self.thread.is_finished()
    }

    /// Blocks until `on_finish` has run.
    pub fn wait(self) {
        let _ = self.thread.join();
    }
}

/// Runs `solve` on a new thread and hands the result to `on_finish`. Besides `Job::cancel`,
/// `stop` can end the solve, e.g. when the poker client starts.
pub fn spawn(
    spot: Spot,
    settings: Settings,
    memory_limit: u64,
    on_progress: impl FnMut(&Progress) + Send + 'static,
    stop: impl Fn() -> bool + Send + 'static,
    on_finish: impl FnOnce(Result<Solution, Error>) + Send + 'static,
) -> Job {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let thread = std::thread::spawn(move || {
        let result = solve(&spot, &settings, memory_limit, on_progress, || flag.load(Ordering::Relaxed) || stop());
        on_finish(result);
    });
    Job { cancel, thread }
}
