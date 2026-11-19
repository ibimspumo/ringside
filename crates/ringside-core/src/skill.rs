//! The Spielwertung: one number for how well the hero decides, with its history per
//! tournament, like a chess rating. It rates decisions when they were made, never their
//! outcome, and it is a pure function of the stored analysis: the same rated decisions in
//! the same tournaments always give the same history, and a new engine that rates the
//! decisions again gives the history of the new ratings.
//!
//! # How a tournament counts
//!
//! Every rated decision `i` brings three numbers:
//!
//! - **cost** `c_i`: the EV the decision lost as a share of the hero's own stack at the
//!   start of the hand (capped at 1): the share of the tournament thrown away on average.
//!   Calling off the stack wrongly costs a large share, a wrong 2-BB call a small one.
//! - **stake** `s_i ∈ [0, 1]`: how much of the tournament was in play, from the pot at the
//!   decision as a share of the own stack: 0 up to [`TRIVIAL_POT`] (a fold of a weak hand
//!   deep-stacked decides nothing), rising to 1 at [`FULL_POT`] (an all-in, a big pot).
//!   The stake follows from the spot alone, never from what the hero did.
//! - **trust** `w_i`: equilibrium, model and solver count fully; the estimate with
//!   [`ESTIMATE_TRUST`] × its measured reliability (`estimate_calibration.json`); forced
//!   moves, unrated and pending decisions not at all.
//!
//! A tournament's burn is `C = Σ w·c`, its real decisions `O = Σ w·s`, its error rate
//! `C / O`: the share of the stack lost per real decision. The performance maps the rate
//! on a log scale: [`REF_RATING`] at [`REF_RATE`], [`POINTS_PER_HALVING`] more for each
//! halving, clamped to [`FLOOR`]–[`CEILING`].
//!
//! # How the rating moves (Glicko-like)
//!
//! The rating `R` carries an uncertainty `σ`. Before a tournament `σ²` grows by
//! [`DRIFT_PER_TOURNAMENT`]² and by [`DRIFT_PER_DAY`]² for every day since the last one
//! (never above [`START_SIGMA`]). The tournament's performance `P` is a measurement with
//! noise `τ² = NOISE² / O`: many real decisions say more than a few. Then
//! `K = σ² / (σ² + τ²)`, `R += clamp(K·(P − R), ±MAX_STEP)` and `σ² = σ²·τ² / (σ² + τ²)`.
//! A tournament with fewer than [`MIN_REAL`] real decisions changes nothing.
//!
//! # Invariants (tested below)
//!
//! 1. Same decisions with other cards or outcomes give the same rating: neither enters.
//! 2. Replacing a decision by a better one (less loss, same spot) never lowers the rating,
//!    of that tournament or any later one.
//! 3. In the same spot and from the same source, a mistake that costs a larger share of
//!    the stack never weighs less than one that costs a smaller share.
//! 4. The step per tournament is at most [`MAX_STEP`]; every counted tournament lowers the
//!    uncertainty against what it was before it, so it falls with experience, and it grows
//!    again over a long pause, up to [`START_SIGMA`].
//! 5. The order of the decisions within a tournament does not matter.
//! 6. Any number of trivial correct decisions (stake 0, cost 0) changes nothing, so a mass
//!    of easy folds never lifts the number above what the real decisions give.
//! 7. Unrated, pending and forced decisions change nothing.
//! 8. The rating stays within [`FLOOR`]–[`CEILING`].
//!
//! ICM and the bubble are not weighed in: the engine rates in chips, the hand history
//! knows neither the field's stacks nor the remaining players, and the stake already
//! makes a decision about the whole stack count fully.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::db::{Rating, Source, SpotType, Status};
use crate::next::day_number;

/// Where a newcomer starts: a casual player who loses about three times as much per real
/// decision as the reference.
pub const START: f64 = 1200.0;
pub const START_SIGMA: f64 = 300.0;
/// The rating of [`REF_RATE`]: Pluribus and the professionals of its 10 000 hands measured
/// by Ringside's engine.
pub const REF_RATING: f64 = 2000.0;
pub const REF_RATE: f64 = 0.0038;
pub const POINTS_PER_HALVING: f64 = 500.0;
pub const FLOOR: f64 = 100.0;
pub const CEILING: f64 = 2400.0;
/// Noise of one real decision's measurement, in rating points.
pub const NOISE: f64 = 1700.0;
pub const DRIFT_PER_TOURNAMENT: f64 = 15.0;
pub const DRIFT_PER_DAY: f64 = 6.0;
pub const MAX_STEP: f64 = 150.0;
/// Real decisions a tournament needs to count.
pub const MIN_REAL: f64 = 1.0;
/// Pot shares of the own stack where the stake starts and where it is full.
pub const TRIVIAL_POT: f64 = 0.02;
pub const FULL_POT: f64 = 0.15;
/// Share of its reliability with which an estimate counts.
pub const ESTIMATE_TRUST: f64 = 0.8;
/// From this uncertainty on the number is shown as provisional.
pub const PROVISIONAL_SIGMA: f64 = 110.0;
/// Tournaments the strengths and weaknesses look back on.
pub const AREA_UNITS: usize = 20;
/// Real decisions an area needs before it is judged, and the weight of the overall
/// rating in it (an area with this many real decisions sits halfway).
const AREA_MIN_REAL: f64 = 4.0;
const AREA_PRIOR: f64 = 40.0;
/// Points off the rating from which an area counts as a strength or a weakness.
const AREA_GAP: f64 = 60.0;
/// Decisions named under "Was hat dich zuletzt bewegt".
const MOMENTS: usize = 6;
/// A good decision in a pot of at least this stake that lost chips was bad luck.
const LUCK_STAKE: f64 = 0.5;

