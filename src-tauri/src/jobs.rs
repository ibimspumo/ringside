//! The task center: every longer calculation registers here, so the app shows one list
//! of what runs, how far it is, what is left and where its result lives, and can stop
//! any of it from anywhere. Tasks run on their own threads and keep running whichever
//! page is open. The list goes to the page as `jobs-changed`, at most four times a second.
//!
//! While PokerStars runs, `should_stop` turns true for every task, as the lock of the
//! calculating areas requires; a task that waits for the client to close instead shows
//! as paused.
//!
//! # Registering a task
//! A new kind of work adds a variant to [`Kind`] and its label in `src/lib/jobs/kinds.ts`.
//! ```ignore
//! let job = jobs::start(Kind::Analysis, "Hand #4711 rechnen", Target::to("hands", Some(4711)));
//! let stop = { let job = job.clone(); move || job.should_stop() };   // user or PokerStars
//! // in the progress callback:
//! job.progress(iteration as f64 / max as f64);   // or job.steps(done, total)
//! job.detail(format!("{iteration} Iterationen"));
//! // at the end (dropping the last clone counts as done, or as cancelled after a cancel):
//! job.finish();  job.fail("…");  job.stopped();
//! ```
//! A task whose calculation cannot stop calls `job.not_cancellable()`; one with its own
//! stop switch can add `job.on_cancel(|| …)`. `Target::to` takes the page ids of
//! `Main.svelte`; the id opens a hand (`hands`), a tournament (`coach`) or a saved
//! solution (`solver`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult};

const EMIT_EVERY: Duration = Duration::from_millis(250);
/// Finished tasks kept for the list until cleared.
const KEEP_FINISHED: usize = 30;
/// A remaining time is only guessed once this much is done and this long has passed.
const ETA_MIN_SHARE: f64 = 0.02;
const ETA_MIN_ELAPSED: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Solver,
    Preflop,
    PushFold,
    Library,
    Packs,
    Coach,
    Rebuild,
    /// Postflop spots for the practice with whole hands.
    Play,
    /// Rating the user's decisions in their hands (preflop batches, queued flop spots).
    Analysis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Running,
    /// Waits until PokerStars is closed.
    Paused,
    Done,
    Cancelled,
    Failed,
}

impl Status {
    fn active(self) -> bool {
        matches!(self, Status::Running | Status::Paused)
    }
}

/// Where a click on the task leads: a page of the app and, for some, what to open there
/// (a hand, a tournament, a saved solution).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Target {
    pub page: &'static str,
    pub id: Option<i64>,
}

