//! How much of a hand history the analysis rates, by the kind of situation and by
//! source. The situation comes from the hand alone, so runs of different versions can
//! be compared line by line (`analysis-coverage`).

use std::collections::BTreeMap;
use std::fmt;

use ringside_core::db::{Decision, Source, Status};
use ringside_core::model::{ActionKind, Hand, Street};

use crate::grade::check;

/// The situation of a decision, first match wins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Situation {
    /// Not No-Limit Hold'em.
    OtherGame,
    /// Two to five players dealt in.
    ShortTable,
    /// Someone limped before the flop (a call of the big blind before any raise), the
    /// small blind's complete included.
    Limp,
    /// After the flop with three or more players.
    Multiway,
    Other,
}

impl Situation {
    pub fn name(self) -> &'static str {
        match self {
            Situation::OtherGame => "anderes Spiel",
            Situation::ShortTable => "Tisch mit 2–5 Spielern",
            Situation::Limp => "Limp vor dem Flop",
            Situation::Multiway => "Multiway nach dem Flop",
            Situation::Other => "übrige",
        }
    }
}

pub fn situation(hand: &Hand, d: &Decision) -> Situation {
    if !hand.variant.contains("Hold'em No Limit") {
        return Situation::OtherGame;
    }
    let dealt = hand.seats.iter().filter(|s| !s.sitting_out || hand.actions.iter().any(|a| a.player == s.name)).count();
    if dealt <= 5 {
        return Situation::ShortTable;
    }
    let pre = || hand.actions.iter().filter(|a| a.street == Street::Preflop && !a.kind.is_post());
    let first_raise = pre().position(|a| a.kind.is_aggressive() || a.all_in).unwrap_or(usize::MAX);
    if pre().take(first_raise).any(|a| a.kind == ActionKind::Call) {
        return Situation::Limp;
    }
    if d.street != Street::Preflop {
        let at_flop: std::collections::HashSet<&str> =
            hand.actions.iter().filter(|a| a.street != Street::Preflop).map(|a| a.player.as_str()).collect();
        if at_flop.len() >= 3 {
            return Situation::Multiway;
        }
    }
    Situation::Other
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub decisions: usize,
    pub rated: usize,
    pub pending: usize,
    pub forced: usize,
    pub unrated: usize,
    pub by_source: BTreeMap<&'static str, usize>,
}

#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub rows: BTreeMap<Situation, Row>,
    /// Reasons of what stays unrated, with counts.
    pub reasons: BTreeMap<String, usize>,
    /// Decisions that break a promise of [`check`], with the first message.
    pub broken: usize,
    pub first_broken: Option<String>,
}

fn source_name(s: Option<Source>) -> &'static str {
    match s {
        Some(Source::Equilibrium) => "Gleichgewicht",
        Some(Source::Model) => "Modell",
        Some(Source::Solver) => "Solver",
        Some(Source::Estimate) => "Einschätzung",
        Some(Source::Forced) => "Keine Entscheidung",
        None => "ohne Quelle",
    }
}

impl Coverage {
    pub fn add(&mut self, hand: &Hand, d: &Decision) {
        let row = self.rows.entry(situation(hand, d)).or_default();
        row.decisions += 1;
        match d.status {
            Status::Rated => row.rated += 1,
            Status::Pending => row.pending += 1,
            Status::Forced => row.forced += 1,
            Status::Unrated => {
                row.unrated += 1;
                *self.reasons.entry(d.reason.clone().unwrap_or_default()).or_default() += 1;
            }
        }
        if d.status != Status::Unrated {
            *row.by_source.entry(source_name(d.source)).or_default() += 1;
        }
        if let Err(e) = check(d) {
            self.broken += 1;
            self.first_broken.get_or_insert(e);
        }
    }

    pub fn total(&self) -> Row {
        let mut t = Row::default();
        for r in self.rows.values() {
            t.decisions += r.decisions;
            t.rated += r.rated;
            t.pending += r.pending;
            t.forced += r.forced;
            t.unrated += r.unrated;
            for (s, n) in &r.by_source {
                *t.by_source.entry(s).or_default() += n;
            }
        }
        t
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let line = |f: &mut fmt::Formatter, name: &str, r: &Row| {
            let sources: Vec<String> = r.by_source.iter().map(|(s, n)| format!("{s} {n}")).collect();
            writeln!(
                f,
                "{name:<26} {:>6} {:>8} {:>8} {:>8} {:>8}   {}",
                r.decisions,
                r.rated,
                r.pending,
                r.forced,
                r.unrated,
                sources.join(", ")
            )
        };
        writeln!(f, "{:<26} {:>6} {:>8} {:>8} {:>8} {:>8}   Quellen", "Lage", "Entsch.", "bewertet", "Solver*", "erzwung.", "offen")?;
        for (s, r) in &self.rows {
            line(f, s.name(), r)?;
        }
        line(f, "gesamt", &self.total())?;
        writeln!(f, "* wartet auf den Postflop-Solver")?;
        for (reason, n) in &self.reasons {
            writeln!(f, "  {n:5} × {reason}")?;
        }
        if self.broken > 0 {
            writeln!(f, "{} Entscheidungen verletzen eine Zusage, z. B. {}", self.broken, self.first_broken.as_deref().unwrap_or(""))?;
        }
        Ok(())
    }
}
