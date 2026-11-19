mod analysis;
mod auto_analysis;
mod charts;
pub mod cloud;
mod coach;
mod drill;
mod importer;
mod jobs;
mod library;
mod memo;
mod memory;
mod next;
mod overlay;
#[cfg(test)]
mod perf;
mod play;
mod ranges;
mod report;
mod settings;
mod skill;
mod solutions;
mod solver;
mod spots;
mod store;

use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ringside_core::db::{Attempt, Db, Filter, GroupRow, HandQuery, HandSummary, PlayerRow, Results, Session, ShownHand};
use ringside_core::model::{GameKind, Hand};
use ringside_core::notes::{Notes, NotesSource, PlayerNote};
use ringside_core::stats::{StatValue, stat_values};
use ringside_core::table::{REFERENCE_ASPECT, SeatPos, layout_key, normalize_layouts, to_reference, visual_index};
use ringside_core::tournament::{Totals, TournamentRow, totals};
use ringside_learn::own::{check_spot, parse_spot_id, prepare_spot};
use ringside_learn::{Answer, Exercise, Feedback, Kind, OwnSpot, Progress, Read, detect_spots, player_read};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use importer::ImportStatus;
use overlay::{ClientWindow, Overlays, Rect};
use settings::{HudConfig, Profile, Settings};

pub struct AppState {
    pub db: Mutex<Db>,
    pub settings: Mutex<Settings>,
    pub import: Mutex<ImportStatus>,
    pub overlays: Overlays,
    /// The PokerStars client is open; advice features stay locked meanwhile.
    pub client_running: AtomicBool,
    pub notes: Mutex<NotesState>,
}

/// The user's player notes from the client, merged from every notes file found.
#[derive(Default)]
pub struct NotesState {
    pub notes: Notes,
    pub sources: Vec<NotesSource>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize)]
struct SeatView {
    seat: u8,
    visual: usize,
    name: String,
    stack: i64,
    stack_bb: f64,
    bounty: Option<i64>,
    is_hero: bool,
    stats: Vec<StatValue>,
    /// Player type; `None` while the client runs.
    read: Option<Read>,
    note: Option<PlayerNote>,
}

/// The hand that just ended, shown briefly on the table.
#[derive(Serialize)]
struct Recap {
    hand_id: u64,
    board: Vec<String>,
    pot: i64,
    /// Cards each player showed (the hero's own cards included), by seat.
    shown: Vec<(usize, Vec<String>)>,
    winners: Vec<(usize, i64)>,
    bounties: Vec<(usize, i64)>,
}

#[derive(Serialize)]
struct TableView {
    table_key: String,
    title: String,
    game: GameKind,
    max_seats: u8,
    big_blind: i64,
    currency: Option<String>,
    bounty_currency: Option<String>,
    hand_id: u64,
    seats: Vec<SeatView>,
    /// Where `layout` is saved: table size and window shape.
    layout_key: String,
    layout: Vec<SeatPos>,
    hud: HudConfig,
    recap: Recap,
}