/// One stored decision of the hero with what the rating needs of its hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Move {
    pub hand_id: u64,
    pub action_index: usize,
    /// The tournament; cash hands count per day.
    pub tournament_id: Option<u64>,
    /// "YYYY/MM/DD HH:MM:SS" as in the hand history.
    pub played_at: String,
    pub spot: SpotType,
    pub status: Status,
    pub rating: Option<Rating>,
    pub source: Option<Source>,
    pub reliability: Option<f64>,
    pub ev_loss_bb: Option<f64>,
    pub pot_bb: Option<f64>,
    /// The hero's own stack at the start of the hand.
    pub chips_bb: Option<f64>,
    /// Effective stack at the start of the hand, for the areas.
    pub stack_bb: Option<f64>,
    pub position: Option<String>,
    /// Shown only, never rated.
    pub hero_net_bb: Option<f64>,
    pub hero_cards: String,
    pub played: String,
    pub best: Option<String>,
}

/// Rated decisions by grade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Grades {
    /// Best or a mix.
    pub perfect: usize,
    pub good: usize,
    pub inaccurate: usize,
    pub mistake: usize,
    pub blunder: usize,
}

impl Grades {
    fn add(&mut self, r: Rating) {
        match r {
            Rating::Best | Rating::Mix => self.perfect += 1,
            Rating::Good => self.good += 1,
            Rating::Inaccuracy => self.inaccurate += 1,
            Rating::Mistake => self.mistake += 1,
            Rating::Blunder => self.blunder += 1,
        }
    }

    fn merge(&mut self, o: &Grades) {
        self.perfect += o.perfect;
        self.good += o.good;
        self.inaccurate += o.inaccurate;
        self.mistake += o.mistake;
        self.blunder += o.blunder;
    }

    pub fn total(&self) -> usize {
        self.perfect + self.good + self.inaccurate + self.mistake + self.blunder
    }
}

/// The areas of strengths and weaknesses, as a casual player thinks of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Area {
    /// First in below 20 BB: all-in or fold.
    PushFold,
    /// Facing an all-in.
    VsAllIn,
    /// First in from 20 BB, except from the small blind.
    Open,
    /// The small blind first in, or the big blind against it.
    BlindVsBlind,
    /// The big blind facing an open or limps.
    BigBlind,
    /// Facing an open or limps elsewhere, 3-bets included.
    VsOpen,
    VsThreeBet,
    /// Flop and turn.
    Postflop,
    River,
}

impl Area {
    pub const ALL: [Area; 9] = [
        Area::PushFold,
        Area::VsAllIn,
        Area::Open,
        Area::BlindVsBlind,
        Area::BigBlind,
        Area::VsOpen,
        Area::VsThreeBet,
        Area::Postflop,
        Area::River,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Area::PushFold => "All-in oder Fold",
            Area::VsAllIn => "Gegen ein All-in",
            Area::Open => "Eröffnen",
            Area::BlindVsBlind => "Blind gegen Blind",
            Area::BigBlind => "Im Big Blind",
            Area::VsOpen => "Gegen eine Eröffnung",
            Area::VsThreeBet => "Gegen eine 3-Bet",
            Area::Postflop => "Flop und Turn",
            Area::River => "River",
        }
    }

    /// The spot types of the area, for the hand filter.
    pub fn spots(self) -> &'static [SpotType] {
        match self {
            Area::PushFold | Area::Open | Area::BlindVsBlind => &[SpotType::Open, SpotType::Steal, SpotType::Shove],
            Area::VsAllIn => &[SpotType::CallShove],
            Area::BigBlind | Area::VsOpen => &[SpotType::VsOpen, SpotType::ThreeBet, SpotType::VsLimp],
            Area::VsThreeBet => &[SpotType::VsThreeBet],
            Area::Postflop => &[SpotType::CBet, SpotType::VsCBet, SpotType::Flop, SpotType::Turn],
            Area::River => &[SpotType::River],
        }
    }

    /// The area of a decision, from the situation alone.
    pub fn of(m: &Move) -> Area {
        let short = m.stack_bb.is_some_and(|s| s < 20.0);
        let pos = m.position.as_deref();
        match m.spot {
            SpotType::Open | SpotType::Steal | SpotType::Shove if short => Area::PushFold,
            SpotType::Open | SpotType::Steal | SpotType::Shove if pos == Some("SB") => Area::BlindVsBlind,
            SpotType::Open | SpotType::Steal | SpotType::Shove => Area::Open,
            SpotType::CallShove => Area::VsAllIn,
            SpotType::VsOpen | SpotType::ThreeBet | SpotType::VsLimp if pos == Some("BB") => Area::BigBlind,
            SpotType::VsOpen | SpotType::ThreeBet | SpotType::VsLimp => Area::VsOpen,
            SpotType::VsThreeBet => Area::VsThreeBet,
            SpotType::CBet | SpotType::VsCBet | SpotType::Flop | SpotType::Turn => Area::Postflop,
            SpotType::River => Area::River,
        }
    }
}