impl Target {
    pub fn to(page: &'static str, id: Option<i64>) -> Target {
        Target { page, id }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct JobView {
    pub id: u64,
    pub kind: Kind,
    pub title: String,
    pub detail: Option<String>,
    pub status: Status,
    /// 0–1; `None` while nobody can say.
    pub progress: Option<f64>,
    pub steps: Option<(u64, u64)>,
    pub eta_secs: Option<f64>,
    pub elapsed_ms: u64,
    /// Unix milliseconds.
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub cancellable: bool,
    /// Asked to stop, not stopped yet.
    pub cancelling: bool,
    /// Ended because PokerStars started.
    pub by_client: bool,
    pub error: Option<String>,
    pub target: Target,
}

#[derive(Clone, Debug, Serialize)]
pub struct JobsView {
    pub jobs: Vec<JobView>,
    /// PokerStars runs; calculations wait or stop.
    pub locked: bool,
}

type Hook = Arc<dyn Fn() + Send + Sync>;

struct Entry {
    view: JobView,
    started: Instant,
    ended: Option<Duration>,
    cancel: Arc<AtomicBool>,
    on_cancel: Option<Hook>,
}

#[derive(Default)]
struct Inner {
    next: u64,
    jobs: Vec<Entry>,
    locked: bool,
    dirty: bool,
}

#[derive(Default)]
pub struct Registry {
    inner: Mutex<Inner>,
    /// Mirrors `Inner::locked` for `should_stop`, which runs inside solver loops.
    locked: AtomicBool,
}

fn unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Registry {
    pub fn start(self: &Arc<Self>, kind: Kind, title: impl Into<String>, target: Target) -> Job {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut inner = self.inner.lock().unwrap();
        inner.next += 1;
        let id = inner.next;
        inner.jobs.push(Entry {
            view: JobView {
                id,
                kind,
                title: title.into(),
                detail: None,
                status: Status::Running,
                progress: None,
                steps: None,
                eta_secs: None,
                elapsed_ms: 0,
                started_at: unix_ms(),
                finished_at: None,
                cancellable: true,
                cancelling: false,
                by_client: false,
                error: None,
                target,
            },
            started: Instant::now(),
            ended: None,
            cancel: cancel.clone(),
            on_cancel: None,
        });
        inner.dirty = true;
        Job(Arc::new(JobInner { id, registry: self.clone(), cancel }))
    }

    fn update(&self, id: u64, f: impl FnOnce(&mut Entry)) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(e) = inner.jobs.iter_mut().find(|e| e.view.id == id && e.view.status.active()) {
            f(e);
            inner.dirty = true;
        }
    }

    fn end(&self, id: u64, status: Status, error: Option<String>) {
        let locked = self.locked.load(Ordering::Relaxed);
        self.update(id, |e| {
            e.view.status = status;
            e.view.error = error;
            e.view.cancelling = false;
            e.view.by_client = status == Status::Cancelled && locked && !e.cancel.load(Ordering::Relaxed);
            e.view.finished_at = Some(unix_ms());
            e.ended = Some(e.started.elapsed());
            if status == Status::Done {
                e.view.progress = Some(1.0);
            }
        });
        let mut inner = self.inner.lock().unwrap();
        let finished = inner.jobs.iter().filter(|e| !e.view.status.active()).count();
        if finished > KEEP_FINISHED {
            let mut drop = finished - KEEP_FINISHED;
            inner.jobs.retain(|e| {
                let old = drop > 0 && !e.view.status.active();
                drop -= old as usize;
                !old
            });
        }
    }

    /// Asks a task to stop. Errors when it cannot.
    pub fn cancel(&self, id: u64) -> Result<(), String> {
        let hook = {
            let mut inner = self.inner.lock().unwrap();
            let e = inner
                .jobs
                .iter_mut()
                .find(|e| e.view.id == id && e.view.status.active())
                .ok_or("Diese Aufgabe läuft nicht mehr.")?;
            if !e.view.cancellable {
                return Err("Diese Aufgabe lässt sich nicht abbrechen.".into());
            }
            e.cancel.store(true, Ordering::SeqCst);
            e.view.cancelling = true;
            let hook = e.on_cancel.clone();
            inner.dirty = true;
            hook
        };
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }

    /// Forgets finished tasks.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.jobs.retain(|e| e.view.status.active());
        inner.dirty = true;
    }

    pub fn set_locked(&self, locked: bool) {
        self.locked.store(locked, Ordering::Relaxed);
        let mut inner = self.inner.lock().unwrap();
        if inner.locked != locked {
            inner.locked = locked;
            inner.dirty = true;
        }
    }

    pub fn view(&self) -> JobsView {
        let inner = self.inner.lock().unwrap();
        let jobs = inner
            .jobs
            .iter()
            .rev()
            .map(|e| {
                let mut v = e.view.clone();
                let elapsed = e.ended.unwrap_or_else(|| e.started.elapsed());
                v.elapsed_ms = elapsed.as_millis() as u64;
                if v.status.active() {
                    if inner.locked && v.status == Status::Running {
                        v.status = Status::Paused;
                    }
                    v.eta_secs = v
                        .progress
                        .filter(|share| (ETA_MIN_SHARE..1.0).contains(share) && elapsed >= ETA_MIN_ELAPSED)
                        .map(|share| elapsed.as_secs_f64() * (1.0 - share) / share);
                }
                v
            })
            .collect();
        JobsView { jobs, locked: inner.locked }
    }

    /// True once after any change.
    fn take_dirty(&self) -> bool {
        std::mem::take(&mut self.inner.lock().unwrap().dirty)
    }
}

struct JobInner {
    id: u64,
    registry: Arc<Registry>,
    cancel: Arc<AtomicBool>,
}

