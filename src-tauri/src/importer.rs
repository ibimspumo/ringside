//! Background thread that keeps the database in sync with the hand history folders.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use ringside_core::db::RebuildProgress;
use ringside_core::import::{Scanner, default_dirs, import_file, import_summary, summary_dirs};
use ringside_core::notes::{load_all, notes_files};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

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
    pub files_read: usize,
    /// Hands added since the app started.
    pub hands_imported: usize,
    /// All hands in the database.
    pub total_hands: i64,
    pub malformed: usize,
    pub last_error: Option<String>,
    /// Hands recomputed so far while the database is upgraded after an update.
    pub upgrade: Option<RebuildProgress>,
}

pub fn effective_dirs(configured: &[String]) -> Vec<PathBuf> {
    if !configured.is_empty() {
        return configured.iter().map(PathBuf::from).collect();
    }
    std::env::var_os("LOCALAPPDATA").map(|d| default_dirs(&PathBuf::from(d))).unwrap_or_default()
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut scanner = Scanner::default();
        let mut summaries = Scanner::default();
        let mut last_dirs = Vec::new();
        let mut first = true;
        let mut notes_stamp = None;
        let mut notes_checked: Option<Instant> = None;
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
                let pause = match step {
                    Ok(progress) => {
                        status.upgrade = Some(progress);
                        if progress.done >= progress.total {
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
            let dirs = effective_dirs(&state.settings.lock().unwrap().hand_history_dirs);
            if dirs != last_dirs {
                scanner = Scanner::default();
                summaries = Scanner::default();
                last_dirs = dirs.clone();
            }

            let changed = scanner.changed(&dirs);
            let mut tables = BTreeSet::new();
            let mut status = state.import.lock().unwrap().clone();
            status.dirs = dirs.iter().map(|d| d.to_string_lossy().to_string()).collect();
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
                        status.malformed += r.malformed;
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
            let refresh = first || !changed.is_empty() || tournaments > 0;
            if refresh && let Ok(total) = state.db.lock().unwrap().hand_count() {
                status.total_hands = total;
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
