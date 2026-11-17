//! Heads-up postflop solver: estimate, a solve in the background with progress events, and
//! the solved tree node by node. Locked while the PokerStars client runs, like the learning
//! area; a running solve stops as soon as the client starts.

use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use ringside_solver::{Error, Estimate, Job, Node, Progress, Settings, Solution, Spot, Step, Summary, estimate, spawn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult, blocking, err};

/// The solver allocates its whole tree up front; bigger spots use 16-bit storage or are refused.
const MEMORY_LIMIT: u64 = 4 << 30;
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

#[derive(Default)]
struct SolverState {
    job: Mutex<Option<Job>>,
    solution: Mutex<Option<Solution>>,
}

static SOLVER: LazyLock<SolverState> = LazyLock::new(SolverState::default);

#[derive(Clone, Serialize)]
struct ProgressEvent {
    run: u64,
    #[serde(flatten)]
    progress: Progress,
}

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum DoneEvent {
    Solved { run: u64, summary: Summary },
    /// `locked`: stopped because PokerStars started.
    Cancelled { run: u64, locked: bool },
    Failed { run: u64, error: String },
}

fn client_running(app: &AppHandle) -> bool {
    app.state::<AppState>().client_running.load(Ordering::Relaxed)
}

/// Memory and rough time for `spot`, before anything is allocated.
#[tauri::command]
pub async fn solver_estimate(spot: Spot, threads: usize, app: AppHandle) -> CmdResult<Estimate> {
    blocking(app, move |_| estimate(&spot, MEMORY_LIMIT, threads).map_err(err)).await
}

/// Starts a solve; progress and the result arrive as `solver-progress` and `solver-done` events
/// tagged with `run`, which the page picks so it can drop events of a solve it replaced.
/// A solve still running is stopped first.
#[tauri::command]
pub async fn solver_start(spot: Spot, settings: Settings, run: u64, app: AppHandle) -> CmdResult<()> {
    blocking(app.clone(), move |_| {
        let mut job = SOLVER.job.lock().unwrap();
        if let Some(old) = job.take() {
            // Two trees at once could need twice the memory.
            old.cancel();
            old.wait();
        }
        *SOLVER.solution.lock().unwrap() = None;

        let progress_app = app.clone();
        let mut last: Option<Instant> = None;
        let on_progress = move |p: &Progress| {
            if last.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) {
                last = Some(Instant::now());
                let _ = progress_app.emit("solver-progress", ProgressEvent { run, progress: *p });
            }
        };
        let stop_app = app.clone();
        let stop = move || client_running(&stop_app);
        let done_app = app.clone();
        let on_finish = move |result: Result<Solution, Error>| {
            let event = match result {
                Ok(solution) => {
                    let summary = solution.summary.clone();
                    *SOLVER.solution.lock().unwrap() = Some(solution);
                    DoneEvent::Solved { run, summary }
                }
                Err(Error::Cancelled) => DoneEvent::Cancelled { run, locked: client_running(&done_app) },
                Err(e) => DoneEvent::Failed { run, error: e.to_string() },
            };
            let _ = done_app.emit("solver-done", event);
        };
        *job = Some(spawn(spot, settings, MEMORY_LIMIT, on_progress, stop, on_finish));
        Ok(())
    })
    .await
}

/// Stops the running solve; it reports `cancelled` once the current iteration ends.
#[tauri::command]
pub fn solver_cancel() {
    if let Some(job) = SOLVER.job.lock().unwrap().as_ref() {
        job.cancel();
    }
}

/// Frees the last solution; trees can take gigabytes.
#[tauri::command]
pub fn solver_clear() {
    *SOLVER.solution.lock().unwrap() = None;
}

/// A node of the last solution, reached from the root by `path`.
#[tauri::command]
pub async fn solver_node(path: Vec<Step>, app: AppHandle) -> CmdResult<Node> {
    blocking(app, move |_| {
        let mut solution = SOLVER.solution.lock().unwrap();
        let solution = solution.as_mut().ok_or("Noch keine Lösung vorhanden.")?;
        solution.node(&path).map_err(err)
    })
    .await
}
