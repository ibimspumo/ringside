//! The spot library in the app: the catalog's spots stored on this computer, downloaded
//! from signed packs and Ringside Cloud, or solved here.
//!
//! The worker solves when the user starts it on the library page (until they stop it,
//! PokerStars starts or no spot is left) and, automatically, when the setting allows it
//! (by default while nobody uses the computer) and the app is not in use. It never runs
//! while PokerStars runs; a solve stops within an iteration when that changes and starts
//! over later. Before it solves a spot it asks the cloud, and afterwards it shares the
//! spot if the user allows it. `shared.json` remembers what became of every shared spot;
//! now and then the worker asks the cloud about the ones still waiting for it. Solves
//! take the cores the power setting allows and at most a quarter of the memory. Progress
//! survives a restart because every finished spot is a file.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, LazyLock, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use ringside_cloud::client::{DEFAULT_BASE_URL, MAX_STATUS_KEYS, RetryPolicy};
use ringside_cloud::kinds::postflop::MAX_BYTES;
use ringside_cloud::kinds::{Postflop, PostflopInput, PostflopSolution};
use ringside_cloud::packs::{self, Packs, newest_version};
use ringside_cloud::worker::{self, Power, Solved, Task};
use ringside_cloud::{Client, Error as CloudError, Kind as _, KeyStatus, SubmitStatus, Work};
use ringside_solver::library::format::{Stored, encode, unpack};
use ringside_solver::library::{PotType, SpotId, catalog, solve_spot};
use ringside_solver::gate::Priority;
use ringside_solver::{Error as SolverError, Progress, Spot, Summary};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cloud::{CloudState, may_talk};
use crate::jobs::{self, Job, Kind, Target};
use crate::settings::{LibraryMode, LibraryPower};
use crate::{AppState, CmdResult, blocking, err, store_settings, unlocked};

/// Without input for this long the computer counts as idle.
const IDLE_AFTER: Duration = Duration::from_secs(5 * 60);
/// The app counts as in use while focused and touched within this time.
const IN_USE_FOR: Duration = Duration::from_secs(2 * 60);
const TICK: Duration = Duration::from_secs(5);
const STATUS_EVERY: Duration = Duration::from_millis(500);
/// Packs are looked for at most this often.
const PACKS_EVERY: Duration = Duration::from_secs(6 * 3600);
/// The verifier runs hourly; asking more often would rarely find anything new.
const CHECK_SHARED_EVERY: Duration = Duration::from_secs(30 * 60);
/// Status requests per check, each for up to `MAX_STATUS_KEYS` spots.
const CHECK_BATCHES: usize = 5;
/// Spots that still have to go up are sent one at a time at this pace, so the server's
/// limit of 60 submissions an hour leaves room for freshly solved ones.
const UPLOAD_EVERY: Duration = Duration::from_secs(90);
const RETRY_FAILED_AFTER_S: u64 = 6 * 3600;
const PACK_PREFIX: &str = "library-v1-";
const SHARED_FILE: &str = "shared.json";

static FOCUSED: AtomicBool = AtomicBool::new(true);
/// Stopped by the user; the automatic rests until they start again, change the mode or
/// restart the app.
static HALTED: AtomicBool = AtomicBool::new(false);
static FOCUS_SINCE: LazyLock<Mutex<Instant>> = LazyLock::new(|| Mutex::new(Instant::now()));
/// Held while `shared.json` is read, changed and written: uploads run beside the worker.
static SHARED_LOG: Mutex<()> = Mutex::new(());

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

/// Why the worker does not solve now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Wait {
    /// PokerStars runs.
    Client,
    /// Stopped by the user on the page or in the task center.
    Halted,
    Off,
    /// Waits until nobody has used the computer for a while.
    Idle,
    /// Idle time is only known on Windows.
    IdleUnknown,
    /// The user works with the app.
    InUse,
    /// A more urgent calculation runs: what the user looks at, new hands, practice.
    Busy,
}

/// Why the worker may not solve now; `None` when it may.
fn blocked(app: &AppHandle, mode: LibraryMode) -> Option<Wait> {
    if app.state::<AppState>().client_running.load(Ordering::Relaxed) {
        return Some(Wait::Client);
    }
    if manual_active(app) {
        return None;
    }
    if HALTED.load(Ordering::Relaxed) && mode != LibraryMode::Off {
        return Some(Wait::Halted);
    }
    match mode {
        LibraryMode::Off => return Some(Wait::Off),
        LibraryMode::Idle if !system_idle().is_some_and(|d| d >= IDLE_AFTER) => {
            return Some(if cfg!(windows) { Wait::Idle } else { Wait::IdleUnknown });
        }
        _ => {}
    }
    app_in_use().then_some(Wait::InUse)
}

fn mode_of(app: &AppHandle) -> LibraryMode {
    app.state::<AppState>().settings.lock().unwrap().library.background
}

fn cores() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get())
}

/// Solver threads for `power`: a quarter, half or all but one of the cores.
fn threads_for(power: LibraryPower, cores: usize) -> usize {
    match power {
        LibraryPower::Gentle => cores.div_ceil(4),
        LibraryPower::Normal => cores.div_ceil(2),
        LibraryPower::Full => cores.saturating_sub(1),
    }
    .max(1)
}

