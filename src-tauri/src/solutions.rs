//! "Meine Lösungen": the solves kept on this computer, described for the list, and the
//! commands of the list. The files themselves belong to the spot store (`store`), which
//! every area asks before it solves:
//! ```ignore
//! let keyed = store::keyed(&spot, model.as_ref())?;
//! store::resolve_app(&app, &Ask::new(&spot, &keyed, target_pct), &policy, keep_waiting, use_found, solve);
//! // after solving:
//! solutions::save(&app, &mut solution, &settings, &SaveInfo { source: Some(SolutionSource::Hand), .. })?;
//! ```
//! The folder stays below a limit the user sets: the oldest solutions go first, pinned
//! ones never.

use ringside_core::db::{SavedSolution, SolutionSource};
use ringside_engine::preflop_table::ModelLine;
use ringside_solver::{Settings, Solution, Spot, Summary};
use ringside_store::Origin;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::store::{self, data};
use crate::{AppState, CmdResult, blocking, err};

const LIMIT_KEY: &str = "solutions_max_mb";
const DEFAULT_MAX_MB: u64 = 500;
const MB: u64 = 1 << 20;

/// What the page knows about a spot that the solver does not: positions, where the ranges
/// came from, and where the spot came from.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SaveInfo {
    /// The spot in plain words; empty for a title made from the board.
    pub title: String,
    pub oop: String,
    pub ip: String,
    pub ranges: String,
    pub source: Option<SolutionSource>,
    pub hand_id: Option<u64>,
    /// The preflop line the ranges come from, when they are exactly the model's: the
    /// spot is then filed under its cloud key and shared.
    pub model: Option<ModelLine>,
}

fn pct(x: f64) -> String {
    let s = format!("{x:.0}");
    s.replace('.', ",")
}

