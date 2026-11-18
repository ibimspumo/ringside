//! Endless practice with whole hands ("Hände spielen"): the current hand, the stock of
//! prepared postflop spots and what finished hands add to the learning area.
//!
//! The stock's worker starts with the first hand and keeps running whichever page is open;
//! it pauses while PokerStars runs. Each round of filling it is a task in the task center
//! (`jobs`) and also goes to the page as `play-pool`. It prefers flops that are solved
//! already and asks the spot store (`store`) before it solves; a spot solved in the
//! background is saved there and offered to Ringside Cloud if the user shares.
//!
//! A hand that reaches a spot nobody prepared waits while the same worker solves it live
//! (`play-live` to the table, a task in the task center, cancellable); the hand then goes
//! on and the page hears `play-hand`. While the hero thinks, the next likely spot is
//! solved ahead.
//!
//! Solved spots take up to a gigabyte each, so the stock has a small budget (live spots
//! count against it) and is freed [`AWAY`] after the page was left (`play_away`); the
//! page fills it again on return.

use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex, Once};
use std::time::{Duration, Instant};

use ringside_core::db::Attempt;
use ringside_learn::Kind as LearnKind;
use ringside_engine::preflop_table::{Family, ModelLine};
use ringside_analysis::assess::Estimator;
use ringside_play::hand::Source;
use ringside_play::pool::{Line, Lookup, TARGET_PCT};
use ringside_play::tree::Tree;
use ringside_solver::gate::Priority;
use ringside_play::{Config, Hand, HandView, Lesson, LiveStatus, Plan, Pool, PoolStatus, Status, Urgency};
use ringside_solver::library::catalog;
use ringside_solver::{Spot, Step};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::jobs::{self, Job, Kind, Target};
use crate::{AppState, CmdResult, blocking, err, now_secs};

struct Current {
    hand: Hand,
    /// A replay of an earlier hand; it does not count again in the learning area.
    repeat: bool,
    recorded: bool,
}

struct PlayState {
    current: Mutex<Option<Current>>,
    worker: Once,
    job: Mutex<Option<Job>>,
    /// The task of the live solve running now.
    live_job: Mutex<Option<Job>>,
    /// Since when the practice page is closed.
    away: Mutex<Option<Instant>>,
}

static PLAY: LazyLock<PlayState> = LazyLock::new(|| PlayState {
    current: Mutex::new(None),
    worker: Once::new(),
    job: Mutex::new(None),
    live_job: Mutex::new(None),
    away: Mutex::new(None),
});

/// How long the stock outlives the closed page, so a quick look elsewhere costs no solve.
const AWAY: Duration = Duration::from_secs(120);
const GB: u64 = 1 << 30;

static POOL: LazyLock<Mutex<Option<Arc<Pool>>>> = LazyLock::new(|| Mutex::new(None));
/// Grades the moves no solver covers, as the hand analysis does.
static ESTIMATE: LazyLock<Arc<Estimator>> = LazyLock::new(|| Arc::new(Estimator::new()));

fn pool(app: &AppHandle) -> Arc<Pool> {
    let mut slot = POOL.lock().unwrap();
    if let Some(p) = slot.as_ref() {
        return p.clone();
    }
    // Few threads and little memory, so the computer and the page stay quick: an eighth
    // of the memory, at most 1.5 GB, which holds one or two solved spots.
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let threads = (cores / 2).clamp(1, 4);
    // A live solve keeps the hand waiting, so it may take all cores but one.
    let live_threads = cores.saturating_sub(1).clamp(1, 8);
    let budget = (crate::solver::installed_memory().unwrap_or(8 * GB) / 8).min(3 * GB / 2);
    let p = Arc::new(Pool::new(Box::new(AppLookup(app.clone())), threads, budget).with_live_threads(live_threads));
    *slot = Some(p.clone());
    p
}

fn client_running(app: &AppHandle) -> bool {
    app.state::<AppState>().client_running.load(Ordering::Relaxed)
}

/// The practice page is open again.
fn here() {
    *PLAY.away.lock().unwrap() = None;
}