/// How far a solve is, 0–1: the error falls roughly geometrically, so the share follows
/// its logarithm from the first measurement to the target.
fn spot_share(first: f64, now: f64, target: f64) -> f64 {
    let (from, to) = (first.max(target * 1.01).ln(), target.ln());
    ((from - now.max(target).ln()) / (from - to)).clamp(0.0, 1.0)
}

const SUITS: [(char, char); 4] = [('s', '♠'), ('h', '♥'), ('d', '♦'), ('c', '♣')];

/// "Du am Button gegen den Big Blind, Flop K♠ 7♦ 2♣", as `titleOf` says it on the page.
fn spot_label(scenario: &str, flop: &str) -> String {
    let seat = |p: &str| -> (String, String) {
        let (at, against) = match p {
            "btn" => ("am Button", "den Button"),
            "co" => ("im Cutoff", "den Cutoff"),
            "utg" => ("in früher Position", "die frühe Position"),
            "sb" => ("im Small Blind", "den Small Blind"),
            "bb" => ("im Big Blind", "den Big Blind"),
            _ => return (format!("auf {}", p.to_uppercase()), p.to_uppercase()),
        };
        (at.into(), against.into())
    };
    let mut seats = scenario.split('-').skip(1);
    let (opener, other) = (seat(seats.next().unwrap_or("")), seat(seats.next().unwrap_or("")));
    let chars: Vec<char> = flop.chars().collect();
    let cards: Vec<String> = chars
        .chunks(2)
        .map(|c| {
            let suit = c.get(1).map(|s| SUITS.iter().find(|(k, _)| k == s).map_or(*s, |(_, v)| *v));
            format!("{}{}", c[0], suit.map(String::from).unwrap_or_default())
        })
        .collect();
    format!("Du {} gegen {}, Flop {}", opener.0, other.1, cards.join(" "))
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// True when `last` is unset or older than `every`, and then marks now.
fn due(last: &Mutex<Option<Instant>>, every: Duration) -> bool {
    let mut last = last.lock().unwrap();
    let due = last.is_none_or(|t| t.elapsed() >= every);
    if due {
        *last = Some(Instant::now());
    }
    due
}

// -----------------------------------------------------------------------------------
// Files.

pub(crate) use crate::store::Source;
use crate::store::{library_path as spot_path, library_status as status_of, read_library, write_atomic as write_file};

/// The library's folder in the data folder.
pub(crate) fn root(app: &AppHandle) -> Option<PathBuf> {
    Some(crate::store::library_root(&crate::data_dir(app).ok()?))
}

fn all_spots() -> impl Iterator<Item = SpotId> {
    let c = catalog();
    (0..c.scenarios.len()).flat_map(move |scenario| (0..c.flops.len()).map(move |flop| SpotId { scenario, flop }))
}

/// Writes a library spot through the store, which files it in its index.
fn write(app: &AppHandle, root: &Path, source: Source, id: SpotId, bytes: &[u8]) -> Result<(), String> {
    let data = crate::data_dir(app).map_err(err)?;
    crate::store::write_library(&app.state::<AppState>(), &data, root, source, id, bytes)
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
// Sharing.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareState {
    /// Uploaded, waits for the verifier (a server from before accepting at once).
    Submitted,
    /// Served to everyone: its form checked out, or the verifier re-solved it.
    #[serde(alias = "verified")]
    Served,
    /// Refused, or withdrawn after a failed sample check.
    Rejected,
    /// The upload failed; tried again later.
    Failed,
}

/// What became of a spot solved here and offered to the cloud.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shared {
    pub state: ShareState,
    /// Unix seconds of the upload or of the failed try.
    pub at: u64,
    /// The verifier's reason for a rejection, or the error of a failed upload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Per spot ("scenario/flop").
type SharedLog = BTreeMap<String, Shared>;

fn shared_key(id: SpotId) -> String {
    let c = catalog();
    format!("{}/{}", c.scenarios[id.scenario].id, c.flops[id.flop])
}

fn read_shared(root: &Path) -> SharedLog {
    std::fs::read(root.join(SHARED_FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write_shared(root: &Path, log: &SharedLog) -> Result<(), String> {
    write_file(&root.join(SHARED_FILE), &serde_json::to_vec(log).map_err(err)?)
}

fn input_of(id: SpotId) -> PostflopInput {
    let c = catalog();
    PostflopInput::new(&c.scenarios[id.scenario].id, &c.flops[id.flop])
}

/// Offers a spot solved here to the cloud and records the outcome. Only with the user's
/// consent (`cloud.share`) and when `may_talk`.
fn share(app: &AppHandle, root: &Path, id: SpotId, stored: &Stored, size: usize) {
    let at = unix_now();
    let shared = if size > MAX_BYTES {
        Shared { state: ShareState::Failed, at, note: Some("zu groß zum Teilen".into()) }
    } else {
        let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() };
        let solution = PostflopSolution::from_stored(stored);
        match Client::with_policy(DEFAULT_BASE_URL, policy).submit_solution::<Postflop>(&input_of(id), &solution) {
            Ok(status) => {
                update(app, |s| s.shared += 1);
                let state = match status {
                    SubmitStatus::Submitted => ShareState::Submitted,
                    SubmitStatus::Accepted | SubmitStatus::AlreadyVerified => ShareState::Served,
                };
                Shared { state, at, note: None }
            }
            Err(e) => {
                note(app, &e);
                Shared { state: ShareState::Failed, at, note: Some(e.to_string()) }
            }
        }
    };
    let _log = SHARED_LOG.lock().unwrap();
    let mut log = read_shared(root);
    log.insert(shared_key(id), shared);
    if let Err(e) = write_shared(root, &log) {
        update(app, |s| s.last_error = Some(format!("Speichern: {e}")));
    }
}

/// The record that follows from the cloud's answer for a spot solved here; `None` when
/// this computer never got it up, so it has to be shared (again).
fn after_check(known: Option<&Shared>, status: KeyStatus, now: u64) -> Option<Shared> {
    let at = known.map_or(now, |s| s.at);
    match (status, known) {
        (KeyStatus::Accepted | KeyStatus::Verified, _) => Some(Shared { state: ShareState::Served, at, note: None }),
        (KeyStatus::Submitted, _) => Some(Shared { state: ShareState::Submitted, at, note: None }),
        (KeyStatus::Rejected(note), Some(_)) => Some(Shared { state: ShareState::Rejected, at, note }),
        // Someone else's copy was rejected or the server has none; ours never went up.
        (KeyStatus::Rejected(_) | KeyStatus::Unknown, _) => None,
    }
}

/// Asks the cloud what became of the spots waiting for the verifier, and about spots
/// solved here before sharing was recorded. Spots it does not have go into the upload
/// queue.
fn check_shared(app: &AppHandle, root: &Path) -> Result<(), CloudError> {
    let _log = SHARED_LOG.lock().unwrap();
    let mut log = read_shared(root);
    let open: Vec<SpotId> = all_spots()
        .filter(|&id| spot_path(root, Source::Local, id).is_file())
        .filter(|&id| log.get(&shared_key(id)).is_none_or(|s| s.state == ShareState::Submitted))
        .collect();
    if open.is_empty() {
        return Ok(());
    }
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy { attempts: 2, ..RetryPolicy::impatient() });
    let mut queue = Vec::new();
    for chunk in open.chunks(MAX_STATUS_KEYS).take(CHECK_BATCHES) {
        let keys = chunk
            .iter()
            .map(|&id| Postflop::key(&input_of(id)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CloudError::Protocol(e.to_string()))?;
        for (&id, (_, status)) in chunk.iter().zip(client.statuses(&keys)?) {
            let k = shared_key(id);
            match after_check(log.get(&k), status, unix_now()) {
                Some(s) => {
                    log.insert(k, s);
                }
                None => {
                    log.remove(&k);
                    queue.push(id);
                }
            }
        }
    }
    write_shared(root, &log).map_err(CloudError::Protocol)?;
    let state = app.state::<LibraryState>();
    let mut upload = state.upload.lock().unwrap();
    for id in queue {
        if !upload.contains(&id) {
            upload.push(id);
        }
    }
    Ok(())
}

/// Sends the next spot of the upload queue, or a failed upload that is old enough.
fn upload_next(app: &AppHandle, root: &Path) {
    let queued = app.state::<LibraryState>().upload.lock().unwrap().pop();
    let id = queued.or_else(|| {
        let log = read_shared(root);
        let now = unix_now();
        all_spots().find(|&id| {
            log.get(&shared_key(id)).is_some_and(|s| {
                s.state == ShareState::Failed && now.saturating_sub(s.at) >= RETRY_FAILED_AFTER_S
            }) && spot_path(root, Source::Local, id).is_file()
        })
    });
    let Some(id) = id else { return };
    let Ok(bytes) = read_library(root, Source::Local, id) else { return };
    match Stored::from_bytes(&bytes, catalog()) {
        Ok(stored) => share(app, root, id, &stored, bytes.len()),
        Err(e) => update(app, |s| s.last_error = Some(format!("{}: {e}", shared_key(id)))),
    }
}

// -----------------------------------------------------------------------------------
// Worker.

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkerStatus {
    /// "off", "waiting", "packs", "cloud", "solving" or "done".
    pub state: &'static str,
    pub reason: Option<Wait>,
    /// Started by the user on the page.
    pub manual: bool,
    pub scenario: Option<String>,
    pub flop: Option<String>,
    pub iteration: u32,
    pub exploitability_pct: f64,
    /// Of the current spot, 0–1, once measured.
    pub progress: Option<f64>,
    pub elapsed_ms: u64,
    pub threads: usize,
    /// This session: spots solved here, spots fetched from the cloud, spots shared.
    pub solved: u32,
    pub fetched: u32,
    pub shared: u32,
    /// The queue's spot solved now for other users, in words, and how many went up this session.
    pub community: Option<String>,
    pub community_done: u32,
    pub last_error: Option<String>,
}

/// A run the user started on the page: one task in the task center for all its spots.
struct Manual {
    job: Job,
    done: u64,
    total: u64,
}

/// A cloud lookup on its own thread: the spot's file if the cloud has it.
type Lookup = JoinHandle<Option<Vec<u8>>>;

#[derive(Default)]
pub struct LibraryState {
    status: Mutex<WorkerStatus>,
    /// Spots that failed this session; tried again after a restart.
    skip: Mutex<HashSet<SpotId>>,
    packs_checked: Mutex<Option<Instant>>,
    shared_checked: Mutex<Option<Instant>>,
    uploaded: Mutex<Option<Instant>>,
    /// Spots solved here that the cloud does not have from this computer yet.
    upload: Mutex<Vec<SpotId>>,
    manual: Mutex<Option<Manual>>,
    /// The upload of the spot solved last, running while the next one is solved.
    sharing: Mutex<Option<JoinHandle<()>>>,
    /// The cloud lookup of the spot expected next, running while the current one is solved.
    ahead: Mutex<Option<(SpotId, Lookup)>>,
    woken: Mutex<bool>,
    wake: Condvar,
}

fn update(app: &AppHandle, f: impl FnOnce(&mut WorkerStatus)) {
    let state = app.state::<LibraryState>();
    let mut s = state.status.lock().unwrap();
    f(&mut s);
    let _ = app.emit("library-status", s.clone());
}

/// Lets the worker look at its situation now instead of after the next pause.
fn wake(app: &AppHandle) {
    let state = app.state::<LibraryState>();
    *state.woken.lock().unwrap() = true;
    state.wake.notify_one();
}

fn manual_active(app: &AppHandle) -> bool {
    app.state::<LibraryState>().manual.lock().unwrap().as_ref().is_some_and(|m| !m.job.cancelled())
}

/// Ends the user's run, if one runs, with `how` (finished or stopped).
fn end_manual(app: &AppHandle, how: impl FnOnce(&Job)) {
    let ended = app.state::<LibraryState>().manual.lock().unwrap().take();
    if let Some(m) = ended {
        how(&m.job);
        update(app, |s| s.manual = false);
    }
}

/// Counts a spot that the user's run added to the library.
fn count_manual(app: &AppHandle) {
    if let Some(m) = app.state::<LibraryState>().manual.lock().unwrap().as_mut() {
        m.done += 1;
        m.job.steps(m.done, m.total.max(m.done));
    }
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
                if tick(&app, offset) {
                    continue;
                }
                let state = app.state::<LibraryState>();
                let woken = state.woken.lock().unwrap();
                let (mut woken, _) = state.wake.wait_timeout_while(woken, TICK, |w| !*w).unwrap();
                *woken = false;
            }
        })
        .expect("library thread starts");
}

