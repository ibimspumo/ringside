//! Hand analysis in the app: a background worker rates the hero's decisions of every
//! stored hand (preflop at once, in batches) and then works through the queue of
//! heads-up postflop spots, marked hands first, then the biggest pots.
//!
//! The bulk run starts with the first click on "Hände analysieren" and stays on, so new
//! hands are analysed as they are imported. Without it, the automatic run of
//! [`crate::auto_analysis`] rates the new hands once PokerStars is closed; their flop
//! spots go before the rest of the history. A single hand asked for in the replayer is
//! analysed at once and its postflop spot jumps the queue, interrupting a running solve,
//! which stays queued. Like the trainer and the solver, everything is locked while
//! PokerStars runs; a running solve stops within an iteration and resumes later, a preflop
//! batch before its next piece of hands.
//!
//! A spot solved before, on the solver page or by an earlier analysis, is rated from the
//! saved solution when every decision of the hand lies in its saved part; every spot
//! solved here is saved under "Meine Lösungen", with the hand it came from.
//!
//! The worker runs whatever page is open. Every change goes out through [`report`] as
//! `analysis-status` (all details) and `analysis-progress` ([`JobProgress`]: title,
//! share, time left and state), the one place an app-wide task list hooks in.
//! `analysis_cancel` stops everything, `analysis_cancel_hand` one hand.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use ringside_analysis::postflop::{JobError, TARGET_PCT, rate, settings, solve_spot};
use ringside_analysis::{ChartTarget, ENGINE, PostflopJob, analyze_hand, analyze_raw, chart_targets, estimate_rest};
use ringside_core::db::{AnalysisCounts, AnalysisSummary, Decision, Filter, SolutionSource, Status};
use ringside_core::model::Street;
use ringside_solver::gate::Priority;
use ringside_solver::saved::NOT_STORED;
use ringside_solver::Solution;
use ringside_store::{Origin, Policy};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::auto_analysis::{Auto, Edge, Phase};
use crate::solutions::{self, SaveInfo};
use crate::store::{self, Ask, Found};
use crate::{AppState, CmdResult, blocking, err, jobs, store_settings};

const ENABLED_KEY: &str = "analysis_enabled";
/// Hands after this id are new for the automatic run.
const AFTER_KEY: &str = "auto_analysis_after";
/// The user cancelled the automatic run; holds until PokerStars closes the next time.
const CANCELLED_KEY: &str = "auto_analysis_cancelled";
const BATCH: usize = 2000;
/// Hands per piece of a batch; PokerStars is checked before each.
const PIECE: usize = 100;
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// Assumed solve time until the first spots were solved here.
const DEFAULT_SOLVE_SECS: f64 = 60.0;

#[derive(Clone, Debug, Default, Serialize)]
pub struct Solving {
    pub hand_id: u64,
    pub iteration: u32,
    pub max_iterations: u32,
    pub exploitability_pct: f64,
    pub elapsed_ms: u64,
}

/// State of a long-running task as the app shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    #[default]
    Waiting,
    Running,
    /// Locked while PokerStars runs; resumes on its own.
    PausedPokerstars,
    Done,
    Cancelled,
    Failed,
}

/// Progress of the analysis in plain words.
#[derive(Clone, Debug, Default, Serialize)]
pub struct JobProgress {
    pub id: &'static str,
    pub title: String,
    pub detail: String,
    /// Done share 0–1, when known.
    pub share: Option<f64>,
    pub eta_secs: Option<f64>,
    pub state: JobState,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AnalysisStatus {
    pub counts: AnalysisCounts,
    /// The bulk run is on.
    pub enabled: bool,
    /// "idle", "preflop", "postflop" or "locked".
    pub state: &'static str,
    pub solving: Option<Solving>,
    /// Remaining time of the whole run in seconds, from measured speeds.
    pub eta_secs: Option<f64>,
    /// Hands queued for solving that the user asked for.
    pub marked: Vec<u64>,
    pub threads: usize,
    pub last_error: Option<String>,
    /// Stopped with `analysis_cancel`; cleared by the next start.
    pub cancelled: bool,
    /// The automatic run over the new hands is on (it may wait for PokerStars to close).
    pub auto_run: bool,
    pub progress: JobProgress,
}

/// Sent as `analysis-auto-done` when the automatic run rated new hands.
#[derive(Clone, Debug, Serialize)]
pub struct AutoDone {
    pub hands: i64,
    /// Rated decisions that are mistakes or worse.
    pub mistakes: i64,
    /// The run's entry in the task center, whose own note this one replaces.
    pub job: Option<u64>,
    /// The newest tournament among the new hands that has a result: its report is ready.
    pub tournament: Option<u64>,
}

/// Which hands a step may work on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// The whole history: the bulk run.
    All,
    /// The hands after this id, and those the user asked for: the automatic run.
    After(u64),
    /// Only the hands the user asked for.
    Marked,
}

impl Scope {
    fn after(self) -> u64 {
        match self {
            Scope::All => 0,
            Scope::After(id) => id,
            Scope::Marked => u64::MAX,
        }
    }
}

