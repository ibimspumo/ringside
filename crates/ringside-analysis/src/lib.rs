//! Hand analysis: every decision the hero made in a hand, rated against what Ringside
//! can compute for it.
//!
//! - **Push/fold** (effective stack up to 20 BB, everyone folded or facing a shove): the
//!   equilibrium of [`ringside_engine::multiway`] from the bundled tables at the nearest
//!   tabulated stack, players and ante. Source [`Source::Equilibrium`].
//! - **Deeper preflop**: Ringside's preflop model ([`ringside_engine::preflop`]) at the
//!   nearest precomputed table size, stack depth and ante structure; how far the hand is
//!   from that configuration is part of the note. Source [`Source::Model`]; a model, not
//!   an equilibrium of real poker.
//! - **Heads-up postflop**: ranges from the preflop model along the hand's preflop line,
//!   board, pot and stacks from the hand, the nearest standard bet sizes; solved later in
//!   a queue ([`postflop`]). Source [`Source::Solver`].
//! - Everything else is stored as not rated, with the reason.

mod fmt;
pub mod grade;
pub mod postflop;
mod preflop;
mod pushfold;
pub mod ranges;
mod seats;

use rayon::prelude::*;
pub use ringside_core::db::{Choice, Decision, HandAnalysis, Rating, Source, SpotType, Status};
use ringside_core::model::{ActionKind, Hand, Street};
use ringside_core::parse_hand;
use ringside_engine::pushfold_table::Ante;
use serde::Serialize;

pub use grade::rate;
pub use postflop::{PostflopJob, solve_job};
pub use preflop::SpotModel;

/// Stored with every decision; a hand analysed with another version is analysed again.
pub const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+analysis.1");

/// The analysis of one hand. Postflop decisions are pending until their job is solved.
pub fn analyze_hand(hand: &Hand) -> HandAnalysis {
    let mut decisions: Vec<Decision> = Vec::new();
    let mut job = None;
    match hand.hero.as_deref() {
        Some(hero) if hero_actions(hand, hero).next().is_some() => {
            let pre = preflop::analyze(hand, hero);
            decisions.extend(pre.decisions);
            let (post, j) = postflop::prepare(hand, hero, pre.flop.as_ref().map_err(String::as_str));
            decisions.extend(post);
            job = j.map(|j| (j.pot_bb, serde_json::to_string(&j).expect("jobs serialize")));
        }
        _ => {}
    }
    decisions.sort_by_key(|d| d.action_index);
    HandAnalysis { hand_id: hand.id, engine: ENGINE.into(), decisions, job }
}

/// Which bundled chart a rated preflop decision was measured against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Pushfold,
    Preflop,
}

/// Where a rated preflop decision sits in the bundled charts, so the charts page can open
/// exactly that spot with the hero's hand.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ChartTarget {
    pub action_index: usize,
    pub kind: ChartKind,
    pub ante: Ante,
    /// Table size of the chart (the preflop model may be larger than the real table).
    pub players: usize,
    /// Tabulated stack the decision was measured at.
    pub stack_bb: f64,
    /// Hero's seat in the chart, in preflop action order.
    pub seat: usize,
    /// Push/fold: who shoved before the hero and who called it.
    pub shover: Option<usize>,
    pub callers: Vec<usize>,
    /// Preflop model: id of the spot on the charts page.
    pub node: Option<usize>,
    /// Hero's hand class, e.g. "62o".
    pub hand: String,
}

/// The chart spots of the hero's rated preflop decisions; empty without a hero.
pub fn chart_targets(hand: &Hand) -> Vec<ChartTarget> {
    match hand.hero.as_deref() {
        Some(hero) => preflop::analyze(hand, hero).targets,
        None => Vec::new(),
    }
}

/// Analyses raw hands in parallel; unparsable hands get an empty analysis, so they are
/// not offered again.
pub fn analyze_raw(rows: &[(u64, String)]) -> Vec<HandAnalysis> {
    rows.par_iter()
        .map(|(id, raw)| match parse_hand(raw) {
            Ok(hand) => analyze_hand(&hand),
            Err(_) => HandAnalysis { hand_id: *id, engine: ENGINE.into(), decisions: Vec::new(), job: None },
        })
        .collect()
}

/// The hero's voluntary actions with their index in `Hand.actions`.
pub(crate) fn hero_actions<'a>(hand: &'a Hand, hero: &'a str) -> impl Iterator<Item = (usize, &'a ringside_core::model::Action)> {
    hand.actions.iter().enumerate().filter(move |(_, a)| a.player == hero && !a.kind.is_post())
}