/// Frees the stock once the page has been closed for [`AWAY`], and a finished hand with
/// it (its result is recorded; the page goes back to the choice). A hand still running
/// keeps its spot. Called by the housekeeping thread.
pub(crate) fn tick() {
    let mut away = PLAY.away.lock().unwrap();
    if !away.is_some_and(|t| t.elapsed() >= AWAY) {
        return;
    }
    *away = None;
    drop(away);
    if let Some(pool) = POOL.lock().unwrap().as_ref() {
        pool.set_plan(None);
        pool.forget_live();
    }
    ringside_play::table::release();
    let mut current = PLAY.current.lock().unwrap();
    if current.as_ref().is_some_and(|c| c.hand.is_over()) {
        *current = None;
    }
}

/// Starts the stock's worker once per process.
fn ensure_worker(app: &AppHandle) {
    PLAY.worker.call_once(|| {
        let app = app.clone();
        let pool = pool(&app);
        std::thread::spawn(move || {
            let paused_app = app.clone();
            let report_app = app.clone();
            let live_app = app.clone();
            pool.run(|| false, move || client_running(&paused_app), move |s| report(&report_app, s), move |s| report_live(&live_app, s));
        });
    });
}

/// The one place the stock reports to: the practice page and the task center.
fn report(app: &AppHandle, s: &PoolStatus) {
    let _ = app.emit("play-pool", s);
    let mut slot = PLAY.job.lock().unwrap();
    match s.status {
        Status::Running => {
            let job = slot.get_or_insert_with(|| {
                let job = jobs::start(Kind::Play, s.title.clone(), Target::to("play", None));
                let app = app.clone();
                job.on_cancel(move || pool(&app).cancel());
                job
            });
            job.progress(s.fraction);
            job.detail(format!("{} von {} Spots bereit", s.ready.min(s.target), s.target));
        }
        Status::PausedClient => {}
        Status::Done | Status::Waiting => {
            if let Some(job) = slot.take() {
                job.finish();
            }
        }
        Status::Cancelled => {
            if let Some(job) = slot.take() {
                job.stopped();
            }
        }
        Status::Error => {
            if let Some(job) = slot.take() {
                job.fail(s.error.clone().unwrap_or_else(|| "Unbekannter Fehler".into()));
            }
        }
    }
}

/// The one place a live solve reports to: the table (`play-live`) and the task center.
/// Once it is over, the waiting hand goes on.
fn report_live(app: &AppHandle, s: &LiveStatus) {
    let _ = app.emit("play-live", s);
    let mut slot = PLAY.live_job.lock().unwrap();
    match s.status {
        Status::Running | Status::Waiting | Status::PausedClient => {
            let title = match s.urgency {
                Urgency::Now => "Spot live rechnen",
                Urgency::Ahead => "Nächsten Spot vorrechnen",
            };
            let job = slot.get_or_insert_with(|| {
                let job = jobs::start(Kind::Play, title, Target::to("play", None));
                let app = app.clone();
                job.on_cancel(move || cancel_live(&app));
                job
            });
            job.progress(s.fraction);
            job.detail(format!("Schnell gerechnet, Genauigkeit {} % des Pots", num(s.target_pct)));
        }
        Status::Done => {
            if let Some(job) = slot.take() {
                job.finish();
            }
        }
        Status::Cancelled => {
            if let Some(job) = slot.take() {
                job.stopped();
            }
        }
        Status::Error => {
            if let Some(job) = slot.take() {
                job.fail(s.error.clone().unwrap_or_else(|| "Unbekannter Fehler".into()));
            }
        }
    }
    drop(slot);
    if matches!(s.status, Status::Done | Status::Cancelled | Status::Error) {
        // The worker reports while it holds the stock; the hand goes on beside it.
        let app = app.clone();
        std::thread::spawn(move || go_on(&app));
    }
}

fn num(x: f64) -> String {
    format!("{x}").replace('.', ",")
}

/// Hands the hand a live spot that is ready (or why there is none) and tells the page.
fn go_on(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut slot = PLAY.current.lock().unwrap();
    let Some(current) = slot.as_mut() else { return };
    let waited = current.hand.need().is_some();
    drive(app, &mut current.hand);
    if waited && current.hand.need().is_none() {
        let _ = finish_if_over(&state, current);
        let _ = app.emit("play-hand", current.hand.view());
    }
}

/// Gives the hand the spot it waits for if it is ready, else asks for it; asks for the
/// next likely spot ahead while the hero thinks.
fn drive(app: &AppHandle, hand: &mut Hand) {
    let p = pool(app);
    while let Some(need) = hand.need().cloned() {
        match p.take_live(&need) {
            Some(result) => {
                if let Err(e) = hand.resume(result) {
                    eprintln!("going on with a live spot: {e}");
                    return;
                }
            }
            None => return p.request(need, Urgency::Now),
        }
    }
    match hand.forecast() {
        Some(need) => p.request(need, Urgency::Ahead),
        None if hand.is_over() => p.forget_live(),
        None => {}
    }
}