impl Drop for JobInner {
    fn drop(&mut self) {
        let status = if self.cancel.load(Ordering::Relaxed) { Status::Cancelled } else { Status::Done };
        self.registry.end(self.id, status, None);
    }
}

/// A registered task. Clones share it; the task ends with `finish`, `fail`, `stopped` or
/// when the last clone is dropped.
#[derive(Clone)]
pub struct Job(Arc<JobInner>);

impl Job {
    fn update(&self, f: impl FnOnce(&mut Entry)) {
        self.0.registry.update(self.0.id, f);
    }

    /// Share done, 0–1.
    pub fn progress(&self, share: f64) {
        let share = if share.is_finite() { share.clamp(0.0, 1.0) } else { 0.0 };
        self.update(|e| {
            e.view.progress = Some(share);
            e.view.status = Status::Running;
        });
    }

    pub fn steps(&self, done: u64, total: u64) {
        self.update(|e| {
            e.view.steps = Some((done, total));
            e.view.progress = (total > 0).then(|| (done as f64 / total as f64).min(1.0));
            e.view.status = Status::Running;
        });
    }

    /// A line under the title, e.g. how exact the solve already is.
    pub fn detail(&self, text: impl Into<String>) {
        let text = text.into();
        self.update(|e| e.view.detail = Some(text));
    }

    pub fn target(&self, target: Target) {
        self.update(|e| e.view.target = target);
    }

    pub fn not_cancellable(&self) {
        self.update(|e| e.view.cancellable = false);
    }

    pub fn on_cancel(&self, hook: impl Fn() + Send + Sync + 'static) {
        self.update(|e| e.on_cancel = Some(Arc::new(hook)));
    }

    /// The user asked to stop.
    pub fn cancelled(&self) -> bool {
        self.0.cancel.load(Ordering::Relaxed)
    }

    /// The user asked to stop or PokerStars runs.
    pub fn should_stop(&self) -> bool {
        self.cancelled() || self.0.registry.locked.load(Ordering::Relaxed)
    }

    pub fn finish(&self) {
        self.0.registry.end(self.0.id, Status::Done, None);
    }

    pub fn fail(&self, error: impl Into<String>) {
        self.0.registry.end(self.0.id, Status::Failed, Some(error.into()));
    }

    /// Ended early, by the user or because PokerStars started.
    pub fn stopped(&self) {
        self.0.registry.end(self.0.id, Status::Cancelled, None);
    }
}

pub static JOBS: LazyLock<Arc<Registry>> = LazyLock::new(Arc::default);

/// Registers a task in the app's task center.
pub fn start(kind: Kind, title: impl Into<String>, target: Target) -> Job {
    JOBS.start(kind, title, target)
}

/// Sends the list to the page whenever it changed, and follows the PokerStars lock.
pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("jobs".into())
        .spawn(move || {
            loop {
                JOBS.set_locked(app.state::<AppState>().client_running.load(Ordering::Relaxed));
                if JOBS.take_dirty() {
                    let _ = app.emit("jobs-changed", JOBS.view());
                }
                std::thread::sleep(EMIT_EVERY);
            }
        })
        .expect("jobs thread starts");
}

#[tauri::command]
pub fn jobs_list() -> JobsView {
    JOBS.view()
}

#[tauri::command]
pub fn jobs_cancel(id: u64) -> CmdResult<()> {
    JOBS.cancel(id)
}

