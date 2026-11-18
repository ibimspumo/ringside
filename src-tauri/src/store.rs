//! The spot store: every solved spot on this computer, whoever solved it, in one index
//! under one key (`ringside_store::key`), and the only place that reads and writes the
//! solution files.
//!
//! - Own solves are snapshots (`ringside_solver::saved`) in `solutions/`, described in
//!   `solver_solutions` for "Meine Lösungen".
//! - Library spots are in the library format under `library/v<catalog>/{local,cloud}/`.
//! - Model flops another player shared are kept as downloaded, in the library format on
//!   their canonical flop, under `store/cloud/`.
//!
//! Every area asks [`resolve`]: on this computer, in the library, in Ringside Cloud (with
//! `cloud.fetch` and when [`crate::cloud::may_talk`]), and only then it solves, holding
//! the claim on the key so that two areas never solve the same spot. Solves take a ticket
//! of [`gate`], which runs one at a time in order of what the user waits for. While
//! PokerStars runs nothing is looked up, solved or asked.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use ringside_cloud::kinds::{Kind, SpotInput, SpotKind};
use ringside_core::db::{NewSolution, SavedSolution, StoreFormat, StoreRow};
use ringside_engine::preflop_table::ModelLine;
use ringside_play::tree::{StoredTree, Tree};
use ringside_solver::gate::Gate;
use ringside_solver::library::format::{Stored, View, header_summary};
use ringside_solver::library::{Catalog, SpotId, catalog};
use ringside_solver::saved::{Snapshot, good_enough, spot_key};
use ringside_solver::{Error as SolverError, Node, Settings, Solution, Spot, Summary};
use ringside_store::key::{catalog_key, local_key};
use ringside_store::{Cloud, InFlight, Keyed, Origin, Policy, Step, key_of};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::solutions::SaveInfo;
use crate::{AppState, CmdResult, err};

static GATE: LazyLock<Gate> = LazyLock::new(Gate::new);
static FLIGHT: LazyLock<InFlight> = LazyLock::new(InFlight::new);

const SOLUTIONS: &str = "solutions";
const DOWNLOADS: &str = "store/cloud";
/// Downloads kept; each is at most `SPOT_MAX_BYTES`, so this stays below 200 MB.
const KEEP_DOWNLOADS: usize = 1000;
/// Set once the files from before the store are in the index.
const MIGRATED_KEY: &str = "spot_store_migrated";

/// One solve at a time, see `ringside_solver::gate`.
pub fn gate() -> &'static Gate {
    &GATE
}

/// Keys being solved now; [`resolve`] claims them around its solve.
pub fn flight() -> &'static InFlight {
    &FLIGHT
}

pub(crate) fn data(app: &AppHandle) -> CmdResult<PathBuf> {
    crate::data_dir(app).map_err(err)
}

/// Writes next to the real name first, so a crash never leaves half a file under it.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().expect("store files have a folder")).map_err(err)?;
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).map_err(err)?;
    std::fs::rename(&tmp, path).map_err(err)
}

/// `path` inside `data`, with forward slashes, as the index keeps it.
fn relative(data: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(data).unwrap_or(path);
    rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn compact(board: &str) -> String {
    board.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

fn origin_text(o: Origin) -> &'static str {
    match o {
        Origin::Own => "own",
        Origin::Library => "library",
        Origin::Cloud => "cloud",
    }
}

pub(crate) fn origin_of(text: &str) -> Origin {
    match text {
        "library" => Origin::Library,
        "cloud" => Origin::Cloud,
        _ => Origin::Own,
    }
}

// -----------------------------------------------------------------------------------
// Library files.

/// Where a library spot came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Solved on this computer.
    Local,
    /// From a signed pack or Ringside Cloud.
    Cloud,
}

pub(crate) fn library_root(data: &Path) -> PathBuf {
    data.join("library").join(format!("v{}", catalog().version))
}

pub(crate) fn library_path(root: &Path, source: Source, id: SpotId) -> PathBuf {
    let c = catalog();
    let dir = match source {
        Source::Local => "local",
        Source::Cloud => "cloud",
    };
    root.join(dir).join(&c.scenarios[id.scenario].id).join(format!("{}.rsls", c.flops[id.flop]))
}

