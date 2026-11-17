mod importer;
mod overlay;
mod settings;

use std::sync::Mutex;

use ringside_core::db::{Db, Filter, GroupRow, HandQuery, HandSummary, Results, Session, ShownHand};
use ringside_core::model::{GameKind, Hand};
use ringside_core::stats::{StatValue, stat_values};
use ringside_core::table::{SeatPos, layout_key, visual_index};
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

fn build_view(db: &Db, settings: &Settings, hand: &Hand, title: String, aspect: f64) -> CmdResult<TableView> {
    let names: Vec<String> = hand.active_players().map(|s| s.name.clone()).collect();
    let game = settings.hud.split_by_game.then_some(hand.game);
    let rows = db.counters_for(&names, game).map_err(err)?;
    let visual_of = |name: &str| hand.seat_of(name).map(|s| visual_index(hand, s, settings.preferred_seating));
    let seats = hand
        .active_players()
        .zip(rows)
        .map(|(seat, row)| {
            let stack = seat.stack + hand.net(&seat.name);
            SeatView {
                seat: seat.seat,
                visual: visual_index(hand, seat.seat, settings.preferred_seating),
                name: seat.name.clone(),
                stack,
                stack_bb: if hand.big_blind > 0 { stack as f64 / hand.big_blind as f64 } else { 0.0 },
                bounty: seat.bounty,
                is_hero: hand.hero.as_deref() == Some(seat.name.as_str()),
                stats: stat_values(&row.counters),
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
        layout_key: layout_key(hand.max_seats, aspect),
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
    build_view(&db, &settings, &hand, info.title, aspect).map(Some)
}

/// The most recent real hand, for the HUD editor's preview.
#[tauri::command]
async fn preview_view(aspect: f64, state: State<'_, AppState>) -> CmdResult<Option<TableView>> {
    let db = state.db.lock().unwrap();
    let q = HandQuery { limit: 1, ..Default::default() };
    let Some(latest) = db.hands(&q).map_err(err)?.into_iter().next() else { return Ok(None) };
    let Some(hand) = db.hand(latest.id).map_err(err)? else { return Ok(None) };
    let settings = state.settings.lock().unwrap();
    build_view(&db, &settings, &hand, hand.table_name.clone(), aspect).map(Some)
}

#[derive(Serialize)]
struct LayoutView {
    key: String,
    layout: Vec<SeatPos>,
}

#[tauri::command]
fn layout_for(max_seats: u8, aspect: f64, state: State<'_, AppState>) -> LayoutView {
    let settings = state.settings.lock().unwrap();
    LayoutView { key: layout_key(max_seats, aspect), layout: settings.layout(max_seats, aspect) }
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
async fn save_settings(settings: Settings, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    store_settings(&app, &state, settings)
}

/// Saves one seat of the layout stored under `key` (see [`layout_key`]).
#[tauri::command]
async fn save_seat(key: String, visual: usize, pos: SeatPos, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let (max_seats, aspect) = parse_layout_key(&key)?;
    let mut settings = state.settings.lock().unwrap().clone();
    let mut layout = settings.layout(max_seats, aspect);
    let slot = layout.get_mut(visual).ok_or("seat outside the table")?;
    *slot = pos.clamped();
    settings.layouts.insert(key, layout);
    store_settings(&app, &state, settings)
}

#[tauri::command]
async fn reset_layout(key: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.layouts.remove(&key);
    store_settings(&app, &state, settings)
}

fn parse_layout_key(key: &str) -> CmdResult<(u8, f64)> {
    let (max, aspect) = key.split_once('@').ok_or("invalid layout key")?;
    Ok((max.parse().map_err(err)?, aspect.parse().map_err(err)?))
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
    let db = state.db.lock().unwrap();
    let heroes = db.heroes().map_err(err)?;
    let hero = match heroes.first() {
        Some(name) => db
            .players(&filter, name, 50)
            .map_err(err)?
            .into_iter()
            .find(|p| &p.name == name)
            .map(|r| PlayerView { stats: stat_values(&r.counters), name: r.name, net: r.net }),
        None => None,
    };
    Ok(Dashboard {
        hand_count: db.hand_count().map_err(err)?,
        results: db.hero_results(&filter, 1500).map_err(err)?,
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

#[tauri::command]
async fn reports(filter: Filter, state: State<'_, AppState>) -> CmdResult<Reports> {
    let db = state.db.lock().unwrap();
    let view = |rows: Vec<GroupRow>| -> Vec<GroupView> {
        rows.into_iter()
            .map(|r| GroupView { stats: stat_values(&r.counters), key: r.key, net: r.net, net_bb: r.net_bb })
            .collect()
    };
    Ok(Reports {
        positions: view(db.hero_groups(&filter, false).map_err(err)?),
        starting_hands: view(db.hero_groups(&filter, true).map_err(err)?),
        sessions: db.sessions(&filter, 30.0).map_err(err)?,
    })
}

#[derive(Serialize)]
struct PlayerView {
    name: String,
    net: i64,
    stats: Vec<StatValue>,
}

#[tauri::command]
async fn players(filter: Filter, search: String, limit: usize, state: State<'_, AppState>) -> CmdResult<Vec<PlayerView>> {
    let db = state.db.lock().unwrap();
    let rows = db.players(&filter, &search, limit).map_err(err)?;
    Ok(rows.into_iter().map(|r| PlayerView { stats: stat_values(&r.counters), name: r.name, net: r.net }).collect())
}

#[tauri::command]
async fn shown_hands(player: String, state: State<'_, AppState>) -> CmdResult<Vec<ShownHand>> {
    state.db.lock().unwrap().shown_hands(&player, 200).map_err(err)
}

#[tauri::command]
async fn hands(query: HandQuery, state: State<'_, AppState>) -> CmdResult<Vec<HandSummary>> {
    state.db.lock().unwrap().hands(&query).map_err(err)
}

#[tauri::command]
async fn hand(id: u64, state: State<'_, AppState>) -> CmdResult<Option<Hand>> {
    state.db.lock().unwrap().hand(id).map_err(err)
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
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let db = Db::open(&dir.join("ringside.sqlite"))?;
            let settings = db
                .setting("settings")?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            app.manage(AppState {
                db: Mutex::new(db),
                settings: Mutex::new(settings),
                import: Mutex::new(ImportStatus::default()),
                overlays: Overlays::default(),
            });
            importer::spawn(app.handle().clone());
            overlay::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the stats window quits the app, overlays included.
            if window.label() == "main" && matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.app_handle().exit(0);
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
            detected_dirs,
            dashboard,
            reports,
            players,
            shown_hands,
            hands,
            hand,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ringside");
}
