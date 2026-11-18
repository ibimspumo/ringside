//! Solver solutions kept on this computer: every finished solve is saved as a snapshot
//! file (`ringside_solver::saved`) in the data folder with a row in the database, so it
//! opens again without solving, survives a restart and is not solved twice.
//!
//! Other calculations look up a spot before they solve it:
//! ```ignore
//! if let Some((id, snapshot)) = solutions::lookup(&app, &spot, target_pct) {
//!     let node = snapshot.node(&path)?;   // NOT_STORED past the next street's first decision
//! }
//! // after solving:
//! solutions::save(&app, &mut solution, &settings, &SaveInfo { source: SolutionSource::Hand, .. })?;
//! ```
//! The folder stays below a limit the user sets: the oldest solutions go first, pinned
//! ones never.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ringside_core::db::{NewSolution, SavedSolution, SolutionSource};
use ringside_solver::saved::{Snapshot, good_enough, spot_key};
use ringside_solver::{Settings, Solution, Spot, Summary};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

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
}

pub(crate) fn dir(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(crate::data_dir(app).map_err(err)?.join("solutions"))
}

fn pct(x: f64) -> String {
    let s = format!("{x:.0}");
    s.replace('.', ",")
}

/// The sizes of the streets still to come, e.g. "Flop 33/75 %, Raise 60 % · Turn 66 %".
fn sizes_text(spot: &Spot) -> String {
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

fn board_text(spot: &Spot) -> String {
    spot.board.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn max_bytes(state: &AppState) -> u64 {
    let mb = state.db.lock().unwrap().setting(LIMIT_KEY).ok().flatten().and_then(|v| v.parse().ok());
    mb.unwrap_or(DEFAULT_MAX_MB) * MB
}

fn write_file(path: &Path, bytes: &[u8]) -> CmdResult<()> {
    std::fs::create_dir_all(path.parent().expect("solution files have a folder")).map_err(err)?;
    // Written next to it first, so a crash never leaves half a file under the real name.
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

/// Deletes the oldest unpinned solutions until the folder fits the limit.
fn prune(dir: &Path, state: &AppState) -> CmdResult<usize> {
    let limit = max_bytes(state);
    let db = state.db.lock().unwrap();
    let over = db.solutions_over_limit(limit).map_err(err)?;
    for s in &over {
        let _ = std::fs::remove_file(dir.join(&s.info.file));
        db.delete_solution(s.id).map_err(err)?;
    }
    Ok(over.len())
}

/// Saves a finished solve and returns its id.
pub fn save(app: &AppHandle, solution: &mut Solution, settings: &Settings, info: &SaveInfo) -> CmdResult<i64> {
    store(&dir(app)?, &app.state::<AppState>(), solution, settings, info)
}

/// [`save`] into the folder `dir`.
pub(crate) fn store(dir: &Path, state: &AppState, solution: &mut Solution, settings: &Settings, info: &SaveInfo) -> CmdResult<i64> {
    let key = spot_key(&solution.spot).map_err(err)?;
    let snapshot = Snapshot::take(solution, settings).map_err(err)?;
    let bytes = snapshot.to_bytes();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let file = format!("{key}-{now}.rsms");
    write_file(&dir.join(&file), &bytes)?;
    let spot = &solution.spot;
    let board = board_text(spot);
    let row = NewSolution {
        spot_key: key,
        title: if info.title.trim().is_empty() { format!("Spot auf {board}") } else { info.title.trim().to_string() },
        board,
        oop: info.oop.clone(),
        ip: info.ip.clone(),
        pot_bb: spot.pot,
        stack_bb: spot.stack,
        ranges: info.ranges.clone(),
        sizes: sizes_text(spot),
        exploitability_pct: solution.summary.exploitability_pct,
        target_pct: settings.target_pct,
        complete: snapshot.complete(),
        source: info.source.unwrap_or(SolutionSource::Own),
        hand_id: info.hand_id,
        file,
        bytes: bytes.len() as u64,
        created_at: crate::now_secs(),
    };
    let id = state.db.lock().unwrap().add_solution(&row).map_err(err)?;
    prune(dir, state)?;
    Ok(id)
}

fn read(dir: &Path, row: &SavedSolution) -> CmdResult<Snapshot> {
    let bytes = std::fs::read(dir.join(&row.info.file)).map_err(|_| "Die Datei dieser Lösung fehlt.".to_string())?;
    Snapshot::from_bytes(&bytes).map_err(err)
}

/// A saved solution of `spot` at least as exact as `target_pct` (percent of the pot), the
/// most exact first. Spots count as equal when they solve to the same tree, see `spot_key`.
pub fn lookup(app: &AppHandle, spot: &Spot, target_pct: f64) -> Option<(i64, Snapshot)> {
    find(&dir(app).ok()?, &app.state::<AppState>(), spot, target_pct)
}

/// [`lookup`] in the folder `dir`.
pub(crate) fn find(dir: &Path, state: &AppState, spot: &Spot, target_pct: f64) -> Option<(i64, Snapshot)> {
    let key = spot_key(spot).ok()?;
    let rows = state.db.lock().unwrap().solutions_for_key(&key).ok()?;
    rows.iter().filter(|r| r.info.exploitability_pct <= target_pct + 1e-9).find_map(|row| {
        let snapshot = read(dir, row).ok()?;
        let same = spot_key(&snapshot.meta.spot).ok()? == key && good_enough(&snapshot.meta, target_pct);
        same.then_some((row.id, snapshot))
    })
}

#[derive(Serialize)]
pub struct SolutionsView {
    items: Vec<SavedSolution>,
    bytes: u64,
    max_mb: u64,
}

#[tauri::command]
pub async fn solutions_list(app: AppHandle) -> CmdResult<SolutionsView> {
    blocking(app, |state| {
        let db = state.db.lock().unwrap();
        Ok(SolutionsView {
            items: db.solutions().map_err(err)?,
            bytes: db.solutions_bytes().map_err(err)?,
            max_mb: db.setting(LIMIT_KEY).ok().flatten().and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_MAX_MB),
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
        let row = state.db.lock().unwrap().solution(id).map_err(err)?.ok_or("Diese Lösung gibt es nicht mehr.")?;
        let snapshot = read(&dir(&handle)?, &row)?;
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
        if let Some(file) = state.db.lock().unwrap().delete_solution(id).map_err(err)? {
            let _ = std::fs::remove_file(dir(&handle)?.join(file));
        }
        Ok(())
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
        prune(&dir(&handle)?, state)
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
    use ringside_solver::{StreetSizes, solve};

    fn river(stack: f64) -> Spot {
        Spot {
            oop_range: "77,65".into(),
            ip_range: "KQ".into(),
            board: "Ks Th 7d 4c 2s".into(),
            pot: 10.0,
            stack,
            sizes: [StreetSizes::default(), StreetSizes::default(), StreetSizes { bet: vec![100.0], raise: vec![] }],
            allin_threshold: 0.0,
        }
    }

    #[test]
    fn sizes_read_as_plain_words() {
        let mut spot = river(10.0);
        assert_eq!(sizes_text(&spot), "River 100 %");
        spot.board = "Ks Th 7d".into();
        spot.sizes[0] = StreetSizes { bet: vec![33.0, 75.0], raise: vec![60.0] };
        assert_eq!(sizes_text(&spot), "Flop 33/75 %, Raise 60 % · Turn nur Check · River 100 %");
    }

    /// The pieces of `save` and `lookup` that need no app: the file goes to disk, the row
    /// finds it again by key, and the limit deletes the oldest unpinned first.
    #[test]
    fn saved_files_are_found_again_and_pruned_by_size() {
        let dir = std::env::temp_dir().join(format!("ringside-solutions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = test_state();
        let settings = Settings { target_pct: 1.0, max_iterations: 300, threads: 1 };
        let mut ids = Vec::new();
        for (i, stack) in [10.0, 12.0, 14.0].into_iter().enumerate() {
            let mut solution = solve(&river(stack), &settings, 1 << 30, |_| {}, || false).unwrap();
            let bytes = Snapshot::take(&mut solution, &settings).unwrap().to_bytes();
            let file = format!("{i}.rsms");
            write_file(&dir.join(&file), &bytes).unwrap();
            let row = NewSolution {
                spot_key: spot_key(&solution.spot).unwrap(),
                title: "Test".into(),
                board: board_text(&solution.spot),
                oop: "BB".into(),
                ip: "BTN".into(),
                pot_bb: 10.0,
                stack_bb: stack,
                ranges: String::new(),
                sizes: sizes_text(&solution.spot),
                exploitability_pct: solution.summary.exploitability_pct,
                target_pct: 1.0,
                complete: true,
                source: SolutionSource::Own,
                hand_id: None,
                file,
                bytes: 3 * MB,
                created_at: i as i64,
            };
            ids.push(state.db.lock().unwrap().add_solution(&row).unwrap());
        }
        // Reuse: the same spot at the same or a looser target opens from disk; a stricter
        // target or another stack solves again.
        let (id, snapshot) = find(&dir, &state, &river(12.0), 1.0).expect("saved spot is reused");
        assert_eq!(id, ids[1]);
        assert!(snapshot.complete() && snapshot.node(&[]).is_ok());
        assert!(find(&dir, &state, &river(12.0), 5.0).is_some());
        let reached = snapshot.meta.summary.exploitability_pct;
        assert!(find(&dir, &state, &river(12.0), reached / 2.0).is_none());
        assert!(find(&dir, &state, &river(13.0), 1.0).is_none());

        state.db.lock().unwrap().set_setting(LIMIT_KEY, "7").unwrap();
        state.db.lock().unwrap().set_solution_pinned(ids[0], true).unwrap();
        assert_eq!(prune(&dir, &state).unwrap(), 1);
        let left: Vec<i64> = state.db.lock().unwrap().solutions().unwrap().iter().map(|s| s.id).collect();
        assert_eq!(left, [ids[2], ids[0]], "the oldest unpinned went");
        assert!(!dir.join("1.rsms").exists() && dir.join("0.rsms").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