fn build_view(
    db: &Db,
    settings: &Settings,
    notes: &Notes,
    hand: &Hand,
    title: String,
    aspect: f64,
    advice: bool,
) -> CmdResult<TableView> {
    let names: Vec<String> = hand.active_players().map(|s| s.name.clone()).collect();
    let game = settings.hud.split_by_game.then_some(hand.game);
    let rows = db.counters_for(&names, game).map_err(err)?;
    let visual_of = |name: &str| hand.seat_of(name).map(|s| visual_index(hand, s, settings.preferred_seating));
    let seats = hand
        .active_players()
        .zip(rows)
        .map(|(seat, row)| {
            let is_hero = hand.hero.as_deref() == Some(seat.name.as_str());
            let counters = if is_hero && settings.hud.hero_session_only {
                db.counters_in_session(&seat.name, hand).unwrap_or(row.counters)
            } else {
                row.counters
            };
            let stack = seat.stack + hand.net(&seat.name);
            let stats = stat_values(&counters);
            SeatView {
                seat: seat.seat,
                visual: visual_index(hand, seat.seat, settings.preferred_seating),
                name: seat.name.clone(),
                stack,
                stack_bb: if hand.big_blind > 0 { stack as f64 / hand.big_blind as f64 } else { 0.0 },
                bounty: seat.bounty,
                is_hero,
                read: advice.then(|| player_read(&stats)).flatten(),
                stats,
                note: notes.get(&seat.name).cloned(),
            }
        })
        .collect();
    let mut shown: Vec<(usize, Vec<String>)> =
        hand.shown.iter().filter_map(|(n, c)| visual_of(n).map(|v| (v, c.clone()))).collect();
    if let Some(hero) = hand.hero.as_deref().and_then(visual_of)
        && !hand.hero_cards.is_empty()
        && !shown.iter().any(|(v, _)| *v == hero)
    {
        shown.push((hero, hand.hero_cards.clone()));
    }
    let per_seat = |list: &[(String, i64)]| -> Vec<(usize, i64)> {
        list.iter().filter_map(|(n, a)| visual_of(n).map(|v| (v, *a))).collect()
    };
    Ok(TableView {
        table_key: hand.table_key(),
        title,
        game: hand.game,
        max_seats: hand.max_seats,
        big_blind: hand.big_blind,
        currency: hand.currency.clone(),
        bounty_currency: hand.bounty_currency.clone(),
        hand_id: hand.id,
        seats,
        layout_key: layout_key(hand.max_seats),
        layout: settings.layout(hand.max_seats, aspect),
        hud: settings.hud.clone(),
        recap: Recap {
            hand_id: hand.id,
            board: hand.board.clone(),
            pot: hand.total_pot,
            shown,
            winners: per_seat(&hand.collected),
            bounties: per_seat(&hand.bounties_won),
        },
    })
}

/// `aspect` is the overlay's width / height, which selects the layout for that window shape.
#[tauri::command]
async fn overlay_view(label: String, aspect: f64, state: State<'_, AppState>) -> CmdResult<Option<TableView>> {
    let Some(info) = state.overlays.by_label.lock().unwrap().get(&label).cloned() else { return Ok(None) };
    let Some(key) = info.table_key else { return Ok(None) };
    let db = state.db.lock().unwrap();
    let Some(hand) = db.latest_hand_for_table(&key).map_err(err)? else { return Ok(None) };
    let settings = state.settings.lock().unwrap();
    let notes = state.notes.lock().unwrap();
    build_view(&db, &settings, &notes.notes, &hand, info.title, aspect, !locked(&state)).map(Some)
}

/// The most recent real hand, for the HUD editor's preview.
#[tauri::command]
async fn preview_view(aspect: f64, state: State<'_, AppState>) -> CmdResult<Option<TableView>> {
    let db = state.db.lock().unwrap();
    let q = HandQuery { limit: 1, ..Default::default() };
    let Some(latest) = db.hands(&q).map_err(err)?.into_iter().next() else { return Ok(None) };
    let Some(hand) = db.hand(latest.id).map_err(err)? else { return Ok(None) };
    let settings = state.settings.lock().unwrap();
    let notes = state.notes.lock().unwrap();
    build_view(&db, &settings, &notes.notes, &hand, hand.table_name.clone(), aspect, !locked(&state)).map(Some)
}

#[derive(Serialize)]
struct LayoutView {
    key: String,
    layout: Vec<SeatPos>,
}

#[tauri::command]
fn layout_for(max_seats: u8, aspect: f64, state: State<'_, AppState>) -> LayoutView {
    let settings = state.settings.lock().unwrap();
    LayoutView { key: layout_key(max_seats), layout: settings.layout(max_seats, aspect) }
}

#[tauri::command]
fn set_hit_regions(label: String, regions: Vec<Rect>, state: State<'_, AppState>) {
    if let Some(info) = state.overlays.by_label.lock().unwrap().get_mut(&label) {
        info.hit_regions = regions;
    }
}

