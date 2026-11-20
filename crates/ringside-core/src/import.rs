//! Incremental import of hand history files the client keeps appending to.

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use crate::db::Db;
use crate::model::Hand;
use crate::parser::{ParseError, complete_prefix_len, parse_hand, split_hands};
use crate::tournament::{is_summary, parse_summaries};

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    Db(rusqlite::Error),
    Zip(zip::result::ZipError),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Io(e) => write!(f, "{e}"),
            ImportError::Db(e) => write!(f, "{e}"),
            ImportError::Zip(e) => write!(f, "ZIP-Archiv unlesbar: {e}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<zip::result::ZipError> for ImportError {
    fn from(e: zip::result::ZipError) -> Self {
        ImportError::Zip(e)
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
    /// The first [`KEEP_MALFORMED`] of them: (hand text, reason), for a bug report.
    pub malformed_hands: Vec<(String, String)>,
}

pub const KEEP_MALFORMED: usize = 20;

/// `HandHistory` folders below `PokerStars*` (.com, .EU, .DE, ...) in the folder the client
/// keeps its data in.
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

/// `TournSummary` folders the client keeps next to the given `HandHistory` folders. A configured
/// folder may also be a player folder inside `HandHistory`.
pub fn summary_dirs(hand_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in hand_dirs {
        let base = dir
            .ancestors()
            .find(|a| a.file_name().is_some_and(|n| n.eq_ignore_ascii_case("HandHistory")))
            .unwrap_or(dir);
        let candidate = base.with_file_name("TournSummary");
        if candidate.is_dir() && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

fn is_history_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt") || e.eq_ignore_ascii_case("zip"))
}

fn is_zip(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
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
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
    String::from_utf16_lossy(&units)
}

/// Summary files are UTF-8 (with or without BOM), UTF-16 or, from older clients, Windows-1252.
fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| if b == 0x80 { '€' } else { b as char }).collect(),
    }
}

/// Imports a tournament summary file if it changed since the last import. Returns the number
/// of tournaments in it. The file is replaced while the tournament runs, so it is read whole
/// and stored over the previous version.
pub fn import_summary(db: &mut Db, path: &Path) -> Result<usize, ImportError> {
    let key = path.to_string_lossy().to_string();
    let bytes = fs::read(path)?;
    // Rewrites can keep the length, so the files table stores a content hash for summaries.
    let stamp = content_stamp(&bytes);
    if db.file_offset(&key)? == stamp {
        return Ok(0);
    }
    let summaries = parse_summaries(&decode_text(&bytes));
    for s in &summaries {
        db.upsert_summary(s)?;
    }
    db.set_file_offset(&key, stamp)?;
    Ok(summaries.len())
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

    let (result, _) = import_hands(db, &text)?;
    if consumed_bytes > offset {
        db.set_file_offset(&key, consumed_bytes)?;
    }
    Ok(result)
}

/// Parses and stores every hand in `text`. Also returns how many were already stored.
fn import_hands(db: &mut Db, text: &str) -> Result<(FileImport, usize), ImportError> {
    let mut result = FileImport::default();
    let mut parsed = Vec::new();
    for chunk in split_hands(text) {
        match parse_hand(chunk) {
            Ok(hand) => parsed.push((hand, chunk.to_string())),
            Err(ParseError::Unsupported(_)) => {}
            Err(ParseError::Malformed(reason)) => {
                result.malformed += 1;
                if result.malformed_hands.len() < KEEP_MALFORMED {
                    result.malformed_hands.push((chunk.to_string(), reason));
                }
            }
        }
    }
    let inserted = db.insert_hands(&parsed)?;
    result.new_hands = inserted.into_iter().cloned().collect();
    let duplicates = parsed.len() - result.new_hands.len();
    Ok((result, duplicates))
}

/// What importing files by hand (a PokerCraft download, a ZIP, files dropped on the window) brought.
#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub files: usize,
    pub new_hands: usize,
    /// Hands that were already stored, from an earlier import or a watched folder.
    pub duplicates: usize,
    pub malformed: usize,
    pub tournaments: usize,
    /// Files that held neither hands nor tournament summaries.
    pub skipped: usize,
    #[serde(skip)]
    pub hands: Vec<Hand>,
    #[serde(skip)]
    pub malformed_hands: Vec<(String, String)>,
}

impl ImportReport {
    fn add_text(&mut self, db: &mut Db, text: &str) -> Result<(), ImportError> {
        self.files += 1;
        if is_summary(text) {
            let summaries = parse_summaries(text);
            for s in &summaries {
                db.upsert_summary(s)?;
            }
            self.tournaments += summaries.len();
            return Ok(());
        }
        let (r, duplicates) = import_hands(db, text)?;
        if r.new_hands.is_empty() && duplicates == 0 && r.malformed == 0 {
            self.skipped += 1;
        }
        self.new_hands += r.new_hands.len();
        self.duplicates += duplicates;
        self.malformed += r.malformed;
        let room = KEEP_MALFORMED.saturating_sub(self.malformed_hands.len());
        self.malformed_hands.extend(r.malformed_hands.into_iter().take(room));
        self.hands.extend(r.new_hands);
        Ok(())
    }

    fn add_zip(&mut self, db: &mut Db, bytes: &[u8]) -> Result<(), ImportError> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.enclosed_name().unwrap_or_default();
            if !entry.is_file() {
                continue;
            }
            let mut data = Vec::new();
            // A download holds text files; anything bigger than this is not one.
            (&mut entry).take(MAX_ENTRY + 1).read_to_end(&mut data)?;
            if data.len() as u64 > MAX_ENTRY {
                self.skipped += 1;
            } else if is_zip(&name) {
                self.add_zip(db, &data)?;
            } else if is_history_file(&name) {
                self.add_text(db, &decode_text(&data))?;
            } else {
                self.skipped += 1;
            }
        }
        Ok(())
    }
}

const MAX_ENTRY: u64 = 512 * 1024 * 1024;

/// Imports hand histories and tournament summaries from files, ZIP archives or folders, all of
/// them read whole. Hands stored before are counted as duplicates, never stored twice.
pub fn import_paths(db: &mut Db, paths: &[PathBuf]) -> Result<ImportReport, ImportError> {
    let mut report = ImportReport::default();
    let mut files = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut found = Vec::new();
            walk(path, &mut found);
            found.sort();
            files.extend(found.into_iter().map(|(p, _, _)| p));
        } else {
            files.push(path.clone());
        }
    }
    for path in files {
        let bytes = fs::read(&path)?;
        if is_zip(&path) {
            report.add_zip(db, &bytes)?;
        } else if is_history_file(&path) {
            report.add_text(db, &decode_text(&bytes))?;
        } else {
            report.skipped += 1;
        }
    }
    Ok(report)
}

/// Imports a file from a watched folder for GGPoker downloads once it changed. GGPoker writes the
/// newest hand first, so files are read whole; stored hands are skipped by their id.
pub fn import_watched(db: &mut Db, path: &Path) -> Result<ImportReport, ImportError> {
    let key = path.to_string_lossy().to_string();
    let bytes = fs::read(path)?;
    let mut report = ImportReport::default();
    let stamp = content_stamp(&bytes);
    if db.file_offset(&key)? == stamp {
        return Ok(report);
    }
    if is_zip(path) {
        report.add_zip(db, &bytes)?;
    } else {
        report.add_text(db, &decode_text(&bytes))?;
    }
    db.set_file_offset(&key, stamp)?;
    Ok(report)
}

/// FNV-1a over the content, kept positive for SQLite.
fn content_stamp(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3)) >> 1
}
