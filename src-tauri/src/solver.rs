//! Heads-up postflop solver: estimate, a solve in the background with progress events, and
//! the solved tree node by node. Locked while the PokerStars client runs, like the learning
//! area; a running solve stops as soon as the client starts. Every solve is a task in the
//! task center and is saved when done; a spot saved before opens instead of solving again.

use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use ringside_solver::library::catalog;
use ringside_solver::library::format::{Stored, View};
use ringside_solver::saved::Snapshot;
use ringside_solver::{Error, Estimate, Job, Node, Progress, Settings, Solution, Spot, Step, Summary, estimate, spawn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::jobs::{self, Kind, Target};
use crate::solutions::{self, SaveInfo};
use crate::{AppState, CmdResult, blocking, err};

/// The solver allocates its whole tree up front; bigger spots use 16-bit storage or are
/// refused. The limit is half the installed memory, so Windows and the app keep room;
/// between 2 and 32 GB, 4 GB when the size cannot be read.
pub(crate) fn memory_limit() -> u64 {
    const GB: u64 = 1 << 30;
    installed_memory().map_or(4 * GB, |total| (total / 2).clamp(2 * GB, 32 * GB))
}

#[cfg(windows)]
fn installed_memory() -> Option<u64> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some(status.ullTotalPhys)
}

