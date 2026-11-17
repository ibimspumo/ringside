//! The spot library in the app: the catalog's spots stored on this computer, downloaded
//! from signed packs and Ringside Cloud, or solved here in the background.
//!
//! The background worker runs only when the setting allows it (by default while nobody
//! uses the computer), never while PokerStars runs and never while the app is in use; a
//! solve stops within an iteration when that changes and starts over later. Before it
//! solves a spot it asks the cloud, and afterwards it shares the spot if the user allows
//! it. Its solves use half the cores and at most a quarter of the memory, so the
//! computer stays responsive. Progress survives a restart because every finished spot
//! is a file.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ringside_cloud::client::{DEFAULT_BASE_URL, RetryPolicy};
use ringside_cloud::kinds::postflop::MAX_BYTES;
use ringside_cloud::kinds::{Postflop, PostflopInput, PostflopSolution};
use ringside_cloud::packs::{self, Packs, newest_version};
use ringside_cloud::{Client, Error as CloudError};
use ringside_solver::library::format::{Stored, unpack};
use ringside_solver::library::{PotType, SpotId, catalog, solve_spot};
use ringside_solver::{Error as SolverError, Progress, Spot, Summary};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cloud::{CloudState, may_talk};
use crate::jobs::{self, Kind, Target};
use crate::settings::LibraryMode;
use crate::{AppState, CmdResult, blocking, err, store_settings};

/// Without input for this long the computer counts as idle.
const IDLE_AFTER: Duration = Duration::from_secs(5 * 60);
/// The app counts as in use while focused and touched within this time.
const IN_USE_FOR: Duration = Duration::from_secs(2 * 60);
const TICK: Duration = Duration::from_secs(5);
const STATUS_EVERY: Duration = Duration::from_millis(500);
/// Packs are looked for at most this often.
const PACKS_EVERY: Duration = Duration::from_secs(6 * 3600);
const PACK_PREFIX: &str = "library-v1-";

static FOCUSED: AtomicBool = AtomicBool::new(true);
/// Stopped in the task center; the worker rests until the mode changes or the app restarts.
static HALTED: AtomicBool = AtomicBool::new(false);
static FOCUS_SINCE: LazyLock<Mutex<Instant>> = LazyLock::new(|| Mutex::new(Instant::now()));

/// Called with the main window's focus changes.
pub fn set_focused(focused: bool) {
    FOCUSED.store(focused, Ordering::Relaxed);
    *FOCUS_SINCE.lock().unwrap() = Instant::now();
}

/// Time since the last keyboard or mouse input anywhere, where the system tells.
#[cfg(windows)]
fn system_idle() -> Option<Duration> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }
    // Both are milliseconds since boot in 32 bits; the difference survives the wrap.
    let now = unsafe { GetTickCount() };
    Some(Duration::from_millis(now.wrapping_sub(info.dwTime) as u64))
}

#[cfg(not(windows))]
fn system_idle() -> Option<Duration> {
    None
}

/// Whether the user works with the app right now.
fn app_in_use() -> bool {
    if !FOCUSED.load(Ordering::Relaxed) {
        return false;
    }
    match system_idle() {
        Some(idle) => idle < IN_USE_FOR,
        None => true,
    }
}

/// Why the worker may not run now, in words for the page; `None` when it may.
fn blocked(app: &AppHandle, mode: LibraryMode) -> Option<&'static str> {
    if app.state::<AppState>().client_running.load(Ordering::Relaxed) {
        return Some("PokerStars läuft");
    }
    if HALTED.load(Ordering::Relaxed) && mode != LibraryMode::Off {
        return Some("von dir angehalten");
    }
    match mode {
        LibraryMode::Off => return Some("ausgeschaltet"),
        LibraryMode::Idle if !system_idle().is_some_and(|d| d >= IDLE_AFTER) => {
            return Some(if cfg!(windows) { "wartet auf Leerlauf" } else { "Leerlauf nur unter Windows erkennbar" });
        }
        _ => {}
    }
    if app_in_use() {
        return Some("du benutzt die App");
    }
    None
}

fn mode_of(app: &AppHandle) -> LibraryMode {
    app.state::<AppState>().settings.lock().unwrap().library.background
}