/// The named levels of the rating, lowest first: (from, name).
pub const TIERS: [(f64, &str); 6] =
    [(0.0, "Einsteiger"), (1000.0, "Auf dem Weg"), (1300.0, "Solide"), (1550.0, "Stark"), (1800.0, "Sehr stark"), (2000.0, "Profi-Niveau")];

pub fn tier(rating: f64) -> &'static str {
    TIERS.iter().rev().find(|(from, _)| rating >= *from).map_or(TIERS[0].1, |t| t.1)
}

/// What a rated decision counts: trust, cost and stake (module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weight {
    pub trust: f64,
    pub cost: f64,
    pub stake: f64,
}

pub fn weigh(m: &Move) -> Option<Weight> {
    if m.status != Status::Rated {
        return None;
    }
    let trust = match m.source? {
        Source::Equilibrium | Source::Model | Source::Solver => 1.0,
        Source::Estimate => ESTIMATE_TRUST * m.reliability?.clamp(0.0, 1.0),
        Source::Forced => return None,
    };
    let chips = m.chips_bb.or(m.stack_bb).filter(|c| c.is_finite() && *c > 0.0)?;
    let loss = m.ev_loss_bb.filter(|l| l.is_finite())?.max(0.0);
    let pot = m.pot_bb.filter(|p| p.is_finite())?.max(0.0);
    m.rating?;
    Some(Weight { trust, cost: (loss / chips).min(1.0), stake: ((pot / chips - TRIVIAL_POT) / (FULL_POT - TRIVIAL_POT)).clamp(0.0, 1.0) })
}

/// The performance of a burn `burn` over `real` real decisions.
pub fn performance(burn: f64, real: f64) -> f64 {
    let rate = burn / real;
    if rate <= 0.0 {
        return CEILING;
    }
    (REF_RATING - POINTS_PER_HALVING * (rate / REF_RATE).log2()).clamp(FLOOR, CEILING)
}

/// The error rate a rating stands for: the inverse of [`performance`].
pub fn rate_at(rating: f64) -> f64 {
    REF_RATE * ((REF_RATING - rating) / POINTS_PER_HALVING).exp2()
}

/// One tournament (or cash day) in the history.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    /// "t:<id>" or "c:<YYYY/MM/DD>".
    pub key: String,
    pub tournament_id: Option<u64>,
    pub started_at: String,
    pub rating: f64,
    pub sigma: f64,
    pub delta: f64,
    /// `None` when the tournament had too few real decisions to count.
    pub performance: Option<f64>,
    pub decisions: usize,
    /// Real decisions, the sum of the stakes.
    pub real: f64,
    /// Share of a stack lost in total, percent.
    pub burned_pct: f64,
    pub grades: Grades,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Strong,
    Even,
    Weak,
    /// Too few real decisions to say.
    Few,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AreaScore {
    pub area: Area,
    pub name: &'static str,
    pub spots: &'static [SpotType],
    pub decisions: usize,
    pub real: f64,
    /// The area's rating, drawn towards the overall rating while it has few decisions.
    pub rating: f64,
    pub verdict: Verdict,
    /// Share of all the stack lost in the window that was lost here, percent.
    pub burn_share_pct: f64,
    pub grades: Grades,
}

