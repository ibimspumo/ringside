mod importer;
mod overlay;
mod settings;

use std::sync::Mutex;

use ringside_core::db::{Db, HandSummary, ResultPoint};
use ringside_core::model::{GameKind, Hand};
use ringside_core::stats::{StatValue, stat_values};
use ringside_core::table::{SeatPos, visual_index};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use importer::ImportStatus;
use overlay::{ClientWindow, Overlays, Rect};
use settings::{HudConfig, Settings};

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

fn game_arg(game: Option<String>) -> Option<GameKind> {
    game.as_deref().and_then(GameKind::parse)
}

#[derive(Serialize)]
struct SeatView {
    seat: u8,
    visual: usize,
    name: String,
    stack: i64,
    stack_bb: f64,
    is_hero: bool,
    stats: Vec<StatValue>,
}

#[derive(Serialize)]
struct TableView {
    table_key: String,
    title: String,
    game: GameKind,
    max_seats: u8,
    big_blind: i64,
    currency: Option<String>,
    hand_id: u64,
    seats: Vec<SeatView>,
    layout: Vec<SeatPos>,
    hud: HudConfig,
}

fn build_view(db: &Db, settings: &Settings, hand: &Hand, title: String) -> CmdResult<TableView> {
    let names: Vec<String> = hand.active_players().map(|s| s.name.clone()).collect();
    let game = settings.hud.split_by_game.then_some(hand.game);
    let rows = db.counters_for(&names, game).map_err(err)?;
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
                is_hero: hand.hero.as_deref() == Some(seat.name.as_str()),
                stats: stat_values(&row.counters),
            }
        })
        .collect();
    Ok(TableView {
        table_key: hand.table_key(),
        title,
        game: hand.game,
        max_seats: hand.max_seats,
        big_blind: hand.big_blind,
        currency: hand.currency.clone(),
        hand_id: hand.id,
        seats,
        layout: settings.layout(hand.max_seats),
        hud: settings.hud.clone(),
    })
}

#[tauri::command]
fn overlay_view(label: String, state: State<AppState>) -> CmdResult<Option<TableView>> {
    let Some(info) = state.overlays.by_label.lock().unwrap().get(&label).cloned() else { return Ok(None) };
    let Some(key) = info.table_key else { return Ok(None) };
    let db = state.db.lock().unwrap();
    let Some(hand) = db.latest_hand_for_table(&key).map_err(err)? else { return Ok(None) };
    let settings = state.settings.lock().unwrap();
    build_view(&db, &settings, &hand, info.title).map(Some)
}

/// The most recent hand of any table, for the HUD preview in the settings.
#[tauri::command]
fn preview_view(state: State<AppState>) -> CmdResult<Option<TableView>> {
    let db = state.db.lock().unwrap();
    let Some(latest) = db.hands(None, None, 1, 0).map_err(err)?.into_iter().next() else { return Ok(None) };
    let Some(hand) = db.hand(latest.id).map_err(err)? else { return Ok(None) };
    let settings = state.settings.lock().unwrap();
    build_view(&db, &settings, &hand, hand.table_name.clone()).map(Some)
}

#[tauri::command]
fn set_hit_regions(label: String, regions: Vec<Rect>, state: State<AppState>) {
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
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(settings: Settings, app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    store_settings(&app, &state, settings)
}

#[tauri::command]
fn save_seat(max_seats: u8, visual: usize, pos: SeatPos, app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    let mut layout = settings.layout(max_seats);
    if let Some(slot) = layout.get_mut(visual) {
        *slot = SeatPos { x: pos.x.clamp(0.0, 1.0), y: pos.y.clamp(0.0, 1.0) };
    }
    settings.layouts.insert(max_seats.to_string(), layout);
    store_settings(&app, &state, settings)
}

#[tauri::command]
fn reset_layout(max_seats: u8, app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.layouts.remove(&max_seats.to_string());
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
fn client_windows(state: State<AppState>) -> CmdResult<Vec<ClientWindowView>> {
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
fn import_status(state: State<AppState>) -> ImportStatus {
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
    results: Vec<ResultPoint>,
}

#[tauri::command]
fn dashboard(game: Option<String>, state: State<AppState>) -> CmdResult<Dashboard> {
    let db = state.db.lock().unwrap();
    Ok(Dashboard {
        hand_count: db.hand_count().map_err(err)?,
        heroes: db.heroes().map_err(err)?,
        results: db.hero_results(game_arg(game)).map_err(err)?,
    })
}

#[derive(Serialize)]
struct PlayerView {
    name: String,
    net: i64,
    stats: Vec<StatValue>,
}

#[tauri::command]
fn players(game: Option<String>, search: String, limit: usize, state: State<AppState>) -> CmdResult<Vec<PlayerView>> {
    let db = state.db.lock().unwrap();
    let rows = db.players(game_arg(game), &search, limit).map_err(err)?;
    Ok(rows.into_iter().map(|r| PlayerView { stats: stat_values(&r.counters), name: r.name, net: r.net }).collect())
}

#[tauri::command]
fn hands(
    game: Option<String>,
    player: Option<String>,
    limit: usize,
    offset: usize,
    state: State<AppState>,
) -> CmdResult<Vec<HandSummary>> {
    state.db.lock().unwrap().hands(game_arg(game), player.as_deref(), limit, offset).map_err(err)
}

#[tauri::command]
fn hand(id: u64, state: State<AppState>) -> CmdResult<Option<Hand>> {
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
            set_hit_regions,
            get_settings,
            save_settings,
            save_seat,
            reset_layout,
            import_status,
            client_windows,
            detected_dirs,
            dashboard,
            players,
            hands,
            hand,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ringside");
}