fn int(n: i64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

fn job_progress(s: &AnalysisStatus) -> JobProgress {
    let c = &s.counts;
    let postflop = |c: &AnalysisCounts| (c.decisions > 0).then(|| 1.0 - c.pending as f64 / c.decisions as f64);
    // Only work that waits for PokerStars to close shows as paused; an idle analysis is no task.
    let waits = s.auto_run || !s.marked.is_empty() || (s.enabled && (c.analyzed < c.hands || c.queued > 0));
    let (state, title, share) = match s.state {
        "locked" if waits => (JobState::PausedPokerstars, "Hand-Analyse pausiert, solange PokerStars läuft".to_string(), None),
        "locked" => (JobState::Waiting, "Hand-Analyse wartet".into(), None),
        "preflop" if s.auto_run => (JobState::Running, "Bewerte deine neuen Hände".into(), None),
        "preflop" => (
            JobState::Running,
            "Analysiere deine Hände".into(),
            (c.hands > 0).then(|| c.analyzed as f64 / c.hands as f64),
        ),
        "postflop" => {
            let hand = s.solving.as_ref().filter(|x| s.marked.contains(&x.hand_id));
            let title = match hand {
                Some(h) => format!("Rechne Hand #{} nach dem Flop", h.hand_id),
                None if s.auto_run => "Rechne Flop-Spots deiner neuen Hände".into(),
                None => "Rechne Flop-Spots deiner Hände".into(),
            };
            let share = match hand {
                Some(h) => Some(h.iteration as f64 / h.max_iterations.max(1) as f64),
                // The counts cover the whole history, not the new hands.
                None if s.auto_run => None,
                None => postflop(c),
            };
            (JobState::Running, title, share)
        }
        _ if s.cancelled => (JobState::Cancelled, "Hand-Analyse abgebrochen".into(), None),
        _ if s.last_error.is_some() => (JobState::Failed, "Hand-Analyse mit Fehler gestoppt".into(), None),
        _ if s.enabled && c.analyzed >= c.hands && c.queued == 0 => (JobState::Done, "Alle Hände analysiert".into(), Some(1.0)),
        _ => (JobState::Waiting, "Hand-Analyse wartet".into(), None),
    };
    let mut detail = format!("{} von {} Händen, {} Entscheidungen bewertet", int(c.analyzed), int(c.hands), int(c.rated));
    if c.queued > 0 {
        detail.push_str(&format!(", {} Flop-Spots warten", int(c.queued)));
    }
    if let Some(e) = &s.last_error {
        detail = e.clone();
    }
    let running = state == JobState::Running;
    JobProgress { id: "analysis", title, detail, share, eta_secs: s.eta_secs.filter(|_| running), state }
}

/// Sends the status to the page and the progress to whoever lists running tasks.
fn report(app: &AppHandle, status: &mut AnalysisStatus) {
    status.progress = job_progress(status);
    sync_job(app, &status.progress);
    let _ = app.emit("analysis-status", &*status);
    let _ = app.emit("analysis-progress", &status.progress);
}

/// Mirrors the analysis in the task center: one entry while it runs or waits for
/// PokerStars to close, ended with the analysis.
fn sync_job(app: &AppHandle, p: &JobProgress) {
    let analysis = app.state::<AnalysisState>();
    let mut slot = analysis.job.lock().unwrap();
    match p.state {
        JobState::Running | JobState::PausedPokerstars => {
            let job = slot.get_or_insert_with(|| {
                let job = jobs::start(jobs::Kind::Analysis, "Hand-Analyse", jobs::Target::to("hands", None));
                let app = app.clone();
                job.on_cancel(move || cancel_all(&app));
                job
            });
            if let Some(share) = p.share {
                job.progress(share);
            }
            job.detail(format!("{} · {}", p.title, p.detail));
        }
        JobState::Done | JobState::Waiting => {
            if let Some(job) = slot.take() {
                job.finish();
            }
        }
        JobState::Cancelled => {
            if let Some(job) = slot.take() {
                job.stopped();
            }
        }
        JobState::Failed => {
            if let Some(job) = slot.take() {
                job.fail(p.detail.clone());
            }
        }
    }
}

#[derive(Default)]
pub struct AnalysisState {
    status: Mutex<AnalysisStatus>,
    wake: Condvar,
    woken: Mutex<bool>,
    /// Stops the running solve so a marked hand goes first; the job stays queued.
    preempt: AtomicBool,
    /// Hands up to this id are analysed; reset when the number of hands changes, since an
    /// import can add older hands.
    cursor: Mutex<(u64, i64)>,
    /// Hands per second of the last preflop batch, as f64 bits.
    preflop_rate: AtomicU64,
    recounted: Mutex<Option<Instant>>,
    /// Seconds of solved spots so far, and how many.
    solved: Mutex<(f64, u32)>,
    /// The entry in the task center while the analysis runs.
    job: Mutex<Option<jobs::Job>>,
    auto: Mutex<Auto>,
}

impl AnalysisState {
    fn poke(&self) {
        *self.woken.lock().unwrap() = true;
        self.wake.notify_all();
    }

    fn sleep(&self, d: Duration) {
        let guard = self.woken.lock().unwrap();
        let (mut guard, _) = self.wake.wait_timeout_while(guard, d, |w| !*w).unwrap();
        *guard = false;
    }
}

/// Half the cores at most, so the app stays usable while the analysis runs.
fn threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get()).div_ceil(2).clamp(1, 8)
}

/// The preflop batches run here instead of on rayon's pool, which takes every core.
static POOL: std::sync::LazyLock<Option<rayon::ThreadPool>> =
    std::sync::LazyLock::new(|| rayon::ThreadPoolBuilder::new().num_threads(threads()).thread_name(|i| format!("analysis-{i}")).build().ok());

fn client_running(app: &AppHandle) -> bool {
    app.state::<AppState>().client_running.load(Ordering::Relaxed)
}

fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().db.lock().unwrap().setting(ENABLED_KEY).ok().flatten().is_some_and(|v| v == "1")
}

fn eta(state: &AnalysisState, counts: &AnalysisCounts) -> Option<f64> {
    let rate = f64::from_bits(state.preflop_rate.load(Ordering::Relaxed));
    let left = (counts.hands - counts.analyzed).max(0) as f64;
    let preflop = if left == 0.0 { 0.0 } else if rate > 0.0 { left / rate } else { return None };
    let (secs, n) = *state.solved.lock().unwrap();
    let per = if n > 0 { secs / n as f64 } else { DEFAULT_SOLVE_SECS };
    Some(preflop + counts.queued as f64 * per)
}

/// Counting holds the database; while a run goes on, a few times a minute is enough.
const RECOUNT_EVERY: Duration = Duration::from_secs(2);