/// A decision of the latest tournament and how far it moved the number.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Moment {
    pub hand_id: u64,
    pub action_index: usize,
    pub played_at: String,
    pub hero_cards: String,
    pub played: String,
    pub best: Option<String>,
    pub rating: Rating,
    pub source: Source,
    pub area: Area,
    /// About this many points of the tournament's change, to first order.
    pub points: f64,
    /// Share of the stack the decision lost, percent.
    pub lost_pct: f64,
    /// The pot at the decision as a share of the own stack, percent.
    pub pot_pct: f64,
    /// Decided well, lost chips anyway.
    pub bad_luck: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SkillProfile {
    /// `None` until a tournament counted.
    pub rating: Option<f64>,
    pub sigma: f64,
    pub provisional: bool,
    pub tier: Option<&'static str>,
    pub tiers: Vec<(f64, &'static str)>,
    pub start: f64,
    pub entries: Vec<Entry>,
    pub grades: Grades,
    pub areas: Vec<AreaScore>,
    pub moments: Vec<Moment>,
}

/// How far a stretch of play moved the rating, for the note after an analysis run and the
/// session review.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Change {
    pub rating: f64,
    pub sigma: f64,
    pub tier: &'static str,
    /// `None` when there was no rating before.
    pub delta: Option<f64>,
}

/// The rating over `moves` against the one over those of them `before` keeps; `None` while
/// nothing counted.
pub fn change(moves: &[Move], before: impl Fn(&Move) -> bool) -> Option<Change> {
    let now = profile(moves);
    let rating = now.rating?;
    let earlier: Vec<Move> = moves.iter().filter(|m| before(m)).cloned().collect();
    let delta = profile(&earlier).rating.map(|r| rating - r);
    Some(Change { rating, sigma: now.sigma, tier: tier(rating), delta })
}

struct Unit<'a> {
    key: String,
    tournament_id: Option<u64>,
    started_at: String,
    moves: Vec<(&'a Move, Weight)>,
}

fn units(moves: &[Move]) -> Vec<Unit<'_>> {
    let mut by: BTreeMap<String, Unit> = BTreeMap::new();
    for m in moves {
        let Some(w) = weigh(m) else { continue };
        let key = match m.tournament_id {
            Some(t) => format!("t:{t}"),
            None => format!("c:{}", m.played_at.get(..10).unwrap_or(&m.played_at)),
        };
        let u = by.entry(key.clone()).or_insert_with(|| Unit {
            key,
            tournament_id: m.tournament_id,
            started_at: m.played_at.clone(),
            moves: Vec::new(),
        });
        if m.played_at < u.started_at {
            u.started_at = m.played_at.clone();
        }
        u.moves.push((m, w));
    }
    let mut list: Vec<Unit> = by.into_values().collect();
    list.sort_by(|a, b| a.started_at.cmp(&b.started_at).then_with(|| a.key.cmp(&b.key)));
    list
}

/// Sums in a fixed order, so the result does not depend on the order of the decisions.
fn sums(moves: &[(&Move, Weight)]) -> (f64, f64) {
    let mut parts: Vec<(f64, f64)> = moves.iter().map(|(_, w)| (w.trust * w.cost, w.trust * w.stake)).collect();
    parts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    parts.iter().fold((0.0, 0.0), |(c, o), p| (c + p.0, o + p.1))
}

/// The rating's state between tournaments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    pub rating: f64,
    pub var: f64,
    /// Day number of the last counted tournament.
    pub day: Option<i64>,
}

impl Default for State {
    fn default() -> State {
        State { rating: START, var: START_SIGMA * START_SIGMA, day: None }
    }
}

/// The uncertainty before a tournament on `day`: it grows per tournament and per day of
/// pause, never above the start.
pub fn drift(s: &mut State, day: Option<i64>) {
    let days = match (s.day, day) {
        (Some(a), Some(b)) => (b - a).max(0) as f64,
        _ => 0.0,
    };
    s.var = (s.var + DRIFT_PER_TOURNAMENT.powi(2) + DRIFT_PER_DAY.powi(2) * days).min(START_SIGMA * START_SIGMA);
}

/// One counted tournament: burn and real decisions on `day`. Returns the change and the
/// gain `K`.
pub fn update(s: &mut State, burn: f64, real: f64, day: Option<i64>) -> (f64, f64) {
    drift(s, day);
    let noise = NOISE * NOISE / real;
    let k = s.var / (s.var + noise);
    let delta = (k * (performance(burn, real) - s.rating)).clamp(-MAX_STEP, MAX_STEP);
    let before = s.rating;
    s.rating = (s.rating + delta).clamp(FLOOR, CEILING);
    s.var = s.var * noise / (s.var + noise);
    if day.is_some() {
        s.day = day;
    }
    (s.rating - before, k)
}