pub(crate) fn library_status(root: &Path, id: SpotId) -> Option<Source> {
    [Source::Local, Source::Cloud].into_iter().find(|&s| library_path(root, s, id).is_file())
}

pub(crate) fn read_library(root: &Path, source: Source, id: SpotId) -> Result<Vec<u8>, String> {
    std::fs::read(library_path(root, source, id)).map_err(err)
}

/// The library spot `id` and where it came from, if it is on this computer.
pub(crate) fn load_library(root: &Path, id: SpotId) -> Option<(Source, Stored, usize)> {
    let source = library_status(root, id)?;
    let bytes = read_library(root, source, id).ok()?;
    let stored = Stored::from_bytes(&bytes, catalog()).ok()?;
    Some((source, stored, bytes.len()))
}

fn library_row(data: &Path, root: &Path, source: Source, id: SpotId, bytes: &[u8], at: i64) -> Result<StoreRow, String> {
    let (_, exploitability) = header_summary(bytes).map_err(err)?;
    let spot = catalog().spot(id);
    Ok(StoreRow {
        id: 0,
        key: catalog_key(id),
        local_key: local_key(&spot)?,
        origin: origin_text(if source == Source::Local { Origin::Own } else { Origin::Library }).into(),
        format: StoreFormat::Library,
        file: relative(data, &library_path(root, source, id)),
        board: catalog().flops[id.flop].clone(),
        input: None,
        exploitability_pct: exploitability as f64,
        complete: false,
        bytes: bytes.len() as u64,
        solution_id: None,
        created_at: at,
    })
}

/// Writes a library spot and files it in the index; `data` is the data folder and `root`
/// the library's folder in it.
pub(crate) fn write_library(state: &AppState, data: &Path, root: &Path, source: Source, id: SpotId, bytes: &[u8]) -> Result<(), String> {
    write_atomic(&library_path(root, source, id), bytes)?;
    let row = library_row(data, root, source, id, bytes, crate::now_secs())?;
    state.db.lock().unwrap().store_put(&row).map_err(err)?;
    Ok(())
}

// -----------------------------------------------------------------------------------
// Own solves.

fn solutions_dir(data: &Path) -> PathBuf {
    data.join(SOLUTIONS)
}

fn read_snapshot(data: &Path, row: &SavedSolution) -> CmdResult<Snapshot> {
    let bytes = std::fs::read(solutions_dir(data).join(&row.info.file)).map_err(|_| "Die Datei dieser Lösung fehlt.".to_string())?;
    Snapshot::from_bytes(&bytes).map_err(err)
}

/// The snapshot of a saved solution, for the solver page.
pub(crate) fn open_snapshot(data: &Path, state: &AppState, id: i64) -> CmdResult<(SavedSolution, Snapshot)> {
    let row = state.db.lock().unwrap().solution(id).map_err(err)?.ok_or("Diese Lösung gibt es nicht mehr.")?;
    let snapshot = read_snapshot(data, &row)?;
    Ok((row, snapshot))
}

/// Deletes a saved solution, its file and its index row.
pub(crate) fn delete_solution(data: &Path, state: &AppState, id: i64) -> CmdResult<()> {
    let db = state.db.lock().unwrap();
    if let Some(file) = db.delete_solution(id).map_err(err)? {
        let _ = std::fs::remove_file(solutions_dir(data).join(&file));
        db.store_delete_file(&format!("{SOLUTIONS}/{file}")).map_err(err)?;
    }
    Ok(())
}

/// Deletes the oldest unpinned solutions until they fit `limit` bytes; returns how many went.
pub(crate) fn prune_solutions(data: &Path, state: &AppState, limit: u64) -> CmdResult<usize> {
    let over = state.db.lock().unwrap().solutions_over_limit(limit).map_err(err)?;
    for s in &over {
        delete_solution(data, state, s.id)?;
    }
    Ok(over.len())
}