/// Refreshes the counters (at most every [`RECOUNT_EVERY`] unless `force`) and sends the
/// status to the page.
fn publish(app: &AppHandle, force: bool, f: impl FnOnce(&mut AnalysisStatus)) {
    let analysis = app.state::<AnalysisState>();
    let due = {
        let mut last = analysis.recounted.lock().unwrap();
        let due = force || last.is_none_or(|t| t.elapsed() >= RECOUNT_EVERY);
        if due {
            *last = Some(Instant::now());
        }
        due
    };
    let counts = due.then(|| app.state::<AppState>().db.lock().unwrap().analysis_counts(ENGINE));
    let mut status = analysis.status.lock().unwrap();
    f(&mut status);
    match counts {
        Some(Ok(c)) => status.counts = c,
        Some(Err(e)) => status.last_error = Some(e.to_string()),
        None => {}
    }
    status.eta_secs = eta(&analysis, &status.counts);
    status.threads = threads();
    report(app, &mut status);
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let cancelled = setting(&app, CANCELLED_KEY).is_some_and(|v| v == "1");
        *app.state::<AnalysisState>().auto.lock().unwrap() = Auto::new(cancelled);
        loop {
            let state = app.state::<AnalysisState>();
            let (was, was_on) = {
                let s = state.status.lock().unwrap();
                (s.state, s.enabled)
            };
            let client = client_running(&app);
            let auto = auto_step(&app, client);
            if client {
                // The overlays need the database while PokerStars runs, so only the change is sent.
                if was != "locked" {
                    publish(&app, true, |s| {
                        s.state = "locked";
                        s.solving = None;
                    });
                }
                state.sleep(Duration::from_secs(2));
                continue;
            }
            if let Some(after) = auto {
                let scope = Scope::After(after);
                if preflop_batch(&app, scope) || solve_next(&app, scope) {
                    continue;
                }
                auto_finish(&app, after);
            }
            let on = enabled(&app);
            if on && preflop_batch(&app, Scope::All) {
                continue;
            }
            if solve_next(&app, if on { Scope::All } else { Scope::Marked }) {
                continue;
            }
            if was != "idle" || was_on != on {
                publish(&app, true, |s| {
                    s.state = "idle";
                    s.enabled = on;
                    s.solving = None;
                });
            }
            state.sleep(Duration::from_secs(5));
        }
    });
}

fn setting(app: &AppHandle, key: &str) -> Option<String> {
    app.state::<AppState>().db.lock().unwrap().setting(key).ok().flatten()
}

fn store(app: &AppHandle, key: &str, value: &str) {
    if let Err(e) = app.state::<AppState>().db.lock().unwrap().set_setting(key, value) {
        eprintln!("analysis: saving {key}: {e}");
    }
}

/// Feeds the automatic run with what PokerStars does. Returns the id after which hands
/// are new while the run may calculate.
fn auto_step(app: &AppHandle, client: bool) -> Option<u64> {
    let on = app.state::<AppState>().settings.lock().unwrap().analysis.auto;
    let analysis = app.state::<AnalysisState>();
    let (edges, may) = {
        let mut auto = analysis.auto.lock().unwrap();
        let edges = auto.observe(client, on);
        (edges, auto.may_compute())
    };
    for edge in edges {
        match edge {
            Edge::OpenedIdle => {
                let last = app.state::<AppState>().db.lock().unwrap().max_hand_id().ok().flatten();
                store(app, AFTER_KEY, &last.unwrap_or(0).to_string());
            }
            Edge::CancelLifted => store(app, CANCELLED_KEY, "0"),
            Edge::Started => {}
        }
    }
    // Without a mark nothing is new yet: it is set the first time PokerStars is seen.
    let after = may.then(|| setting(app, AFTER_KEY).and_then(|v| v.parse().ok())).flatten();
    if may && after.is_none() {
        analysis.auto.lock().unwrap().finish();
    }
    let running = analysis.auto.lock().unwrap().phase() == Phase::Running;
    if analysis.status.lock().unwrap().auto_run != running {
        let mut status = analysis.status.lock().unwrap();
        status.auto_run = running;
        report(app, &mut status);
    }
    after
}

/// Ends the automatic run once nothing of it is left: the hands rated are no longer new,
/// and if there were any the page hears how many.
fn auto_finish(app: &AppHandle, after: u64) {
    let analysis = app.state::<AnalysisState>();
    if !analysis.auto.lock().unwrap().finish() {
        return;
    }
    let main = app.state::<AppState>();
    let since = {
        let db = main.db.lock().unwrap();
        db.analysis_since(ENGINE, after).map(|s| (s, db.finished_tournament_after(after).ok().flatten()))
    };
    match since {
        Ok((since, tournament)) => {
            if let Some(last) = since.last_hand {
                store(app, AFTER_KEY, &last.to_string());
            }
            if since.hands > 0 {
                let job = analysis.job.lock().unwrap().as_ref().map(jobs::Job::id);
                let _ = app.emit("analysis-auto-done", AutoDone { hands: since.hands, mistakes: since.mistakes, job, tournament });
            }
            publish(app, true, |s| s.auto_run = false);
        }
        Err(e) => publish(app, true, |s| {
            s.auto_run = false;
            s.last_error = Some(e.to_string());
        }),
    }
}

/// How far a preflop batch got.
#[derive(Debug, Default, PartialEq)]
struct Pieces {
    /// Hands analysed and saved.
    done: usize,
    /// The id of the last of them.
    last: Option<u64>,
    /// Ended early because `stop` said so.
    stopped: bool,
    error: Option<String>,
}

/// Analyses `rows` (by id) `piece` hands at a time and saves each piece. Before each piece
/// it asks `stop`, so nothing calculates once PokerStars runs; what is saved stays.
fn analyze_pieces(main: &AppState, rows: &[(u64, String)], piece: usize, stop: impl Fn() -> bool) -> Pieces {
    let mut out = Pieces::default();
    for chunk in rows.chunks(piece.max(1)) {
        if stop() {
            out.stopped = true;
            break;
        }
        let results = match POOL.as_ref() {
            Some(pool) => pool.install(|| analyze_raw(chunk)),
            None => analyze_raw(chunk),
        };
        if let Err(e) = main.db.lock().unwrap().save_analysis(&results) {
            out.error = Some(e.to_string());
            break;
        }
        out.done += chunk.len();
        out.last = chunk.last().map(|(id, _)| *id);
    }
    out
}

