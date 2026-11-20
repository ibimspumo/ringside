//! Background thread that keeps the database in sync with the hand history folders.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ringside_core::db::RebuildProgress;
use ringside_core::import::{ImportReport, import_paths};
use ringside_core::import::{Scanner, default_dirs, import_file, import_summary, import_watched, summary_dirs};
use ringside_core::notes::{load_all, notes_files};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::jobs::{self, Kind, Target};
use crate::{AppState, NotesState};

/// How often the client's notes files are checked for changes.
const NOTES_EVERY: Duration = Duration::from_secs(5);

type NotesStamp = Vec<(PathBuf, Option<SystemTime>, u64)>;

/// Reloads the notes when a notes file appeared, vanished or was rewritten. Returns true when
/// they were reloaded.
fn refresh_notes(state: &AppState, last: &mut Option<NotesStamp>) -> bool {
    let files =
        std::env::var_os("LOCALAPPDATA").map(|d| notes_files(&PathBuf::from(d))).unwrap_or_default();
    let stamp: NotesStamp = files
        .iter()
        .map(|f| {
            let meta = std::fs::metadata(f).ok();
            (f.clone(), meta.as_ref().and_then(|m| m.modified().ok()), meta.map_or(0, |m| m.len()))
        })
        .collect();
    if last.as_ref() == Some(&stamp) {
        return false;
    }
    *last = Some(stamp);
    let (notes, sources) = load_all(&files);
    *state.notes.lock().unwrap() = NotesState { notes, sources };
    true
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportStatus {
    pub dirs: Vec<String>,
    /// Watched folders for GGPoker downloads.
    pub gg_dirs: Vec<String>,
    /// Stored hands per client ("pokerstars", "ggpoker"), so the client filter only shows with both.
    pub sites: std::collections::BTreeMap<String, i64>,
    pub files_read: usize,
    /// Hands added since the app started.
    pub hands_imported: usize,
    /// The lowest id of the hands added since the automatic analysis last looked.
    #[serde(skip)]
    pub first_new: Option<u64>,
    /// All hands in the database.
    pub total_hands: i64,
    pub malformed: usize,
    pub last_error: Option<String>,
    /// Hands recomputed so far while the database is upgraded after an update.
    pub upgrade: Option<RebuildProgress>,
}

fn note_new(status: &mut ImportStatus, ids: impl Iterator<Item = u64>) {
    if let Some(low) = ids.min() {
        status.first_new = Some(status.first_new.map_or(low, |f| f.min(low)));
    }
}

fn refresh_counts(state: &AppState, status: &mut ImportStatus) {
    let db = state.db.lock().unwrap();
    if let Ok(total) = db.hand_count() {
        status.total_hands = total;
    }
    if let Ok(sites) = db.hands_per_site() {
        status.sites = sites.into_iter().map(|(s, n)| (s.as_str().to_string(), n)).collect();
    }
}

/// Imports files picked or dropped by the user and tells the app about it like a folder scan.
pub fn import_files(app: &AppHandle, paths: &[PathBuf]) -> Result<ImportReport, String> {
    let state = app.state::<AppState>();
    let report = {
        let mut db = state.db.lock().unwrap();
        import_paths(&mut db, paths).map_err(|e| e.to_string())?
    };
    let mut status = state.import.lock().unwrap().clone();
    status.files_read += report.files;
    status.hands_imported += report.new_hands;
    note_new(&mut status, report.hands.iter().map(|h| h.id));
    status.malformed += report.malformed;
    refresh_counts(&state, &mut status);
    *state.import.lock().unwrap() = status.clone();
    let _ = app.emit("import-status", &status);
    Ok(report)
}

pub fn effective_dirs(configured: &[String]) -> Vec<PathBuf> {
    if !configured.is_empty() {
        return configured.iter().map(PathBuf::from).collect();
    }
    // Windows keeps the client data in %LOCALAPPDATA%, macOS in ~/Library/Application Support.
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    };
    base.map(|d| default_dirs(&d)).unwrap_or_default()
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut scanner = Scanner::default();
        let mut summaries = Scanner::default();
        let mut gg = Scanner::default();
        let mut last_dirs = Vec::new();
        let mut last_gg: Vec<PathBuf> = Vec::new();
        let mut first = true;
        let mut notes_stamp = None;
        let mut notes_checked: Option<Instant> = None;
        let mut rebuild: Option<jobs::Job> = None;
        loop {
            let state = app.state::<AppState>();
            if notes_checked.is_none_or(|t| t.elapsed() >= NOTES_EVERY) {
                notes_checked = Some(Instant::now());
                if refresh_notes(&state, &mut notes_stamp) {
                    let _ = app.emit("notes-changed", ());
                }
            }
            // Finish a database upgrade first, in small steps so overlays and the app stay usable.
            let step = {
                let mut db = state.db.lock().unwrap();
                match db.needs_rebuild() {
                    Ok(true) => Some(db.rebuild_step(2000)),
                    _ => None,
                }
            };
            if let Some(step) = step {
                let mut status = state.import.lock().unwrap();
                let task = rebuild.get_or_insert_with(|| {
                    let task = jobs::start(Kind::Rebuild, "Datenbank nach dem Update auffrischen", Target::to("dashboard", None));
                    task.not_cancellable();
                    task
                });
                let pause = match step {
                    Ok(progress) => {
                        status.upgrade = Some(progress);
                        task.steps(progress.done as u64, progress.total as u64);
                        task.detail(format!("{} von {} Händen neu berechnet", progress.done, progress.total));
                        if progress.done >= progress.total {
                            if let Some(task) = rebuild.take() {
                                task.finish();
                            }
                            // Totals are complete now; open overlays reload their numbers and
                            // the next pass reports the normal import state again.
                            let _ = app.emit("stats-rebuilt", ());
                            first = true;
                        }
                        20
                    }
                    Err(e) => {
                        status.last_error = Some(format!("Datenbank-Upgrade: {e}"));
                        5000
                    }
                };
                let _ = app.emit("import-status", &*status);
                drop(status);
                std::thread::sleep(Duration::from_millis(pause));
                continue;
            }
            let (dirs, gg_dirs) = {
                let settings = state.settings.lock().unwrap();
                (effective_dirs(&settings.hand_history_dirs), settings.gg_dirs.iter().map(PathBuf::from).collect::<Vec<_>>())
            };
            if dirs != last_dirs {
                scanner = Scanner::default();
                summaries = Scanner::default();
                last_dirs = dirs.clone();
            }
            if gg_dirs != last_gg {
                gg = Scanner::default();
                last_gg = gg_dirs.clone();
            }

            let changed = scanner.changed(&dirs);
            let mut tables = BTreeSet::new();
            let mut status = state.import.lock().unwrap().clone();
            status.dirs = dirs.iter().map(|d| d.to_string_lossy().to_string()).collect();
            status.gg_dirs = gg_dirs.iter().map(|d| d.to_string_lossy().to_string()).collect();
            status.upgrade = None;
            status.files_read += changed.len();
            for path in &changed {
                let result = {
                    let mut db = state.db.lock().unwrap();
                    import_file(&mut db, path)
                };
                match result {
                    Ok(r) => {
                        status.hands_imported += r.new_hands.len();
                        note_new(&mut status, r.new_hands.iter().map(|h| h.id));
                        status.malformed += r.malformed;
                        app.state::<crate::cloud::CloudState>().add_malformed(r.malformed_hands);
                        tables.extend(r.new_hands.iter().map(|h| h.table_key()));
                    }
                    Err(e) => status.last_error = Some(format!("{}: {e}", path.display())),
                }
            }
            let mut tournaments = 0;
            for path in summaries.changed(&summary_dirs(&dirs)) {
                let result = {
                    let mut db = state.db.lock().unwrap();
                    import_summary(&mut db, &path)
                };
                match result {
                    Ok(n) => tournaments += n,
                    Err(e) => status.last_error = Some(format!("{}: {e}", path.display())),
                }
            }
            let gg_changed = gg.changed(&gg_dirs);
            status.files_read += gg_changed.len();
            for path in &gg_changed {
                let result = {
                    let mut db = state.db.lock().unwrap();
                    import_watched(&mut db, path)
                };
                match result {
                    Ok(r) => {
                        status.hands_imported += r.new_hands;
                        note_new(&mut status, r.hands.iter().map(|h| h.id));
                        status.malformed += r.malformed;
                        tournaments += r.tournaments;
                        app.state::<crate::cloud::CloudState>().add_malformed(r.malformed_hands);
                    }
                    Err(e) => status.last_error = Some(format!("{}: {e}", path.display())),
                }
            }
            let refresh = first || !changed.is_empty() || !gg_changed.is_empty() || tournaments > 0;
            if refresh {
                refresh_counts(&state, &mut status);
            }
            *state.import.lock().unwrap() = status.clone();
            if refresh {
                first = false;
                let _ = app.emit("import-status", &status);
            }
            for key in tables {
                let _ = app.emit("table-updated", key);
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
    });
}