/// The history of the rating over `moves` (any order), and what it says now.
pub fn profile(moves: &[Move]) -> SkillProfile {
    let units = units(moves);
    let mut s = State::default();
    let mut entries = Vec::with_capacity(units.len());
    let mut counted: Vec<usize> = Vec::new();
    let mut grades = Grades::default();
    let mut last: Option<(usize, f64, f64, f64)> = None;
    for (i, u) in units.iter().enumerate() {
        let (burn, real) = sums(&u.moves);
        let mut g = Grades::default();
        u.moves.iter().filter_map(|(m, _)| m.rating).for_each(|r| g.add(r));
        grades.merge(&g);
        let (delta, performance) = if real >= MIN_REAL {
            let before = s.rating;
            let (delta, k) = update(&mut s, burn, real, day_number(&u.started_at));
            counted.push(i);
            last = Some((i, before, k, real));
            (delta, Some(performance(burn, real)))
        } else {
            (0.0, None)
        };
        entries.push(Entry {
            key: u.key.clone(),
            tournament_id: u.tournament_id,
            started_at: u.started_at.clone(),
            rating: s.rating,
            sigma: s.var.sqrt(),
            delta,
            performance,
            decisions: u.moves.len(),
            real,
            burned_pct: burn * 100.0,
            grades: g,
        });
    }
    let rating = (!counted.is_empty()).then_some(s.rating);
    let window: Vec<&Unit> = counted.iter().rev().take(AREA_UNITS).map(|&i| &units[i]).collect();
    let moments = last.map(|(i, before, k, real)| moments(&units[i], before, k, real)).unwrap_or_default();
    SkillProfile {
        rating,
        sigma: s.var.sqrt(),
        provisional: s.var.sqrt() > PROVISIONAL_SIGMA,
        tier: rating.map(tier),
        tiers: TIERS.to_vec(),
        start: START,
        entries,
        grades,
        areas: rating.map(|r| areas(&window, r)).unwrap_or_default(),
        moments,
    }
}

fn areas(window: &[&Unit], rating: f64) -> Vec<AreaScore> {
    let mut by: BTreeMap<Area, Vec<(&Move, Weight)>> = BTreeMap::new();
    for u in window {
        for &(m, w) in &u.moves {
            by.entry(Area::of(m)).or_default().push((m, w));
        }
    }
    let total_burn: f64 = by.values().map(|v| sums(v).0).sum();
    let mut out: Vec<AreaScore> = by
        .into_iter()
        .map(|(area, v)| {
            let (burn, real) = sums(&v);
            let mut grades = Grades::default();
            v.iter().filter_map(|(m, _)| m.rating).for_each(|r| grades.add(r));
            let own = if real > 0.0 { performance(burn, real) } else { rating };
            let r = rating + (own - rating) * real / (real + AREA_PRIOR);
            let verdict = if real < AREA_MIN_REAL {
                Verdict::Few
            } else if r >= rating + AREA_GAP {
                Verdict::Strong
            } else if r <= rating - AREA_GAP {
                Verdict::Weak
            } else {
                Verdict::Even
            };
            AreaScore {
                area,
                name: area.name(),
                spots: area.spots(),
                decisions: v.len(),
                real,
                rating: r,
                verdict,
                burn_share_pct: if total_burn > 0.0 { burn / total_burn * 100.0 } else { 0.0 },
                grades,
            }
        })
        .collect();
    out.sort_by(|a, b| a.area.cmp(&b.area));
    out
}

/// Whether the played action put chips at risk: a fold or a check loses nothing by luck.
fn risked(played: &str) -> bool {
    !(played.starts_with("Fold") || played.starts_with("Check"))
}