/// Analyses the next batch of hands in `scope`; false when none is left. True as well when
/// PokerStars stopped it, so the caller does not take the rest for done.
fn preflop_batch(app: &AppHandle, scope: Scope) -> bool {
    let main = app.state::<AppState>();
    let analysis = app.state::<AnalysisState>();
    let rows = {
        let db = main.db.lock().unwrap();
        let after = if scope == Scope::All {
            let mut cursor = analysis.cursor.lock().unwrap();
            let total = db.hand_count().unwrap_or(0);
            if total != cursor.1 {
                *cursor = (0, total);
            }
            cursor.0
        } else {
            scope.after()
        };
        db.analysis_candidates(ENGINE, after, BATCH)
    };
    let rows = match rows {
        Ok(rows) => rows,
        Err(e) => {
            publish(app, true, |s| s.last_error = Some(e.to_string()));
            return false;
        }
    };
    if rows.is_empty() {
        return false;
    }
    let started = Instant::now();
    let pieces = analyze_pieces(&main, &rows, PIECE, || client_running(app));
    // Only past what is saved: the rest comes with the next batch.
    if scope == Scope::All
        && let Some(last) = pieces.last
    {
        let mut cursor = analysis.cursor.lock().unwrap();
        cursor.0 = cursor.0.max(last);
    }
    if pieces.done > 0 {
        let rate = pieces.done as f64 / started.elapsed().as_secs_f64().max(1e-3);
        analysis.preflop_rate.store(rate.to_bits(), Ordering::Relaxed);
    }
    if pieces.stopped {
        return true;
    }
    publish(app, false, |s| {
        s.state = "preflop";
        if let Some(e) = &pieces.error {
            s.last_error = Some(e.clone());
        }
    });
    pieces.error.is_none()
}

/// How a spot of the analysis is listed in "Meine Lösungen".
fn save_info(job: &PostflopJob) -> SaveInfo {
    let [oop, ip] = job.positions.clone();
    let notes: Vec<&str> = job.range_notes.iter().map(String::as_str).filter(|n| !n.is_empty()).collect();
    SaveInfo {
        title: format!("{ip} gegen {oop}"),
        oop,
        ip,
        ranges: if notes.is_empty() { "aus dem Preflop-Modell".into() } else { notes.join(" · ") },
        source: Some(SolutionSource::Hand),
        hand_id: Some(job.hand_id),
        model: job.model.clone(),
    }
}

/// What rating a job gave: the decisions, and whether nothing was solved.
type Rated = Result<(Vec<Decision>, bool), JobError>;

/// Rates `job` from the spot store (`store::resolve`): an own solve as exact as a queued
/// one, if every decision lies in its saved part; a library spot or a spot Ringside Cloud
/// serves (accepted on its form or verified) if every hero decision is on the flop (they store the flop per hand, but
/// no EV per action, so the rating goes by frequency). Otherwise it solves with `solve`,
/// files the solution in the store under `data` and offers it to the cloud, or, if this
/// computer cannot solve it, asks the community to. The flag is true when nothing was
/// solved. Without `app` (tests) the cloud is left out.
fn rate_job(data: &Path, state: &AppState, job: &PostflopJob, app: Option<&AppHandle>, solve: impl FnOnce() -> Result<Solution, JobError>) -> Rated {
    let keyed = store::keyed(&job.spot, job.model.as_ref()).ok_or_else(|| JobError::Failed("Spot unlesbar".into()))?;
    let policy = match app {
        Some(app) => store::policy(app, true, true),
        None => Policy { solve: true, others: true, ..Policy::default() },
    };
    let flop_only = job.actions.iter().filter(|a| a.hero.is_some()).all(|a| a.street == Street::Flop);
    let use_found = |found: Found| -> Option<Rated> {
        let note = match found.origin() {
            Origin::Own if found.saved_id().is_some() => None,
            Origin::Cloud => Some(" Lösung aus Ringside Cloud, von der Community gerechnet; ohne EV je Aktion, bewertet nach Häufigkeit."),
            _ => Some(" Lösung aus der Spot-Bibliothek; ohne EV je Aktion, bewertet nach Häufigkeit."),
        };
        if note.is_some() && !flop_only {
            return None;
        }
        let mut decisions = rate(job, &found.summary(), found.saved_id(), |path| found.node(path)).ok()?;
        for d in decisions.iter_mut().filter(|_| note.is_some()) {
            d.note.push_str(note.unwrap_or_default());
        }
        Some(Ok((decisions, true)))
    };
    let solve_and_save = || -> Option<Rated> {
        let mut solution = match solve() {
            Ok(s) => s,
            Err(e) => return Some(Err(e)),
        };
        let saved = store::save_solution(data, state, &mut solution, &settings(threads()), &save_info(job));
        // Not saved only costs the reuse; the rating stands either way.
        let saved = saved.inspect_err(|e| eprintln!("saving the analysis solve of hand {}: {e}", job.hand_id)).ok();
        if let Err(e) = store::prune_solutions(data, state, solutions::max_bytes(state)) {
            eprintln!("keeping the solutions within their limit: {e}");
        }
        if let (Some(app), Some((_, keyed))) = (app, &saved) {
            store::share(app, keyed, &mut solution);
        }
        let summary = solution.summary.clone();
        Some(rate(job, &summary, saved.map(|(id, _)| id), |path| solution.node(path)).map(|d| (d, false)))
    };
    let ask = Ask::new(&job.spot, &keyed, TARGET_PCT);
    let cloud = |a: &Ask| app.and_then(|app| store::find_cloud(app, data, a));
    let keep_waiting = || app.is_none_or(|app| !client_running(app));
    let rated = store::resolve((data, state), &ask, &policy, cloud, keep_waiting, use_found, solve_and_save);
    match rated {
        Some((_, Err(JobError::Failed(why)))) => {
            if let (Some(app), Some(ringside_store::Cloud::Spot(input))) = (app, &keyed.cloud) {
                crate::cloud::request_spot(app, input.clone());
            }
            Err(JobError::Failed(why))
        }
        Some((_, rated)) => rated,
        // PokerStars opened while another area solved the same spot.
        None => Err(JobError::Cancelled),
    }
}

