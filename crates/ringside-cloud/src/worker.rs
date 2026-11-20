//! Solving spots of the community queue ("Mitrechnen"): the app does it in a run the user
//! started, `ringside-worker` on a computer without the app. Both read the leased input
//! with [`Task::parse`], compute it with [`solve`] and submit the [`Solved`] result;
//! [`run`] is the worker's loop around that.
//!
//! A stopped solve gives `Ok(None)` and nothing is submitted, so a stop never leaves half
//! a result behind; the lease is given back when the server allows it, else it runs out.
//! A spot that cannot be solved here is reported as failed instead and not taken again.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use ringside_engine::solve_push_fold;
use ringside_solver::library::format::Stored;
use ringside_solver::library::{SpotId, catalog, solve_spot};
use ringside_solver::{Error as SolverError, Progress};
use serde_json::Value;

use crate::client::{Client, Error, SubmitStatus, Work};
use crate::kinds::pushfold_mw::reference;
use crate::kinds::{
    self, Kind, Postflop, PostflopInput, PostflopSolution, PushFoldHu, PushFoldHuInput, PushFoldHuSolution, PushFoldMw,
    PushFoldMwInput, PushFoldMwSolution, SpotInput, SpotKind, SpotSolution,
};

/// Kinds a client solves for the queue, with the engine version it computes.
pub const KINDS: [(&str, u32); 4] = [
    (SpotKind::NAME, SpotKind::ENGINE_VERSION),
    (Postflop::NAME, Postflop::ENGINE_VERSION),
    (PushFoldMw::NAME, PushFoldMw::ENGINE_VERSION),
    (PushFoldHu::NAME, PushFoldHu::ENGINE_VERSION),
];

/// A leased spot, read.
#[derive(Clone, Debug, PartialEq)]
pub enum Task {
    Spot(SpotInput),
    Postflop { input: PostflopInput, id: SpotId },
    PushFoldMw(PushFoldMwInput),
    PushFoldHu(PushFoldHuInput),
}

impl Task {
    pub fn parse(work: &Work) -> Result<Task, String> {
        fn input<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, String> {
            serde_json::from_value(v.clone()).map_err(|e| format!("Eingabe unlesbar: {e}"))
        }
        Ok(match work.kind.as_str() {
            "spot" => Task::Spot(input(&work.input)?),
            "postflop" => {
                let input: PostflopInput = input(&work.input)?;
                let c = catalog();
                let id = SpotId {
                    scenario: c.scenario(&input.scenario).ok_or("unbekannter Spot")?,
                    flop: c.flop(&input.flop).ok_or("unbekannter Flop")?,
                };
                Task::Postflop { input, id }
            }
            "pushfold-mw" => Task::PushFoldMw(input(&work.input)?),
            "pushfold-hu" => Task::PushFoldHu(input(&work.input)?),
            other => return Err(format!("unbekannte Art {other}")),
        })
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Task::Spot(_) => SpotKind::NAME,
            Task::Postflop { .. } => Postflop::NAME,
            Task::PushFoldMw(_) => PushFoldMw::NAME,
            Task::PushFoldHu(_) => PushFoldHu::NAME,
        }
    }

    /// For a log line, e.g. "srp-btn-bb-40 AsKhTd" or "6p ICM".
    pub fn short_name(&self) -> String {
        let bb = |mbb: u32| format!("{}bb", mbb as f64 / 1000.0);
        match self {
            Task::Spot(i) => format!("{}p {}bb {}", i.players, i.stack_bb, i.board),
            Task::Postflop { input, .. } => format!("{} {}", input.scenario, input.flop),
            Task::PushFoldMw(i) => {
                let icm = if i.payouts_ppm.is_empty() { "chipEV" } else { "ICM" };
                let shortest = i.stacks_mbb.iter().copied().min().unwrap_or(0);
                format!("{}p {icm} min {}", i.stacks_mbb.len(), bb(shortest))
            }
            Task::PushFoldHu(i) => format!("HU {}", bb(i.stack_mbb)),
        }
    }
}

/// A computed spot, ready to submit.
#[derive(Clone, Debug)]
pub enum Solved {
    Spot { input: SpotInput, stored: Stored },
    Postflop { input: PostflopInput, id: SpotId, stored: Stored },
    /// `exploitability` in the kind's units, as the verifier measures it.
    PushFoldMw { input: PushFoldMwInput, solution: PushFoldMwSolution, exploitability: f64 },
    PushFoldHu { input: PushFoldHuInput, solution: PushFoldHuSolution, exploitability: f64 },
}