/// Saves a fresh solve with its description and files it under its key; returns the
/// solution's id in "Meine Lösungen". The caller shares it ([`share`]).
pub(crate) fn save_solution(data: &Path, state: &AppState, solution: &mut Solution, settings: &Settings, info: &SaveInfo) -> CmdResult<(i64, Keyed)> {
    let keyed = key_of(&solution.spot, info.model.as_ref())?;
    let local = spot_key(&solution.spot).map_err(err)?;
    let snapshot = Snapshot::take(solution, settings).map_err(err)?;
    let bytes = snapshot.to_bytes();
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let file = format!("{local}-{now_ms}.rsms");
    write_atomic(&solutions_dir(data).join(&file), &bytes)?;
    let spot = &solution.spot;
    let board = crate::solutions::board_text(spot);
    let row = NewSolution {
        spot_key: local,
        title: if info.title.trim().is_empty() { format!("Spot auf {board}") } else { info.title.trim().to_string() },
        board: board.clone(),
        oop: info.oop.clone(),
        ip: info.ip.clone(),
        pot_bb: spot.pot,
        stack_bb: spot.stack,
        ranges: info.ranges.clone(),
        sizes: crate::solutions::sizes_text(spot),
        exploitability_pct: solution.summary.exploitability_pct,
        target_pct: settings.target_pct,
        complete: snapshot.complete(),
        source: info.source.unwrap_or(ringside_core::db::SolutionSource::Own),
        hand_id: info.hand_id,
        file: file.clone(),
        bytes: bytes.len() as u64,
        created_at: crate::now_secs(),
    };
    let db = state.db.lock().unwrap();
    let id = db.add_solution(&row).map_err(err)?;
    db.store_put(&snapshot_row(&keyed, &row, id)).map_err(err)?;
    Ok((id, keyed))
}

fn snapshot_row(keyed: &Keyed, row: &NewSolution, id: i64) -> StoreRow {
    StoreRow {
        id: 0,
        key: keyed.key.clone(),
        local_key: keyed.local.clone(),
        origin: origin_text(Origin::Own).into(),
        format: StoreFormat::Snapshot,
        file: format!("{SOLUTIONS}/{}", row.file),
        board: row.board.clone(),
        input: None,
        exploitability_pct: row.exploitability_pct,
        complete: row.complete,
        bytes: row.bytes,
        solution_id: Some(id),
        created_at: row.created_at,
    }
}

// -----------------------------------------------------------------------------------
// Downloads of model flops.

fn download_path(data: &Path, key: &str) -> PathBuf {
    let name = key.split_once(':').map_or(key, |(_, h)| h);
    data.join(DOWNLOADS).join(format!("{name}.rsls"))
}

/// Keeps a verified model flop from the cloud, on its canonical flop.
pub(crate) fn save_download(data: &Path, state: &AppState, input: &SpotInput, stored: &Stored) -> Result<(), String> {
    let key = ringside_store::key::spot_kind_key(input)?;
    let path = download_path(data, &key);
    let bytes = stored.to_bytes();
    write_atomic(&path, &bytes)?;
    let row = StoreRow {
        id: 0,
        key,
        local_key: local_key(&input.spot()?)?,
        origin: origin_text(Origin::Cloud).into(),
        format: StoreFormat::Library,
        file: relative(data, &path),
        board: input.board.clone(),
        input: Some(serde_json::to_string(input).map_err(err)?),
        exploitability_pct: stored.exploitability_pct as f64,
        complete: false,
        bytes: bytes.len() as u64,
        solution_id: None,
        created_at: crate::now_secs(),
    };
    let db = state.db.lock().unwrap();
    db.store_put(&row).map_err(err)?;
    let downloads = db.store_rows(Some(origin_text(Origin::Cloud))).map_err(err)?;
    for old in downloads.iter().skip(KEEP_DOWNLOADS) {
        let _ = std::fs::remove_file(data.join(&old.file));
        db.store_delete_file(&old.file).map_err(err)?;
    }
    Ok(())
}

// -----------------------------------------------------------------------------------
// Finding a spot.

/// A stored solution of the spot asked for, ready to be read node by node.
pub enum Found {
    /// An own solve, with its id in "Meine Lösungen".
    Snapshot { id: i64, snapshot: Box<Snapshot> },
    /// The library format, on the board asked for.
    Stored { stored: Box<Stored>, catalog: Box<Catalog>, origin: Origin },
}

impl Found {
    pub fn origin(&self) -> Origin {
        match self {
            Found::Snapshot { .. } => Origin::Own,
            Found::Stored { origin, .. } => *origin,
        }
    }

    pub fn saved_id(&self) -> Option<i64> {
        match self {
            Found::Snapshot { id, .. } => Some(*id),
            Found::Stored { .. } => None,
        }
    }