/// Solves the next queued spot in `scope`; false when there is nothing to do.
fn solve_next(app: &AppHandle, scope: Scope) -> bool {
    let main = app.state::<AppState>();
    let state = app.state::<AnalysisState>();
    let next = main.db.lock().unwrap().next_postflop_job(scope.after());
    let Ok(Some((hand_id, json))) = next else { return false };
    state.preempt.store(false, Ordering::SeqCst);
    let job: Result<PostflopJob, _> = serde_json::from_str(&json);
    let result = match (job, store::data(app)) {
        (Ok(job), Ok(data)) => {
            let started = Instant::now();
            let mut last: Option<Instant> = None;
            let progress_app = app.clone();
            let priority = {
                let status = state.status.lock().unwrap();
                match scope {
                    _ if status.marked.contains(&hand_id) => Priority::Viewed,
                    Scope::After(_) => Priority::NewHands,
                    Scope::All | Scope::Marked => Priority::History,
                }
            };
            let solve = || {
                let ticket = store::gate().enter(priority);
                while !ticket.hold(Duration::from_secs(1)) {
                    if client_running(app) || state.preempt.load(Ordering::Relaxed) {
                        return Err(JobError::Cancelled);
                    }
                }
                solve_spot(
                    &job,
                    threads(),
                    crate::solver::memory_limit() / 2,
                    move |p| {
                        if last.is_none_or(|t| t.elapsed() >= PROGRESS_EVERY) {
                            last = Some(Instant::now());
                            let solving = Solving {
                                hand_id,
                                iteration: p.iteration,
                                max_iterations: p.max_iterations,
                                exploitability_pct: p.exploitability_pct,
                                elapsed_ms: p.elapsed_ms,
                            };
                            let state = progress_app.state::<AnalysisState>();
                            let mut status = state.status.lock().unwrap();
                            status.state = "postflop";
                            status.solving = Some(solving);
                            report(&progress_app, &mut status);
                        }
                    },
                    || client_running(app) || state.preempt.load(Ordering::Relaxed) || ticket.should_yield(),
                )
            };
            let result = rate_job(&data, &main, &job, Some(app), solve);
            if let Ok((_, false)) = result {
                let mut solved = state.solved.lock().unwrap();
                solved.0 += started.elapsed().as_secs_f64();
                solved.1 += 1;
            }
            result.map(|(decisions, _)| decisions)
        }
        (Err(e), _) => Err(JobError::Failed(format!("Gespeicherter Spot unlesbar: {e}"))),
        (_, Err(e)) => Err(JobError::Failed(e)),
    };
    let (decisions, why) = match result {
        Ok(decisions) => (decisions, String::new()),
        Err(JobError::Cancelled) => return true,
        Err(JobError::Failed(reason)) => (pending(&main, hand_id), reason),
        Err(JobError::NotStored) => (pending(&main, hand_id), NOT_STORED.to_string()),
    };
    // What the solver leaves without a rating, the estimate rates.
    let hand = main.db.lock().unwrap().hand(hand_id).ok().flatten();
    let decisions = match hand {
        Some(hand) => estimate_rest(&hand, decisions, &format!("Postflop-Solve nicht möglich: {why}")),
        None => unrated(decisions, &why),
    };
    let saved = main.db.lock().unwrap().finish_postflop(hand_id, &decisions);
    publish(app, true, |s| {
        s.marked.retain(|&h| h != hand_id);
        s.solving = None;
        if let Err(e) = &saved {
            s.last_error = Some(e.to_string());
        }
    });
    let _ = app.emit("analysis-hand", hand_id);
    true
}

/// The pending decisions of a hand.
fn pending(main: &AppState, hand_id: u64) -> Vec<Decision> {
    let list = main.db.lock().unwrap().hand_decisions(hand_id).unwrap_or_default();
    list.into_iter().filter(|d| d.status == Status::Pending).collect()
}

/// Pending decisions of a hand that is gone, stored as not rated with `reason`.
fn unrated(decisions: Vec<Decision>, reason: &str) -> Vec<Decision> {
    decisions
        .into_iter()
        .map(|mut d| {
            if d.status == Status::Pending {
                d.status = Status::Unrated;
                d.reason = Some(format!("Postflop-Solve nicht möglich: {reason}"));
            }
            d
        })
        .collect()
}

#[tauri::command]
pub async fn analysis_status(app: AppHandle) -> CmdResult<AnalysisStatus> {
    let handle = app.clone();
    blocking(app, move |state| {
        let counts = state.db.lock().unwrap().analysis_counts(ENGINE).map_err(err)?;
        let analysis = handle.state::<AnalysisState>();
        let mut status = analysis.status.lock().unwrap().clone();
        status.eta_secs = eta(&analysis, &counts);
        status.counts = counts;
        status.enabled = enabled(&handle);
        status.threads = threads();
        status.progress = job_progress(&status);
        Ok(status)
    })
    .await
}

/// Starts (`true`) or pauses the analysis of the whole history.
#[tauri::command]
pub async fn analysis_run(on: bool, app: AppHandle) -> CmdResult<()> {
    let handle = app.clone();
    blocking(app, move |state| {
        state.db.lock().unwrap().set_setting(ENABLED_KEY, if on { "1" } else { "0" }).map_err(err)?;
        let analysis = handle.state::<AnalysisState>();
        let mut status = analysis.status.lock().unwrap();
        if !on && status.solving.as_ref().is_some_and(|s| !status.marked.contains(&s.hand_id)) {
            analysis.preempt.store(true, Ordering::SeqCst);
        }
        status.enabled = on;
        if on {
            status.cancelled = false;
            status.last_error = None;
        }
        report(&handle, &mut status);
        drop(status);
        analysis.poke();
        Ok(())
    })
    .await
}

/// Stops the whole analysis: the run pauses, hands asked for go back in line and a
/// running solve stops (its spot stays queued). What is rated stays stored.
#[tauri::command]
pub async fn analysis_cancel(app: AppHandle) -> CmdResult<()> {
    let handle = app.clone();
    blocking(app, move |_| {
        cancel_all(&handle);
        Ok(())
    })
    .await
}