fn store_settings(app: &AppHandle, state: &AppState, settings: Settings) -> CmdResult<()> {
    let json = serde_json::to_string(&settings).map_err(err)?;
    state.db.lock().unwrap().set_setting("settings", &json).map_err(err)?;
    *state.settings.lock().unwrap() = settings.clone();
    app.emit("settings-changed", settings).map_err(err)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
async fn save_settings(mut settings: Settings, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    // A settings page opened before the cloud notice was answered must not undo it.
    settings.cloud = state.settings.lock().unwrap().cloud;
    settings.library = state.settings.lock().unwrap().library;
    settings.analysis = state.settings.lock().unwrap().analysis;
    store_settings(&app, &state, settings)
}

/// Saves one seat dragged in a window of shape `aspect`. The table size's layout is stored
/// once, so the change shows on every table of that size and in the editor.
#[tauri::command]
async fn save_seat(
    key: String,
    visual: usize,
    pos: SeatPos,
    aspect: f64,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let max_seats: u8 = key.parse().map_err(|_| "invalid layout key")?;
    let mut settings = state.settings.lock().unwrap().clone();
    let mut layout = settings.layout(max_seats, ringside_core::table::REFERENCE_ASPECT);
    let slot = layout.get_mut(visual).ok_or("seat outside the table")?;
    *slot = to_reference(pos.clamped(), aspect);
    settings.layouts.insert(key, layout);
    store_settings(&app, &state, settings)
}

#[tauri::command]
async fn reset_layout(key: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.layouts.remove(&key);
    store_settings(&app, &state, settings)
}

#[tauri::command]
async fn export_profile(path: String, state: State<'_, AppState>) -> CmdResult<()> {
    let profile = Profile::from_settings(&state.settings.lock().unwrap());
    let json = serde_json::to_string_pretty(&profile).map_err(err)?;
    std::fs::write(&path, json).map_err(err)
}

#[tauri::command]
async fn import_profile(path: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let text = std::fs::read_to_string(&path).map_err(err)?;
    let profile = Profile::parse(&text)?;
    let mut settings = state.settings.lock().unwrap().clone();
    profile.apply_to(&mut settings);
    store_settings(&app, &state, settings)
}

/// Back to the shipped profile; folders stay as they are.
#[tauri::command]
async fn reset_profile(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings::BUILTIN.clone().apply_to(&mut settings);
    settings.layouts.clear();
    store_settings(&app, &state, settings)
}

#[derive(Serialize)]
struct ClientWindowView {
    #[serde(flatten)]
    window: ClientWindow,
    has_hands: bool,
}

/// What the tracker currently sees of the poker client, for troubleshooting.
#[tauri::command]
async fn client_windows(state: State<'_, AppState>) -> CmdResult<Vec<ClientWindowView>> {
    let windows = state.overlays.client_windows.lock().unwrap().clone();
    let db = state.db.lock().unwrap();
    windows
        .into_iter()
        .map(|window| {
            let has_hands = match &window.table_key {
                Some(key) => db.latest_hand_for_table(key).map_err(err)?.is_some(),
                None => false,
            };
            Ok(ClientWindowView { window, has_hands })
        })
        .collect()
}

#[derive(Serialize)]
struct LiveTable {
    title: String,
    table_key: String,
    /// None until a hand of this table has been imported.
    table: Option<TableView>,
}

/// Tables the tracker currently sees, each with the players of its latest hand.
#[tauri::command]
async fn live_tables(state: State<'_, AppState>) -> CmdResult<Vec<LiveTable>> {
    let windows = state.overlays.client_windows.lock().unwrap().clone();
    let advice = !locked(&state);
    let db = state.db.lock().unwrap();
    let settings = state.settings.lock().unwrap();
    let notes = state.notes.lock().unwrap();
    let mut seen = HashSet::new();
    windows
        .into_iter()
        .filter_map(|w| Some((w.title, w.table_key?)))
        .filter(|(_, key)| seen.insert(key.clone()))
        .map(|(title, table_key)| {
            let table = match db.latest_hand_for_table(&table_key).map_err(err)? {
                Some(hand) => Some(build_view(&db, &settings, &notes.notes, &hand, title.clone(), REFERENCE_ASPECT, advice)?),
                None => None,
            };
            Ok(LiveTable { title, table_key, table })
        })
        .collect()
}

#[tauri::command]
fn poker_client_running(state: State<'_, AppState>) -> bool {
    state.client_running.load(Ordering::Relaxed)
}

#[derive(Serialize)]
struct TournamentsView {
    rows: Vec<TournamentRow>,
    totals: Vec<Totals>,
}

#[tauri::command]
async fn tournaments(filter: Filter, state: State<'_, AppState>) -> CmdResult<TournamentsView> {
    let rows = state.db.lock().unwrap().tournaments(&filter).map_err(err)?;
    Ok(TournamentsView { totals: totals(&rows), rows })
}

#[tauri::command]
fn player_note(player: String, state: State<'_, AppState>) -> Option<PlayerNote> {
    state.notes.lock().unwrap().notes.get(&player).cloned()
}

/// The notes files found, for the settings page.
#[tauri::command]
fn notes_sources(state: State<'_, AppState>) -> Vec<NotesSource> {
    state.notes.lock().unwrap().sources.clone()
}

#[tauri::command]
fn import_status(state: State<'_, AppState>) -> ImportStatus {
    state.import.lock().unwrap().clone()
}

#[tauri::command]
fn detected_dirs() -> Vec<String> {
    importer::effective_dirs(&[]).iter().map(|p| p.to_string_lossy().to_string()).collect()
}

#[derive(Serialize)]
struct Dashboard {
    hand_count: i64,
    heroes: Vec<String>,
    results: Results,
    hero: Option<PlayerView>,
}

#[tauri::command]
async fn dashboard(filter: Filter, state: State<'_, AppState>) -> CmdResult<Dashboard> {
    dashboard_view(&state, &filter)
}

fn dashboard_view(state: &AppState, filter: &Filter) -> CmdResult<Dashboard> {
    let db = state.db.lock().unwrap();
    let heroes = db.heroes().map_err(err)?;
    let hero = match heroes.first() {
        Some(name) => db
            .players(filter, name, 50)
            .map_err(err)?
            .into_iter()
            .find(|p| &p.name == name)
            .map(|r| PlayerView::new(r, !locked(state))),
        None => None,
    };
    Ok(Dashboard {
        hand_count: db.hand_count().map_err(err)?,
        results: db.hero_results(filter, 1500).map_err(err)?,
        heroes,
        hero,
    })
}

#[derive(Serialize)]
struct GroupView {
    key: String,
    net: i64,
    net_bb: f64,
    stats: Vec<StatValue>,
}

#[derive(Serialize)]
struct Reports {
    positions: Vec<GroupView>,
    starting_hands: Vec<GroupView>,
    sessions: Vec<Session>,
}

/// Rates the hero's stats against published reference ranges and says what to change: advice.
#[tauri::command]
async fn leaks(filter: Filter, state: State<'_, AppState>) -> CmdResult<ringside_core::leaks::LeakReport> {
    leak_report(&state, &filter)
}

fn leak_report(state: &AppState, filter: &Filter) -> CmdResult<ringside_core::leaks::LeakReport> {
    unlocked(state)?;
    Ok(ringside_core::leaks::find(&hero_cells(&state.db.lock().unwrap(), filter)?))
}

static CELLS: memo::Memo<((u64, u64), String), Vec<ringside_core::leaks::Cell>> = memo::Memo::new();

/// The leak finder's cells, which the leaks page and „Was als Nächstes?“ read alike.
fn hero_cells(db: &Db, filter: &Filter) -> CmdResult<Vec<ringside_core::leaks::Cell>> {
    CELLS.get_or((db.version(), format!("{filter:?}")), || db.hero_cells(filter).map_err(err))
}

#[tauri::command]
async fn reports(filter: Filter, state: State<'_, AppState>) -> CmdResult<Reports> {
    reports_view(&state.db.lock().unwrap(), &filter)
}

/// The hero's results by position alone, for the overview.
#[tauri::command]
async fn positions(filter: Filter, state: State<'_, AppState>) -> CmdResult<Vec<GroupView>> {
    Ok(group_view(state.db.lock().unwrap().hero_groups(&filter, false).map_err(err)?))
}

fn group_view(rows: Vec<GroupRow>) -> Vec<GroupView> {
    rows.into_iter().map(|r| GroupView { stats: stat_values(&r.counters), key: r.key, net: r.net, net_bb: r.net_bb }).collect()
}

fn reports_view(db: &Db, filter: &Filter) -> CmdResult<Reports> {
    let view = group_view;
    Ok(Reports {
        positions: view(db.hero_groups(filter, false).map_err(err)?),
        starting_hands: view(db.hero_groups(filter, true).map_err(err)?),
        sessions: db.sessions(filter).map_err(err)?,
    })
}

#[derive(Serialize)]
struct PlayerView {
    name: String,
    net: i64,
    stats: Vec<StatValue>,
    /// Player type; `None` while the client runs.
    read: Option<Read>,
}

impl PlayerView {
    fn new(row: PlayerRow, advice: bool) -> Self {
        let stats = stat_values(&row.counters);
        PlayerView { read: advice.then(|| player_read(&stats)).flatten(), stats, name: row.name, net: row.net }
    }
}

#[tauri::command]
async fn players(filter: Filter, search: String, limit: usize, state: State<'_, AppState>) -> CmdResult<Vec<PlayerView>> {
    let db = state.db.lock().unwrap();
    let rows = db.players(&filter, &search, limit).map_err(err)?;
    let advice = !locked(&state);
    Ok(rows.into_iter().map(|r| PlayerView::new(r, advice)).collect())
}

#[tauri::command]
async fn shown_hands(player: String, state: State<'_, AppState>) -> CmdResult<Vec<ShownHand>> {
    state.db.lock().unwrap().shown_hands(&player, 200).map_err(err)
}

#[tauri::command]
async fn hands(query: HandQuery, state: State<'_, AppState>) -> CmdResult<Vec<HandSummary>> {
    hand_list(&state, query)
}

/// While the client runs the list keeps results and cards but drops every rating: no worst
/// decision per hand, and no filter or order by ratings, which would give them away.
fn hand_list(state: &AppState, mut query: HandQuery) -> CmdResult<Vec<HandSummary>> {
    let locked = locked(state);
    if locked {
        query.decisions = None;
        query.sort_by_loss = false;
    }
    let mut rows = state.db.lock().unwrap().hands(&query).map_err(err)?;
    if locked {
        rows.iter_mut().for_each(|r| r.worst = None);
    }
    Ok(rows)
}

#[tauri::command]
async fn hand(id: u64, state: State<'_, AppState>) -> CmdResult<Option<Hand>> {
    state.db.lock().unwrap().hand(id).map_err(err)
}

/// PokerStars allows no advice while its client runs: no ratings against a model or solver, no
/// recommendations. Own stats, results and the hand history stay.
fn locked(state: &AppState) -> bool {
    state.client_running.load(Ordering::Relaxed)
}

fn unlocked(state: &AppState) -> CmdResult<()> {
    if locked(state) { Err("gesperrt".into()) } else { Ok(()) }
}

/// Runs `f` off the async runtime: exact equities and Nash solves take a while.
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        unlocked(&state)?;
        f(&state)
    })
    .await
    .map_err(err)?
}

