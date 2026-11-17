//! The fixed list of learning concepts a coached spot can be tagged with. Each concept
//! points at an exercise kind of `ringside-learn` (where there is one) and at articles
//! of the knowledge area, so a weakness found by the coach leads straight to practice.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Concept {
    pub id: &'static str,
    pub name: &'static str,
    /// `ringside_learn::Kind` id of the matching exercise.
    pub exercise: Option<&'static str>,
    /// Article ids in the knowledge area.
    pub articles: &'static [&'static str],
}

const fn c(
    id: &'static str,
    name: &'static str,
    exercise: Option<&'static str>,
    articles: &'static [&'static str],
) -> Concept {
    Concept { id, name, exercise, articles }
}

pub const CONCEPTS: &[Concept] = &[
    c("push_fold_sb", "Push/Fold im Small Blind", Some("push_fold_sb"), &["push-fold", "effektiver-stack"]),
    c("push_fold_call", "Callen gegen einen Shove", Some("push_fold_call"), &["push-fold"]),
    c("short_stack", "Kurzer Stack außerhalb der Blinds", Some("push_fold_open"), &["push-fold", "m-ratio", "fold-equity"]),
    c("pot_odds", "Pot Odds", Some("pot_odds"), &["pot-odds", "mdf"]),
    c("equity", "Equity gegen Ranges", Some("equity_range"), &["equity", "range"]),
    c("outs", "Outs und Draws", Some("outs"), &["outs", "regel-2-4", "implizite-odds"]),
    c("position", "Position", Some("position"), &["position", "equity-realisierung"]),
    c("preflop_range", "Starthände und Ranges", Some("combos"), &["range", "kombinatorik", "three-bet"]),
    c("fold_equity", "Fold Equity und Semi-Bluffs", None, &["fold-equity", "semi-bluff"]),
    c("value_bet", "Value Bets und Bluffs", None, &["value-bluff", "alpha"]),
    c("cbet", "Continuation Bet", None, &["c-bet-warum", "cbet"]),
    c("stack_depth", "Stacktiefe und SPR", None, &["effektiver-stack", "spr"]),
    c("icm", "ICM und Bubble", None, &["icm", "bubble-factor"]),
    c("bounty", "Bounty-Turniere", None, &["bounty-pot-odds"]),
    c("all_in_ev", "All-in-EV und Varianz", Some("all_in"), &["ev", "entscheidung-ergebnis"]),
];

pub fn concept(id: &str) -> Option<&'static Concept> {
    CONCEPTS.iter().find(|c| c.id == id)
}

/// Accepts the id or the German name, ignoring case and surrounding space.
pub fn resolve(text: &str) -> Option<&'static Concept> {
    let t = text.trim();
    CONCEPTS.iter().find(|c| c.id.eq_ignore_ascii_case(t) || c.name.to_lowercase() == t.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exercises_exist_in_the_learning_crate() {
        for c in CONCEPTS {
            if let Some(kind) = c.exercise {
                assert!(ringside_learn::Kind::parse(kind).is_some(), "{} → {kind}", c.id);
            }
        }
    }

    #[test]
    fn articles_exist_in_the_knowledge_area() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/lib/learn/wissen/articles.ts");
        let Ok(text) = std::fs::read_to_string(path) else { return };
        for c in CONCEPTS {
            for a in c.articles {
                assert!(text.contains(&format!("id: \"{a}\"")), "Artikel {a} von {} fehlt", c.id);
            }
        }
    }

    #[test]
    fn the_app_shows_the_same_names() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/lib/coach/concepts.ts");
        let text = std::fs::read_to_string(path).unwrap();
        for c in CONCEPTS {
            assert!(text.contains(&format!("  {}: \"{}\",", c.id, c.name)), "{} fehlt in concepts.ts", c.id);
        }
        let entries = text.lines().filter(|l| l.trim_end().ends_with("\",")).count();
        assert_eq!(entries, CONCEPTS.len(), "concepts.ts hat Einträge, die es nicht gibt");
    }

    #[test]
    fn ids_are_unique_and_resolvable() {
        for (i, c) in CONCEPTS.iter().enumerate() {
            assert!(CONCEPTS[..i].iter().all(|o| o.id != c.id));
            assert_eq!(resolve(c.name), Some(c));
            assert_eq!(resolve(&c.id.to_uppercase()), Some(c));
        }
    }
}