/// Downloads and uploads; they cost the computer nothing worth asking for, so they run
/// whatever the mode.
fn cloud_chores(app: &AppHandle, root: &Path) {
    let cloud = app.state::<AppState>().settings.lock().unwrap().cloud;
    let state = app.state::<LibraryState>();
    if cloud.fetch && may_talk(app) && due(&state.packs_checked, PACKS_EVERY) {
        update(app, |s| (s.state, s.reason) = ("packs", None));
        if let Err(e) = import_packs(app, root) {
            eprintln!("library packs: {e}");
            update(app, |s| s.last_error = Some(format!("Pakete: {e}")));
        }
    }
    if (cloud.fetch || cloud.share) && may_talk(app) && due(&state.shared_checked, CHECK_SHARED_EVERY) {
        if let Err(e) = check_shared(app, root) {
            note(app, &e);
        }
        if let Err(e) = crate::cloud::check_spots(app) {
            note(app, &e);
        }
    }
    if cloud.share && may_talk(app) && due(&state.uploaded, UPLOAD_EVERY) {
        upload_next(app, root);
    }
}

/// One step of the worker; true when it should go on with the next spot at once.
fn tick(app: &AppHandle, offset: usize) -> bool {
    let Some(root) = root(app) else { return false };
    let state = app.state::<LibraryState>();
    // A lookup made during the previous solve; any older one may be out of date.
    let ahead = state.ahead.lock().unwrap().take();
    let cancelled = state.manual.lock().unwrap().as_ref().is_some_and(|m| m.job.cancelled());
    if cancelled {
        HALTED.store(true, Ordering::Relaxed);
        end_manual(app, Job::stopped);
    }
    if app.state::<AppState>().client_running.load(Ordering::Relaxed) {
        end_manual(app, Job::stopped);
    }
    cloud_chores(app, &root);

    let mode = mode_of(app);
    if let Some(reason) = blocked(app, mode) {
        let st = if reason == Wait::Off { "off" } else { "waiting" };
        update(app, |s| (s.state, s.reason, s.progress) = (st, Some(reason), None));
        return false;
    }
    let Some(id) = next_open(app, &root, offset, None) else {
        // The queue hands out a few spots a minute per computer, so the pause costs nothing.
        if manual_active(app) && community(app, &root) {
            return false;
        }
        end_manual(app, Job::finish);
        update(app, |s| (s.state, s.reason, s.scenario, s.flop, s.progress) = ("done", None, None, None, None));
        return false;
    };
    let c = catalog();
    let (scenario, flop) = (c.scenarios[id.scenario].id.clone(), c.flops[id.flop].clone());
    let label = spot_label(&scenario, &flop);
    let manual = state.manual.lock().unwrap().as_ref().map(|m| m.job.clone());
    update(app, |s| {
        (s.scenario, s.flop, s.iteration, s.exploitability_pct, s.elapsed_ms, s.progress) =
            (Some(scenario.clone()), Some(flop.clone()), 0, 0.0, 0, None);
    });

    let cloud = app.state::<AppState>().settings.lock().unwrap().cloud;
    if cloud.fetch && may_talk(app) {
        update(app, |s| (s.state, s.reason) = ("cloud", None));
        if let Some(job) = &manual {
            job.detail(format!("Schaut in der Cloud nach: {label}"));
        }
        let found = match ahead {
            Some((next, looking)) if next == id => looking.join().ok().flatten(),
            _ => download(app, id),
        };
        if let Some(bytes) = found
            && write(app, &root, Source::Cloud, id, &bytes).is_ok()
        {
            update(app, |s| s.fetched += 1);
            count_manual(app);
            return true;
        }
        if let Some(next) = next_open(app, &root, offset, Some(id)) {
            let app = app.clone();
            *state.ahead.lock().unwrap() = Some((next, std::thread::spawn(move || download(&app, next))));
        }
    }

    let power = app.state::<AppState>().settings.lock().unwrap().library.power;
    let threads = threads_for(power, cores());
    let ticket = crate::store::gate().enter(Priority::Community);
    if !ticket.hold(TICK) {
        update(app, |s| (s.state, s.reason, s.progress) = ("waiting", Some(Wait::Busy), None));
        return false;
    }
    update(app, |s| (s.state, s.reason, s.threads) = ("solving", None, threads));
    let task = match &manual {
        Some(job) => {
            job.detail(format!("{label} – 0 %"));
            job.clone()
        }
        None => {
            let job = jobs::start(Kind::Library, format!("Bibliothek: {label}"), Target::to("library", None));
            job.detail("Automatisch, solange du die App nicht benutzt");
            job
        }
    };
    let is_manual = manual.is_some();
    let progress_task = task.clone();
    let target = c.target_pct;
    let mut first: Option<f64> = None;
    let progress_app = app.clone();
    let mut last: Option<Instant> = None;
    let on_progress = move |p: &Progress| {
        let share = (p.iteration > 0)
            .then(|| spot_share(*first.get_or_insert(p.exploitability_pct), p.exploitability_pct, target));
        if !is_manual && let Some(share) = share {
            progress_task.progress(share);
        }
        if last.is_none_or(|t| t.elapsed() >= STATUS_EVERY) {
            last = Some(Instant::now());
            update(&progress_app, |s| {
                (s.iteration, s.exploitability_pct, s.elapsed_ms, s.progress) =
                    (p.iteration, p.exploitability_pct, p.elapsed_ms, share);
            });
            if is_manual {
                progress_task.detail(format!("{label} – {} %", (share.unwrap_or(0.0) * 100.0).round()));
            }
        }
    };
    let stop_app = app.clone();
    let stop_task = task.clone();
    let stop = || {
        if stop_task.cancelled() {
            HALTED.store(true, Ordering::Relaxed);
        }
        blocked(&stop_app, mode_of(&stop_app)).is_some() || ticket.should_yield()
    };
    let limit = crate::solver::memory_limit() / 2;
    let result = solve_spot(c, id, threads, limit, on_progress, stop);
    drop(ticket);
    // The user's run is one task for all its spots; it ends in `end_manual`.
    if !is_manual {
        match &result {
            Ok(_) => task.finish(),
            Err(SolverError::Cancelled) => task.stopped(),
            Err(e) => task.fail(e.to_string()),
        }
    }
    match result {
        Ok(stored) => {
            let bytes = stored.to_bytes();
            if let Err(e) = write(app, &root, Source::Local, id, &bytes) {
                update(app, |s| s.last_error = Some(format!("Speichern: {e}")));
                state.skip.lock().unwrap().insert(id);
                return true;
            }
            update(app, |s| (s.solved, s.progress) = (s.solved + 1, Some(1.0)));
            count_manual(app);
            if cloud.share && may_talk(app) {
                share_aside(app, &root, id, stored, bytes.len());
            }
            true
        }
        Err(SolverError::Cancelled) => false,
        Err(e) => {
            state.skip.lock().unwrap().insert(id);
            update(app, |s| s.last_error = Some(format!("{scenario} {flop}: {e}")));
            true
        }
    }
}