/// Where the database and caches live. Debug builds take `RINGSIDE_DATA_DIR` instead, so the
/// end-to-end run (and `tauri dev`) can work on its own data; release builds ignore it.
fn data_dir(app: &AppHandle) -> tauri::Result<std::path::PathBuf> {
    match std::env::var_os("RINGSIDE_DATA_DIR") {
        Some(dir) if cfg!(debug_assertions) => Ok(dir.into()),
        _ => app.path().app_data_dir(),
    }
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn parse_kind(kind: &str) -> CmdResult<Kind> {
    Kind::parse(kind).ok_or_else(|| format!("Unbekannte Übung '{kind}'"))
}

fn record(state: &AppState, exercise: &Exercise, answer: &Answer, feedback: &Feedback) -> CmdResult<()> {
    let attempt = Attempt {
        kind: exercise.kind.id().into(),
        level: exercise.level,
        seed: exercise.hand_id.is_none().then_some(exercise.seed),
        hand_id: exercise.hand_id,
        answer: serde_json::to_string(answer).map_err(err)?,
        score: feedback.score,
        ev_loss_bb: feedback.ev_loss_bb,
        at: now_secs(),
    };
    state.db.lock().unwrap().add_attempt(&attempt).map_err(err)
}

fn check_own(state: &AppState, spot_id: &str, answer: &Answer) -> CmdResult<Feedback> {
    let (_, hand_id) = parse_spot_id(spot_id).ok_or("Ungültige Spot-ID")?;
    let hand = state.db.lock().unwrap().hand(hand_id).map_err(err)?.ok_or("Hand nicht gefunden")?;
    let (spot, feedback) = check_spot(&hand, spot_id, answer)?;
    record(state, &spot.exercise, answer, &feedback)?;
    Ok(feedback)
}

/// A generated exercise; without `seed` a fresh random one.
#[tauri::command]
async fn learn_exercise(kind: String, level: u8, seed: Option<u64>, app: AppHandle) -> CmdResult<Exercise> {
    blocking(app, move |_| {
        // 53 bits, so the seed survives a round trip through a JavaScript number.
        let seed = seed.unwrap_or_else(|| RandomState::new().hash_one(now_secs()) >> 11);
        ringside_learn::generate(parse_kind(&kind)?, level, seed)
    })
    .await
}

/// Grades and records an answer. The exercise is rebuilt from its kind, level and seed
/// (or its hand), so only those fields of `exercise` matter.
#[tauri::command]
async fn learn_check(exercise: Exercise, answer: Answer, app: AppHandle) -> CmdResult<Feedback> {
    blocking(app, move |state| match exercise.hand_id {
        Some(hand_id) => check_own(state, &format!("{}:{hand_id}", exercise.kind.id()), &answer),
        None => {
            let (exercise, feedback) =
                ringside_learn::check(exercise.kind, exercise.level, exercise.seed, &answer)?;
            record(state, &exercise, &answer, &feedback)?;
            Ok(feedback)
        }
    })
    .await
}

/// Spots from the hero's own hands, newest first; `kind` limits them to one kind.
#[tauri::command]
async fn learn_own_spots(filter: Filter, kind: Option<String>, limit: usize, app: AppHandle) -> CmdResult<Vec<OwnSpot>> {
    const PAGE: usize = 500;
    const MAX_SCANNED: usize = 20_000;
    blocking(app, move |state| {
        let kind = kind.as_deref().map(parse_kind).transpose()?;
        let mut spots = Vec::new();
        let mut offset = 0;
        while spots.len() < limit && offset < MAX_SCANNED {
            let raws = state.db.lock().unwrap().learn_candidates(&filter, PAGE, offset).map_err(err)?;
            if raws.is_empty() {
                break;
            }
            offset += raws.len();
            for hand in raws.iter().filter_map(|raw| ringside_core::parse_hand(raw).ok()) {
                spots.extend(detect_spots(&hand, kind));
                if spots.len() >= limit {
                    break;
                }
            }
        }
        spots.truncate(limit);
        Ok(spots)
    })
    .await
}

/// Grades and records an answer to a spot from `learn_own_spots`.
#[tauri::command]
async fn learn_check_own(spot_id: String, answer: Answer, app: AppHandle) -> CmdResult<Feedback> {
    blocking(app, move |state| check_own(state, &spot_id, &answer)).await
}

/// Solves the table of a push/fold spot from `learn_own_spots` while the user still
/// looks at it, so that `learn_check_own` answers at once.
#[tauri::command]
async fn learn_prepare_own(spot_id: String, app: AppHandle) -> CmdResult<()> {
    blocking(app, move |state| {
        let (_, hand_id) = parse_spot_id(&spot_id).ok_or("Ungültige Spot-ID")?;
        let hand = state.db.lock().unwrap().hand(hand_id).map_err(err)?.ok_or("Hand nicht gefunden")?;
        prepare_spot(&hand, &spot_id)
    })
    .await
}

#[tauri::command]
async fn learn_progress(app: AppHandle) -> CmdResult<Progress> {
    blocking(app, |state| {
        let attempts = state.db.lock().unwrap().attempts().map_err(err)?;
        Ok(ringside_learn::progress(&attempts, now_secs()))
    })
    .await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.unminimize();
                let _ = main.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let dir = data_dir(app.handle())?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("ringside.sqlite"))?;
            let mut settings: Settings = db
                .setting("settings")?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            normalize_layouts(&mut settings.layouts);
            app.manage(AppState {
                db: Mutex::new(db),
                settings: Mutex::new(settings),
                import: Mutex::new(ImportStatus::default()),
                overlays: Overlays::default(),
                // Until the first scan says otherwise, so nothing calculates in the moment before it.
                client_running: AtomicBool::new(cfg!(windows)),
                notes: Mutex::new(NotesState::default()),
            });
            app.manage(coach::CoachState::default());
            app.manage(analysis::AnalysisState::default());
            app.manage(cloud::CloudState::default());
            let handle = app.handle().clone();
            ringside_learn::set_solver(move |params| cloud::pushfold(&handle, params));
            app.manage(library::LibraryState::default());
            importer::spawn(app.handle().clone());
            overlay::spawn(app.handle().clone());
            store::spawn_migration(app.handle().clone());
            analysis::spawn(app.handle().clone());
            library::spawn(app.handle().clone());
            jobs::spawn(app.handle().clone());
            memory::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the stats window quits the app, overlays included.
            if window.label() == "main" && matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.app_handle().exit(0);
            }
            if let tauri::WindowEvent::Focused(focused) = event
                && window.label() == "main"
            {
                library::set_focused(*focused);
            }
        })
        .invoke_handler(tauri::generate_handler![
            overlay_view,
            preview_view,
            layout_for,
            set_hit_regions,
            get_settings,
            save_settings,
            save_seat,
            reset_layout,
            export_profile,
            import_profile,
            reset_profile,
            import_status,
            client_windows,
            live_tables,
            poker_client_running,
            detected_dirs,
            dashboard,
            positions,
            reports,
            leaks,
            ranges::range_groups,
            ranges::range_chart,
            tournaments,
            player_note,
            notes_sources,
            players,
            shown_hands,
            hands,
            hand,
            learn_exercise,
            learn_check,
            learn_own_spots,
            learn_check_own,
            learn_prepare_own,
            learn_progress,
            drill::learn_profile,
            drill::learn_drill_real,
            drill::learn_drill_variant,
            drill::learn_drill_check,
            spots::learn_key_spots,
            spots::learn_key_spot,
            spots::learn_key_spot_check,
            charts::pushfold_grid,
            charts::pushfold_chart,
            charts::preflop_grid,
            charts::preflop_spots,
            charts::table_opens,
            charts::preflop_node,
            charts::preflop_cancel,
            analysis::analysis_status,
            analysis::analysis_run,
            analysis::analysis_hands,
            analysis::analysis_cancel,
            analysis::analysis_cancel_hand,
            analysis::analysis_decisions,
            analysis::analysis_chart_target,
            analysis::analysis_summary,
            analysis::analysis_set_auto,
            report::tournament_report,
            next::next_steps,
            report::session_review,
            skill::skill_profile,
            solver::solver_estimate,
            solver::solver_start,
            solver::solver_model_spot,
            solver::solver_cancel,
            solver::solver_clear,
            solver::solver_node,
            solver::solver_page,
            solutions::solutions_list,
            solutions::solutions_open,
            solutions::solutions_pin,
            solutions::solutions_delete,
            solutions::solutions_set_limit,
            jobs::jobs_list,
            jobs::jobs_cancel,
            jobs::jobs_clear,
            coach::coach_status,
            coach::coach_set_key,
            coach::coach_test_key,
            coach::coach_delete_key,
            coach::coach_models,
            coach::coach_set_model,
            coach::coach_set_effort,
            coach::coach_prepare,
            coach::coach_run,
            coach::coach_cancel,
            coach::coach_reports,
            coach::coach_report,
            coach::coach_delete_report,
            coach::coach_concepts,
            coach::coach_ask_prepare,
            coach::coach_ask,
            cloud::cloud_settings,
            cloud::cloud_save_settings,
            cloud::cloud_ack_notice,
            cloud::cloud_malformed,
            cloud::cloud_dismiss_malformed,
            cloud::cloud_send_report,
            library::library_overview,
            library::library_set_mode,
            library::library_set_power,
            library::library_run,
            library::library_open,
            play::play_prepare,
            play::play_next,
            play::play_repeat,
            play::play_current,
            play::play_act,
            play::play_live_status,
            play::play_live_cancel,
            play::play_hint,
            play::play_pool_status,
            play::play_pool_cancel,
            play::play_away,
            play::play_open_solver,
            play::play_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ringside");
}

#[cfg(test)]
mod tests {
    use ringside_core::db::{DecisionFilter, HandQuery};

    use super::*;
    use crate::solutions::{analysed_state as analysed, test_state};

    fn query() -> HandQuery {
        HandQuery { limit: 100, ..Default::default() }
    }

    #[test]
    fn the_leak_finder_is_locked_while_pokerstars_runs() {
        let state = analysed();
        assert!(leak_report(&state, &Filter::default()).is_ok());
        state.client_running.store(true, Ordering::Relaxed);
        assert_eq!(leak_report(&state, &Filter::default()).unwrap_err(), "gesperrt");
    }

    #[test]
    fn the_hand_list_keeps_results_but_loses_ratings_while_pokerstars_runs() {
        let state = analysed();
        let open = hand_list(&state, query()).unwrap();
        assert!(open.iter().any(|h| h.worst.is_some()), "the fixtures must give a rated hand");

        state.client_running.store(true, Ordering::Relaxed);
        let plain = hand_list(&state, query()).unwrap();
        assert_eq!(plain.len(), open.len());
        assert!(plain.iter().all(|h| h.worst.is_none()));
        assert!(plain.iter().zip(&open).all(|(a, b)| a.id == b.id && a.hero_net == b.hero_net));

        // Filtering or sorting by rating would show which hands went wrong.
        let by_rating = HandQuery {
            decisions: Some(DecisionFilter { unrated: true, ..Default::default() }),
            sort_by_loss: true,
            ..query()
        };
        let ids = |l: &[HandSummary]| l.iter().map(|h| h.id).collect::<Vec<_>>();
        assert_eq!(ids(&hand_list(&state, by_rating).unwrap()), ids(&plain));
    }

    #[test]
    fn the_gate_of_blocking_commands_refuses_while_pokerstars_runs() {
        let state = test_state();
        assert!(unlocked(&state).is_ok());
        state.client_running.store(true, Ordering::Relaxed);
        assert_eq!(unlocked(&state).unwrap_err(), "gesperrt");
    }
}