#[tauri::command]
pub fn jobs_clear() {
    JOBS.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> Arc<Registry> {
        Arc::default()
    }

    fn one(r: &Registry, id: u64) -> JobView {
        r.view().jobs.into_iter().find(|j| j.id == id).unwrap()
    }

    fn id_of(job: &Job) -> u64 {
        job.0.id
    }

    #[test]
    fn a_task_reports_progress_and_ends_once() {
        let r = registry();
        let job = r.start(Kind::Solver, "Spot lösen", Target::to("solver", None));
        assert!(r.take_dirty());
        assert!(!r.take_dirty());
        job.steps(3, 12);
        job.detail("0,8 % ausbeutbar");
        let v = one(&r, id_of(&job));
        assert_eq!((v.status, v.progress, v.steps), (Status::Running, Some(0.25), Some((3, 12))));
        assert_eq!(v.detail.as_deref(), Some("0,8 % ausbeutbar"));
        assert!(r.take_dirty());

        job.target(Target::to("solver", Some(9)));
        job.finish();
        job.fail("später");
        let v = one(&r, id_of(&job));
        assert_eq!((v.status, v.progress, v.error), (Status::Done, Some(1.0), None));
        assert_eq!(v.target.id, Some(9));

        // Dropping the last clone of an unfinished task ends it as done.
        let other = r.start(Kind::Coach, "Coach", Target::to("coach", None));
        let id = id_of(&other);
        let clone = other.clone();
        drop(other);
        assert_eq!(one(&r, id).status, Status::Running);
        drop(clone);
        assert_eq!(one(&r, id).status, Status::Done);
        r.clear();
        assert!(r.view().jobs.is_empty());
    }

    #[test]
    fn cancel_reaches_the_task_and_its_hook() {
        let r = registry();
        let job = r.start(Kind::Library, "Bibliothek", Target::to("library", None));
        let hooked = Arc::new(AtomicBool::new(false));
        let flag = hooked.clone();
        job.on_cancel(move || flag.store(true, Ordering::SeqCst));
        assert!(!job.should_stop());
        r.cancel(id_of(&job)).unwrap();
        assert!(job.cancelled() && job.should_stop() && hooked.load(Ordering::SeqCst));
        assert!(one(&r, id_of(&job)).cancelling);
        let id = id_of(&job);
        drop(job);
        let v = one(&r, id);
        assert_eq!((v.status, v.cancelling, v.by_client), (Status::Cancelled, false, false));
        assert!(r.cancel(id).is_err(), "ended tasks cannot be cancelled");

        let fixed = r.start(Kind::Rebuild, "Datenbank", Target::to("dashboard", None));
        fixed.not_cancellable();
        assert!(r.cancel(id_of(&fixed)).is_err());
        assert!(!fixed.cancelled());
    }

    #[test]
    fn pokerstars_pauses_waiting_tasks_and_stops_running_ones() {
        let r = registry();
        let solve = r.start(Kind::Solver, "Spot lösen", Target::to("solver", None));
        let queue = r.start(Kind::Library, "Bibliothek", Target::to("library", None));
        r.set_locked(true);
        let view = r.view();
        assert!(view.locked);
        assert!(view.jobs.iter().all(|j| j.status == Status::Paused));
        assert!(solve.should_stop() && !solve.cancelled());
        // The solve gives up; it ended because of the client, not the user.
        solve.stopped();
        let v = one(&r, id_of(&solve));
        assert_eq!((v.status, v.by_client), (Status::Cancelled, true));
        r.set_locked(false);
        assert_eq!(one(&r, id_of(&queue)).status, Status::Running);
        assert!(!queue.should_stop());
    }

    #[test]
    fn remaining_time_follows_the_share_done() {
        let r = registry();
        let job = r.start(Kind::Preflop, "Preflop", Target::to("charts", None));
        job.progress(0.5);
        assert_eq!(one(&r, id_of(&job)).eta_secs, None, "too early to say");
        r.update(id_of(&job), |e| e.started -= Duration::from_secs(10));
        let eta = one(&r, id_of(&job)).eta_secs.unwrap();
        assert!((9.5..=11.0).contains(&eta), "{eta}");
        job.progress(1.0);
        assert_eq!(one(&r, id_of(&job)).eta_secs, None);
    }

    #[test]
    fn old_finished_tasks_are_dropped() {
        let r = registry();
        let running = r.start(Kind::Library, "läuft", Target::to("library", None));
        for i in 0..KEEP_FINISHED + 5 {
            r.start(Kind::PushFold, format!("{i}"), Target::to("charts", None)).finish();
        }
        let view = r.view();
        assert_eq!(view.jobs.len(), KEEP_FINISHED + 1);
        assert!(view.jobs.iter().any(|j| j.id == id_of(&running)));
        assert_eq!(view.jobs[0].title, format!("{}", KEEP_FINISHED + 4), "newest first");
    }
}