/// The spot's file if the cloud has a solution of it whose form checks out.
pub(crate) fn download(app: &AppHandle, id: SpotId) -> Option<Vec<u8>> {
    match Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient()).get_solution::<Postflop>(&input_of(id)) {
        Ok(found) => found.and_then(|f| f.bytes().ok()),
        Err(e) => {
            note(app, &e);
            None
        }
    }
}

/// A catalog spot solved elsewhere in the app (practice, the solver page): kept in the
/// library unless it is there already, and shared like the library's own solves when it
/// is as exact as the library's.
pub(crate) fn offer(app: &AppHandle, id: SpotId, solution: &mut ringside_solver::Solution) {
    let c = catalog();
    let Some(root) = root(app) else { return };
    if status_of(&root, id).is_some() || solution.summary.exploitability_pct > c.target_pct + 1e-6 {
        return;
    }
    // A solve on a suit renaming of the catalog flop is kept on the catalog flop.
    let scenario = &c.scenarios[id.scenario];
    let stored = match encode(solution, c, scenario, &solution.spot.board.clone()).and_then(|s| s.renamed(scenario, &c.flops[id.flop])) {
        Ok(s) => s,
        Err(e) => return eprintln!("library: keeping {}: {e}", shared_key(id)),
    };
    let bytes = stored.to_bytes();
    if let Err(e) = write(app, &root, Source::Local, id, &bytes) {
        return eprintln!("library: keeping {}: {e}", shared_key(id));
    }
    let cloud = app.state::<AppState>().settings.lock().unwrap().cloud;
    if cloud.share && may_talk(app) {
        share_aside(app, &root, id, stored, bytes.len());
    }
}

