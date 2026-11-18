//! The display words of the glossary (`src/lib/ui/words.ts`) that texts built in Rust need.
//! The glossary is the one source; a test keeps this list equal to it.

use crate::stats::StatId;

/// Glossary id and display word.
pub const WORDS: &[(&str, &str)] = &[
    ("vpip", "VPIP"),
    ("pfr", "Preflop-Raise"),
    ("three_bet", "3-Bet"),
    ("fold_to_three_bet", "Fold auf 3-Bet"),
    ("steal", "Steal"),
    ("fold_to_steal", "Fold auf Steal"),
    ("cbet", "C-Bet"),
    ("fold_to_cbet", "Fold auf C-Bet"),
    ("wtsd", "Showdown"),
    ("wsd", "Gewonnen am Showdown"),
    ("af", "Aggression"),
    ("afq", "Aggressions-Frequenz"),
    ("leaks", "Schwachstellen"),
];

/// The glossary id of a stat, the same as its serde name.
pub fn stat_id(s: StatId) -> &'static str {
    match s {
        StatId::Hands => "hands",
        StatId::Vpip => "vpip",
        StatId::Pfr => "pfr",
        StatId::ThreeBet => "three_bet",
        StatId::FoldToThreeBet => "fold_to_three_bet",
        StatId::Steal => "steal",
        StatId::FoldToSteal => "fold_to_steal",
        StatId::Cbet => "cbet",
        StatId::FoldToCbet => "fold_to_cbet",
        StatId::Wtsd => "wtsd",
        StatId::Wsd => "wsd",
        StatId::Af => "af",
        StatId::Afq => "afq",
    }
}

/// The display word of a glossary id; the id itself for a word the list lacks.
pub fn word(id: &str) -> &str {
    WORDS.iter().find(|(k, _)| *k == id).map_or(id, |(_, w)| w)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every id and word here must stand the same way in the glossary of the UI.
    #[test]
    fn words_match_the_glossary_of_the_ui() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/lib/ui/words.ts");
        let ts = std::fs::read_to_string(path).expect("glossary of the UI");
        let entry = regex::Regex::new(r#"(?m)^  (\w+): \{\s*word: "([^"]+)""#).unwrap();
        let glossary: std::collections::HashMap<&str, &str> =
            entry.captures_iter(&ts).map(|c| (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str())).collect();
        assert!(glossary.len() > WORDS.len(), "the pattern no longer reads words.ts");
        for (id, w) in WORDS {
            assert_eq!(glossary.get(id), Some(w), "„{id}“ differs between words.rs and words.ts");
        }
    }

    #[test]
    fn every_checked_stat_has_a_word() {
        for s in StatId::ALL.into_iter().filter(|&s| s != StatId::Hands) {
            assert_ne!(word(stat_id(s)), stat_id(s), "{s:?}");
        }
    }
}