    pub fn node(&self, path: &[ringside_solver::Step]) -> Result<Node, SolverError> {
        match self {
            Found::Snapshot { snapshot, .. } => snapshot.node(path),
            Found::Stored { stored, catalog, .. } => View::new(stored, catalog)?.node(path),
        }
    }

    /// What the stored solution says about its solve.
    pub fn summary(&self) -> Summary {
        match self {
            Found::Snapshot { snapshot, .. } => snapshot.meta.summary.clone(),
            Found::Stored { stored, .. } => Summary {
                iterations: stored.iterations,
                exploitability: 0.0,
                exploitability_pct: stored.exploitability_pct as f64,
                converged: true,
                elapsed_ms: 0,
                memory_bytes: 0,
                compressed: false,
                threads: 0,
            },
        }
    }

    pub fn into_tree(self) -> Box<dyn Tree> {
        match self {
            Found::Snapshot { snapshot, .. } => Box::new(*snapshot),
            Found::Stored { stored, catalog, .. } => Box::new(StoredTree { stored: *stored, catalog: *catalog }),
        }
    }
}

/// A spot asked for.
pub struct Ask<'a> {
    pub spot: &'a Spot,
    pub keyed: &'a Keyed,
    /// Own solves must be at least this exact (percent of the pot); library and cloud
    /// spots are solved to their own, fixed precision and always count.
    pub target_pct: f64,
}

impl<'a> Ask<'a> {
    pub fn new(spot: &'a Spot, keyed: &'a Keyed, target_pct: f64) -> Ask<'a> {
        Ask { spot, keyed, target_pct }
    }
}

/// A model flop stored on its canonical flop, renamed to the board asked for.
fn on_board(input: &SpotInput, stored: Stored, board: &str) -> Option<(Stored, Catalog)> {
    let board = compact(board);
    let catalog = input.catalog(Some(&board)).ok()?;
    let renamed = stored.renamed(&catalog.scenarios[0], &board).ok()?;
    Some((renamed, catalog))
}

/// The best entry of the index for `ask`; with `others` false only own solves.
pub(crate) fn find_local(data: &Path, state: &AppState, ask: &Ask, others: bool) -> Option<Found> {
    let rows = state.db.lock().unwrap().store_find(&ask.keyed.key, &ask.keyed.local).ok()?;
    let board = compact(&ask.spot.board);
    rows.into_iter().filter(|r| others || r.origin == origin_text(Origin::Own)).find_map(|row| match row.format {
        StoreFormat::Snapshot => {
            // A snapshot holds its dealt board; only the same spot reads it.
            if row.local_key != ask.keyed.local || row.exploitability_pct > ask.target_pct + 1e-9 {
                return None;
            }
            let id = row.solution_id?;
            let saved = state.db.lock().unwrap().solution(id).ok()??;
            let snapshot = read_snapshot(data, &saved).ok()?;
            let same = spot_key(&snapshot.meta.spot).ok()? == saved.info.spot_key && good_enough(&snapshot.meta, ask.target_pct);
            same.then(|| Found::Snapshot { id, snapshot: Box::new(snapshot) })
        }
        StoreFormat::Library => {
            let bytes = std::fs::read(data.join(&row.file)).ok()?;
            let origin = origin_of(&row.origin);
            match &row.input {
                Some(json) => {
                    let input: SpotInput = serde_json::from_str(json).ok()?;
                    let stored = Stored::from_bytes(&bytes, &input.catalog(None).ok()?).ok()?;
                    let (stored, catalog) = on_board(&input, stored, &board)?;
                    Some(Found::Stored { stored: Box::new(stored), catalog: Box::new(catalog), origin })
                }
                None if row.board == board => {
                    let stored = Stored::from_bytes(&bytes, catalog()).ok()?;
                    Some(Found::Stored { stored: Box::new(stored), catalog: Box::new(catalog().clone()), origin })
                }
                None => None,
            }
        }
    })
}

/// A library spot on this computer that the index does not know yet.
fn find_library(data: &Path, state: &AppState, ask: &Ask) -> Option<Found> {
    let Some(Cloud::Catalog { id, .. }) = &ask.keyed.cloud else { return None };
    let root = library_root(data);
    let (source, stored, _) = load_library(&root, *id)?;
    if let Ok(bytes) = read_library(&root, source, *id)
        && let Ok(row) = library_row(data, &root, source, *id, &bytes, crate::now_secs())
    {
        let _ = state.db.lock().unwrap().store_put(&row);
    }
    let origin = if source == Source::Local { Origin::Own } else { Origin::Library };
    Some(Found::Stored { stored: Box::new(stored), catalog: Box::new(catalog().clone()), origin })
}