/// Each decision's share of the tournament's change, to first order: the tournament
/// moved by `K·(P − R)`, and near the rating `P − R ≈ (B / ln 2)·Σ w·(e·s − c) / (e·O)` with
/// `e` the error rate of the rating before.
fn moments(u: &Unit, before: f64, k: f64, real: f64) -> Vec<Moment> {
    let e = rate_at(before);
    let scale = k * POINTS_PER_HALVING / std::f64::consts::LN_2 / (e * real);
    let mut list: Vec<Moment> = u
        .moves
        .iter()
        .filter_map(|&(m, w)| {
            let points = (scale * w.trust * (e * w.stake - w.cost)).clamp(-MAX_STEP, MAX_STEP);
            let rating = m.rating?;
            let good = rating <= Rating::Good;
            Some(Moment {
                hand_id: m.hand_id,
                action_index: m.action_index,
                played_at: m.played_at.clone(),
                hero_cards: m.hero_cards.clone(),
                played: m.played.clone(),
                best: m.best.clone(),
                rating,
                source: m.source?,
                area: Area::of(m),
                points,
                lost_pct: w.cost * 100.0,
                pot_pct: m.pot_bb.unwrap_or(0.0) / m.chips_bb.or(m.stack_bb).unwrap_or(1.0) * 100.0,
                bad_luck: good && risked(&m.played) && w.stake >= LUCK_STAKE && m.hero_net_bb.is_some_and(|n| n < 0.0),
            })
        })
        .filter(|m| m.points.abs() >= 0.5)
        .collect();
    // The costliest mistakes first, then what went right: bad luck before routine, big pots first.
    let order = |m: &Moment| (m.points >= 0.0, !m.bad_luck, -m.points.abs(), -m.pot_pct);
    list.sort_by(|a, b| {
        let (x, y) = (order(a), order(b));
        x.0.cmp(&y.0)
            .then(x.1.cmp(&y.1))
            .then(x.2.total_cmp(&y.2))
            .then(x.3.total_cmp(&y.3))
            .then(a.hand_id.cmp(&b.hand_id))
            .then(a.action_index.cmp(&b.action_index))
    });
    let bad = list.iter().take_while(|m| m.points < 0.0).count().min(MOMENTS / 2);
    let good: Vec<Moment> = list.iter().skip_while(|m| m.points < 0.0).take(MOMENTS - bad).cloned().collect();
    list.truncate(bad);
    list.extend(good);
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SplitMix64: the property tests draw from fixed seeds, so a failure reproduces.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn f(&mut self) -> f64 {
            (self.next() >> 11) as f64 / (1u64 << 53) as f64
        }
        fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
            xs[(self.next() % xs.len() as u64) as usize]
        }
    }

    const SOURCES: [Source; 4] = [Source::Equilibrium, Source::Model, Source::Solver, Source::Estimate];

    fn rating_for(loss: f64, pot: f64) -> Rating {
        let pct = loss / pot * 100.0;
        match pct {
            p if p <= 0.0 => Rating::Best,
            p if p <= 5.0 => Rating::Good,
            p if p <= 15.0 => Rating::Inaccuracy,
            p if p <= 35.0 => Rating::Mistake,
            _ => Rating::Blunder,
        }
    }

    fn mv(hand_id: u64, tournament: u64, day: u32, source: Source, loss: f64, pot: f64, chips: f64) -> Move {
        Move {
            hand_id,
            action_index: 3,
            tournament_id: Some(tournament),
            played_at: format!("2026/{:02}/{:02} 20:{:02}:00", 1 + day / 28, 1 + day % 28, hand_id % 60),
            spot: SpotType::VsOpen,
            status: Status::Rated,
            rating: Some(rating_for(loss, pot)),
            source: Some(source),
            reliability: (source == Source::Estimate).then_some(0.76),
            ev_loss_bb: Some(loss),
            pot_bb: Some(pot),
            chips_bb: Some(chips),
            stack_bb: Some(chips),
            position: Some("CO".into()),
            hero_net_bb: Some(0.0),
            hero_cards: "AhKd".into(),
            played: "Call".into(),
            best: Some("Fold".into()),
        }
    }

    /// A history of `tournaments` tournaments of real-looking decisions.
    fn history(rng: &mut Rng, tournaments: u64) -> Vec<Move> {
        let mut out = Vec::new();
        let mut id = 1;
        for t in 0..tournaments {
            let day = (t * 2 + rng.next() % 3) as u32;
            for _ in 0..(5 + rng.next() % 40) {
                let chips = 8.0 + rng.f() * 90.0;
                let pot = 1.5 + rng.f() * chips * if rng.f() < 0.2 { 1.5 } else { 0.25 };
                let loss = if rng.f() < 0.85 { 0.0 } else { rng.f() * pot * 0.4 };
                out.push(mv(id, 1000 + t, day, rng.pick(&SOURCES), loss, pot, chips));
                id += 1;
            }
        }
        out
    }

    fn final_rating(moves: &[Move]) -> f64 {
        profile(moves).rating.unwrap_or(START)
    }

    fn ratings(moves: &[Move]) -> Vec<f64> {
        profile(moves).entries.iter().map(|e| e.rating).collect()
    }

    fn cases(n: u64, mut f: impl FnMut(&mut Rng)) {
        for seed in 0..n {
            f(&mut Rng(seed));
        }
    }

    #[test]
    fn cards_and_outcomes_do_not_enter() {
        cases(100, |rng| {
            let a = history(rng, 6);
            let mut b = a.clone();
            for m in &mut b {
                m.hero_cards = rng.pick(&["7c2d", "AsAh", "Td9d"]).into();
                m.hero_net_bb = Some(rng.f() * 200.0 - 100.0);
                m.hand_id += 10_000;
            }
            assert_eq!(ratings(&a), ratings(&b));
        });
    }

    #[test]
    fn a_better_decision_never_lowers_the_rating() {
        cases(300, |rng| {
            let a = history(rng, 8);
            let i = (rng.next() % a.len() as u64) as usize;
            let mut b = a.clone();
            let loss = a[i].ev_loss_bb.unwrap() * rng.f();
            b[i].ev_loss_bb = Some(loss);
            b[i].rating = Some(rating_for(loss, b[i].pot_bb.unwrap()));
            for (x, y) in ratings(&a).iter().zip(ratings(&b)) {
                assert!(y >= x - 1e-9, "better decision {i} lowered {x} to {y}");
            }
        });
    }

    #[test]
    fn a_larger_share_of_the_stack_weighs_at_least_as_much() {
        cases(300, |rng| {
            let mut base = history(rng, 5);
            let last = base.last().unwrap().clone();
            let source = rng.pick(&SOURCES);
            let pot = 20.0;
            let small = rng.f() * 15.0;
            let large = small + rng.f() * 5.0;
            // Same spot, same source and grade, a larger share of the same stack.
            let make = |loss: f64| {
                let mut m = mv(9_999, last.tournament_id.unwrap(), 0, source, loss, pot, 40.0);
                m.played_at = last.played_at.clone();
                m.rating = Some(Rating::Blunder);
                m
            };
            base.push(make(small));
            let a = final_rating(&base);
            *base.last_mut().unwrap() = make(large);
            let b = final_rating(&base);
            assert!(b <= a + 1e-9, "losing {large} BB rated {b}, losing {small} BB rated {a}");
        });
    }

    #[test]
    fn a_call_off_blunder_costs_more_than_a_small_preflop_mistake() {
        let mut rng = Rng(7);
        let base = history(&mut rng, 6);
        let t = base.last().unwrap().tournament_id.unwrap();
        let with = |m: Move| {
            let mut v = base.clone();
            let mut m = m;
            m.played_at = base.last().unwrap().played_at.clone();
            v.push(m);
            final_rating(&v)
        };
        // Calling 25 BB off wrongly with 30 BB vs a 2-BB call before the flop.
        let call_off = with(mv(50_000, t, 0, Source::Equilibrium, 9.0, 28.0, 30.0));
        let small_call = with(mv(50_000, t, 0, Source::Model, 1.0, 4.0, 30.0));
        assert!(call_off + 5.0 < small_call, "{call_off} vs {small_call}");
    }

    #[test]
    fn the_step_is_bounded_and_the_uncertainty_follows_experience() {
        cases(200, |rng| {
            let moves = history(rng, 12);
            let p = profile(&moves);
            let mut s = State::default();
            for e in &p.entries {
                assert!(e.delta.abs() <= MAX_STEP + 1e-9);
                assert!((FLOOR..=CEILING).contains(&e.rating));
                if e.performance.is_some() {
                    drift(&mut s, day_number(&e.started_at));
                    assert!(e.sigma < s.var.sqrt(), "a tournament did not lower the uncertainty");
                    s.var = e.sigma.powi(2);
                    s.day = day_number(&e.started_at);
                }
            }
            let counted: Vec<f64> = p.entries.iter().filter(|e| e.performance.is_some()).map(|e| e.sigma).collect();
            assert!(counted.last() < counted.first(), "experience did not lower the uncertainty: {counted:?}");
        });
        // A long pause lets it grow again, never above the start.
        let settled = State { rating: 1500.0, var: 60.0f64.powi(2), day: Some(0) };
        let (mut soon, mut late, mut never) = (settled, settled, settled);
        drift(&mut soon, Some(3));
        drift(&mut late, Some(200));
        drift(&mut never, Some(100_000));
        assert!(settled.var < soon.var && soon.var < late.var && late.var < never.var);
        assert_eq!(never.var, START_SIGMA.powi(2));
    }

    #[test]
    fn the_order_within_a_tournament_does_not_matter() {
        cases(100, |rng| {
            let a = history(rng, 6);
            let mut b = a.clone();
            for i in (1..b.len()).rev() {
                let j = (rng.next() % (i as u64 + 1)) as usize;
                b.swap(i, j);
            }
            let (pa, pb) = (profile(&a), profile(&b));
            assert_eq!(pa.entries, pb.entries);
            assert_eq!(pa.rating, pb.rating);
        });
    }

    #[test]
    fn trivial_correct_decisions_change_nothing() {
        cases(100, |rng| {
            let a = history(rng, 6);
            let mut b = a.clone();
            for k in 0..(rng.next() % 2000) {
                let t = a[(rng.next() % a.len() as u64) as usize].clone();
                let chips = 60.0 + rng.f() * 100.0;
                // Pot below the trivial share of the stack, nothing lost.
                let mut m = mv(100_000 + k, t.tournament_id.unwrap(), 0, rng.pick(&SOURCES), 0.0, chips * TRIVIAL_POT * rng.f(), chips);
                m.played_at = t.played_at.clone();
                b.push(m);
            }
            assert_eq!(ratings(&a), ratings(&b));
            // A tournament of trivial folds alone adds no counted entry either.
            let mut c = a.clone();
            c.push(mv(900_000, 77, 300, Source::Model, 0.0, 1.5, 100.0));
            assert_eq!(final_rating(&a), final_rating(&c));
        });
    }

    #[test]
    fn unrated_pending_and_forced_decisions_change_nothing() {
        cases(100, |rng| {
            let a = history(rng, 6);
            let mut b = a.clone();
            for (k, status) in [Status::Unrated, Status::Pending, Status::Forced].into_iter().enumerate() {
                let mut m = a[(rng.next() % a.len() as u64) as usize].clone();
                m.hand_id = 500_000 + k as u64;
                m.status = status;
                m.ev_loss_bb = if status == Status::Unrated { Some(50.0) } else { None };
                m.source = Some(if status == Status::Forced { Source::Forced } else { Source::Solver });
                b.push(m);
            }
            assert_eq!(profile(&a).entries, profile(&b).entries);
        });
    }

    #[test]
    fn an_estimate_weighs_less_than_a_computed_verdict() {
        let at = |source| {
            let mut v = vec![mv(1, 1, 0, Source::Model, 0.0, 20.0, 40.0); 10];
            v.push(mv(2, 1, 0, source, 8.0, 20.0, 40.0));
            final_rating(&v)
        };
        assert!(at(Source::Estimate) > at(Source::Solver));
    }

    #[test]
    fn early_tournaments_move_more_than_later_ones() {
        let mut rng = Rng(3);
        let p = profile(&history(&mut rng, 40));
        let counted: Vec<&Entry> = p.entries.iter().filter(|e| e.performance.is_some()).collect();
        let mean = |es: &[&Entry]| es.iter().map(|e| e.delta.abs()).sum::<f64>() / es.len() as f64;
        assert!(mean(&counted[..5]) > 2.0 * mean(&counted[30..]));
        assert!(p.sigma < 110.0 && !p.provisional);
    }

    #[test]
    fn no_history_has_no_rating() {
        let p = profile(&[]);
        assert_eq!(p.rating, None);
        assert!(p.entries.is_empty() && p.areas.is_empty() && p.moments.is_empty());
    }

    #[test]
    fn tiers_cover_the_scale() {
        assert_eq!(tier(FLOOR), "Einsteiger");
        assert_eq!(tier(START), "Auf dem Weg");
        assert_eq!(tier(REF_RATING), "Profi-Niveau");
        assert!(TIERS.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn the_latest_tournament_names_what_moved_it() {
        let t = 5;
        let mut v: Vec<Move> = (0..20).map(|i| mv(i, t, 0, Source::Model, 0.0, 6.0, 30.0)).collect();
        let mut blunder = mv(100, t, 0, Source::Equilibrium, 12.0, 30.0, 30.0);
        blunder.spot = SpotType::CallShove;
        let mut unlucky = mv(101, t, 0, Source::Equilibrium, 0.0, 30.0, 30.0);
        unlucky.spot = SpotType::CallShove;
        unlucky.hero_net_bb = Some(-30.0);
        v.extend([blunder, unlucky]);
        let mut folded = mv(102, t, 0, Source::Equilibrium, 0.0, 30.0, 30.0);
        folded.spot = SpotType::CallShove;
        folded.played = "Fold".into();
        folded.hero_net_bb = Some(-1.0);
        v.push(folded);
        let p = profile(&v);
        assert!(p.moments.iter().filter(|m| m.hand_id == 102).all(|m| !m.bad_luck), "a fold loses no chips by luck");
        assert_eq!(p.moments[0].hand_id, 100);
        assert!(p.moments[0].points < 0.0 && p.moments[0].area == Area::VsAllIn);
        let luck = p.moments.iter().find(|m| m.hand_id == 101).expect("the good call is named");
        assert!(luck.bad_luck && luck.points > 0.0);
    }

    #[test]
    fn a_change_compares_with_the_rating_before() {
        let mut rng = Rng(11);
        let moves = history(&mut rng, 8);
        let split = moves[moves.len() / 2].hand_id;
        let c = change(&moves, |m| m.hand_id < split).unwrap();
        let before: Vec<Move> = moves.iter().filter(|m| m.hand_id < split).cloned().collect();
        assert_eq!(c.rating, final_rating(&moves));
        assert_eq!(c.delta, Some(final_rating(&moves) - final_rating(&before)));
        assert_eq!(change(&moves, |_| false).unwrap().delta, None);
        assert_eq!(change(&[], |_| true), None);
    }

    #[test]
    fn areas_follow_the_situation() {
        let mut m = mv(1, 1, 0, Source::Model, 0.0, 3.0, 15.0);
        m.spot = SpotType::Steal;
        assert_eq!(Area::of(&m), Area::PushFold);
        m.stack_bb = Some(40.0);
        m.position = Some("SB".into());
        assert_eq!(Area::of(&m), Area::BlindVsBlind);
        m.spot = SpotType::VsOpen;
        m.position = Some("BB".into());
        assert_eq!(Area::of(&m), Area::BigBlind);
        for a in Area::ALL {
            assert!(!a.spots().is_empty());
        }
    }
}
