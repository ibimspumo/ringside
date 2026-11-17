//! Hand analysis in the app: a background worker rates the hero's decisions of every
//! stored hand (preflop at once, in batches) and then works through the queue of
//! heads-up postflop spots, marked hands first, then the biggest pots.
//!
//! The bulk run starts with the first click on "Hände analysieren" and stays on, so new
//! hands are analysed as they are imported. A single hand asked for in the replayer is
//! analysed at once and its postflop spot jumps the queue, interrupting a running solve,
//! which stays queued. Like the trainer and the solver, everything is locked while
//! PokerStars runs; a running solve stops within an iteration and resumes later.
//!
//! The worker runs whatever page is open. Every change goes out through [`report`] as
//! `analysis-status` (all details) and `analysis-progress` ([`JobProgress`]: title,
//! share, time left and state), the one place an app-wide task list hooks in.
//! `analysis_cancel` stops everything, `analysis_cancel_hand` one hand.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use ringside_analysis::postflop::JobError;
use ringside_analysis::{ENGINE, PostflopJob, analyze_hand, analyze_raw, solve_job};
use ringside_core::db::{AnalysisCounts, AnalysisSummary, Decision, Filter, Status};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult, blocking, err, jobs};

const ENABLED_KEY: &str = "analysis_enabled";
const BATCH: usize = 2000;
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
    pub progress: JobProgress,
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
    let (state, title, share) = match s.state {
        "locked" => (JobState::PausedPokerstars, "Hand-Analyse pausiert, solange PokerStars läuft".to_string(), None),
        "preflop" => (
            JobState::Running,
            "Analysiere deine Hände".into(),
            (c.hands > 0).then(|| c.analyzed as f64 / c.hands as f64),
        ),
        "postflop" => {
            let hand = s.solving.as_ref().filter(|x| s.marked.contains(&x.hand_id));
            let title = match hand {
                Some(h) => format!("Rechne Hand #{} nach dem Flop", h.hand_id),
                None => "Rechne Flop-Spots deiner Hände".into(),
            };
            let share = match hand {
                Some(h) => Some(h.iteration as f64 / h.max_iterations.max(1) as f64),
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

fn threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get()).div_ceil(2).clamp(1, 8)
}

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
        loop {
            let state = app.state::<AnalysisState>();
            let (was, was_on) = {
                let s = state.status.lock().unwrap();
                (s.state, s.enabled)
            };
            if client_running(&app) {
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
            let on = enabled(&app);
            if on && preflop_batch(&app) {
                continue;
            }
            if solve_next(&app, on) {
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

/// Analyses the next batch of hands; false when none is left.
fn preflop_batch(app: &AppHandle) -> bool {
    let main = app.state::<AppState>();
    let analysis = app.state::<AnalysisState>();
    let rows = {
        let db = main.db.lock().unwrap();
        let mut cursor = analysis.cursor.lock().unwrap();
        let total = db.hand_count().unwrap_or(0);
        if total != cursor.1 {
            *cursor = (0, total);
        }
        let rows = db.analysis_candidates(ENGINE, cursor.0, BATCH);
        if let Ok(Some((last, _))) = rows.as_ref().map(|r| r.last()) {
            cursor.0 = *last;
        }
        rows
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
    let results = analyze_raw(&rows);
    let saved = main.db.lock().unwrap().save_analysis(&results);
    let rate = rows.len() as f64 / started.elapsed().as_secs_f64().max(1e-3);
    app.state::<AnalysisState>().preflop_rate.store(rate.to_bits(), Ordering::Relaxed);
    publish(app, false, |s| {
        s.state = "preflop";
        s.enabled = true;
        if let Err(e) = &saved {
            s.last_error = Some(e.to_string());
        }
    });
    saved.is_ok()
}

/// Rated decisions for `job` from a solution computed before (spot library, saved
/// solver solutions), so the queue never solves a spot twice. Neither store can be
/// searched by an arbitrary spot yet: the library holds only its catalog spots, whose
/// ranges (nine players, big-blind ante) differ from those of most hands.
fn stored_solution(_app: &AppHandle, _job: &PostflopJob) -> Option<Vec<Decision>> {
    None
}

/// Solves the next queued spot (only marked ones while the bulk run is off); false when
/// there is nothing to do.
fn solve_next(app: &AppHandle, bulk: bool) -> bool {
    let main = app.state::<AppState>();
    let state = app.state::<AnalysisState>();
    let next = main.db.lock().unwrap().next_postflop_job();
    let Ok(Some((hand_id, json))) = next else { return false };
    let marked = state.status.lock().unwrap().marked.contains(&hand_id);
    if !bulk && !marked {
        return false;
    }
    state.preempt.store(false, Ordering::SeqCst);
    let job: Result<PostflopJob, _> = serde_json::from_str(&json);
    let result = match job {
        Ok(job) if let Some(decisions) = stored_solution(app, &job) => Ok(decisions),
        Ok(job) => {
            let started = Instant::now();
            let mut last: Option<Instant> = None;
            let progress_app = app.clone();
            let result = solve_job(
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
                || client_running(app) || state.preempt.load(Ordering::Relaxed),
            );
            if result.is_ok() {
                let mut solved = state.solved.lock().unwrap();
                solved.0 += started.elapsed().as_secs_f64();
                solved.1 += 1;
            }
            result
        }
        Err(e) => Err(JobError::Failed(format!("Gespeicherter Spot unlesbar: {e}"))),
    };
    let decisions = match result {
        Ok(decisions) => decisions,
        Err(JobError::Cancelled) => return true,
        Err(JobError::Failed(reason)) => failed(&main, hand_id, &reason),
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

/// The pending decisions of a hand, stored as not rated with `reason`.
fn failed(main: &AppState, hand_id: u64, reason: &str) -> Vec<Decision> {
    let list = main.db.lock().unwrap().hand_decisions(hand_id).unwrap_or_default();
    list.into_iter()
        .filter(|d| d.status == Status::Pending)
        .map(|mut d| {
            d.status = Status::Unrated;
            d.reason = Some(format!("Postflop-Solve nicht möglich: {reason}"));
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

/// Stops the whole analysis, from the page or the task center.
fn cancel_all(app: &AppHandle) {
    {
        let main = app.state::<AppState>();
        let db = main.db.lock().unwrap();
        // Best effort: the run stops either way; a failed write shows up as the next error.
        let _ = db.set_setting(ENABLED_KEY, "0");
        let _ = db.unmark_all_postflop();
    }
    let analysis = app.state::<AnalysisState>();
    let mut status = analysis.status.lock().unwrap();
    analysis.preempt.store(status.solving.is_some(), Ordering::SeqCst);
    status.enabled = false;
    status.marked.clear();
    status.cancelled = true;
    report(app, &mut status);
    drop(status);
    analysis.poke();
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

#[tauri::command]
pub async fn analysis_summary(filter: Filter, app: AppHandle) -> CmdResult<AnalysisSummary> {
    blocking(app, move |state| state.db.lock().unwrap().analysis_summary(&filter, ENGINE, 5).map_err(err)).await
}
