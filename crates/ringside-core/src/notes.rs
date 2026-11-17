//! The player notes and colour labels the user keeps in the PokerStars client
//! (`notes.<name>.xml` in the client's data folder). Read only: the client rewrites the file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Label {
    pub name: String,
    /// "#RRGGBB"
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerNote {
    pub label: Option<Label>,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Notes {
    players: HashMap<String, PlayerNote>,
}

impl Notes {
    pub fn get(&self, player: &str) -> Option<&PlayerNote> {
        self.players.get(player)
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    /// Merges notes from several files (e.g. .com and .EU clients); later files win.
    pub fn extend(&mut self, other: Notes) {
        self.players.extend(other.players);
    }
}

pub fn parse_notes(xml: &str) -> Result<Notes, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let mut labels: HashMap<i64, Label> = HashMap::new();
    for (position, node) in doc.descendants().filter(|n| n.has_tag_name("label")).enumerate() {
        let index = node.attribute("index").and_then(|i| i.parse().ok()).unwrap_or(position as i64);
        let Some(color) = node.attribute("color").filter(|c| c.len() == 6 && c.chars().all(|ch| ch.is_ascii_hexdigit()))
        else {
            continue;
        };
        let name = node.text().unwrap_or_default().trim().to_string();
        labels.insert(index, Label { name, color: format!("#{}", color.to_ascii_uppercase()) });
    }
    let mut players = HashMap::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("note")) {
        let Some(player) = node.attribute("player") else { continue };
        // -1 or a missing attribute means "no label".
        let label = node.attribute("label").and_then(|l| l.parse::<i64>().ok()).and_then(|l| labels.get(&l).cloned());
        let text = node.text().unwrap_or_default().trim().to_string();
        if label.is_some() || !text.is_empty() {
            players.insert(player.to_string(), PlayerNote { label, text });
        }
    }
    Ok(Notes { players })
}

/// One notes file as found on disk, for the settings page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NotesSource {
    pub path: String,
    /// Players with a label or a note in this file.
    pub players: usize,
    pub error: Option<String>,
}

/// Reads and merges `files` in order; a broken or unreadable file is reported and skipped.
pub fn load_all(files: &[PathBuf]) -> (Notes, Vec<NotesSource>) {
    let mut notes = Notes::default();
    let mut sources = Vec::new();
    for path in files {
        let parsed = fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| parse_notes(&String::from_utf8_lossy(&bytes)));
        let path = path.to_string_lossy().to_string();
        match parsed {
            Ok(n) => {
                sources.push(NotesSource { path, players: n.len(), error: None });
                notes.extend(n);
            }
            Err(e) => sources.push(NotesSource { path, players: 0, error: Some(e) }),
        }
    }
    (notes, sources)
}

/// `notes.*.xml` files in the client data folders below `%LOCALAPPDATA%` ("PokerStars*").
pub fn notes_files(local_app_data: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(local_app_data) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("PokerStars"))
        .filter_map(|e| fs::read_dir(e.path()).ok())
        .flat_map(|dir| dir.flatten().map(|f| f.path()))
        .filter(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            name.starts_with("notes.") && name.ends_with(".xml")
        })
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<notes version="1">
  <labels>
    <label index="0" color="30DBFF">Fisch</label>
    <label index="1" color="ff304e">Reg &amp; aggressiv</label>
  </labels>
  <note player="fraquinho10" label="1" update="1397960262">3-bettet light aus den Blinds</note>
  <note player="Lee, Ann" label="0" update="1269280282" />
  <note player="nur text" label="-1" update="1269280282">callt alles</note>
  <note player="leer" label="-1" update="1269280282" />
</notes>"#;

    #[test]
    fn labels_colours_and_text() {
        let notes = parse_notes(SAMPLE).unwrap();
        let reg = notes.get("fraquinho10").unwrap();
        assert_eq!(reg.label, Some(Label { name: "Reg & aggressiv".into(), color: "#FF304E".into() }));
        assert_eq!(reg.text, "3-bettet light aus den Blinds");
        assert_eq!(notes.get("Lee, Ann").unwrap().label.as_ref().unwrap().color, "#30DBFF");
        assert_eq!(notes.get("nur text").unwrap().label, None);
        assert!(notes.get("leer").is_none(), "no label and no text: nothing to show");
        assert_eq!(notes.len(), 3);
    }

    #[test]
    fn files_below_local_app_data_are_found_and_merged() {
        let dir = tempfile::tempdir().unwrap();
        let eu = dir.path().join("PokerStars.EU");
        let com = dir.path().join("PokerStars");
        fs::create_dir_all(&eu).unwrap();
        fs::create_dir_all(&com).unwrap();
        fs::create_dir_all(dir.path().join("Other")).unwrap();
        fs::write(eu.join("notes.Hero.xml"), SAMPLE).unwrap();
        fs::write(com.join("notes.hero.xml"), SAMPLE.replace("3-bettet", "4-bettet")).unwrap();
        fs::write(com.join("notes.broken.xml"), "<notes><note").unwrap();
        fs::write(com.join("user.ini"), "").unwrap();
        fs::write(dir.path().join("Other").join("notes.x.xml"), SAMPLE).unwrap();

        let files = notes_files(dir.path());
        assert_eq!(files.len(), 3);
        let (notes, sources) = load_all(&files);
        assert_eq!(sources.iter().map(|s| (s.players, s.error.is_some())).collect::<Vec<_>>(), [(0, true), (3, false), (3, false)]);
        assert_eq!(notes.len(), 3);
        assert_eq!(notes.get("fraquinho10").unwrap().text, "3-bettet light aus den Blinds", "later file wins");
    }

    #[test]
    fn broken_file_is_an_error_not_a_panic() {
        assert!(parse_notes("<notes><note player=").is_err());
    }
}