/// Asks the cloud for `ask` and keeps what it gets.
pub(crate) fn find_cloud(app: &AppHandle, data: &Path, ask: &Ask) -> Option<Found> {
    let state = app.state::<AppState>();
    match ask.keyed.cloud.as_ref()? {
        Cloud::Spot(input) => {
            let stored = crate::cloud::download_spot(app, input)?;
            if let Err(e) = save_download(data, &state, input, &stored) {
                eprintln!("store: keeping a cloud spot: {e}");
            }
            let (stored, catalog) = on_board(input, stored, &ask.spot.board)?;
            Some(Found::Stored { stored: Box::new(stored), catalog: Box::new(catalog), origin: Origin::Cloud })
        }
        Cloud::Catalog { id, .. } => {
            let bytes = crate::library::download(app, *id)?;
            let root = library_root(data);
            let stored = Stored::from_bytes(&bytes, catalog()).ok()?;
            if let Err(e) = write_library(&state, data, &root, Source::Cloud, *id, &bytes) {
                eprintln!("store: keeping a library spot: {e}");
            }
            Some(Found::Stored { stored: Box::new(stored), catalog: Box::new(catalog().clone()), origin: Origin::Library })
        }
    }
}

/// What a lookup may do now. `others`: spots others solved count (always, except for
/// practice with "nur eigene, frisch gegebene Spots"); `solve`: the caller solves on a miss.
pub(crate) fn policy(app: &AppHandle, others: bool, solve: bool) -> Policy {
    let state = app.state::<AppState>();
    let fetch = state.settings.lock().unwrap().cloud.fetch;
    Policy { locked: crate::unlocked(&state).is_err(), fetch, may_talk: crate::cloud::may_talk(app), others, solve }
}

/// Looks for `ask` in the order of [`ringside_store::steps`], in the store in the data
/// folder with its index in the database; `use_found` turns a stored
/// solution into the caller's result (or `None` when it does not serve, e.g. a decision
/// past its saved part) and `solve` solves on a miss, holding the claim on the key.
pub(crate) fn resolve<T>(
    (data, state): (&Path, &AppState),
    ask: &Ask,
    policy: &Policy,
    mut cloud: impl FnMut(&Ask) -> Option<Found>,
    keep_waiting: impl Fn() -> bool,
    mut use_found: impl FnMut(Found) -> Option<T>,
    solve: impl FnOnce() -> Option<T>,
) -> Option<(Step, T)> {
    let mut solve = Some(solve);
    ringside_store::resolve(policy, &FLIGHT, &ask.keyed.key, keep_waiting, |step| match step {
        Step::Local => find_local(data, state, ask, policy.others).and_then(&mut use_found),
        Step::Library => find_library(data, state, ask).and_then(&mut use_found),
        Step::Cloud => cloud(ask).and_then(&mut use_found),
        Step::Solve => solve.take().and_then(|f| f()),
    })
}

/// [`resolve`] in the app: the cloud is asked when the policy allows it.
pub(crate) fn resolve_app<T>(
    app: &AppHandle,
    ask: &Ask,
    policy: &Policy,
    keep_waiting: impl Fn() -> bool,
    use_found: impl FnMut(Found) -> Option<T>,
    solve: impl FnOnce() -> Option<T>,
) -> Option<(Step, T)> {
    let data = data(app).ok()?;
    let state = app.state::<AppState>();
    resolve((&data, &state), ask, policy, |a| find_cloud(app, &data, a), keep_waiting, use_found, solve)
}

/// Offers a fresh solve to the cloud, if the user shares and its ranges come from the
/// preflop model or the catalog. Never the hand, the cards or names: only the abstract
/// spot leaves the computer (`ringside_cloud::kinds`).
pub(crate) fn share(app: &AppHandle, keyed: &Keyed, solution: &mut Solution) {
    match &keyed.cloud {
        Some(Cloud::Spot(input)) => crate::cloud::share_spot(app, input.clone(), solution),
        Some(Cloud::Catalog { id, .. }) => crate::library::offer(app, *id, solution),
        None => {}
    }
}

