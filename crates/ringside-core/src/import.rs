//! Incremental import of hand history files the client keeps appending to.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::db::Db;
use crate::model::Hand;
use crate::parser::{ParseError, complete_prefix_len, parse_hand, split_hands};

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Db(rusqlite::Error),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "{e}"),
            ImportError::Db(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<rusqlite::Error> for ImportError {
    fn from(e: rusqlite::Error) -> Self {
        ImportError::Db(e)
    }
}

#[derive(Debug, Default)]
pub struct FileImport {
    pub new_hands: Vec<Hand>,
    pub malformed: usize,
}

/// `HandHistory` folders below `%LOCALAPPDATA%\PokerStars*` (.com, .EU, .DE, ...).
pub fn default_dirs(local_app_data: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(local_app_data) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("PokerStars"))
        .map(|e| e.path().join("HandHistory"))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn is_history_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt"))
}

fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64, Option<SystemTime>)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let path = entry.path();
        if meta.is_dir() {
            walk(&path, out);
        } else if is_history_file(&path) {
            out.push((path, meta.len(), meta.modified().ok()));
        }
    }
}

/// Remembers file sizes between polls so only grown files are read again.
#[derive(Default)]
pub struct Scanner {
    seen: HashMap<PathBuf, (u64, Option<SystemTime>)>,
}

impl Scanner {
    /// Files that are new or changed since the last call.
    pub fn changed(&mut self, dirs: &[PathBuf]) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for dir in dirs {
            walk(dir, &mut found);
        }
        let mut changed = Vec::new();
        for (path, len, modified) in found {
            let state = (len, modified);
            if self.seen.get(&path) != Some(&state) {
                self.seen.insert(path.clone(), state);
                changed.push(path);
            }
        }
        changed.sort();
        changed
    }
}

fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

/// Imports the complete hands appended to `path` since the last import.
pub fn import_file(db: &mut Db, path: &Path) -> Result<FileImport, ImportError> {
    let key = path.to_string_lossy().to_string();
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut offset = db.file_offset(&key)?;
    if offset > len {
        offset = 0;
    }

    let mut head = [0u8; 2];
    let utf16 = file.read(&mut head)? == 2 && head == [0xFF, 0xFE];

    let (text, consumed_bytes) = if utf16 {
        // Rare legacy encoding: re-read the whole file and let the primary key dedupe.
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(2))?;
        file.read_to_end(&mut bytes)?;
        let text = decode_utf16(&bytes);
        let complete = complete_prefix_len(&text);
        let consumed = if complete == text.len() { len } else { 0 };
        (text[..complete].to_string(), consumed)
    } else {
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(offset))?;
        file.read_to_end(&mut bytes)?;
        let valid = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(e) => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or_default(),
        };
        let complete = complete_prefix_len(valid);
        (valid[..complete].to_string(), offset + complete as u64)
    };

    let mut result = FileImport::default();
    let mut parsed = Vec::new();
    for chunk in split_hands(&text) {
        match parse_hand(chunk) {
            Ok(hand) => parsed.push((hand, chunk.to_string())),
            Err(ParseError::Unsupported(_)) => {}
            Err(ParseError::Malformed(_)) => result.malformed += 1,
        }
    }
    let inserted = db.insert_hands(&parsed)?;
    result.new_hands = inserted.into_iter().cloned().collect();
    if consumed_bytes > offset {
        db.set_file_offset(&key, consumed_bytes)?;
    }
    Ok(result)
}
