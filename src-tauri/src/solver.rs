//! Heads-up postflop solver: estimate, a solve in the background with progress events, and
//! the solved tree node by node. Locked while the PokerStars client runs, like the learning
//! area; a running solve stops as soon as the client starts. Every solve is a task in the
//! task center and is saved when done; a spot saved before opens instead of solving again.
//!
//! A solved tree takes gigabytes, its saved snapshot about a megabyte. The tree stays only
//! while it is used: once no node was asked for in [`IDLE`], or the page has been closed
//! for [`AWAY`], the snapshot replaces it (`solver-released`). The page looks the same; past
//! the next street's first decision it offers to solve again.

use std::sync::atomic::Ordering;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use ringside_solver::gate::Priority;
use ringside_solver::library::catalog;
use ringside_solver::library::format::{Stored, View};
use ringside_solver::saved::Snapshot;
use ringside_solver::{Error, Estimate, Job, Node, Progress, Settings, Solution, Spot, Step, Summary, estimate, spawn};
use ringside_store::Policy;
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
pub(crate) fn installed_memory() -> Option<u64> {
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some(status.ullTotalPhys)
}

#[cfg(target_os = "macos")]
pub(crate) fn installed_memory() -> Option<u64> {
    let out = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn installed_memory() -> Option<u64> {
    None
}
const PROGRESS_EVERY: Duration = Duration::from_millis(150);
const IDLE: Duration = Duration::from_secs(180);
/// Long enough for a quick look at another page.
const AWAY: Duration = Duration::from_secs(30);

/// What the solver page browses.
enum Shown {
    None,
    /// A solve in memory and the exploitability it aimed for.
    Live(Box<Solution>, f64),
    /// A spot from the library.
    Stored(Stored),
    /// A saved solution, or a solve that was not used for a while.
    Saved(Box<Snapshot>),
}

struct Use {
    last: Instant,
    /// Since when the page is closed.
    away: Option<Instant>,
}

struct SolverState {
    job: Mutex<Option<Job>>,
    shown: Mutex<Shown>,
    used: Mutex<Use>,
}

static SOLVER: LazyLock<SolverState> = LazyLock::new(|| SolverState {
    job: Mutex::new(None),
    shown: Mutex::new(Shown::None),
    used: Mutex::new(Use { last: Instant::now(), away: None }),
});

fn show(shown: Shown) {
    touch();
    *SOLVER.shown.lock().unwrap() = shown;
}

fn touch() {
    let mut used = SOLVER.used.lock().unwrap();
    used.last = Instant::now();
    used.away = None;
}

/// Whether a solve in memory makes room for its snapshot, given the time since the last
/// node was asked for and since the page closed.
fn due(idle: Duration, away: Option<Duration>) -> bool {
    idle >= IDLE || (idle >= AWAY && away.is_some_and(|a| a >= AWAY))
}

/// Replaces a solve nobody uses by its snapshot. Called by the housekeeping thread.
pub(crate) fn tick(app: &AppHandle) {
    let (idle, away) = {
        let used = SOLVER.used.lock().unwrap();
        (used.last.elapsed(), used.away.map(|t| t.elapsed()))
    };
    if !due(idle, away) {
        return;
    }
    let mut shown = SOLVER.shown.lock().unwrap();
    let Shown::Live(solution, target_pct) = &mut *shown else { return };
    let settings = Settings { target_pct: *target_pct, max_iterations: 0, threads: 1 };
    *shown = match Snapshot::take(solution, &settings) {
        Ok(snapshot) => Shown::Saved(Box::new(snapshot)),
        Err(e) => {
            eprintln!("snapshot of the solution: {e}");
            Shown::None
        }
    };
    drop(shown);
    let _ = app.emit("solver-released", ());
}

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
        show(Shown::None);

        let keyed = crate::store::keyed(&spot, info.model.as_ref());
        if !fresh && let Some(keyed) = &keyed {
            // Only own solves: the page shows every node, library and cloud spots only the flop.
            let policy = Policy { others: false, ..crate::store::policy(&app, false, false) };
            let ask = crate::store::Ask::new(&spot, keyed, settings.target_pct);
            let found = crate::store::resolve_app(&app, &ask, &policy, || false, Some, || None);
            if let Some((_, crate::store::Found::Snapshot { id, snapshot })) = found {
                let summary = snapshot.meta.summary.clone();
                show(Shown::Saved(snapshot));
                let _ = app.emit("solver-done", DoneEvent::Solved { run, summary, saved: Some(id), reused: true });
                return Ok(());
            }
        }
        // What the user looks at goes first; a background solve gives way within an iteration.
        let ticket = crate::store::gate().enter(Priority::Viewed);
        while !ticket.hold(Duration::from_secs(1)) {
            if client_running(&app) {
                return Err("Solange PokerStars läuft, rechnet Ringside nicht.".into());
            }
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
            drop(ticket);
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
                    show(Shown::Live(Box::new(solution), solve_settings.target_pct));
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

/// What the solver page picks: table, the two seats by position name, the pot type
/// ("srp", "3bet", "limp") and the depth.
#[derive(Debug, serde::Deserialize)]
pub struct ModelPick {
    players: usize,
    ante: ringside_engine::pushfold_table::Ante,
    stack: f64,
    seats: [String; 2],
    pot: String,
}

/// The model's own flop for the pick: its exact ranges, pot and stack and the preflop line.
/// The page solves it as the practice table and the cloud know it, so it can be shared.
#[derive(Debug, Serialize)]
pub struct ModelSpot {
    model: ringside_engine::preflop_table::ModelLine,
    oop_range: String,
    ip_range: String,
    pot: f64,
    stack: f64,
}

fn model_spot(pick: &ModelPick) -> Option<ModelSpot> {
    use ringside_play::scenario::PotType;
    let key = ringside_play::TableKey { players: pick.players, ante: pick.ante, stack_bb: pick.stack };
    let table = ringside_play::table::deep(key)?;
    let names = ringside_engine::multiway::position_names(pick.players);
    let pot_type = match pick.pot.as_str() {
        "srp" => PotType::Srp,
        "3bet" => PotType::ThreeBet,
        _ => PotType::Limped,
    };
    let seats = |sc: &ringside_play::scenario::Scenario| {
        let mut got = [names.get(sc.oop).copied()?, names.get(sc.ip).copied()?];
        let mut want = [pick.seats[0].as_str(), pick.seats[1].as_str()];
        got.sort_unstable();
        want.sort_unstable();
        Some(got == want)
    };
    // The plainest line to this flop: an open and a call before a 4-bet pot.
    let sc = table
        .scenarios
        .iter()
        .filter(|sc| sc.pot_type == pot_type && seats(sc) == Some(true))
        .min_by(|a, b| a.line.len().cmp(&b.line.len()).then(b.reach.total_cmp(&a.reach)))?;
    Some(ModelSpot {
        model: ringside_engine::preflop_table::ModelLine {
            family: ringside_engine::preflop_table::Family::Plain,
            ante: pick.ante,
            players: pick.players,
            stack_bb: pick.stack,
            line: sc.line.clone(),
        },
        oop_range: sc.range(0),
        ip_range: sc.range(1),
        pot: sc.pot,
        stack: sc.stack,
    })
}

/// The model's own spot for the page's pick, if the model has a table at that depth.
#[tauri::command]
pub async fn solver_model_spot(pick: ModelPick, app: AppHandle) -> CmdResult<Option<ModelSpot>> {
    blocking(app, move |_| Ok(model_spot(&pick))).await
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
    show(Shown::None);
}

/// The solver page opened or closed; a solve in memory goes [`AWAY`] after it closed.
#[tauri::command]
pub fn solver_page(open: bool) {
    if open {
        touch();
    } else {
        SOLVER.used.lock().unwrap().away.get_or_insert_with(Instant::now);
    }
}

/// Shows a library spot on the solver page in place of the last solution.
pub(crate) fn show_stored(stored: Stored) {
    show(Shown::Stored(stored));
}

/// Shows a saved solution on the solver page in place of the last solution.
pub(crate) fn show_saved(snapshot: Snapshot) {
    show(Shown::Saved(Box::new(snapshot)));
}

/// Shows a solve made elsewhere (a practice hand's spot, solved to `target_pct`).
pub(crate) fn show_solution(solution: Solution, target_pct: f64) {
    show(Shown::Live(Box::new(solution), target_pct));
}

/// A node of the last solution (or library spot), reached from the root by `path`.
#[tauri::command]
pub async fn solver_node(path: Vec<Step>, app: AppHandle) -> CmdResult<Node> {
    blocking(app, move |_| {
        touch();
        match &mut *SOLVER.shown.lock().unwrap() {
            Shown::None => Err("Noch keine Lösung vorhanden.".into()),
            Shown::Live(solution, _) => solution.node(&path).map_err(err),
            Shown::Stored(stored) => View::new(stored, catalog()).and_then(|v| v.node(&path)).map_err(err),
            Shown::Saved(saved) => saved.node(&path).map_err(err),
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_gets_the_models_own_spot_and_it_is_shareable() {
        let pick = ModelPick { players: 6, ante: ringside_engine::pushfold_table::Ante::BigBlind, stack: 40.0, seats: ["BB".into(), "BTN".into()], pot: "srp".into() };
        let m = model_spot(&pick).expect("the model has BTN against BB");
        let spot = Spot {
            oop_range: m.oop_range.clone(),
            ip_range: m.ip_range.clone(),
            board: "Kh7c2h".into(),
            pot: m.pot,
            stack: m.stack,
            sizes: Default::default(),
            allin_threshold: 0.0,
        };
        let keyed = ringside_store::key_of(&spot, Some(&m.model)).unwrap();
        assert!(keyed.key.starts_with("spot:"), "the page's spot is filed under its cloud key: {keyed:?}");
        assert!(model_spot(&ModelPick { stack: 41.0, ..pick }).is_none(), "no table at that depth");
    }

    #[test]
    fn a_solve_goes_when_idle_or_soon_after_the_page_closed() {
        let secs = Duration::from_secs;
        assert!(!due(secs(60), None));
        assert!(due(secs(181), None));
        assert!(due(secs(40), Some(secs(31))));
        assert!(!due(secs(40), Some(secs(10))));
        assert!(!due(secs(5), Some(secs(300))), "a solve that just finished stays a moment");
    }
}