fn cancel_live(app: &AppHandle) {
    pool(app).cancel_live();
    go_on(app);
}

// -----------------------------------------------------------------------------------
// Stored solutions.

struct AppLookup(AppHandle);

/// Practice plays the plain tables.
fn model_line(table: ringside_play::TableKey, line: &[ringside_engine::preflop::Step]) -> ModelLine {
    ModelLine { family: Family::Plain, ante: table.ante, players: table.players, stack_bb: table.stack_bb, line: line.to_vec() }
}

/// "Üben mit fertigen Spots anderer" (`cloud.practice_ready`).
fn practice_ready(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.lock().unwrap().cloud.practice_ready
}

/// The ready flops of a line, kept for a while: the cloud is asked at most every
/// [`CLOUD_EVERY`] per line, and not more often than [`STATUS_GAP`] in all.
type ReadyCache = std::collections::HashMap<String, (Instant, Vec<String>)>;
static READY: LazyLock<Mutex<ReadyCache>> = LazyLock::new(Default::default);
static LAST_STATUS: Mutex<Option<Instant>> = Mutex::new(None);
const CLOUD_EVERY: Duration = Duration::from_secs(30 * 60);
const STATUS_GAP: Duration = Duration::from_secs(5);

/// Flops of the scenario's line at `table` that are solved already: in the store (own,
/// library, other players' downloads) and, with `cloud.fetch`, served by the cloud.
/// The candidates are the library's representative flops.
fn ready_flops(app: &AppHandle, table: ringside_play::TableKey, scenario: &ringside_play::scenario::Scenario) -> Vec<String> {
    let c = catalog();
    let model = model_line(table, &scenario.line);
    let Some(base) = c.flops.first().and_then(|f| ringside_cloud::kinds::SpotInput::from_model(&model, &scenario.spot(f)).ok()) else {
        // Not shareable: only the library's own catalog spots can be ready.
        let keys: Vec<(String, String)> = c
            .flops
            .iter()
            .filter_map(|f| ringside_store::key::catalog_id(&scenario.spot(f)).map(|id| (ringside_store::key::catalog_key(id), f.clone())))
            .collect();
        return crate::store::ready_flops(&app.state::<AppState>(), &keys, true);
    };
    let keys: Vec<(String, String)> = c
        .flops
        .iter()
        .filter_map(|f| match ringside_store::key::catalog_id(&scenario.spot(f)) {
            Some(id) => Some((ringside_store::key::catalog_key(id), f.clone())),
            None => Some((crate::store::model_key(&ringside_store::key::on_flop(&base, f).ok()?)?, f.clone())),
        })
        .collect();
    let mut ready = crate::store::ready_flops(&app.state::<AppState>(), &keys, true);
    let line_key = format!("{:?}", model);
    let cached = READY.lock().unwrap().get(&line_key).filter(|(at, _)| at.elapsed() < CLOUD_EVERY).map(|(_, f)| f.clone());
    let in_cloud = match cached {
        Some(flops) => flops,
        None => {
            let mut last = LAST_STATUS.lock().unwrap();
            if last.is_some_and(|t| t.elapsed() < STATUS_GAP) {
                Vec::new()
            } else {
                *last = Some(Instant::now());
                drop(last);
                let spot_keys: Vec<String> =
                    keys.iter().filter_map(|(k, _)| k.strip_prefix("spot:").map(str::to_string)).collect();
                let served = crate::cloud::served(app, &spot_keys);
                let flops: Vec<String> = keys
                    .iter()
                    .filter(|(k, _)| k.strip_prefix("spot:").is_some_and(|h| served.iter().any(|v| v == h)))
                    .map(|(_, f)| f.clone())
                    .collect();
                READY.lock().unwrap().insert(line_key, (Instant::now(), flops.clone()));
                flops
            }
        }
    };
    for f in in_cloud {
        if !ready.contains(&f) {
            ready.push(f);
        }
    }
    ready
}

/// The right to solve a practice spot: its key and a ticket of the store's gate.
struct AppSlot {
    _claim: ringside_store::Claim<'static>,
    ticket: ringside_solver::gate::Ticket<'static>,
}