/// The flops of `keys` ((key, flop) pairs) that are ready on this computer: own solves, the
/// library, kept downloads; with `others` false only own solves.
pub(crate) fn ready_flops(state: &AppState, keys: &[(String, String)], others: bool) -> Vec<String> {
    let db = state.db.lock().unwrap();
    keys.iter()
        .filter(|(key, _)| {
            db.store_find(key, key).is_ok_and(|rows| rows.iter().any(|r| others || r.origin == origin_text(Origin::Own)))
        })
        .map(|(_, flop)| flop.clone())
        .collect()
}

/// The cloud key of a model flop, for [`ready_flops`].
pub(crate) fn model_key(input: &SpotInput) -> Option<String> {
    SpotKind::key(input).ok().map(|k| format!("{}:{k}", SpotKind::NAME))
}

/// The key of `spot` with its model line, if known.
pub(crate) fn keyed(spot: &Spot, model: Option<&ModelLine>) -> Option<Keyed> {
    key_of(spot, model).ok()
}

// -----------------------------------------------------------------------------------
// Taking over the files from before the store.

/// What [`migrate`] filed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Migrated {
    pub solutions: usize,
    pub library: usize,
    /// Rows of "Meine Lösungen" whose file is gone.
    pub missing: usize,
}

/// Files every solution saved before the store and every library spot in the index, once.
pub(crate) fn migrate(data: &Path, state: &AppState) -> CmdResult<Migrated> {
    if state.db.lock().unwrap().setting(MIGRATED_KEY).map_err(err)?.is_some() {
        return Ok(Migrated::default());
    }
    let mut out = Migrated::default();
    let saved = state.db.lock().unwrap().solutions().map_err(err)?;
    for row in saved {
        let Ok(bytes) = std::fs::read(solutions_dir(data).join(&row.info.file)) else {
            out.missing += 1;
            continue;
        };
        let Ok(meta) = Snapshot::meta(&bytes) else {
            out.missing += 1;
            continue;
        };
        // The model line of an old solve is not known; the local key finds it from every area.
        let keyed = key_of(&meta.spot, None)?;
        state.db.lock().unwrap().store_put(&snapshot_row(&keyed, &row.info, row.id)).map_err(err)?;
        out.solutions += 1;
    }
    let root = library_root(data);
    let c = catalog();
    for id in c.ids() {
        for source in [Source::Local, Source::Cloud] {
            let Ok(bytes) = read_library(&root, source, id) else { continue };
            let at = std::fs::metadata(library_path(&root, source, id))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs() as i64);
            if let Ok(row) = library_row(data, &root, source, id, &bytes, at) {
                state.db.lock().unwrap().store_put(&row).map_err(err)?;
                out.library += 1;
            }
        }
    }
    state.db.lock().unwrap().set_setting(MIGRATED_KEY, "1").map_err(err)?;
    Ok(out)
}