// -----------------------------------------------------------------------------------
// Files.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Solved on this computer.
    Local,
    /// From a signed pack or Ringside Cloud.
    Cloud,
}

pub(crate) fn root(app: &AppHandle) -> Option<PathBuf> {
    Some(crate::data_dir(app).ok()?.join("library").join(format!("v{}", catalog().version)))
}

pub(crate) fn spot_path(root: &Path, source: Source, id: SpotId) -> PathBuf {
    let c = catalog();
    let dir = match source {
        Source::Local => "local",
        Source::Cloud => "cloud",
    };
    root.join(dir).join(&c.scenarios[id.scenario].id).join(format!("{}.rsls", c.flops[id.flop]))
}

pub(crate) fn status_of(root: &Path, id: SpotId) -> Option<Source> {
    [Source::Local, Source::Cloud].into_iter().find(|&s| spot_path(root, s, id).is_file())
}

fn write(root: &Path, source: Source, id: SpotId, bytes: &[u8]) -> Result<(), String> {
    let path = spot_path(root, source, id);
    std::fs::create_dir_all(path.parent().expect("spot files have a directory")).map_err(err)?;
    // Written next to it first, so a crash never leaves half a file under the real name.
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, bytes).map_err(err)?;
    std::fs::rename(&tmp, &path).map_err(err)
}

fn disk_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => disk_bytes(&e.path()),
            Ok(_) => e.metadata().map_or(0, |m| m.len()),
            Err(_) => 0,
        })
        .sum()
}

// -----------------------------------------------------------------------------------
// Worker.

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkerStatus {
    /// "off", "waiting", "packs", "cloud", "solving" or "done".
    pub state: &'static str,
    /// Why it waits.
    pub reason: Option<&'static str>,
    pub scenario: Option<String>,
    pub flop: Option<String>,
    pub iteration: u32,
    pub exploitability_pct: f64,
    pub elapsed_ms: u64,
    pub threads: usize,
    /// This session: spots solved here, spots fetched from the cloud, spots shared.
    pub solved: u32,
    pub fetched: u32,
    pub shared: u32,
    pub last_error: Option<String>,
}

#[derive(Default)]
pub struct LibraryState {
    status: Mutex<WorkerStatus>,
    /// Spots that failed this session; tried again after a restart.
    skip: Mutex<HashSet<SpotId>>,
    packs_checked: Mutex<Option<Instant>>,
}

fn update(app: &AppHandle, f: impl FnOnce(&mut WorkerStatus)) {
    let state = app.state::<LibraryState>();
    let mut s = state.status.lock().unwrap();
    f(&mut s);
    let _ = app.emit("library-status", s.clone());
}

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("library".into())
        .spawn(move || {
            // Each start begins at another flop, so computers that solve at the same time
            // rarely pick the same spot.
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos() as usize);
            let offset = nanos % catalog().flops.len();
            loop {
                tick(&app, offset);
                std::thread::sleep(TICK);
            }
        })
        .expect("library thread starts");
}

fn background_threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get()).div_ceil(2).max(1)
}