#[cfg(target_os = "macos")]
fn installed_memory() -> Option<u64> {
    let out = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn installed_memory() -> Option<u64> {
    None
}
const PROGRESS_EVERY: Duration = Duration::from_millis(150);

#[derive(Default)]
struct SolverState {
    job: Mutex<Option<Job>>,
    solution: Mutex<Option<Solution>>,
    /// A spot from the library, shown instead of a solution.
    stored: Mutex<Option<Stored>>,
    /// A saved solution, shown instead of a solution.
    saved: Mutex<Option<Snapshot>>,
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
    /// `saved`: its id in the saved solutions; `reused`: opened from there, not solved.
    Solved { run: u64, summary: Summary, saved: Option<i64>, reused: bool },
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
    blocking(app, move |_| estimate(&spot, memory_limit(), threads).map_err(err)).await
}

fn decimal(x: f64) -> String {
    format!("{x:.2}").replace('.', ",")
}

/// Share of the way from the first measurement down to the target, on a log scale, or of
/// the iterations if those run out first.
fn share_done(first: f64, now: f64, target: f64, p: &Progress) -> f64 {
    let iterations = p.iteration as f64 / p.max_iterations.max(1) as f64;
    let (from, to) = (first.max(target * 1.01).ln(), target.ln());
    let way = (from - now.max(target).ln()) / (from - to);
    way.max(iterations).clamp(0.0, 1.0)
}

/// Starts a solve; progress and the result arrive as `solver-progress` and `solver-done` events
/// tagged with `run`, which the page picks so it can drop events of a solve it replaced.
/// A solve still running is stopped first. A saved solution of the same spot, at least as
/// exact, opens instead unless `fresh`. `info` describes the spot for the saved list.
#[tauri::command]
pub async fn solver_start(
    spot: Spot,
    settings: Settings,
    run: u64,
    info: SaveInfo,
    fresh: bool,
    app: AppHandle,
) -> CmdResult<()> {
    blocking(app.clone(), move |_| {
        let mut job = SOLVER.job.lock().unwrap();
        if let Some(old) = job.take() {
            // Two trees at once could need twice the memory.
            old.cancel();
            old.wait();
        }
        *SOLVER.solution.lock().unwrap() = None;
        *SOLVER.stored.lock().unwrap() = None;
        *SOLVER.saved.lock().unwrap() = None;

        if !fresh && let Some((id, snapshot)) = solutions::lookup(&app, &spot, settings.target_pct) {
            let summary = snapshot.meta.summary.clone();
            *SOLVER.saved.lock().unwrap() = Some(snapshot);
            let _ = app.emit("solver-done", DoneEvent::Solved { run, summary, saved: Some(id), reused: true });
            return Ok(());
        }

        let title = if info.title.trim().is_empty() { "Spot lösen".to_string() } else { format!("Lösen: {}", info.title.trim()) };
        let task = jobs::start(Kind::Solver, title, Target::to("solver", None));
        task.detail(format!("Board {}", spot.board));
        let progress_app = app.clone();
        let progress_task = task.clone();
        let target = settings.target_pct;
        let mut first: Option<f64> = None;
        let mut last: Option<Instant> = None;
        let on_progress = move |p: &Progress| {
            if last.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) {
                last = Some(Instant::now());
                let _ = progress_app.emit("solver-progress", ProgressEvent { run, progress: *p });
            }
            if p.iteration > 0 {
                let start = *first.get_or_insert(p.exploitability_pct);
                progress_task.progress(share_done(start, p.exploitability_pct, target, p));
                progress_task.detail(format!(
                    "Iteration {} · noch {} % ausbeutbar, Ziel {} %",
                    p.iteration,
                    decimal(p.exploitability_pct),
                    decimal(target)
                ));
            }
        };
        let stop_app = app.clone();
        let stop_task = task.clone();
        let stop = move || stop_task.should_stop() || client_running(&stop_app);
        let done_app = app.clone();
        let solve_settings = settings.clone();
        let on_finish = move |result: Result<Solution, Error>| {
            let event = match result {
                Ok(mut solution) => {
                    let summary = solution.summary.clone();
                    task.detail("Wird gespeichert …");
                    let saved = match solutions::save(&done_app, &mut solution, &solve_settings, &info) {
                        Ok(id) => Some(id),
                        Err(e) => {
                            eprintln!("saving the solution: {e}");
                            None
                        }
                    };
                    *SOLVER.solution.lock().unwrap() = Some(solution);
                    task.detail(format!("Fertig · {} % ausbeutbar", decimal(summary.exploitability_pct)));
                    task.target(Target::to("solver", saved));
                    task.finish();
                    DoneEvent::Solved { run, summary, saved, reused: false }
                }
                Err(Error::Cancelled) => {
                    task.stopped();
                    DoneEvent::Cancelled { run, locked: client_running(&done_app) }
                }
                Err(e) => {
                    task.fail(e.to_string());
                    DoneEvent::Failed { run, error: e.to_string() }
                }
            };
            let _ = done_app.emit("solver-done", event);
        };
        *job = Some(spawn(spot, settings, memory_limit(), on_progress, stop, on_finish));
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
    *SOLVER.stored.lock().unwrap() = None;
    *SOLVER.saved.lock().unwrap() = None;
}

/// Shows a library spot on the solver page in place of the last solution.
pub(crate) fn show_stored(stored: Stored) {
    *SOLVER.solution.lock().unwrap() = None;
    *SOLVER.saved.lock().unwrap() = None;
    *SOLVER.stored.lock().unwrap() = Some(stored);
}

/// Shows a saved solution on the solver page in place of the last solution.
pub(crate) fn show_saved(snapshot: Snapshot) {
    *SOLVER.solution.lock().unwrap() = None;
    *SOLVER.stored.lock().unwrap() = None;
    *SOLVER.saved.lock().unwrap() = Some(snapshot);
}

/// Shows a solve made elsewhere (a practice hand's spot) on the solver page.
pub(crate) fn show_solution(solution: Solution) {
    *SOLVER.stored.lock().unwrap() = None;
    *SOLVER.saved.lock().unwrap() = None;
    *SOLVER.solution.lock().unwrap() = Some(solution);
}

/// A node of the last solution (or library spot), reached from the root by `path`.
#[tauri::command]
pub async fn solver_node(path: Vec<Step>, app: AppHandle) -> CmdResult<Node> {
    blocking(app, move |_| {
        if let Some(stored) = SOLVER.stored.lock().unwrap().as_ref() {
            return View::new(stored, catalog()).and_then(|v| v.node(&path)).map_err(err);
        }
        if let Some(saved) = SOLVER.saved.lock().unwrap().as_ref() {
            return saved.node(&path).map_err(err);
        }
        let mut solution = SOLVER.solution.lock().unwrap();
        let solution = solution.as_mut().ok_or("Noch keine Lösung vorhanden.")?;
        solution.node(&path).map_err(err)
    })
    .await
}