impl ringside_play::Slot for AppSlot {
    fn hold(&self, timeout: Duration) -> bool {
        self.ticket.hold(timeout)
    }

    fn should_yield(&self) -> bool {
        self.ticket.should_yield()
    }
}

impl Lookup for AppLookup {
    fn load(&self, spot: &Spot, line: Line, max_pct: f64) -> Option<Box<dyn Tree>> {
        let model = line.map(|(table, line)| model_line(table, line));
        let keyed = crate::store::keyed(spot, model.as_ref())?;
        let policy = crate::store::policy(&self.0, practice_ready(&self.0), false);
        let ask = crate::store::Ask::new(spot, &keyed, max_pct);
        crate::store::resolve_app(&self.0, &ask, &policy, || false, |found| Some(found.into_tree()), || None).map(|(_, tree)| tree)
    }

    fn flops(&self, table: ringside_play::TableKey, scenario: &ringside_play::scenario::Scenario) -> Vec<String> {
        // "Nur eigene, frisch gegebene Spots": random flops, solved here.
        if !practice_ready(&self.0) {
            return Vec::new();
        }
        ready_flops(&self.0, table, scenario)
    }

    fn slot(&self, spot: &Spot, line: Line, urgent: bool) -> Option<Box<dyn ringside_play::Slot>> {
        let model = line.map(|(table, line)| model_line(table, line));
        let keyed = crate::store::keyed(spot, model.as_ref())?;
        let claim = crate::store::flight().claim(&keyed.key)?;
        let priority = if urgent { Priority::Viewed } else { Priority::Practice };
        Some(Box::new(AppSlot { _claim: claim, ticket: crate::store::gate().enter(priority) }))
    }

    fn save(
        &self,
        table: ringside_play::TableKey,
        seats: [usize; 2],
        line: Option<&[ringside_engine::preflop::Step]>,
        solution: &mut ringside_solver::Solution,
        settings: &ringside_solver::Settings,
    ) {
        let names = ringside_engine::multiway::position_names(table.players);
        let info = crate::solutions::SaveInfo {
            title: String::new(),
            oop: names[seats[0]].to_string(),
            ip: names[seats[1]].to_string(),
            ranges: format!("Preflop-Modell, {} Spieler, {} BB", table.players, table.stack_bb),
            source: Some(ringside_core::db::SolutionSource::Training),
            hand_id: None,
            model: line.map(|line| model_line(table, line)),
        };
        if let Err(e) = crate::solutions::save(&self.0, solution, settings, &info) {
            eprintln!("saving a practice solve: {e}");
        }
    }
}

// -----------------------------------------------------------------------------------
// The learning area.

#[derive(Serialize, Deserialize)]
struct Record {
    mode: ringside_play::Mode,
    players: usize,
    stack_bb: f64,
    position: String,
    cards: String,
    board: String,
    grades: Vec<Option<ringside_play::Grade>>,
    net_bb: f64,
    postflop: bool,
}

fn record(state: &AppState, hand: &Hand) -> CmdResult<()> {
    let summary = hand.summary();
    if summary.graded == 0 {
        return Ok(());
    }
    let view = hand.view();
    let cards = |c: &[ringside_engine::Card]| c.iter().map(|c| c.to_string()).collect::<String>();
    let record = Record {
        mode: view.config.mode,
        players: view.players,
        stack_bb: view.stack_bb,
        position: view.positions[view.hero].to_string(),
        cards: cards(&view.hero_cards),
        board: cards(&view.board),
        grades: view.verdicts.iter().map(|v| v.judgement.as_ref().map(|j| j.grade)).collect(),
        net_bb: view.outcome.as_ref().map_or(0.0, |o| o.net[view.hero]),
        postflop: view.verdicts.iter().any(|v| v.lesson.street != ringside_play::hand::Street::Preflop && v.lesson.source.is_some_and(|s| s != Source::Model && s != Source::Equilibrium)),
    };
    let attempt = Attempt {
        kind: LearnKind::FullHands.id().into(),
        level: 1,
        seed: Some(view.seed),
        hand_id: None,
        answer: serde_json::to_string(&record).map_err(err)?,
        score: summary.score,
        ev_loss_bb: Some(summary.ev_loss_bb),
        at: now_secs(),
    };
    state.db.lock().unwrap().add_attempt(&attempt).map_err(err)
}