fn tick(app: &AppHandle, offset: usize) {
    let Some(root) = root(app) else { return };
    let cloud = app.state::<AppState>().settings.lock().unwrap().cloud;
    // Packs are downloads, not work for this computer, so they come whatever the mode.
    if cloud.fetch && may_talk(app) {
        let due = app.state::<LibraryState>().packs_checked.lock().unwrap().is_none_or(|t| t.elapsed() >= PACKS_EVERY);
        if due {
            update(app, |s| (s.state, s.reason) = ("packs", None));
            *app.state::<LibraryState>().packs_checked.lock().unwrap() = Some(Instant::now());
            if let Err(e) = import_packs(app, &root) {
                eprintln!("library packs: {e}");
                update(app, |s| s.last_error = Some(format!("Pakete: {e}")));
            }
        }
    }
    let mode = mode_of(app);
    if let Some(reason) = blocked(app, mode) {
        let state = if mode == LibraryMode::Off { "off" } else { "waiting" };
        update(app, |s| (s.state, s.reason) = (state, Some(reason)));
        return;
    }
    let Some(id) = next_open(app, &root, offset) else {
        update(app, |s| (s.state, s.reason, s.scenario, s.flop) = ("done", None, None, None));
        return;
    };
    let c = catalog();
    let (scenario, flop) = (c.scenarios[id.scenario].id.clone(), c.flops[id.flop].clone());
    let input = PostflopInput::new(&scenario, &flop);
    update(app, |s| {
        (s.scenario, s.flop, s.iteration, s.exploitability_pct, s.elapsed_ms) =
            (Some(scenario.clone()), Some(flop.clone()), 0, 0.0, 0);
    });

    if cloud.fetch && may_talk(app) {
        update(app, |s| (s.state, s.reason) = ("cloud", None));
        match Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient()).get_solution::<Postflop>(&input) {
            Ok(Some(found)) => {
                if let Ok(bytes) = found.bytes()
                    && write(&root, Source::Cloud, id, &bytes).is_ok()
                {
                    update(app, |s| s.fetched += 1);
                    return;
                }
            }
            Ok(None) => {}
            Err(e) => note(app, &e),
        }
    }

    let threads = background_threads();
    update(app, |s| (s.state, s.reason, s.threads) = ("solving", None, threads));
    let s = &c.scenarios[id.scenario];
    let task = jobs::start(Kind::Library, format!("Bibliothek: {} gegen {} auf {flop}", s.ip, s.oop), Target::to("library", None));
    task.detail("Rechnet, solange du die App nicht benutzt");
    let progress_task = task.clone();
    let target = c.target_pct;
    let mut first: Option<f64> = None;
    let progress_app = app.clone();
    let mut last: Option<Instant> = None;
    let on_progress = move |p: &Progress| {
        if last.is_none_or(|t| t.elapsed() >= STATUS_EVERY) {
            last = Some(Instant::now());
            update(&progress_app, |s| {
                (s.iteration, s.exploitability_pct, s.elapsed_ms) = (p.iteration, p.exploitability_pct, p.elapsed_ms);
            });
        }
        if p.iteration > 0 {
            let start = *first.get_or_insert(p.exploitability_pct);
            let (from, to) = (start.max(target * 1.01).ln(), target.ln());
            progress_task.progress((from - p.exploitability_pct.max(target).ln()) / (from - to));
        }
    };
    let stop_app = app.clone();
    let stop_task = task.clone();
    let stop = move || {
        if stop_task.cancelled() {
            HALTED.store(true, Ordering::Relaxed);
        }
        blocked(&stop_app, mode_of(&stop_app)).is_some()
    };
    let limit = crate::solver::memory_limit() / 2;
    let result = solve_spot(c, id, threads, limit, on_progress, stop);
    match &result {
        Ok(_) => task.finish(),
        Err(SolverError::Cancelled) => task.stopped(),
        Err(e) => task.fail(e.to_string()),
    }
    match result {
        Ok(stored) => {
            let bytes = stored.to_bytes();
            if let Err(e) = write(&root, Source::Local, id, &bytes) {
                update(app, |s| s.last_error = Some(format!("Speichern: {e}")));
                app.state::<LibraryState>().skip.lock().unwrap().insert(id);
                return;
            }
            update(app, |s| s.solved += 1);
            let share = app.state::<AppState>().settings.lock().unwrap().cloud.share;
            if share && bytes.len() <= MAX_BYTES && may_talk(app) {
                let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() };
                let solution = PostflopSolution::from_stored(&stored);
                match Client::with_policy(DEFAULT_BASE_URL, policy).submit_solution::<Postflop>(&input, &solution) {
                    Ok(_) => update(app, |s| s.shared += 1),
                    Err(e) => note(app, &e),
                }
            }
        }
        Err(SolverError::Cancelled) => {}
        Err(e) => {
            app.state::<LibraryState>().skip.lock().unwrap().insert(id);
            update(app, |s| s.last_error = Some(format!("{scenario} {flop}: {e}")));
        }
    }
}

fn note(app: &AppHandle, e: &CloudError) {
    eprintln!("library cloud: {e}");
    app.state::<CloudState>().note(e);
}

/// The first spot that is neither stored nor skipped: scenarios in catalog order, flops
/// from `offset` on.
fn next_open(app: &AppHandle, root: &Path, offset: usize) -> Option<SpotId> {
    let c = catalog();
    let skip = app.state::<LibraryState>().skip.lock().unwrap().clone();
    let n = c.flops.len();
    (0..c.scenarios.len())
        .flat_map(|scenario| (0..n).map(move |k| SpotId { scenario, flop: (k + offset) % n }))
        .find(|id| !skip.contains(id) && status_of(root, *id).is_none())
}