/// Stops the whole analysis, from the page or the task center. The automatic run rests
/// until PokerStars closes the next time.
fn cancel_all(app: &AppHandle) {
    {
        let main = app.state::<AppState>();
        let db = main.db.lock().unwrap();
        // Best effort: the run stops either way; a failed write shows up as the next error.
        let _ = db.set_setting(ENABLED_KEY, "0");
        let _ = db.set_setting(CANCELLED_KEY, "1");
        let _ = db.unmark_all_postflop();
    }
    let analysis = app.state::<AnalysisState>();
    analysis.auto.lock().unwrap().cancel();
    let mut status = analysis.status.lock().unwrap();
    analysis.preempt.store(status.solving.is_some(), Ordering::SeqCst);
    status.enabled = false;
    status.auto_run = false;
    status.marked.clear();
    status.cancelled = true;
    report(app, &mut status);
    drop(status);
    analysis.poke();
}

/// Switches the automatic run over new hands on or off; works while PokerStars runs, it
/// only changes the setting. Off stops a running solve of the run unless the bulk run or
/// the user asked for that spot.
#[tauri::command]
pub async fn analysis_set_auto(on: bool, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.analysis.auto = on;
    store_settings(&app, &state, settings)?;
    let analysis = app.state::<AnalysisState>();
    let status = analysis.status.lock().unwrap();
    if !on && !status.enabled && status.solving.as_ref().is_some_and(|s| !status.marked.contains(&s.hand_id)) {
        analysis.preempt.store(true, Ordering::SeqCst);
    }
    drop(status);
    analysis.poke();
    Ok(())
}

/// Stops the analysis of one hand: its postflop spot goes back in line (solved later by
/// the bulk run, if that is on) and a running solve of it stops.
#[tauri::command]
pub async fn analysis_cancel_hand(hand_id: u64, app: AppHandle) -> CmdResult<()> {
    let handle = app.clone();
    blocking(app, move |state| {
        state.db.lock().unwrap().unmark_postflop(hand_id).map_err(err)?;
        let analysis = handle.state::<AnalysisState>();
        let mut status = analysis.status.lock().unwrap();
        status.marked.retain(|&h| h != hand_id);
        if status.solving.as_ref().is_some_and(|s| s.hand_id == hand_id) {
            analysis.preempt.store(true, Ordering::SeqCst);
        }
        report(&handle, &mut status);
        drop(status);
        analysis.poke();
        Ok(())
    })
    .await
}

/// Analyses hands now (one from the replayer, or a whole tournament) and puts their
/// postflop spots at the front of the queue. Returns the first hand's decisions as far as
/// they are rated already.
#[tauri::command]
pub async fn analysis_hands(hand_ids: Vec<u64>, app: AppHandle) -> CmdResult<Vec<Decision>> {
    let handle = app.clone();
    blocking(app, move |state| {
        let analysis = handle.state::<AnalysisState>();
        let mut fresh = Vec::new();
        for &id in &hand_ids {
            let db = state.db.lock().unwrap();
            let stored = db.hand_decisions(id).map_err(err)?;
            if (stored.is_empty() || stored.iter().any(|d| d.engine != ENGINE))
                && let Some(hand) = db.hand(id).map_err(err)?
            {
                fresh.push(analyze_hand(&hand));
            }
        }
        let mut db = state.db.lock().unwrap();
        db.save_analysis(&fresh).map_err(err)?;
        let mut marked = Vec::new();
        for &id in &hand_ids {
            if db.mark_postflop(id).map_err(err)? {
                marked.push(id);
            }
        }
        if !marked.is_empty() {
            let mut status = analysis.status.lock().unwrap();
            status.cancelled = false;
            for id in marked {
                if !status.marked.contains(&id) {
                    status.marked.push(id);
                }
            }
            if status.solving.as_ref().is_some_and(|s| !status.marked.contains(&s.hand_id)) {
                analysis.preempt.store(true, Ordering::SeqCst);
            }
            drop(status);
            analysis.poke();
        }
        match hand_ids.first() {
            Some(&id) => db.hand_decisions(id).map_err(err),
            None => Ok(Vec::new()),
        }
    })
    .await
}

/// The stored decisions of a hand; empty when it was not analysed yet.
#[tauri::command]
pub async fn analysis_decisions(hand_id: u64, app: AppHandle) -> CmdResult<Vec<Decision>> {
    blocking(app, move |state| state.db.lock().unwrap().hand_decisions(hand_id).map_err(err)).await
}

/// The chart spot a rated preflop decision was measured against, for "In den Charts
/// öffnen"; None when no bundled table fits.
#[tauri::command]
pub async fn analysis_chart_target(hand_id: u64, action_index: usize, app: AppHandle) -> CmdResult<Option<ChartTarget>> {
    blocking(app, move |state| {
        let hand = state.db.lock().unwrap().hand(hand_id).map_err(err)?;
        Ok(hand.and_then(|h| chart_targets(&h).into_iter().find(|t| t.action_index == action_index)))
    })
    .await
}

