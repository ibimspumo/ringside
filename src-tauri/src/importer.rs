//! Background thread that keeps the database in sync with the hand history folders.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use ringside_core::db::RebuildProgress;
use ringside_core::import::{Scanner, default_dirs, import_file};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

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
        let mut last_dirs = Vec::new();
        let mut first = true;
        loop {
            let state = app.state::<AppState>();
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
            let refresh = first || !changed.is_empty();
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