#[derive(Default, Serialize, Deserialize)]
struct PacksSeen {
    serial: u64,
    imported: Vec<String>,
}

/// Imports library packs from the signed index that are not imported yet.
fn import_packs(app: &AppHandle, root: &Path) -> Result<usize, String> {
    let seen_path = root.join("packs.json");
    let mut seen: PacksSeen =
        std::fs::read(&seen_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let client = Packs::new(packs::DEFAULT_BASE_URL);
    let index = client.index(seen.serial)?;
    let mut imported = 0;
    let new: Vec<_> = index
        .packs
        .iter()
        .filter(|(n, _)| n.starts_with(PACK_PREFIX))
        .filter_map(|(name, versions)| newest_version(versions.keys()).map(|v| (name, v, &versions[v])))
        .filter(|(name, version, _)| !seen.imported.contains(&format!("{name}@{version}")))
        .collect();
    let total: usize = new.iter().map(|(_, _, p)| p.files.len()).sum();
    let task = (total > 0).then(|| jobs::start(Kind::Packs, "Bibliothek: Pakete laden", Target::to("library", None)));
    let mut done = 0;
    for (name, version, pack) in new {
        for f in &pack.files {
            if !may_talk(app) || task.as_ref().is_some_and(|t| t.should_stop()) {
                if let Some(t) = &task {
                    t.stopped();
                }
                return Err("unterbrochen".into());
            }
            if let Some(t) = &task {
                t.steps(done as u64, total as u64);
                t.detail(format!("Datei {} von {total}", done + 1));
            }
            let bytes = client.file(name, version, f).inspect_err(|e| {
                if let Some(t) = &task {
                    t.fail(e.to_string());
                }
            })?;
            imported += import_pack(root, &bytes)?;
            done += 1;
        }
        seen.imported.push(format!("{name}@{version}"));
    }
    seen.serial = index.serial;
    std::fs::create_dir_all(root).map_err(err)?;
    std::fs::write(&seen_path, serde_json::to_vec(&seen).map_err(err)?).map_err(err)?;
    if imported > 0 {
        update(app, |s| s.fetched += imported as u32);
    }
    Ok(imported)
}

/// Stores the spots of a pack that are not on this computer yet; each is read and checked
/// against the catalog first.
fn import_pack(root: &Path, bytes: &[u8]) -> Result<usize, String> {
    let c = catalog();
    let mut n = 0;
    for spot in unpack(bytes).map_err(err)? {
        let stored = Stored::from_bytes(spot, c).map_err(err)?;
        let id = SpotId {
            scenario: c.scenario(&stored.scenario).ok_or("scenario")?,
            flop: c.flop(&stored.flop).ok_or("flop")?,
        };
        if status_of(root, id).is_none() {
            write(root, Source::Cloud, id, spot)?;
            n += 1;
        }
    }
    Ok(n)
}

// -----------------------------------------------------------------------------------
// Commands.

#[derive(Serialize)]
struct ScenarioView {
    id: String,
    pot_type: PotType,
    oop: String,
    ip: String,
    stack_bb: f64,
    pot: f64,
    stack: f64,
    /// Per catalog flop: null (open), "local" or "cloud".
    status: Vec<Option<Source>>,
}

#[derive(Serialize)]
pub struct Overview {
    catalog: u32,
    target_pct: f64,
    flops: Vec<String>,
    flop_weights: Vec<f64>,
    scenarios: Vec<ScenarioView>,
    disk_bytes: u64,
    mode: LibraryMode,
    worker: WorkerStatus,
}

/// The catalog with what is stored of it, the disk use and the worker's state.
#[tauri::command]
pub async fn library_overview(app: AppHandle) -> CmdResult<Overview> {
    blocking(app.clone(), move |state| {
        let root = root(&app).ok_or("Kein Datenordner")?;
        let c = catalog();
        let scenarios = c
            .scenarios
            .iter()
            .enumerate()
            .map(|(scenario, s)| ScenarioView {
                id: s.id.clone(),
                pot_type: s.pot_type,
                oop: s.oop.clone(),
                ip: s.ip.clone(),
                stack_bb: s.stack_bb,
                pot: s.pot,
                stack: s.stack,
                status: (0..c.flops.len()).map(|flop| status_of(&root, SpotId { scenario, flop })).collect(),
            })
            .collect();
        Ok(Overview {
            catalog: c.version,
            target_pct: c.target_pct,
            flops: c.flops.clone(),
            flop_weights: c.flop_weights.clone(),
            scenarios,
            disk_bytes: disk_bytes(&root),
            mode: state.settings.lock().unwrap().library.background,
            worker: app.state::<LibraryState>().status.lock().unwrap().clone(),
        })
    })
    .await
}

#[tauri::command]
pub async fn library_set_mode(mode: LibraryMode, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.library.background = mode;
    HALTED.store(false, Ordering::Relaxed);
    store_settings(&app, &state, settings)
}

#[derive(Serialize)]
pub struct Opened {
    spot: Spot,
    summary: Summary,
    oop: String,
    ip: String,
    oop_note: String,
    ip_note: String,
    source: Source,
}

/// Loads a stored spot into the solver page; its nodes come from `solver_node`.
#[tauri::command]
pub async fn library_open(scenario: String, flop: String, app: AppHandle) -> CmdResult<Opened> {
    blocking(app.clone(), move |_| {
        let c = catalog();
        let id = SpotId {
            scenario: c.scenario(&scenario).ok_or("Unbekannter Spot")?,
            flop: c.flop(&flop).ok_or("Unbekannter Flop")?,
        };
        let root = root(&app).ok_or("Kein Datenordner")?;
        let source = status_of(&root, id).ok_or("Dieser Spot ist noch nicht in der Bibliothek.")?;
        let bytes = std::fs::read(spot_path(&root, source, id)).map_err(err)?;
        let stored = Stored::from_bytes(&bytes, c).map_err(err)?;
        let s = &c.scenarios[id.scenario];
        let spot = s.spot(&flop);
        let summary = Summary {
            iterations: stored.iterations,
            exploitability: stored.exploitability_pct as f64 / 100.0 * s.pot,
            exploitability_pct: stored.exploitability_pct as f64,
            converged: stored.exploitability_pct as f64 <= c.target_pct + 1e-6,
            elapsed_ms: 0,
            memory_bytes: bytes.len() as u64,
            compressed: false,
            threads: 0,
        };
        crate::solver::show_stored(stored);
        Ok(Opened {
            spot,
            summary,
            oop: s.oop.clone(),
            ip: s.ip.clone(),
            oop_note: s.oop_note.clone(),
            ip_note: s.ip_note.clone(),
            source,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_import_only_new_catalog_spots() {
        let dir = std::env::temp_dir().join(format!("ringside-library-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(import_pack(&dir, b"not a pack").is_err());
        let empty = ringside_solver::library::format::pack(&[]);
        assert_eq!(import_pack(&dir, &empty).unwrap(), 0);
        let id = SpotId { scenario: 0, flop: 3 };
        write(&dir, Source::Local, id, b"x").unwrap();
        assert_eq!(status_of(&dir, id), Some(Source::Local));
        assert_eq!(status_of(&dir, SpotId { scenario: 0, flop: 4 }), None);
        assert_eq!(disk_bytes(&dir), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The app's download path against the real CDN: signed index, file hash, every spot
    /// read against the catalog. Needs the network: `cargo test -p ringside -- --ignored`.
    #[test]
    #[ignore]
    fn downloads_the_signed_library_pack() {
        let dir = std::env::temp_dir().join(format!("ringside-library-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let client = Packs::new(packs::DEFAULT_BASE_URL);
        let index = client.index(0).expect("signed index");
        let mut imported = 0;
        for (name, versions) in index.packs.iter().filter(|(n, _)| n.starts_with(PACK_PREFIX)) {
            let version = newest_version(versions.keys()).expect("a version");
            for f in &versions[version].files {
                let bytes = client.file(name, version, f).expect("file matches the index");
                let n = import_pack(&dir, &bytes).expect("spots read against the catalog");
                println!("{name} {version} {}: {n} spots, {} bytes", f.path, bytes.len());
                imported += n;
            }
        }
        assert!(imported > 0, "no library pack in the index");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