/// Runs [`migrate`] off the main thread at start.
pub fn spawn_migration(app: AppHandle) {
    std::thread::spawn(move || {
        let Ok(data) = data(&app) else { return };
        match migrate(&data, &app.state::<AppState>()) {
            Ok(m) if m != Migrated::default() => {
                eprintln!("spot store: {} solutions, {} library spots filed ({} files missing)", m.solutions, m.library, m.missing)
            }
            Ok(_) => {}
            Err(e) => eprintln!("spot store: migration failed: {e}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solutions::test_state;
    use ringside_solver::{StreetSizes, solve};
    use std::cell::Cell;

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringside-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn river() -> Spot {
        Spot {
            oop_range: "77,65".into(),
            ip_range: "KQ".into(),
            board: "Ks Th 7d 4c 2s".into(),
            pot: 10.0,
            stack: 10.0,
            sizes: [StreetSizes::default(), StreetSizes::default(), StreetSizes { bet: vec![100.0], raise: vec![] }],
            allin_threshold: 0.0,
        }
    }

    const SETTINGS: Settings = Settings { target_pct: 1.0, max_iterations: 300, threads: 1 };

    fn all() -> Policy {
        Policy { locked: false, fetch: true, may_talk: true, others: true, solve: true }
    }

    /// Resolves `spot` as the areas do; `solves` counts the solves.
    fn ask_for(data: &Path, state: &AppState, spot: &Spot, policy: &Policy, solves: &Cell<u32>, cloud: &Cell<u32>) -> Option<Step> {
        let keyed = key_of(spot, None).unwrap();
        let ask = Ask::new(spot, &keyed, 1.0);
        let found = resolve(
            (data, state),
            &ask,
            policy,
            |_| {
                cloud.set(cloud.get() + 1);
                None
            },
            || false,
            |f| Some(f.origin()),
            || {
                solves.set(solves.get() + 1);
                let mut solution = solve(spot, &SETTINGS, 1 << 30, |_| {}, || false).ok()?;
                save_solution(data, state, &mut solution, &SETTINGS, &SaveInfo::default()).ok()?;
                Some(Origin::Own)
            },
        );
        found.map(|(s, _)| s)
    }

    #[test]
    fn a_stored_spot_is_never_solved_again() {
        let (data, state) = (folder("twice"), test_state());
        let (solves, cloud) = (Cell::new(0), Cell::new(0));
        assert_eq!(ask_for(&data, &state, &river(), &all(), &solves, &cloud), Some(Step::Solve));
        assert_eq!((solves.get(), cloud.get()), (1, 1), "looked in the cloud before solving");
        for _ in 0..3 {
            assert_eq!(ask_for(&data, &state, &river(), &all(), &solves, &cloud), Some(Step::Local));
        }
        assert_eq!((solves.get(), cloud.get()), (1, 1), "found on this computer: no cloud, no solve");
        let rows = state.db.lock().unwrap().store_rows(None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].origin.as_str(), rows[0].key.starts_with("local:")), ("own", true));
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn with_pokerstars_running_nothing_happens() {
        let (data, state) = (folder("locked"), test_state());
        let (solves, cloud) = (Cell::new(0), Cell::new(0));
        let locked = Policy { locked: true, ..all() };
        assert_eq!(ask_for(&data, &state, &river(), &locked, &solves, &cloud), None);
        assert_eq!((solves.get(), cloud.get()), (0, 0));
        assert!(!data.exists(), "nothing written");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn a_library_spot_is_found_before_the_cloud_and_filed() {
        let (data, state) = (folder("library"), test_state());
        let (id, stored) = library_spot(3);
        let spot = catalog().spot(id);
        let root = library_root(&data);
        write_atomic(&library_path(&root, Source::Cloud, id), &stored.to_bytes()).unwrap();
        let (solves, cloud) = (Cell::new(0), Cell::new(0));
        assert_eq!(ask_for(&data, &state, &spot, &all(), &solves, &cloud), Some(Step::Library));
        assert_eq!((solves.get(), cloud.get()), (0, 0));
        assert_eq!(ask_for(&data, &state, &spot, &all(), &solves, &cloud), Some(Step::Local), "now in the index");
        // Only own spots for "nur eigene, frisch gegebene Spots": a pack's spot does not count.
        let own_only = Policy { others: false, ..all() };
        let keyed = key_of(&spot, None).unwrap();
        assert!(find_local(&data, &state, &Ask::new(&spot, &keyed, 1.0), own_only.others).is_none());
        let _ = std::fs::remove_dir_all(&data);
    }

    /// A small library spot (a 3-bet pot solves in about a second), solved once per test run.
    fn library_spot(flop: usize) -> (SpotId, Stored) {
        static SOLVED: std::sync::Mutex<Vec<(SpotId, Stored)>> = std::sync::Mutex::new(Vec::new());
        let c = catalog();
        let id = SpotId { scenario: c.scenario("3bp-btn-bb-40").unwrap(), flop };
        let mut solved = SOLVED.lock().unwrap();
        if let Some((_, s)) = solved.iter().find(|(i, _)| *i == id) {
            return (id, s.clone());
        }
        let stored = ringside_solver::library::solve_spot(c, id, 4, 4 << 30, |_| {}, || false).unwrap();
        solved.push((id, stored.clone()));
        (id, stored)
    }

    #[test]
    fn the_limit_deletes_the_oldest_unpinned_solution_with_its_index_row() {
        let (data, state) = (folder("prune"), test_state());
        let mut ids = Vec::new();
        for stack in [10.0, 12.0, 14.0] {
            let spot = Spot { stack, ..river() };
            let mut solution = solve(&spot, &SETTINGS, 1 << 30, |_| {}, || false).unwrap();
            ids.push(save_solution(&data, &state, &mut solution, &SETTINGS, &SaveInfo::default()).unwrap().0);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let files = |state: &AppState| state.db.lock().unwrap().store_rows(None).unwrap().into_iter().map(|r| r.file).collect::<Vec<_>>();
        let before = files(&state);
        let sizes: Vec<u64> = state.db.lock().unwrap().solutions().unwrap().iter().map(|s| s.info.bytes).collect();
        state.db.lock().unwrap().set_solution_pinned(ids[0], true).unwrap();
        // Room for two: the oldest unpinned one goes.
        assert_eq!(prune_solutions(&data, &state, sizes.iter().sum::<u64>() - 1).unwrap(), 1);
        let left: Vec<i64> = state.db.lock().unwrap().solutions().unwrap().iter().map(|s| s.id).collect();
        assert!(left.contains(&ids[0]) && left.contains(&ids[2]) && !left.contains(&ids[1]), "{left:?}");
        let after = files(&state);
        assert_eq!(after.len(), 2);
        let gone: Vec<&String> = before.iter().filter(|f| !after.contains(f)).collect();
        assert!(gone.len() == 1 && !data.join(gone[0]).exists(), "file and index row go together");
        let _ = std::fs::remove_dir_all(&data);
    }

    #[test]
    fn files_from_before_the_store_are_taken_over() {
        let (data, state) = (folder("migrate"), test_state());
        // A solution saved by the old code: file and row, no index entry.
        let mut solution = solve(&river(), &SETTINGS, 1 << 30, |_| {}, || false).unwrap();
        let bytes = Snapshot::take(&mut solution, &SETTINGS).unwrap().to_bytes();
        write_atomic(&solutions_dir(&data).join("old.rsms"), &bytes).unwrap();
        let row = NewSolution {
            spot_key: spot_key(&solution.spot).unwrap(),
            title: "Alt".into(),
            board: "KsTh7d4c2s".into(),
            oop: String::new(),
            ip: String::new(),
            pot_bb: 10.0,
            stack_bb: 10.0,
            ranges: String::new(),
            sizes: String::new(),
            exploitability_pct: solution.summary.exploitability_pct,
            target_pct: 1.0,
            complete: true,
            source: ringside_core::db::SolutionSource::Own,
            hand_id: None,
            file: "old.rsms".into(),
            bytes: bytes.len() as u64,
            created_at: 5,
        };
        let old = state.db.lock().unwrap().add_solution(&row).unwrap();
        let gone = state.db.lock().unwrap().add_solution(&NewSolution { file: "gone.rsms".into(), ..row.clone() }).unwrap();
        // Library spots solved here and from a pack.
        let root = library_root(&data);
        let ((a, sa), (b, sb)) = (library_spot(3), library_spot(5));
        write_atomic(&library_path(&root, Source::Local, a), &sa.to_bytes()).unwrap();
        write_atomic(&library_path(&root, Source::Cloud, b), &sb.to_bytes()).unwrap();

        assert_eq!(migrate(&data, &state).unwrap(), Migrated { solutions: 1, library: 2, missing: 1 });
        assert_eq!(migrate(&data, &state).unwrap(), Migrated::default(), "only once");
        let db = state.db.lock().unwrap();
        let mine = db.store_row_of_solution(old).unwrap().expect("the old solution is filed");
        assert_eq!((mine.key.as_str(), mine.origin.as_str(), mine.file.as_str()), (format!("local:{}", row.spot_key).as_str(), "own", "solutions/old.rsms"));
        assert!(db.store_row_of_solution(gone).unwrap().is_none());
        let lib = db.store_find(&catalog_key(a), "").unwrap();
        assert_eq!((lib.len(), lib[0].origin.as_str(), lib[0].exploitability_pct as f32), (1, "own", sa.exploitability_pct));
        assert_eq!(db.store_find(&catalog_key(b), "").unwrap()[0].origin, "library");
        drop(db);
        // The old solution opens from every area that asks for its spot.
        let (solves, cloud) = (Cell::new(0), Cell::new(0));
        assert_eq!(ask_for(&data, &state, &river(), &all(), &solves, &cloud), Some(Step::Local));
        assert_eq!(solves.get(), 0);
        let _ = std::fs::remove_dir_all(&data);
    }
}
