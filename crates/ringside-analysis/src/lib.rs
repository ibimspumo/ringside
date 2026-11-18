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

pub mod calibration;
pub mod coverage;
pub mod estimate;
mod fmt;
pub mod grade;
pub mod postflop;
mod preflop;
mod pushfold;
pub mod ranges;
pub mod replay;
mod seats;

use rayon::prelude::*;
pub use ringside_core::db::{Choice, Decision, HandAnalysis, Rating, Source, SpotType, Status};
use ringside_core::model::{ActionKind, Hand, Street};
use ringside_core::parse_hand;

pub use postflop::{PostflopJob, solve_job};

/// Stored with every decision; a hand analysed with another version is analysed again.
pub const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+analysis.2");

/// The analysis of one hand. Postflop decisions are pending until their job is solved;
/// what neither the tables nor the solver cover is estimated ([`replay::rate`]), forced
/// moves are no decisions. Hands of other games have none.
pub fn analyze_hand(hand: &Hand) -> HandAnalysis {
    let mut decisions: Vec<Decision> = Vec::new();
    let mut job = None;
    match hand.hero.as_deref() {
        Some(hero) if hero_actions(hand, hero).next().is_some() && hand.variant.contains("Hold'em No Limit") => {
            let pre = preflop::analyze(hand, hero);
            decisions.extend(pre.decisions);
            let (post, j) = postflop::prepare(hand, hero, pre.flop.as_ref().map_err(String::as_str));
            decisions.extend(post);
            job = j.map(|j| (j.pot_bb, serde_json::to_string(&j).expect("jobs serialize")));
            for d in &mut decisions {
                if let Some(f) = forced(hand, hero, d) {
                    *d = f;
                } else if d.status == Status::Unrated {
                    let why = d.reason.clone().unwrap_or_default();
                    if let Ok(e) = replay::rate(hand, hero, d.action_index, d.spot, pre.ranges.as_ref(), &why) {
                        *d = e;
                    }
                }
            }
        }
        _ => {}
    }
    decisions.sort_by_key(|d| d.action_index);
    HandAnalysis { hand_id: hand.id, engine: ENGINE.into(), decisions, job }
}

/// The hero's action as a forced move, if it was one: with nobody left who could call a
/// bet there is nothing to decide but to check; and calling the last few chips, when the
/// pot asks for less than a tenth of equity, is right with any two cards.
fn forced(hand: &Hand, hero: &str, d: &Decision) -> Option<Decision> {
    let spot = replay::spot_at(hand, hero, d.action_index, None).ok()?;
    let a = &hand.actions[d.action_index];
    let call = spot.to_call();
    let why = if call <= 1e-9 && spot.opponents.iter().all(|o| o.behind <= 1e-9) {
        "Alle anderen sind schon all-in: Setzen kann niemand mehr callen, hier gab es nichts zu entscheiden."
    } else if a.kind == ActionKind::Call && call >= spot.hero_behind - 1e-9 && call / (spot.pot + call) <= FORCED_ODDS {
        "Nur noch ein paar Chips dahinter: Der Call kostet so wenig im Verhältnis zum Pot, dass er mit jeder Hand richtig ist."
    } else {
        return None;
    };
    let mut f = base(hand.id, d.action_index, a.street, d.spot, fmt::real_action(hand, a));
    f.status = Status::Forced;
    f.source = Some(Source::Forced);
    f.why = why.into();
    Some(f)
}

/// Equity a call needs, from which calling the rest of the stack is no decision.
const FORCED_ODDS: f64 = 0.1;

/// Solves a hand's postflop job and rates its decisions.
pub fn settle_job(hand: &Hand, job: &PostflopJob, threads: usize, memory_limit: u64) -> Vec<Decision> {
    let solved: Vec<Decision> = solve_job(job, threads, memory_limit, |_| {}, || false).unwrap_or_default();
    let pending: Vec<Decision> = analyze_hand(hand).decisions.into_iter().filter(|d| d.status == Status::Pending).collect();
    let solved = if solved.is_empty() { pending } else { solved };
    estimate_rest(hand, solved, "Der Solver konnte diesen Spot nicht rechnen.")
}

/// Decisions of a solved job the solver left without a rating (or `pending` ones of a
/// job that failed), rated by the estimate instead.
pub fn estimate_rest(hand: &Hand, decisions: Vec<Decision>, failed: &str) -> Vec<Decision> {
    let Some(hero) = hand.hero.as_deref() else { return decisions };
    let ranges = preflop::analyze(hand, hero).ranges;
    decisions
        .into_iter()
        .map(|d| {
            if !matches!(d.status, Status::Unrated | Status::Pending) {
                return d;
            }
            let why = match d.status {
                Status::Pending => failed.to_string(),
                _ => d.reason.clone().unwrap_or_default(),
            };
            replay::rate(hand, hero, d.action_index, d.spot, ranges.as_ref(), &why).unwrap_or(d)
        })
        .collect()
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
        pot_bb: None,
        reliability: None,
    }
}

/// A decision that is not rated, with the reason.
pub(crate) fn unrated(hand: &Hand, index: usize, spot: SpotType, reason: impl Into<String>) -> Decision {
    let a = &hand.actions[index];
    let mut d = base(hand.id, index, a.street, spot, fmt::real_action(hand, a));
    d.reason = Some(reason.into());
    d
}

/// A decision at `hand.actions[index]` before its verdict.
pub(crate) fn base_at(hand: &Hand, index: usize, spot: SpotType) -> Decision {
    let a = &hand.actions[index];
    base(hand.id, index, a.street, spot, fmt::real_action(hand, a))
}

/// Chips in the middle before action `index` (antes, blinds and every bet so far), BB.
pub(crate) fn pot_before(hand: &Hand, index: usize) -> f64 {
    hand.actions[..index].iter().map(|a| a.amount as f64).sum::<f64>() / hand.big_blind.max(1) as f64
}

/// Fills in the verdict with the practice's grades ([`grade::verdict`]) at a pot of
/// `pot_bb`. `why` is the reason in plain words; the headline is written where it is
/// shown, with the practice's sentences.
pub(crate) fn score(
    mut d: Decision,
    source: Source,
    strategy: Vec<Choice>,
    pot_bb: f64,
    note: String,
    reason: impl FnOnce(&Decision) -> String,
) -> Decision {
    d.source = Some(source);
    d.note = note;
    d.pot_bb = Some(pot_bb);
    let Some(v) = grade::verdict(&strategy, pot_bb) else {
        d.strategy = strategy;
        d.status = Status::Unrated;
        d.reason = Some("Die gespielte Aktion kommt in der Berechnung nicht vor.".into());
        return d;
    };
    // Best and mix point at the main move; below that, at the one worth most.
    let best = if v.rating <= Rating::Mix { v.main } else { v.best };
    d.best = strategy.get(best).map(|c| c.label.clone());
    d.played_freq = Some(v.freq);
    d.ev_loss_bb = v.loss_bb;
    d.rating = Some(v.rating);
    d.status = Status::Rated;
    d.reason = None;
    d.strategy = strategy;
    d.why = reason(&d);
    d
}

pub(crate) fn is_raise(kind: ActionKind) -> bool {
    matches!(kind, ActionKind::Bet | ActionKind::Raise)
}