#[tauri::command]
pub async fn analysis_summary(filter: Filter, app: AppHandle) -> CmdResult<AnalysisSummary> {
    blocking(app, move |state| state.db.lock().unwrap().analysis_summary(&filter, ENGINE, 5).map_err(err)).await
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use ringside_analysis::postflop::JobAction;
    use ringside_core::db::SpotType;
    use ringside_core::model::{ActionKind, Street};
    use ringside_solver::{Settings, Spot, StreetSizes, solve};

    use super::*;
    use crate::solutions::test_state;

    /// The hero on the button with AhKh against the big blind; tiny ranges and stacks so a
    /// solve takes milliseconds.
    fn job(hand_id: u64, board: &[&str], actions: &[(Street, bool, ActionKind, f64, Option<SpotType>)]) -> PostflopJob {
        let sizes = StreetSizes { bet: vec![50.0], raise: vec![] };
        PostflopJob {
            hand_id,
            spot: Spot {
                oop_range: "QQ,77,KJs".into(),
                ip_range: "AA,AKs,T9s".into(),
                board: "Kd7s2c".into(),
                pot: 10.0,
                stack: 10.0,
                sizes: [sizes.clone(), sizes.clone(), sizes],
                allin_threshold: 150.0,
            },
            hero_oop: false,
            hero_combo: "AhKh".into(),
            board: board.iter().map(|c| c.to_string()).collect(),
            actions: actions
                .iter()
                .enumerate()
                .map(|(index, &(street, oop, kind, to_bb, hero))| JobAction {
                    index,
                    street,
                    oop,
                    kind,
                    to_bb,
                    all_in: false,
                    hero: hero.map(|s| (s, format!("{kind:?}"))),
                })
                .collect(),
            positions: ["BB".into(), "BTN".into()],
            range_notes: ["BB: Call".into(), "BTN: Open".into()],
            players: 6,
            note: String::new(),
            pot_bb: 20.0,
            model: None,
        }
    }

    fn flop_cbet(hand_id: u64) -> PostflopJob {
        job(
            hand_id,
            &["Kd", "7s", "2c"],
            &[(Street::Flop, true, ActionKind::Check, 0.0, None), (Street::Flop, false, ActionKind::Bet, 5.0, Some(SpotType::CBet))],
        )
    }

    fn folder(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ringside-analysis-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn fresh(job: &PostflopJob, calls: &Cell<u32>) -> Result<Solution, JobError> {
        calls.set(calls.get() + 1);
        solve(&job.spot, &settings(1), 1 << 30, |_| {}, || false).map_err(|e| JobError::Failed(e.to_string()))
    }

    fn link(d: &Decision) -> serde_json::Value {
        serde_json::from_str(d.solver.as_deref().expect("rated decisions link the solver")).unwrap()
    }

    #[test]
    fn pokerstars_opening_stops_a_batch_after_the_running_piece_and_loses_no_hand() {
        let state = test_state();
        let fixtures = ["cash.txt", "knockout.txt", "tournament.txt"]
            .map(|f| std::fs::read_to_string(format!("{}/../crates/ringside-core/tests/fixtures/{f}", env!("CARGO_MANIFEST_DIR"))).unwrap());
        let hands: Vec<_> = fixtures
            .iter()
            .flat_map(|text| ringside_core::split_hands(text))
            .map(|raw| (ringside_core::parse_hand(raw).unwrap(), raw.to_string()))
            .collect();
        assert_eq!(hands.len(), 5);
        state.db.lock().unwrap().insert_hands(&hands).unwrap();
        let candidates = |after: u64| state.db.lock().unwrap().analysis_candidates(ENGINE, after, BATCH).unwrap();
        let analyzed = || state.db.lock().unwrap().analysis_counts(ENGINE).unwrap().analyzed;
        let rows = candidates(0);
        let ids: Vec<u64> = rows.iter().map(|(id, _)| *id).collect();

        // PokerStars runs from the start: nothing is calculated.
        state.client_running.store(true, Ordering::SeqCst);
        let none = analyze_pieces(&state, &rows, 2, || state.client_running.load(Ordering::SeqCst));
        assert_eq!(none, Pieces { stopped: true, ..Default::default() });
        assert_eq!(analyzed(), 0);

        // PokerStars opens while the first piece is calculated: only that piece is done.
        state.client_running.store(false, Ordering::SeqCst);
        let first = analyze_pieces(&state, &rows, 2, || state.client_running.swap(true, Ordering::SeqCst));
        assert_eq!(first, Pieces { done: 2, last: Some(ids[1]), stopped: true, error: None });
        assert_eq!(analyzed(), 2);
        // The cursor goes to the last calculated hand; from there, and in the automatic run's
        // scope (the unanalysed hands after its mark), the rest is still waiting.
        let rest: Vec<u64> = ids[2..].to_vec();
        assert_eq!(candidates(first.last.unwrap()).iter().map(|(id, _)| *id).collect::<Vec<_>>(), rest);
        assert_eq!(candidates(0).iter().map(|(id, _)| *id).collect::<Vec<_>>(), rest);

        // The next run after PokerStars closed catches up.
        state.client_running.store(false, Ordering::SeqCst);
        let next = analyze_pieces(&state, &candidates(first.last.unwrap()), 2, || state.client_running.load(Ordering::SeqCst));
        assert_eq!(next, Pieces { done: 3, last: Some(ids[4]), stopped: false, error: None });
        assert_eq!(analyzed(), 5);
        assert!(candidates(0).is_empty());
    }

    #[test]
    fn a_spot_solved_on_the_solver_page_is_rated_without_solving() {
        let (dir, state) = (folder("reuse"), test_state());
        let job = flop_cbet(1);
        // As the solver page saves it: stricter than the queue, from the user's own setup.
        let page = Settings { target_pct: 0.5, max_iterations: 1000, threads: 1 };
        let mut solution = solve(&job.spot, &page, 1 << 30, |_| {}, || false).unwrap();
        let info = SaveInfo { source: Some(SolutionSource::Own), ..Default::default() };
        let (id, _) = store::save_solution(&dir, &state, &mut solution, &page, &info).unwrap();

        let (decisions, reused) = rate_job(&dir, &state, &job, None, || panic!("the saved solution is used, nothing is solved")).unwrap();
        assert!(reused);
        let [d] = decisions.as_slice() else { panic!("{decisions:?}") };
        assert_eq!(d.status, Status::Rated, "{d:?}");
        assert_eq!(link(d)["saved"], id);
        assert_eq!(state.db.lock().unwrap().solutions().unwrap().len(), 1, "reusing saves nothing new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_analysis_spot_lands_in_my_solutions_and_is_reused_next_time() {
        let (dir, state) = (folder("save"), test_state());
        let calls = Cell::new(0);
        let job = flop_cbet(42);
        let (first, reused) = rate_job(&dir, &state, &job, None, || fresh(&job, &calls)).unwrap();
        assert!(!reused);
        assert_eq!(calls.get(), 1);

        let saved = state.db.lock().unwrap().solutions().unwrap();
        let [row] = saved.as_slice() else { panic!("{saved:?}") };
        assert_eq!((row.info.source, row.info.hand_id), (SolutionSource::Hand, Some(42)));
        assert_eq!((row.info.title.as_str(), row.info.oop.as_str(), row.info.ip.as_str()), ("BTN gegen BB", "BB", "BTN"));
        assert_eq!(link(&first[0])["saved"], row.id);

        let (again, reused) = rate_job(&dir, &state, &job, None, || fresh(&job, &calls)).unwrap();
        assert!(reused && calls.get() == 1);
        assert_eq!(again[0].strategy.len(), first[0].strategy.len());
        assert_eq!(again[0].best, first[0].best);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_decision_past_the_saved_part_is_solved_again() {
        let (dir, state) = (folder("deeper"), test_state());
        let calls = Cell::new(0);
        let first = flop_cbet(7);
        rate_job(&dir, &state, &first, None, || fresh(&first, &calls)).unwrap();
        // Same spot, but the hero acts after the big blind on the turn: the snapshot keeps
        // only the turn's first decision.
        let turn = job(
            8,
            &["Kd", "7s", "2c", "3h"],
            &[
                (Street::Flop, true, ActionKind::Check, 0.0, None),
                (Street::Flop, false, ActionKind::Check, 0.0, Some(SpotType::Flop)),
                (Street::Turn, true, ActionKind::Check, 0.0, None),
                (Street::Turn, false, ActionKind::Bet, 5.0, Some(SpotType::Turn)),
            ],
        );
        let keyed = store::keyed(&turn.spot, None).unwrap();
        let found = store::find_local(&dir, &state, &Ask::new(&turn.spot, &keyed, TARGET_PCT), true).expect("the flop spot is saved");
        let from_snapshot = rate(&turn, &found.summary(), None, |path| found.node(path));
        assert!(matches!(from_snapshot, Err(JobError::NotStored)), "{from_snapshot:?}");
        let (decisions, reused) = rate_job(&dir, &state, &turn, None, || fresh(&turn, &calls)).unwrap();
        assert!(!reused);
        assert_eq!(calls.get(), 2);
        assert!(decisions.iter().all(|d| d.status == Status::Rated), "{decisions:?}");
        let path = &link(&decisions[1])["path"];
        assert_eq!(path.as_array().unwrap().len(), 4, "{path}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two analysis runs over the user's own hand histories, kept outside the repository:
    /// `RINGSIDE_REAL_HH=/path/to/HandHistory/<name> cargo test --release -p ringside --lib real_second_run -- --ignored --nocapture`.
    /// The first run solves the spots estimated below `RINGSIDE_REAL_MAX_SECS` (default
    /// 60), the second reuses what it saved. Also says how close the library comes.
    #[test]
    #[ignore = "needs RINGSIDE_REAL_HH"]
    fn real_second_run() {
        use ringside_solver::library::catalog::catalog;
        use ringside_solver::saved::spot_key;
        let hh = std::env::var("RINGSIDE_REAL_HH").expect("RINGSIDE_REAL_HH");
        let max_secs: f64 = std::env::var("RINGSIDE_REAL_MAX_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60.0);
        let mut jobs: Vec<PostflopJob> = Vec::new();
        for entry in std::fs::read_dir(&hh).unwrap() {
            let text = String::from_utf8_lossy(&std::fs::read(entry.unwrap().path()).unwrap()).into_owned();
            for hand in ringside_core::split_hands(&text).into_iter().filter_map(|raw| ringside_core::parse_hand(raw).ok()) {
                if let Some((_, json)) = analyze_hand(&hand).job {
                    jobs.push(serde_json::from_str(&json).unwrap());
                }
            }
        }
        // Stored: the flop, and on the turn only its first action.
        let in_saved_part = |j: &PostflopJob| {
            j.actions.iter().filter(|a| a.hero.is_some()).all(|a| match a.street {
                Street::Flop => true,
                Street::Turn => j.actions.iter().find(|x| x.street == Street::Turn).is_some_and(|x| x.index == a.index),
                _ => false,
            })
        };
        let fits = jobs.iter().filter(|j| in_saved_part(j)).count();
        println!("{} Spots, {fits} mit allen Entscheidungen im gespeicherten Teil (Flop + Turn-Einstieg)", jobs.len());

        let lib = catalog();
        let same = |a: f64, b: f64| (a - b).abs() < 0.005;
        let (mut depth, mut sizes, mut ranges) = (0, 0, 0);
        for j in &jobs {
            let near: Vec<_> = lib.scenarios.iter().filter(|s| same(s.pot, j.spot.pot) && same(s.stack, j.spot.stack)).collect();
            depth += usize::from(!near.is_empty());
            sizes += usize::from(lib.scenarios.iter().any(|s| s.sizes == j.spot.sizes && same(s.allin_threshold, j.spot.allin_threshold)));
            let key = |o: &str, i: &str| {
                spot_key(&Spot { oop_range: o.into(), ip_range: i.into(), pot: 1.0, stack: 1.0, ..j.spot.clone() }).ok()
            };
            let mine = key(&j.spot.oop_range, &j.spot.ip_range);
            ranges += usize::from(lib.scenarios.iter().any(|s| key(&s.oop_range, &s.ip_range) == mine));
        }
        println!("Bibliothek: Pot+Stack gleich {depth}, Größen gleich {sizes}, Ranges gleich {ranges} von {}", jobs.len());

        let threads = threads();
        let small: Vec<&PostflopJob> = jobs
            .iter()
            .filter(|j| ringside_solver::estimate(&j.spot, 8 << 30, threads).is_ok_and(|e| e.fits && e.seconds <= max_secs))
            .collect();
        let (dir, state) = (folder("real"), test_state());
        let calls = Cell::new(0);
        for run in 1..=2 {
            let t = Instant::now();
            let before = calls.get();
            let mut reused = 0;
            for j in &small {
                let solve = || {
                    calls.set(calls.get() + 1);
                    solve_spot(j, threads, 8 << 30, |_| {}, || false)
                };
                if let Ok((_, true)) = rate_job(&dir, &state, j, None, solve) {
                    reused += 1;
                }
            }
            println!(
                "Lauf {run}: {} Spots (Schätzung bis {max_secs} s), {reused} wiederverwendet, {} gelöst, {:.1} s mit {threads} Threads",
                small.len(),
                calls.get() - before,
                t.elapsed().as_secs_f64()
            );
        }
        let bytes = state.db.lock().unwrap().solutions_bytes().unwrap();
        println!("Gespeichert: {:.1} MB", bytes as f64 / 1e6);
        let _ = std::fs::remove_dir_all(&dir);
    }

}
