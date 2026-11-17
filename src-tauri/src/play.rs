//! Endless practice with whole hands ("Hände spielen"): the current hand, the stock of
//! prepared postflop spots and what finished hands add to the learning area.
//!
//! The stock's worker starts with the first hand and keeps running whichever page is open;
//! it pauses while PokerStars runs. Each round of filling it is a task in the task center
//! (`jobs`) and also goes to the page as `play-pool`. Before it solves a spot it looks for a
//! saved solution (`solutions`) and in the spot library.

use std::sync::atomic::Ordering;
use std::sync::{Arc, LazyLock, Mutex, Once};

use ringside_core::db::Attempt;
use ringside_learn::Kind as LearnKind;
use ringside_play::hand::Source;
use ringside_play::pool::{Lookup, TARGET_PCT};
use ringside_play::tree::{LibraryTree, Tree};
use ringside_play::{Config, Hand, HandView, Lesson, Plan, Pool, PoolStatus, Status};
use ringside_solver::library::format::Stored;
use ringside_solver::library::{SpotId, catalog};
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
}

static PLAY: LazyLock<PlayState> =
    LazyLock::new(|| PlayState { current: Mutex::new(None), worker: Once::new(), job: Mutex::new(None) });

static POOL: LazyLock<Mutex<Option<Arc<Pool>>>> = LazyLock::new(|| Mutex::new(None));

fn pool(app: &AppHandle) -> Arc<Pool> {
    let mut slot = POOL.lock().unwrap();
    if let Some(p) = slot.as_ref() {
        return p.clone();
    }
    // Few threads and a quarter of the memory, so the computer and the page stay quick.
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let threads = (cores / 2).clamp(1, 4);
    let p = Arc::new(Pool::new(Box::new(AppLookup(app.clone())), threads, crate::solver::memory_limit() / 2));
    *slot = Some(p.clone());
    p
}

fn client_running(app: &AppHandle) -> bool {
    app.state::<AppState>().client_running.load(Ordering::Relaxed)
}

/// Starts the stock's worker once per process.
fn ensure_worker(app: &AppHandle) {
    PLAY.worker.call_once(|| {
        let app = app.clone();
        let pool = pool(&app);
        std::thread::spawn(move || {
            let paused_app = app.clone();
            let report_app = app.clone();
            pool.run(|| false, move || client_running(&paused_app), move |s| report(&report_app, s));
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

// -----------------------------------------------------------------------------------
// Stored solutions.

struct AppLookup(AppHandle);

/// The catalog scenario with exactly these ranges, pot, stack and sizes.
fn catalog_scenario(spot: &Spot) -> Option<usize> {
    catalog().scenarios.iter().position(|s| {
        s.oop_range == spot.oop_range
            && s.ip_range == spot.ip_range
            && (s.pot - spot.pot).abs() < 0.011
            && (s.stack - spot.stack).abs() < 0.011
            && s.sizes == spot.sizes
            && s.allin_threshold == spot.allin_threshold
    })
}

impl Lookup for AppLookup {
    fn load(&self, spot: &Spot) -> Option<Box<dyn Tree>> {
        if let Some((_, snapshot)) = crate::solutions::lookup(&self.0, spot, TARGET_PCT) {
            return Some(Box::new(snapshot));
        }
        let scenario = catalog_scenario(spot)?;
        let id = SpotId { scenario, flop: catalog().flop(&spot.board)? };
        let root = crate::library::root(&self.0)?;
        let source = crate::library::status_of(&root, id)?;
        let bytes = std::fs::read(crate::library::spot_path(&root, source, id)).ok()?;
        Some(Box::new(LibraryTree(Stored::from_bytes(&bytes, catalog()).ok()?)))
    }

    fn flops(&self, spot: &Spot) -> Vec<String> {
        let (Some(scenario), Some(root)) = (catalog_scenario(spot), crate::library::root(&self.0)) else { return Vec::new() };
        let c = catalog();
        (0..c.flops.len())
            .filter(|&flop| crate::library::status_of(&root, SpotId { scenario, flop }).is_some())
            .map(|flop| c.flops[flop].clone())
            .collect()
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
        postflop: view.verdicts.iter().any(|v| v.lesson.source != Source::Model && v.lesson.source != Source::Equilibrium),
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
    blocking(app.clone(), move |state| {
        config.validate()?;
        let p = pool(&app);
        let tables = config.deep_keys();
        p.set_plan((!tables.is_empty()).then_some(Plan { tables, hero_seat: config.hero_seat }));
        ensure_worker(&app);
        let hand = Hand::start(&config, seed.unwrap_or_else(new_seed), Some(&p))?;
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
    blocking(app, move |_| {
        let mut slot = PLAY.current.lock().unwrap();
        let old = slot.as_ref().ok_or("Noch keine Hand gespielt.")?;
        let hand = Hand::play(old.hand.deal.clone())?;
        let view = hand.view();
        *slot = Some(Current { hand, repeat: true, recorded: false });
        Ok(view)
    })
    .await
}

/// The hand on screen, e.g. after coming back to the page.
#[tauri::command]
pub async fn play_current(app: AppHandle) -> CmdResult<Option<HandView>> {
    blocking(app, move |_| Ok(PLAY.current.lock().unwrap().as_ref().map(|c| c.hand.view()))).await
}

#[tauri::command]
pub async fn play_act(choice: usize, app: AppHandle) -> CmdResult<HandView> {
    blocking(app, move |state| {
        let mut slot = PLAY.current.lock().unwrap();
        let current = slot.as_mut().ok_or("Noch keine Hand gespielt.")?;
        current.hand.act(choice)?;
        finish_if_over(state, current)?;
        Ok(current.hand.view())
    })
    .await
}

/// The strategy for the hero's hand before the move; the move then counts only half.
#[tauri::command]
pub async fn play_hint(app: AppHandle) -> CmdResult<Lesson> {
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
        crate::solver::show_solution(solution);
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