/// [`share`] on its own thread, so the next solve need not wait for the network. One
/// upload at a time: a slow one holds up the next spot as before.
fn share_aside(app: &AppHandle, root: &Path, id: SpotId, stored: Stored, size: usize) {
    let state = app.state::<LibraryState>();
    let mut sharing = state.sharing.lock().unwrap();
    if let Some(previous) = sharing.take() {
        let _ = previous.join();
    }
    let (app, root) = (app.clone(), root.to_path_buf());
    *sharing = Some(std::thread::spawn(move || share(&app, &root, id, &stored, size)));
}

// -----------------------------------------------------------------------------------
// The community queue: once the library has no spot left, a run the user started solves
// spots other Ringside users need (`POST /v1/work/next`) and submits them.

/// The queue's spot in words.
fn community_label(work: &Work) -> String {
    let flop = |f: &str| spot_label("", f).rsplit_once("Flop ").map_or(String::new(), |(_, cards)| cards.to_string());
    match work.kind.as_str() {
        "spot" => {
            let i = &work.input;
            format!(
                "Spot eines Ringside-Nutzers: {} Spieler, {} BB, Flop {}",
                i["players"],
                i["stack_bb"],
                flop(i["board"].as_str().unwrap_or(""))
            )
        }
        "postflop" => spot_label(work.input["scenario"].as_str().unwrap_or(""), work.input["flop"].as_str().unwrap_or("")),
        "pushfold-mw" => format!(
            "Push/Fold mit {} Spielern{}",
            work.input["stacks_mbb"].as_array().map_or(0, Vec::len),
            if work.input["payouts_ppm"].as_array().is_some_and(|p| !p.is_empty()) { " am Final Table (ICM)" } else { "" }
        ),
        _ => "Push/Fold heads-up".into(),
    }
}