fn finish_if_over(state: &AppState, current: &mut Current) -> CmdResult<()> {
    if current.hand.is_over() && !current.recorded {
        current.recorded = true;
        if !current.repeat {
            record(state, &current.hand)?;
        }
    }
    Ok(())
}

// -----------------------------------------------------------------------------------
// Commands.

fn new_seed() -> u64 {
    use std::hash::{BuildHasher, RandomState};
    // 53 bits, so the seed survives a round trip through a JavaScript number.
    RandomState::new().hash_one(std::time::SystemTime::now()) >> 11
}

/// Prepares spots for `config` without dealing yet (the start screen calls this as soon
/// as a choice is made).
#[tauri::command]
pub async fn play_prepare(config: Config, app: AppHandle) -> CmdResult<PoolStatus> {
    here();
    blocking(app.clone(), move |_| {
        config.validate()?;
        let p = pool(&app);
        let tables = config.deep_keys();
        p.set_plan((!tables.is_empty()).then_some(Plan { tables, hero_seat: config.hero_seat }));
        ensure_worker(&app);
        Ok(p.status())
    })
    .await
}

/// Deals a new hand; `seed` repeats a hand of the session history.
#[tauri::command]
pub async fn play_next(config: Config, seed: Option<u64>, app: AppHandle) -> CmdResult<HandView> {
    here();
    blocking(app.clone(), move |state| {
        config.validate()?;
        let p = pool(&app);
        let tables = config.deep_keys();
        p.set_plan((!tables.is_empty()).then_some(Plan { tables, hero_seat: config.hero_seat }));
        ensure_worker(&app);
        p.forget_live();
        let mut hand = Hand::start(&config, seed.unwrap_or_else(new_seed), Some(&p))?;
        hand.set_assess(ESTIMATE.clone());
        drive(&app, &mut hand);
        let mut current = Current { hand, repeat: seed.is_some(), recorded: false };
        finish_if_over(state, &mut current)?;
        let view = current.hand.view();
        *PLAY.current.lock().unwrap() = Some(current);
        Ok(view)
    })
    .await
}

/// The same hand again: same cards, board and opponents.
#[tauri::command]
pub async fn play_repeat(app: AppHandle) -> CmdResult<HandView> {
    here();
    blocking(app.clone(), move |_| {
        let mut slot = PLAY.current.lock().unwrap();
        let old = slot.as_ref().ok_or("Noch keine Hand gespielt.")?;
        pool(&app).forget_live();
        let mut hand = Hand::play(old.hand.deal.clone())?;
        hand.set_assess(ESTIMATE.clone());
        drive(&app, &mut hand);
        let view = hand.view();
        *slot = Some(Current { hand, repeat: true, recorded: false });
        Ok(view)
    })
    .await
}

/// The hand on screen, e.g. after coming back to the page.
#[tauri::command]
pub async fn play_current(app: AppHandle) -> CmdResult<Option<HandView>> {
    here();
    blocking(app, move |_| Ok(PLAY.current.lock().unwrap().as_ref().map(|c| c.hand.view()))).await
}

#[tauri::command]
pub async fn play_act(choice: usize, app: AppHandle) -> CmdResult<HandView> {
    here();
    blocking(app.clone(), move |state| {
        let mut slot = PLAY.current.lock().unwrap();
        let current = slot.as_mut().ok_or("Noch keine Hand gespielt.")?;
        current.hand.act(choice)?;
        drive(&app, &mut current.hand);
        finish_if_over(state, current)?;
        Ok(current.hand.view())
    })
    .await
}

/// Where the live solve the hand waits for stands.
#[tauri::command]
pub async fn play_live_status(app: AppHandle) -> CmdResult<Option<LiveStatus>> {
    blocking(app.clone(), move |_| Ok(pool(&app).live_status())).await
}

/// Stops the live solve; the hand goes on without a solution.
#[tauri::command]
pub async fn play_live_cancel(app: AppHandle) -> CmdResult<Option<HandView>> {
    blocking(app.clone(), move |_| {
        cancel_live(&app);
        Ok(PLAY.current.lock().unwrap().as_ref().map(|c| c.hand.view()))
    })
    .await
}

/// The strategy for the hero's hand before the move; the move then counts only half.
#[tauri::command]
pub async fn play_hint(app: AppHandle) -> CmdResult<Lesson> {
    here();
    blocking(app, move |_| {
        let mut slot = PLAY.current.lock().unwrap();
        let current = slot.as_mut().ok_or("Noch keine Hand gespielt.")?;
        current.hand.hint().ok_or_else(|| "Du bist gerade nicht am Zug.".into())
    })
    .await
}