impl Solved {
    pub fn kind(&self) -> &'static str {
        match self {
            Solved::Spot { .. } => SpotKind::NAME,
            Solved::Postflop { .. } => Postflop::NAME,
            Solved::PushFoldMw { .. } => PushFoldMw::NAME,
            Solved::PushFoldHu { .. } => PushFoldHu::NAME,
        }
    }

    /// What the solver measured last, with its unit.
    pub fn exploitability(&self) -> (f64, &'static str) {
        match self {
            Solved::Spot { stored, .. } | Solved::Postflop { stored, .. } => (stored.exploitability_pct as f64, "% pot"),
            Solved::PushFoldMw { input, exploitability, .. } => {
                (*exploitability, if input.payouts_ppm.is_empty() { "bb" } else { "% pool" })
            }
            Solved::PushFoldHu { exploitability, .. } => (*exploitability, "bb"),
        }
    }

    /// The JSON body of `POST /v1/solutions`.
    pub fn submission(&self) -> Value {
        match self {
            Solved::Spot { input, stored } => kinds::submission::<SpotKind>(input, &SpotSolution::from_stored(stored)),
            Solved::Postflop { input, stored, .. } => {
                kinds::submission::<Postflop>(input, &PostflopSolution::from_stored(stored))
            }
            Solved::PushFoldMw { input, solution, .. } => kinds::submission::<PushFoldMw>(input, solution),
            Solved::PushFoldHu { input, solution, .. } => kinds::submission::<PushFoldHu>(input, solution),
        }
    }

    pub fn submit(&self, client: &Client) -> Result<SubmitStatus, Error> {
        match self {
            Solved::Spot { input, stored } => client.submit_solution::<SpotKind>(input, &SpotSolution::from_stored(stored)),
            Solved::Postflop { input, stored, .. } => {
                client.submit_solution::<Postflop>(input, &PostflopSolution::from_stored(stored))
            }
            Solved::PushFoldMw { input, solution, .. } => client.submit_solution::<PushFoldMw>(input, solution),
            Solved::PushFoldHu { input, solution, .. } => client.submit_solution::<PushFoldHu>(input, solution),
        }
    }
}

/// What a solve may use.
#[derive(Clone, Copy, Debug)]
pub struct Power {
    pub threads: usize,
    /// Bytes one postflop tree may take; a larger one is an error.
    pub memory: u64,
}

/// Computes `task`. `Ok(None)` when `stop` ended it; push/fold solves take seconds and
/// do not look at `stop`.
pub fn solve(
    task: &Task,
    power: Power,
    on_progress: impl FnMut(&Progress),
    stop: impl Fn() -> bool,
) -> Result<Option<Solved>, String> {
    let finish = |r: Result<Stored, SolverError>| match r {
        Ok(stored) => Ok(Some(stored)),
        Err(SolverError::Cancelled) => Ok(None),
        Err(e) => Err(e.to_string()),
    };
    Ok(Some(match task {
        Task::Spot(input) => {
            let c = input.catalog(None)?;
            let id = SpotId { scenario: 0, flop: 0 };
            let Some(stored) = finish(solve_spot(&c, id, power.threads, power.memory, on_progress, stop))? else {
                return Ok(None);
            };
            Solved::Spot { input: input.clone(), stored }
        }
        Task::Postflop { input, id } => {
            let Some(stored) = finish(solve_spot(catalog(), *id, power.threads, power.memory, on_progress, stop))? else {
                return Ok(None);
            };
            Solved::Postflop { input: input.clone(), id: *id, stored }
        }
        Task::PushFoldMw(input) => {
            let solution = reference(input)?;
            if !solution.converged {
                return Err("nicht konvergiert".into());
            }
            let exploitability = solution.exploitability * input.unit();
            Solved::PushFoldMw { input: input.clone(), solution: PushFoldMwSolution::from(&*solution), exploitability }
        }
        Task::PushFoldHu(input) => {
            let solution = solve_push_fold(&input.params()).map_err(|e| e.to_string())?;
            let exploitability = solution.sb_gain_bb + solution.bb_gain_bb;
            Solved::PushFoldHu { input: *input, solution: PushFoldHuSolution::from(&solution), exploitability }
        }
    }))
}

// The worker's loop.