/// Solves one spot of the queue and submits it. Only in a run the user started, with
/// sharing on, as the result goes up. False when there is nothing to do.
fn community(app: &AppHandle, root: &Path) -> bool {
    let cloud = app.state::<AppState>().settings.lock().unwrap().cloud;
    let Some(job) = app.state::<LibraryState>().manual.lock().unwrap().as_ref().map(|m| m.job.clone()) else { return false };
    if !cloud.share || !may_talk(app) {
        return false;
    }
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy { attempts: 2, ..RetryPolicy::impatient() });
    let work = match client.next_work(&worker::KINDS) {
        Ok(Some(w)) => w,
        Ok(None) => return false,
        Err(e) => {
            note(app, &e);
            return false;
        }
    };
    let label = community_label(&work);
    update(app, |s| {
        (s.state, s.reason, s.scenario, s.flop, s.community, s.progress) = ("solving", None, None, None, Some(label.clone()), None);
    });
    job.detail(format!("Für die Community: {label}"));
    let result = solve_for_community(app, root, &work, &job, &label);
    match result {
        Ok(true) => {
            update(app, |s| (s.community_done, s.progress) = (s.community_done + 1, Some(1.0)));
            count_manual(app);
        }
        Ok(false) => {}
        Err(e) => update(app, |s| s.last_error = Some(format!("{label}: {e}"))),
    }
    update(app, |s| s.community = None);
    true
}

/// Ok(true) once the solution went up, Ok(false) when stopped.
fn solve_for_community(app: &AppHandle, root: &Path, work: &Work, job: &Job, label: &str) -> Result<bool, String> {
    let task = Task::parse(work)?;
    if let Task::Postflop { id, .. } = task {
        match status_of(root, id) {
            Some(Source::Local) => {
                let bytes = read_library(root, Source::Local, id)?;
                let stored = Stored::from_bytes(&bytes, catalog()).map_err(err)?;
                share(app, root, id, &stored, bytes.len());
                return Ok(true);
            }
            // Someone else's copy is here already; the queue hears of it from them.
            Some(Source::Cloud) => return Ok(false),
            None => {}
        }
    }
    let power = app.state::<AppState>().settings.lock().unwrap().library.power;
    let threads = threads_for(power, cores());
    let progress_app = app.clone();
    let progress_job = job.clone();
    let label_text = label.to_string();
    let mut first: Option<f64> = None;
    let mut last: Option<Instant> = None;
    let on_progress = move |p: &Progress| {
        let share = (p.iteration > 0).then(|| spot_share(*first.get_or_insert(p.exploitability_pct), p.exploitability_pct, 0.5));
        if last.is_none_or(|t| t.elapsed() >= STATUS_EVERY) {
            last = Some(Instant::now());
            update(&progress_app, |s| {
                (s.iteration, s.exploitability_pct, s.elapsed_ms, s.progress, s.threads) =
                    (p.iteration, p.exploitability_pct, p.elapsed_ms, share, threads);
            });
            progress_job.detail(format!("Für die Community: {label_text} – {} %", (share.unwrap_or(0.0) * 100.0).round()));
        }
    };
    let ticket = crate::store::gate().enter(Priority::Community);
    if !ticket.hold(TICK) {
        return Ok(false);
    }
    let stop_app = app.clone();
    let stop_job = job.clone();
    let stop = || {
        if stop_job.cancelled() {
            HALTED.store(true, Ordering::Relaxed);
        }
        stop_job.cancelled() || blocked(&stop_app, mode_of(&stop_app)).is_some() || ticket.should_yield()
    };
    let power = Power { threads, memory: crate::solver::memory_limit() / 2 };
    let solved = worker::solve(&task, power, on_progress, stop)?;
    drop(ticket);
    let Some(solved) = solved else { return Ok(false) };
    if let Solved::Postflop { id, stored, .. } = &solved {
        let bytes = stored.to_bytes();
        write(app, root, Source::Local, *id, &bytes)?;
        share(app, root, *id, stored, bytes.len());
        return Ok(true);
    }
    let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() };
    solved.submit(&Client::with_policy(DEFAULT_BASE_URL, policy)).map(|_| true).map_err(|e| e.to_string())
}

