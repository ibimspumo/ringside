//! Background thread that keeps the database in sync with the hand history folders.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use ringside_core::import::{Scanner, default_dirs, import_file};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportStatus {
    pub dirs: Vec<String>,
    pub files_read: usize,
    pub hands_imported: usize,
    pub malformed: usize,
    pub last_error: Option<String>,
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
        loop {
            let state = app.state::<AppState>();
            let dirs = effective_dirs(&state.settings.lock().unwrap().hand_history_dirs);
            if dirs != last_dirs {
                scanner = Scanner::default();
                last_dirs = dirs.clone();
            }

            let changed = scanner.changed(&dirs);
            let mut tables = BTreeSet::new();
            let mut status = state.import.lock().unwrap().clone();
            status.dirs = dirs.iter().map(|d| d.to_string_lossy().to_string()).collect();
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
            *state.import.lock().unwrap() = status.clone();
            if !changed.is_empty() {
                let _ = app.emit("import-status", &status);
            }
            for key in tables {
                let _ = app.emit("table-updated", key);
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
    });
}