/// A decision without a verdict yet.
pub(crate) fn base(hand_id: u64, index: usize, street: Street, spot: SpotType, played: String) -> Decision {
    Decision {
        hand_id,
        action_index: index,
        street,
        spot,
        status: Status::Unrated,
        played,
        best: None,
        strategy: Vec::new(),
        played_freq: None,
        ev_loss_bb: None,
        rating: None,
        source: None,
        why: String::new(),
        note: String::new(),
        reason: None,
        solver: None,
        engine: ENGINE.into(),
    }
}

/// A decision that is not rated, with the reason.
pub(crate) fn unrated(hand: &Hand, index: usize, spot: SpotType, reason: impl Into<String>) -> Decision {
    let a = &hand.actions[index];
    let mut d = base(hand.id, index, a.street, spot, fmt::real_action(hand, a));
    d.reason = Some(reason.into());
    d
}

/// A rated decision from the options the source considers.
pub(crate) fn rated(
    hand: &Hand,
    index: usize,
    spot: SpotType,
    source: Source,
    strategy: Vec<Choice>,
    note: String,
    reason: impl FnOnce(&Decision) -> String,
) -> Decision {
    let a = &hand.actions[index];
    score(base(hand.id, index, a.street, spot, fmt::real_action(hand, a)), source, strategy, note, reason)
}

/// Fills in the verdict: the most frequent action is the recommendation, the loss is
/// the best EV minus the EV of what was played.
pub(crate) fn score(
    mut d: Decision,
    source: Source,
    strategy: Vec<Choice>,
    note: String,
    reason: impl FnOnce(&Decision) -> String,
) -> Decision {
    let played = strategy.iter().find(|c| c.played);
    let best_ev = strategy.iter().filter_map(|c| c.ev_bb).fold(f64::NEG_INFINITY, f64::max);
    let best = strategy
        .iter()
        .max_by(|a, b| a.freq.total_cmp(&b.freq).then(a.ev_bb.unwrap_or(0.0).total_cmp(&b.ev_bb.unwrap_or(0.0))));
    d.best = best.map(|c| c.label.clone());
    d.played_freq = played.map(|c| c.freq);
    d.ev_loss_bb = played.and_then(|c| c.ev_bb).filter(|_| best_ev.is_finite()).map(|ev| (best_ev - ev).max(0.0));
    d.rating = played.map(|c| rate(d.ev_loss_bb, c.freq));
    // Actions of a mixed strategy have equal EV in an equilibrium; what is left is noise.
    if played.is_some_and(|c| c.freq >= grade::MIXED_FREQ) {
        d.ev_loss_bb = d.ev_loss_bb.map(|_| 0.0);
    }
    d.status = if d.rating.is_some() { Status::Rated } else { Status::Unrated };
    d.reason = d.rating.is_none().then(|| "Die gespielte Aktion kommt in der Berechnung nicht vor.".to_string());
    d.source = Some(source);
    d.strategy = strategy;
    d.note = note;
    if d.rating.is_some() {
        d.why = format!("{} Grund: {}", headline(&d), reason(&d));
    }
    d
}

/// The verdict as a sentence with what to do next.
fn headline(d: &Decision) -> String {
    let who = match d.source {
        Some(Source::Equilibrium) => "das Gleichgewicht",
        Some(Source::Model) => "das Modell",
        _ => "der Solver",
    };
    let freq = d.played_freq.unwrap_or(0.0);
    let best = d.best.as_deref().unwrap_or("?");
    match d.rating {
        Some(Rating::Best) => format!("Gut gespielt: {} ist hier die beste Wahl.", action_name(&d.played)),
        Some(Rating::Good) if freq >= grade::MIXED_FREQ => format!(
            "Passt: {who} spielt {} mit deiner Hand zu {} %, meist aber {best}.",
            action_name(&d.played),
            (freq * 100.0).round()
        ),
        _ => match d.ev_loss_bb {
            Some(loss) if loss >= 0.05 => format!(
                "Hier wäre {best} besser gewesen – {} kostet dich etwa {}.",
                action_name(&d.played),
                fmt::bb(loss)
            ),
            _ => format!("Hier wäre {best} besser gewesen: {who} spielt {} mit deiner Hand nur zu {} %.", action_name(&d.played), (freq * 100.0).round()),
        },
    }
}

/// "Call 3 BB" → "dein Call"; sizes are in the action list anyway.
fn action_name(played: &str) -> String {
    let word = played.split_whitespace().next().unwrap_or(played);
    format!("dein {word}")
}

pub(crate) fn is_raise(kind: ActionKind) -> bool {
    matches!(kind, ActionKind::Bet | ActionKind::Raise)
}