/// The sizes of the streets still to come, e.g. "Flop 33/75 %, Raise 60 % · Turn 66 %".
pub(crate) fn sizes_text(spot: &Spot) -> String {
    let dealt = spot.board.chars().filter(|c| c.is_ascii_alphanumeric()).count() / 2;
    let first = dealt.clamp(3, 5) - 3;
    ["Flop", "Turn", "River"]
        .iter()
        .zip(&spot.sizes)
        .skip(first)
        .map(|(street, s)| {
            let list = |v: &[f64]| v.iter().map(|&x| pct(x)).collect::<Vec<_>>().join("/");
            match (s.bet.is_empty(), s.raise.is_empty()) {
                (true, _) => format!("{street} nur Check"),
                (false, true) => format!("{street} {} %", list(&s.bet)),
                (false, false) => format!("{street} {} %, Raise {} %", list(&s.bet), list(&s.raise)),
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

pub(crate) fn board_text(spot: &Spot) -> String {
    spot.board.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

pub(crate) fn max_bytes(state: &AppState) -> u64 {
    let mb = state.db.lock().unwrap().setting(LIMIT_KEY).ok().flatten().and_then(|v| v.parse().ok());
    mb.unwrap_or(DEFAULT_MAX_MB) * MB
}

/// Saves a finished solve in the store, keeps the folder within its limit and offers the
/// spot to the cloud if its ranges come from the model; returns its id.
pub fn save(app: &AppHandle, solution: &mut Solution, settings: &Settings, info: &SaveInfo) -> CmdResult<i64> {
    let state = app.state::<AppState>();
    let data = data(app)?;
    let (id, keyed) = store::save_solution(&data, &state, solution, settings, info)?;
    store::prune_solutions(&data, &state, max_bytes(&state))?;
    store::share(app, &keyed, solution);
    Ok(id)
}

/// A saved solution with what became of it in Ringside Cloud, if it was offered.
#[derive(Serialize)]
pub struct Listed {
    #[serde(flatten)]
    saved: SavedSolution,
    origin: Origin,
    /// Its ranges come from the preflop model, so it can be shared.
    shareable: bool,
    shared: Option<crate::library::Shared>,
}

/// A spot in the store that someone else solved: a model flop from the cloud.
#[derive(Serialize)]
pub struct Fetched {
    id: i64,
    origin: Origin,
    board: String,
    exploitability_pct: f64,
    bytes: u64,
    created_at: i64,
    /// Players, depth and positions, e.g. "9 Spieler, 40 BB: BTN gegen BB".
    title: String,
}

/// Library spots in the store by origin.
#[derive(Default, Serialize)]
pub struct LibraryCount {
    own: i64,
    library: i64,
}

#[derive(Serialize)]
pub struct SolutionsView {
    items: Vec<Listed>,
    fetched: Vec<Fetched>,
    library: LibraryCount,
    bytes: u64,
    max_mb: u64,
    /// The user shares own solutions.
    share: bool,
}

fn fetched_title(input: &str) -> String {
    let Ok(input) = serde_json::from_str::<ringside_cloud::kinds::SpotInput>(input) else { return "Spot aus der Cloud".into() };
    let names = ringside_engine::multiway::position_names(input.players as usize);
    let seats: Vec<&str> = input.line.iter().map(|(seat, _, _)| names.get(*seat as usize).copied().unwrap_or("?")).collect();
    let (first, last) = (seats.first().copied().unwrap_or("?"), seats.last().copied().unwrap_or("?"));
    format!("{} Spieler, {} BB: {first} gegen {last}", input.players, input.stack_bb)
}

#[tauri::command]
pub async fn solutions_list(app: AppHandle) -> CmdResult<SolutionsView> {
    let log = crate::cloud::spot_log(&app);
    blocking(app, move |state| {
        let db = state.db.lock().unwrap();
        let rows = db.store_rows(None).map_err(err)?;
        let items = db
            .solutions()
            .map_err(err)?
            .into_iter()
            .map(|saved| {
                let row = rows.iter().find(|r| r.solution_id == Some(saved.id));
                let shared = log.get(&saved.info.spot_key).map(|s| s.shared.clone());
                let shareable = row.is_some_and(|r| !r.key.starts_with("local:"));
                Listed { saved, origin: Origin::Own, shareable, shared }
            })
            .collect();
        let mut library = LibraryCount::default();
        let mut fetched = Vec::new();
        for r in &rows {
            match (r.input.as_deref(), store::origin_of(&r.origin), r.solution_id) {
                (Some(input), origin, _) => fetched.push(Fetched {
                    id: r.id,
                    origin,
                    board: r.board.clone(),
                    exploitability_pct: r.exploitability_pct,
                    bytes: r.bytes,
                    created_at: r.created_at,
                    title: fetched_title(input),
                }),
                (None, Origin::Own, None) => library.own += 1,
                (None, Origin::Library, None) => library.library += 1,
                _ => {}
            }
        }
        Ok(SolutionsView {
            items,
            fetched,
            library,
            bytes: db.solutions_bytes().map_err(err)?,
            max_mb: db.setting(LIMIT_KEY).ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_MAX_MB),
            share: state.settings.lock().unwrap().cloud.share,
        })
    })
    .await
}

#[derive(Serialize)]
pub struct Opened {
    id: i64,
    spot: Spot,
    summary: Summary,
    info: SavedSolution,
}

/// Shows a saved solution on the solver page; its nodes come from `solver_node`.
#[tauri::command]
pub async fn solutions_open(id: i64, app: AppHandle) -> CmdResult<Opened> {
    let handle = app.clone();
    blocking(app, move |state| {
        let (row, snapshot) = store::open_snapshot(&data(&handle)?, state, id)?;
        let opened = Opened { id, spot: snapshot.meta.spot.clone(), summary: snapshot.meta.summary.clone(), info: row };
        crate::solver::show_saved(snapshot);
        Ok(opened)
    })
    .await
}

#[tauri::command]
pub async fn solutions_pin(id: i64, pinned: bool, app: AppHandle) -> CmdResult<()> {
    blocking(app, move |state| {
        state.db.lock().unwrap().set_solution_pinned(id, pinned).map_err(err)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn solutions_delete(id: i64, app: AppHandle) -> CmdResult<()> {
    let handle = app.clone();
    blocking(app, move |state| {
        store::delete_solution(&data(&handle)?, state, id)
    })
    .await
}

/// Sets the disk limit in MB and deletes what no longer fits; returns how many went.
#[tauri::command]
pub async fn solutions_set_limit(max_mb: u64, app: AppHandle) -> CmdResult<usize> {
    let handle = app.clone();
    blocking(app, move |state| {
        if !(50..=100_000).contains(&max_mb) {
            return Err("Zwischen 50 MB und 100 GB.".into());
        }
        state.db.lock().unwrap().set_setting(LIMIT_KEY, &max_mb.to_string()).map_err(err)?;
        store::prune_solutions(&data(&handle)?, state, max_bytes(state))
    })
    .await
}

/// An app state on an empty database in memory.
#[cfg(test)]
pub(crate) fn test_state() -> AppState {
    AppState {
        db: std::sync::Mutex::new(ringside_core::db::Db::open_in_memory().unwrap()),
        settings: Default::default(),
        import: Default::default(),
        overlays: Default::default(),
        client_running: Default::default(),
        notes: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringside_solver::StreetSizes;

    #[test]
    fn sizes_read_as_plain_words() {
        let mut spot = Spot {
            oop_range: "77,65".into(),
            ip_range: "KQ".into(),
            board: "Ks Th 7d 4c 2s".into(),
            pot: 10.0,
            stack: 10.0,
            sizes: [StreetSizes::default(), StreetSizes::default(), StreetSizes { bet: vec![100.0], raise: vec![] }],
            allin_threshold: 0.0,
        };
        assert_eq!(sizes_text(&spot), "River 100 %");
        spot.board = "Ks Th 7d".into();
        spot.sizes[0] = StreetSizes { bet: vec![33.0, 75.0], raise: vec![60.0] };
        assert_eq!(sizes_text(&spot), "Flop 33/75 %, Raise 60 % · Turn nur Check · River 100 %");
    }
}