/// Where the work comes from and goes to: the cloud, or a fake in tests.
pub trait Queue {
    fn next(&mut self, kinds: &[(&str, u32)]) -> Result<Option<Work>, Error>;
    fn submit(&mut self, solved: &Solved) -> Result<SubmitStatus, Error>;
    /// Gives a lease back early.
    fn release(&mut self, key: &str) -> Result<(), Error>;
    /// Reports the held spot as unsolvable here; false when the server does not know how.
    fn fail(&mut self, key: &str, note: &str) -> Result<bool, Error>;
}

impl Queue for Client {
    fn next(&mut self, kinds: &[(&str, u32)]) -> Result<Option<Work>, Error> {
        self.next_work(kinds)
    }

    fn submit(&mut self, solved: &Solved) -> Result<SubmitStatus, Error> {
        Solved::submit(solved, self)
    }

    fn release(&mut self, key: &str) -> Result<(), Error> {
        self.release_work(key)
    }

    fn fail(&mut self, key: &str, note: &str) -> Result<bool, Error> {
        self.fail_work(key, note)
    }
}

pub struct RunOptions {
    pub kinds: Vec<(&'static str, u32)>,
    /// Spots to take at most, failed ones included.
    pub max: Option<u32>,
    /// Take no new spot after this; the one running is finished.
    pub until: Option<Instant>,
    /// Solve, but submit nothing and give the lease back.
    pub dry_run: bool,
    pub power: Power,
    /// First wait after a failed request; it doubles up to `max_backoff`.
    pub backoff: Duration,
    pub max_backoff: Duration,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Served to everyone from now on.
    Accepted,
    /// Waits for the verifier (a server from before accepting at once).
    Submitted,
    /// Someone was faster; nothing was stored.
    AlreadyVerified,
    DryRun,
    /// Stopped during the solve or before the submit; nothing went up.
    Stopped,
    Failed(String),
}

/// One spot the loop took.
#[derive(Clone, Debug)]
pub struct Done {
    pub key: String,
    pub kind: String,
    pub name: String,
    pub seconds: f64,
    pub exploitability: Option<(f64, &'static str)>,
    pub outcome: Outcome,
}

/// Why the loop ended.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// Nothing open among the kinds.
    Empty,
    Max,
    Time,
    Stopped,
    /// A request the server refused for good (not a network or rate-limit error).
    Error(String),
}

/// Why a spot handed out again in the same run is not solved a second time.
const SOLVED_IN_VAIN: &str = "in diesem Lauf schon gescheitert";

/// How long a solved spot may wait to go up: past one rate-limit hour of the server.
const SUBMIT_PATIENCE: Duration = Duration::from_secs(70 * 60);

/// What [`run`] reports as it goes.
pub enum Event<'a> {
    /// A spot was taken and is being solved.
    Started { key: &'a str, kind: &'a str, name: &'a str },
    Done(&'a Done),
}

/// Takes spots from `queue` until it is empty, `max` or `until` is reached or `stop` says
/// so.
pub fn run(queue: &mut impl Queue, opts: &RunOptions, stop: &dyn Fn() -> bool, mut report: impl FnMut(Event)) -> End {
    let mut taken = 0u32;
    // Solves are deterministic: a spot that failed here fails again.
    let mut failed: HashSet<String> = HashSet::new();
    loop {
        if stop() {
            return End::Stopped;
        }
        if opts.max.is_some_and(|m| taken >= m) {
            return End::Max;
        }
        if opts.until.is_some_and(|t| Instant::now() >= t) {
            return End::Time;
        }
        let work = match retrying(opts, opts.until, stop, || queue.next(&opts.kinds)) {
            Ok(Some(w)) => w,
            Ok(None) => return End::Empty,
            Err(Retry::Stopped) => return End::Stopped,
            Err(Retry::Time) => return End::Time,
            Err(Retry::Failed(e)) => return End::Error(e.to_string()),
        };
        taken += 1;
        let started = Instant::now();
        let task = Task::parse(&work);
        let name = task.as_ref().map_or_else(|_| work.key.chars().take(12).collect(), Task::short_name);
        report(Event::Started { key: &work.key, kind: &work.kind, name: &name });
        let solved = if failed.contains(&work.key) {
            Err(SOLVED_IN_VAIN.to_string())
        } else {
            task.and_then(|t| solve(&t, opts.power, |_| {}, stop))
        };
        let unsolvable = solved.is_err();
        let seconds = started.elapsed().as_secs_f64();
        let exploitability = solved.as_ref().ok().and_then(Option::as_ref).map(Solved::exploitability);
        let outcome = match solved {
            Err(e) => Outcome::Failed(e),
            Ok(None) => Outcome::Stopped,
            Ok(Some(_)) if stop() => Outcome::Stopped,
            Ok(Some(_)) if opts.dry_run => Outcome::DryRun,
            // The server takes a submission after the lease ran out all the same.
            Ok(Some(s)) => match retrying(opts, Some(Instant::now() + SUBMIT_PATIENCE), stop, || queue.submit(&s)) {
                Ok(SubmitStatus::Accepted) => Outcome::Accepted,
                Ok(SubmitStatus::Submitted) => Outcome::Submitted,
                Ok(SubmitStatus::AlreadyVerified) => Outcome::AlreadyVerified,
                Err(Retry::Stopped) => Outcome::Stopped,
                Err(Retry::Time) => Outcome::Failed("submit gave up".into()),
                Err(Retry::Failed(e)) => Outcome::Failed(e.to_string()),
            },
        };
        if let (true, Outcome::Failed(note)) = (unsolvable, &outcome) {
            failed.insert(work.key.clone());
            // Not given back: a server without the failure report would hand it straight back.
            let _ = queue.fail(&work.key, note);
        } else if !matches!(outcome, Outcome::Accepted | Outcome::Submitted | Outcome::AlreadyVerified) {
            // Best effort: a lease that is not given back runs out.
            let _ = queue.release(&work.key);
        }
        let d = Done { key: work.key, kind: work.kind, name, seconds, exploitability, outcome };
        report(Event::Done(&d));
        if d.outcome == Outcome::Stopped {
            return End::Stopped;
        }
    }
}

enum Retry {
    Stopped,
    Time,
    Failed(Error),
}

/// Calls `f` until it succeeds, waiting after network, server and rate-limit errors with
/// a growing backoff (or the server's Retry-After); gives up at `until` or on `stop`.
fn retrying<T>(
    opts: &RunOptions,
    until: Option<Instant>,
    stop: &dyn Fn() -> bool,
    mut f: impl FnMut() -> Result<T, Error>,
) -> Result<T, Retry> {
    let mut backoff = opts.backoff;
    loop {
        let wait = match f() {
            Ok(v) => return Ok(v),
            Err(Error::RateLimited { retry_after }) => retry_after.unwrap_or(backoff).max(opts.backoff),
            Err(Error::Network(_)) | Err(Error::Rejected { status: 500.., .. }) => backoff,
            Err(e) => return Err(Retry::Failed(e)),
        };
        backoff = (backoff * 2).min(opts.max_backoff);
        let resume = Instant::now() + wait;
        if until.is_some_and(|t| resume > t) {
            return Err(Retry::Time);
        }
        while Instant::now() < resume {
            if stop() {
                return Err(Retry::Stopped);
            }
            std::thread::sleep((resume - Instant::now()).min(Duration::from_millis(200)));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::VecDeque;

    use super::*;
    use crate::canonical::canonicalize;
    use crate::kinds::Verdict;

    fn hu(stack_mbb: u32) -> Work {
        let input = PushFoldHuInput { stack_mbb, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 };
        Work {
            key: PushFoldHu::key(&input).unwrap(),
            kind: PushFoldHu::NAME.into(),
            engine_version: PushFoldHu::ENGINE_VERSION,
            input: serde_json::to_value(input).unwrap(),
            lease_until: 0,
        }
    }

    /// The library's smallest catalog spot.
    fn postflop() -> Work {
        let input = PostflopInput::new("3bp-btn-bb-40", "Ks9h4d");
        Work {
            key: Postflop::key(&input).unwrap(),
            kind: Postflop::NAME.into(),
            engine_version: Postflop::ENGINE_VERSION,
            input: serde_json::to_value(&input).unwrap(),
            lease_until: 0,
        }
    }

    #[derive(Default)]
    struct Fake {
        open: VecDeque<Work>,
        /// Network errors before each hand-out.
        flaky: u32,
        /// Time `next` takes.
        slow: Duration,
        submitted: Vec<Value>,
        released: Vec<String>,
        /// Given-back spots are first in line again, as on the server.
        requeue: bool,
        handed: Vec<Work>,
        failed: Vec<(String, String)>,
        /// A server from before failure reports.
        no_fail: bool,
    }

    impl Queue for Fake {
        fn next(&mut self, _: &[(&str, u32)]) -> Result<Option<Work>, Error> {
            if self.flaky > 0 {
                self.flaky -= 1;
                return Err(Error::Network("down".into()));
            }
            std::thread::sleep(self.slow);
            let work = self.open.pop_front();
            self.handed.extend(work.clone());
            Ok(work)
        }

        fn submit(&mut self, solved: &Solved) -> Result<SubmitStatus, Error> {
            self.submitted.push(solved.submission());
            Ok(SubmitStatus::Accepted)
        }

        fn release(&mut self, key: &str) -> Result<(), Error> {
            self.released.push(key.into());
            if self.requeue {
                let work = self.handed.iter().find(|w| w.key == key).cloned();
                self.open.push_front(work.expect("only handed-out spots are given back"));
            }
            Ok(())
        }

        fn fail(&mut self, key: &str, note: &str) -> Result<bool, Error> {
            if self.no_fail {
                return Ok(false);
            }
            self.failed.push((key.into(), note.into()));
            Ok(true)
        }
    }

    /// A spot no computer can solve here: its scenario is unknown.
    fn unsolvable() -> Work {
        let input = PostflopInput::new("3bp-btn-bb-40", "Ks9h4d");
        let mut value = serde_json::to_value(&input).unwrap();
        value["scenario"] = "gibt-es-nicht".into();
        Work { key: "f".repeat(64), kind: Postflop::NAME.into(), engine_version: Postflop::ENGINE_VERSION, input: value, lease_until: 0 }
    }

    fn opts() -> RunOptions {
        RunOptions {
            kinds: KINDS.to_vec(),
            max: None,
            until: None,
            dry_run: false,
            power: Power { threads: 4, memory: 4 << 30 },
            backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(4),
        }
    }

    fn collect(queue: &mut Fake, opts: &RunOptions, stop: &dyn Fn() -> bool) -> (End, Vec<Done>) {
        let mut done = Vec::new();
        let end = run(queue, opts, stop, |e| {
            if let Event::Done(d) = e {
                done.push(d.clone());
            }
        });
        (end, done)
    }

    #[test]
    fn a_solved_spot_is_a_submission_the_verifier_accepts() {
        let mut q = Fake { open: [hu(10_000)].into(), ..Fake::default() };
        let (end, done) = collect(&mut q, &opts(), &|| false);
        assert_eq!(end, End::Empty);
        assert_eq!(done[0].outcome, Outcome::Accepted);
        assert!(done[0].exploitability.is_some_and(|(e, _)| e <= PushFoldHu::TOLERANCE), "{:?}", done[0]);
        assert!(q.released.is_empty(), "a submitted spot keeps its lease until the server marks it");

        let body = &q.submitted[0];
        let entry = kinds::find(body["kind"].as_str().unwrap(), body["engine_version"].as_u64().unwrap() as u32).unwrap();
        let input = canonicalize(&body["input"]).unwrap();
        let verdict = (entry.check)(&done[0].key, &input, &body["result"].to_string());
        assert!(matches!(verdict, Verdict::Verified { .. }), "{verdict:?}");
    }

    #[test]
    fn an_icm_gap_reports_what_the_verifier_measures() {
        let seed = crate::operator::preflop_seeds().unwrap().into_iter().find(|s| s.kind == PushFoldMw::NAME).unwrap();
        let work = Work {
            key: seed.key,
            kind: seed.kind.into(),
            engine_version: seed.engine_version,
            input: serde_json::from_str(&seed.input).unwrap(),
            lease_until: 0,
        };
        let mut q = Fake { open: [work].into(), ..Fake::default() };
        let (_, done) = collect(&mut q, &opts(), &|| false);
        let body = &q.submitted[0];
        let verdict = (kinds::find(PushFoldMw::NAME, PushFoldMw::ENGINE_VERSION).unwrap().check)(
            &done[0].key,
            &canonicalize(&body["input"]).unwrap(),
            &body["result"].to_string(),
        );
        let Verdict::Verified { exploitability, .. } = verdict else { panic!("{verdict:?}") };
        let (reported, unit) = done[0].exploitability.unwrap();
        assert_eq!(unit, "% pool");
        assert!((reported - exploitability).abs() <= PushFoldMw::TOLERANCE, "{reported} vs {exploitability}");
    }

    #[test]
    fn a_spot_that_fails_is_not_taken_again_in_the_session() {
        let mut q = Fake { open: [unsolvable(), hu(10_000)].into(), requeue: true, ..Fake::default() };
        let (end, done) = collect(&mut q, &RunOptions { max: Some(10), ..opts() }, &|| false);
        let tries = done.iter().filter(|d| d.key == unsolvable().key).count();
        assert_eq!(tries, 1, "{:?}", done.iter().map(|d| (&d.key[..8], &d.outcome)).collect::<Vec<_>>());
        assert_eq!((end, q.submitted.len()), (End::Empty, 1), "the next spot is solved instead");
        assert_eq!(q.failed, [(unsolvable().key, "unbekannter Spot".to_string())]);
        assert!(q.released.is_empty(), "a failed spot is reported, not given back");

        // An old server hands it out again once its lease ran out; it is not solved again.
        let mut q = Fake { open: [unsolvable(), hu(10_000), unsolvable()].into(), no_fail: true, ..Fake::default() };
        let (_, done) = collect(&mut q, &opts(), &|| false);
        let outcomes: Vec<_> = done.iter().map(|d| &d.outcome).collect();
        let vain = Outcome::Failed(SOLVED_IN_VAIN.into());
        assert_eq!(outcomes, [&Outcome::Failed("unbekannter Spot".into()), &Outcome::Accepted, &vain]);
        assert!(q.released.is_empty());
    }

    #[test]
    fn a_dry_run_submits_nothing_and_gives_the_lease_back() {
        let mut q = Fake { open: [hu(10_000), hu(12_000)].into(), ..Fake::default() };
        let (end, done) = collect(&mut q, &RunOptions { dry_run: true, ..opts() }, &|| false);
        assert_eq!(end, End::Empty);
        assert!(q.submitted.is_empty());
        assert_eq!(done.iter().map(|d| &d.outcome).collect::<Vec<_>>(), [&Outcome::DryRun, &Outcome::DryRun]);
        assert_eq!(q.released, done.iter().map(|d| d.key.clone()).collect::<Vec<_>>());
    }

    #[test]
    fn max_limits_the_spots_taken() {
        let mut q = Fake { open: [hu(10_000), hu(11_000), hu(12_000)].into(), ..Fake::default() };
        let (end, done) = collect(&mut q, &RunOptions { max: Some(2), ..opts() }, &|| false);
        assert_eq!((end, done.len(), q.submitted.len(), q.open.len()), (End::Max, 2, 2, 1));
    }

    #[test]
    fn after_the_time_no_spot_is_taken_but_the_running_one_finishes() {
        let mut q = Fake { open: [hu(10_000)].into(), ..Fake::default() };
        let (end, done) = collect(&mut q, &RunOptions { until: Some(Instant::now()), ..opts() }, &|| false);
        assert_eq!((end, done.len(), q.open.len()), (End::Time, 0, 1));

        // The hand-out ends after the deadline; the spot is still solved and submitted.
        let mut q = Fake { open: [hu(10_000), hu(11_000)].into(), slow: Duration::from_millis(50), ..Fake::default() };
        let until = Instant::now() + Duration::from_millis(20);
        let (end, done) = collect(&mut q, &RunOptions { until: Some(until), ..opts() }, &|| false);
        assert_eq!((end, done.len(), q.submitted.len(), q.open.len()), (End::Time, 1, 1, 1));
    }

    #[test]
    fn network_errors_are_retried() {
        let mut q = Fake { open: [hu(10_000)].into(), flaky: 3, ..Fake::default() };
        let (end, done) = collect(&mut q, &opts(), &|| false);
        assert_eq!((end, done.len(), q.submitted.len()), (End::Empty, 1, 1));
    }

    #[test]
    fn a_stop_during_the_solve_leaves_nothing_behind() {
        let mut q = Fake { open: [postflop(), hu(10_000)].into(), ..Fake::default() };
        // The solver asks before every iteration; the third question is mid-solve.
        let asked = Cell::new(0);
        let stop = || {
            asked.set(asked.get() + 1);
            asked.get() > 3
        };
        let (end, done) = collect(&mut q, &opts(), &stop);
        assert_eq!(end, End::Stopped);
        assert_eq!(done.len(), 1);
        assert_eq!((done[0].kind.as_str(), &done[0].outcome), ("postflop", &Outcome::Stopped));
        assert!(done[0].exploitability.is_none());
        assert!(q.submitted.is_empty());
        assert_eq!(q.released, [done[0].key.clone()]);
        assert_eq!(q.open.len(), 1, "no further spot is taken");
    }
}