#[tauri::command]
pub async fn play_pool_status(app: AppHandle) -> CmdResult<PoolStatus> {
    blocking(app.clone(), move |_| Ok(pool(&app).status())).await
}

/// Stops preparing spots until the next hand or choice.
#[tauri::command]
pub fn play_pool_cancel(app: AppHandle) {
    pool(&app).cancel();
}

/// The practice page was closed; the stock goes [`AWAY`] later unless it opens again.
#[tauri::command]
pub fn play_away() {
    PLAY.away.lock().unwrap().get_or_insert_with(Instant::now);
}

#[derive(Serialize)]
pub struct Crumb {
    step: Step,
    kind: Option<ringside_solver::ActionKind>,
    amount: f64,
}

#[derive(Serialize)]
pub struct Opened {
    spot: Spot,
    summary: ringside_solver::Summary,
    oop: String,
    ip: String,
    line: Vec<Crumb>,
    hero: Option<(ringside_solver::Player, Vec<String>)>,
}

/// Moves the finished hand's solve to the solver page, at the hero's costliest postflop
/// decision. The hand cannot be repeated afterwards.
#[tauri::command]
pub async fn play_open_solver(app: AppHandle) -> CmdResult<Opened> {
    blocking(app.clone(), move |_| {
        let mut slot = PLAY.current.lock().unwrap();
        let current = slot.as_ref().ok_or("Noch keine Hand gespielt.")?;
        let (id, path) = current.hand.worst_postflop().ok_or("Diese Hand hatte keine Entscheidung nach dem Flop.")?;
        let view = current.hand.view();
        let spot_info = view.spot.clone().ok_or("Kein Postflop-Spot")?;
        let tree = current.hand.tree().ok_or("Kein Postflop-Spot")?;
        let mut line = Vec::new();
        {
            let mut t = tree.lock().unwrap();
            for k in 0..path.len() {
                let crumb = match &path[k] {
                    Step::Card(_) => Crumb { step: path[k].clone(), kind: None, amount: 0.0 },
                    Step::Action(a) => {
                        let node = t.node(&path[..k]).map_err(err)?;
                        let action = node.actions.get(*a).ok_or("Pfad passt nicht")?;
                        Crumb { step: path[k].clone(), kind: Some(action.kind), amount: action.amount }
                    }
                };
                line.push(crumb);
            }
        }
        let hero_player = if view.hero == spot_info.oop { ringside_solver::Player::Oop } else { ringside_solver::Player::Ip };
        let hero_cards = view.hero_cards.iter().map(|c| c.to_string()).collect();
        // Only this hand and the stock hold the tree; both let go of it before it moves.
        pool(&app).release(id);
        *slot = None;
        drop(slot);
        let tree = Arc::try_unwrap(tree).map_err(|_| "Der Spot wird gerade noch benutzt.")?.into_inner().unwrap();
        let solution = tree.into_solution().ok_or("Dieser Spot stammt aus einer gespeicherten Lösung und lässt sich hier nicht öffnen.")?;
        let opened = Opened {
            spot: solution.spot.clone(),
            summary: solution.summary.clone(),
            oop: view.positions[spot_info.oop].to_string(),
            ip: view.positions[spot_info.ip].to_string(),
            line,
            hero: Some((hero_player, hero_cards)),
        };
        crate::solver::show_solution(solution, TARGET_PCT);
        Ok(opened)
    })
    .await
}

#[derive(Serialize)]
pub struct PastHand {
    seed: Option<u64>,
    at: i64,
    score: f64,
    ev_loss_bb: Option<f64>,
    #[serde(flatten)]
    record: Record,
}

/// The latest practiced hands from the database, newest first.
#[tauri::command]
pub async fn play_history(limit: usize, app: AppHandle) -> CmdResult<Vec<PastHand>> {
    blocking(app, move |state| {
        let attempts = state.db.lock().unwrap().attempts().map_err(err)?;
        let id = LearnKind::FullHands.id();
        Ok(attempts
            .into_iter()
            .rev()
            .filter(|a| a.kind == id)
            .filter_map(|a| {
                let record = serde_json::from_str(&a.answer).ok()?;
                Some(PastHand { seed: a.seed, at: a.at, score: a.score, ev_loss_bb: a.ev_loss_bb, record })
            })
            .take(limit)
            .collect())
    })
    .await
}