fn note(app: &AppHandle, e: &CloudError) {
    eprintln!("library cloud: {e}");
    app.state::<CloudState>().note(e);
}

/// The first spot that is neither stored, skipped nor `except`: scenarios in catalog
/// order, flops from `offset` on.
fn next_open(app: &AppHandle, root: &Path, offset: usize, except: Option<SpotId>) -> Option<SpotId> {
    let c = catalog();
    let skip = app.state::<LibraryState>().skip.lock().unwrap().clone();
    let n = c.flops.len();
    (0..c.scenarios.len())
        .flat_map(|scenario| (0..n).map(move |k| SpotId { scenario, flop: (k + offset) % n }))
        .find(|id| Some(*id) != except && !skip.contains(id) && status_of(root, *id).is_none())
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
            imported += import_pack(&app.state::<AppState>(), &crate::data_dir(app).map_err(err)?, root, &bytes)?;
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
fn import_pack(state: &AppState, data: &Path, root: &Path, bytes: &[u8]) -> Result<usize, String> {
    let c = catalog();
    let mut n = 0;
    for spot in unpack(bytes).map_err(err)? {
        let stored = Stored::from_bytes(spot, c).map_err(err)?;
        let id = SpotId {
            scenario: c.scenario(&stored.scenario).ok_or("scenario")?,
            flop: c.flop(&stored.flop).ok_or("flop")?,
        };
        if status_of(root, id).is_none() {
            crate::store::write_library(state, data, root, Source::Cloud, id, spot)?;
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
    /// Per catalog flop, for spots solved here: what became of sharing it, if known.
    shared: Vec<Option<Shared>>,
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
    power: LibraryPower,
    cores: usize,
    /// The user shares own solutions.
    share: bool,
    worker: WorkerStatus,
}

/// The catalog with what is stored and shared of it, the disk use and the worker's state.
#[tauri::command]
pub async fn library_overview(app: AppHandle) -> CmdResult<Overview> {
    blocking(app.clone(), move |state| {
        let root = root(&app).ok_or("Kein Datenordner")?;
        let c = catalog();
        let log = read_shared(&root);
        let scenarios = c
            .scenarios
            .iter()
            .enumerate()
            .map(|(scenario, s)| {
                let status: Vec<_> =
                    (0..c.flops.len()).map(|flop| status_of(&root, SpotId { scenario, flop })).collect();
                let shared = (0..c.flops.len())
                    .map(|flop| {
                        let id = SpotId { scenario, flop };
                        (status[flop] == Some(Source::Local)).then(|| log.get(&shared_key(id)).cloned()).flatten()
                    })
                    .collect();
                ScenarioView {
                    id: s.id.clone(),
                    pot_type: s.pot_type,
                    oop: s.oop.clone(),
                    ip: s.ip.clone(),
                    stack_bb: s.stack_bb,
                    pot: s.pot,
                    stack: s.stack,
                    status,
                    shared,
                }
            })
            .collect();
        let (library, cloud) = {
            let settings = state.settings.lock().unwrap();
            (settings.library, settings.cloud)
        };
        Ok(Overview {
            catalog: c.version,
            target_pct: c.target_pct,
            flops: c.flops.clone(),
            flop_weights: c.flop_weights.clone(),
            scenarios,
            disk_bytes: disk_bytes(&root),
            mode: library.background,
            power: library.power,
            cores: cores(),
            share: cloud.share,
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
    store_settings(&app, &state, settings)?;
    wake(&app);
    Ok(())
}

/// Takes effect with the next spot.
#[tauri::command]
pub async fn library_set_power(power: LibraryPower, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.library.power = power;
    store_settings(&app, &state, settings)
}

/// Starts solving now, also while the app is in use, until stopped, PokerStars starts or
/// no spot is left; `run: false` stops any solving until started again.
#[tauri::command]
pub async fn library_run(run: bool, app: AppHandle) -> CmdResult<()> {
    blocking(app.clone(), move |state| {
        if !run {
            HALTED.store(true, Ordering::Relaxed);
            end_manual(&app, Job::stopped);
            wake(&app);
            return Ok(());
        }
        unlocked(state).map_err(|_| "Solange PokerStars läuft, rechnet Ringside nicht.")?;
        HALTED.store(false, Ordering::Relaxed);
        let lib = app.state::<LibraryState>();
        if lib.manual.lock().unwrap().is_none() {
            let root = root(&app).ok_or("Kein Datenordner")?;
            let skip = lib.skip.lock().unwrap().clone();
            let total = all_spots().filter(|id| !skip.contains(id) && status_of(&root, *id).is_none()).count() as u64;
            let job = jobs::start(Kind::Library, "Bibliothek: Mitrechnen", Target::to("library", None));
            job.steps(0, total);
            job.detail("Startet …");
            *lib.manual.lock().unwrap() = Some(Manual { job, done: 0, total });
        }
        update(&app, |s| s.manual = true);
        wake(&app);
        Ok(())
    })
    .await
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
        let bytes = read_library(&root, source, id)?;
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
        let state = crate::solutions::test_state();
        assert!(import_pack(&state, &dir, &dir, b"not a pack").is_err());
        let empty = ringside_solver::library::format::pack(&[]);
        assert_eq!(import_pack(&state, &dir, &dir, &empty).unwrap(), 0);
        let id = SpotId { scenario: 0, flop: 3 };
        write_file(&spot_path(&dir, Source::Local, id), b"x").unwrap();
        assert_eq!(status_of(&dir, id), Some(Source::Local));
        assert_eq!(status_of(&dir, SpotId { scenario: 0, flop: 4 }), None);
        assert_eq!(disk_bytes(&dir), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_log_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("ringside-library-shared-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(read_shared(&dir).is_empty());
        let id = SpotId { scenario: 1, flop: 2 };
        let entry = Shared { state: ShareState::Rejected, at: 7, note: Some("too exploitable".into()) };
        write_shared(&dir, &SharedLog::from([(shared_key(id), entry.clone())])).unwrap();
        assert_eq!(read_shared(&dir).get(&shared_key(id)), Some(&entry));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cloud_answers_update_what_was_shared() {
        let sent = Shared { state: ShareState::Submitted, at: 100, note: None };
        for status in [KeyStatus::Accepted, KeyStatus::Verified] {
            let served = after_check(Some(&sent), status, 500).unwrap();
            assert_eq!((served.state, served.at), (ShareState::Served, 100), "keeps the upload time");
        }
        let rejected = after_check(Some(&sent), KeyStatus::Rejected(Some("why".into())), 500).unwrap();
        assert_eq!((rejected.state, rejected.note.as_deref()), (ShareState::Rejected, Some("why")));
        assert_eq!(after_check(Some(&sent), KeyStatus::Unknown, 500), None, "lost upload goes up again");
        assert_eq!(after_check(None, KeyStatus::Submitted, 500).map(|s| (s.state, s.at)), Some((ShareState::Submitted, 500)));
        assert_eq!(after_check(None, KeyStatus::Rejected(None), 500), None, "someone else's rejected copy");
    }

    #[test]
    fn power_sets_the_threads() {
        assert_eq!([LibraryPower::Gentle, LibraryPower::Normal, LibraryPower::Full].map(|p| threads_for(p, 16)), [4, 8, 15]);
        assert_eq!([LibraryPower::Gentle, LibraryPower::Normal, LibraryPower::Full].map(|p| threads_for(p, 1)), [1, 1, 1]);
    }

    #[test]
    fn spot_progress_follows_the_log_of_the_error() {
        assert_eq!(spot_share(20.0, 20.0, 0.5), 0.0);
        assert!((spot_share(20.0, 20.0_f64.sqrt() * 0.5_f64.sqrt(), 0.5) - 0.5).abs() < 0.01);
        assert_eq!(spot_share(20.0, 0.3, 0.5), 1.0);
    }

    #[test]
    fn labels_say_the_spot_in_words() {
        assert_eq!(spot_label("srp-btn-bb-40", "Ks7d2c"), "Du am Button gegen den Big Blind, Flop K♠ 7♦ 2♣");
        assert_eq!(spot_label("3bp-hj-co-75", "AhAd5s"), "Du auf HJ gegen den Cutoff, Flop A♥ A♦ 5♠");
    }

    /// The app's download path against the real CDN: signed index, file hash, every spot
    /// read against the catalog. Needs the network: `cargo test -p ringside -- --ignored`.
    #[test]
    #[ignore]
    fn downloads_the_signed_library_pack() {
        let dir = std::env::temp_dir().join(format!("ringside-library-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = crate::solutions::test_state();
        let client = Packs::new(packs::DEFAULT_BASE_URL);
        let index = client.index(0).expect("signed index");
        let mut imported = 0;
        for (name, versions) in index.packs.iter().filter(|(n, _)| n.starts_with(PACK_PREFIX)) {
            let version = newest_version(versions.keys()).expect("a version");
            for f in &versions[version].files {
                let bytes = client.file(name, version, f).expect("file matches the index");
                let n = import_pack(&state, &dir, &dir, &bytes).expect("spots read against the catalog");
                println!("{name} {version} {}: {n} spots, {} bytes", f.path, bytes.len());
                imported += n;
            }
        }
        assert!(imported > 0, "no library pack in the index");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
