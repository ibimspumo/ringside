//! „Was als Nächstes?“: one ranked list of what to work on, built from every source that
//! knows something about the hero's game – kinds of spot of the playing profile, leaks of the
//! stats, the costliest single decisions, concepts from coach reports and due reviews. Every
//! page reads this list; a page with its own focus filters it ([`within`]) instead of
//! ranking again, so all pages name the same first step.
//!
//! An entry weighs what it costs in big blinds over all the times it came up (cost per time ×
//! times), times how far its source can be trusted, times how recent it is. Sources without a
//! cost in big blinds (coach concepts, reviews) get a small stand-in cost, so a clear loss in
//! the own hands always comes first.

use std::collections::HashSet;

use serde::Serialize;

use crate::db::{CostlyDecision, Source, SpotType, Status};
use crate::leaks::{BANDS, Check, Direction, LeakReport};
use crate::model::GameKind;
use crate::profile::{Group, Lage, Profile, Seat, SpotKind};
use crate::report::Practice as KindLoss;
use crate::stats::StatId;
use crate::words::{stat_id, word};

/// Days after which a loss counts half.
const HALF_LIFE_DAYS: f64 = 30.0;
/// An old loss never counts less than this share: habits do not vanish by themselves.
const MIN_FRESH: f64 = 0.25;
/// Agreement assumed for estimates whose reliability is unknown.
const UNKNOWN_RELIABILITY: f64 = 0.5;
/// A leak's cost compares results and does not prove the value is the cause: it weighs its
/// 95 % lower bound, a quarter of it, times the confidence that the value is off.
const LEAK_TRUST: f64 = 0.25;
/// Stand-in cost of a leak without a clear cost, per unit of its score (confidence × distance).
const LEAK_PLAIN_BB: f64 = 1.0;
const LEAK_SCORE_CAP: f64 = 3.0;
/// One decision is no habit: its loss counts this much.
const ONE_OFF: f64 = 0.35;
/// Single decisions below this loss are no step of their own.
const MIN_DECISION_BB: f64 = 1.0;
/// How many single decisions the list takes at most.
const MAX_DECISIONS: usize = 3;
/// Stand-in costs of a coach concept per mistake and per questionable decision.
const CONCEPT_MISTAKE_BB: f64 = 0.75;
const CONCEPT_QUESTION_BB: f64 = 0.35;
/// The coach is a language model's judgement.
const COACH_TRUST: f64 = 0.4;
/// Stand-in costs of a due review, and of one whose last answers were mostly wrong.
const REVIEW_BB: f64 = 0.4;
const REVIEW_SHAKY_BB: f64 = 0.8;
/// Below this mean score a skill counts as shaky.
const SHAKY: f64 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// A kind of spot of the playing profile.
    Spot,
    /// A stat outside its reference range.
    Leak,
    /// One costly decision.
    Decision,
    /// A concept the coach found shaky.
    Concept,
    /// An exercise due for review.
    Review,
}

/// Where „Üben“ or „Nachlesen“ leads, in the shape of the UI's learn targets.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Target {
    Spot { spot: String },
    Skill { kind: String, level: u8 },
    Article { article: String },
}

/// Which hands „Hände ansehen“ opens.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum Hands {
    /// The hands of a kind of spot of the playing profile.
    Spot { key: String, label: String },
    /// One decision in the replayer.
    Decision { hand_id: u64, action_index: usize },
    /// The hands behind a stat's value.
    Stat { stat: StatId, made: Option<bool>, positions: Vec<&'static str>, band: u8, game: GameKind, label: String },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NextStep {
    /// Stable over loads: "spot:vs_open|BB|1|9", "leak:steal|tournament|BTN|1", …
    pub id: String,
    pub kind: StepKind,
    /// Plain German: „Gegen eine Erhöhung im Big Blind“.
    pub title: String,
    /// One sentence why.
    pub why: String,
    /// The number with its meaning: „kostet dich im Schnitt etwa 0,4 BB pro Mal, 31-mal gespielt“.
    pub number: String,
    /// Glossary id of the poker word in the title, for its explanation.
    pub term: Option<&'static str>,
    pub per_time_bb: Option<f64>,
    pub times: Option<usize>,
    /// Cost over all times, BB; `None` for sources without a cost in BB.
    pub total_bb: Option<f64>,
    /// How far the source can be trusted, 0–1.
    pub trust: f64,
    /// How recent, 0–1.
    pub fresh: f64,
    /// The number rests on an estimate („geschätzt“), not on a computed solution.
    pub estimate: bool,
    /// The weight the list is sorted by.
    pub score: f64,
    /// Key of the playing profile's kind of spot, for a spot.
    pub spot_key: Option<String>,
    /// Spot types of the analysis it is about, for the hand list's filter.
    pub spots: Vec<SpotType>,
    /// The hand, for a single decision.
    pub hand_id: Option<u64>,
    pub practice: Option<Target>,
    pub hands: Option<Hands>,
    /// Article id in „Wissen“.
    pub read: Option<&'static str>,
}

/// A concept from the stored coach reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Concept {
    pub id: String,
    pub name: String,
    /// Exercise kind that trains it, with the level to practise next.
    pub exercise: Option<(String, u8)>,
    pub article: Option<&'static str>,
    pub mistakes: usize,
    pub questionable: usize,
    pub tournaments: usize,
}

/// An exercise due for review.
#[derive(Clone, Debug, PartialEq)]
pub struct Review {
    pub kind: String,
    pub name: String,
    /// Level to practise next.
    pub level: u8,
    /// Days since the last attempt.
    pub days: f64,
    pub mean_score: Option<f64>,
}

pub struct Inputs<'a> {
    pub profile: &'a Profile,
    pub leaks: &'a LeakReport,
    pub costliest: &'a [CostlyDecision],
    pub concepts: &'a [Concept],
    pub reviews: &'a [Review],
}

/// The ranked list, heaviest first. Equal input gives an equal list in any order of the input.
pub fn rank(input: &Inputs) -> Vec<NextStep> {
    // Recency counts from the newest hand of the data, not from the clock, so the list only
    // changes when the data does.
    let now = input
        .profile
        .groups
        .iter()
        .map(|g| g.last_played_at.as_str())
        .chain(input.costliest.iter().map(|d| d.played_at.as_str()))
        .filter_map(day_number)
        .max();
    let mut out: Vec<NextStep> = Vec::new();
    out.extend(input.profile.groups.iter().filter(|g| g.rank.is_some()).map(|g| spot(g, now)));
    out.extend(input.leaks.leaks.iter().filter(|c| !c.uncertain && c.direction.is_some()).map(leak));
    let mut decisions: Vec<NextStep> = input.costliest.iter().filter_map(|d| decision(d, now)).collect();
    sort(&mut decisions);
    out.extend(decisions.into_iter().take(MAX_DECISIONS));
    out.extend(input.concepts.iter().filter_map(concept));
    let trained: HashSet<&str> = input.concepts.iter().filter_map(|c| c.exercise.as_ref().map(|e| e.0.as_str())).collect();
    out.extend(input.reviews.iter().filter(|r| !trained.contains(r.kind.as_str())).map(review));
    sort(&mut out);
    out
}

/// The list as a page about one part of the history shows it (a tournament's report, a
/// session's review): the same entries in the same order, kept when they happened in that part,
/// then its kinds of spot that are too rare in the whole list to be ranked there. `kinds` is the
/// part's loss per kind of spot, costliest first; `hands` its hand ids; `part` names it in the
/// number, e.g. „in diesem Turnier“.
pub fn within(list: &[NextStep], kinds: &[KindLoss], hands: &HashSet<u64>, part: &str) -> Vec<NextStep> {
    let lost: Vec<&KindLoss> = kinds.iter().filter(|k| k.loss_bb > 0.0).collect();
    let keys: HashSet<&str> = lost.iter().map(|k| k.key.as_str()).collect();
    let mut out: Vec<NextStep> = list
        .iter()
        .filter(|s| match s.kind {
            StepKind::Spot => s.spot_key.as_deref().is_some_and(|k| keys.contains(k)),
            StepKind::Decision => s.hand_id.is_some_and(|h| hands.contains(&h)),
            _ => false,
        })
        .cloned()
        .collect();
    let listed: HashSet<String> = out.iter().filter_map(|s| s.spot_key.clone()).collect();
    out.extend(lost.into_iter().filter(|k| !listed.contains(&k.key)).filter_map(|k| rare_kind(k, part)));
    out
}

/// A part of the history's loss per kind of spot, costliest first, from its playing profile.
pub fn kind_losses(profile: &Profile) -> Vec<KindLoss> {
    let mut kinds: Vec<KindLoss> = profile
        .groups
        .iter()
        .filter(|g| g.loss_bb > 0.0)
        .map(|g| KindLoss {
            key: g.key.clone(),
            title: g.title.clone(),
            detail: g.detail.clone(),
            lage: g.lage,
            decisions: g.decisions,
            flawed: g.flawed,
            loss_bb: g.loss_bb,
        })
        .collect();
    kinds.sort_by(|a, b| b.loss_bb.total_cmp(&a.loss_bb).then(a.key.cmp(&b.key)));
    kinds
}

fn sort(list: &mut [NextStep]) {
    list.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.kind.cmp(&b.kind)).then(a.id.cmp(&b.id)));
}

/// Days since 1970 of "YYYY/MM/DD …" (the hand history's format).
pub(crate) fn day_number(played_at: &str) -> Option<i64> {
    let date = played_at.split_whitespace().next()?;
    let mut parts = date.split(['/', '-']).map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn freshness(last: &str, now: Option<i64>) -> f64 {
    match (day_number(last), now) {
        (Some(last), Some(now)) => 0.5f64.powf((now - last).max(0) as f64 / HALF_LIFE_DAYS).max(MIN_FRESH),
        _ => 1.0,
    }
}

fn num(x: f64, digits: usize) -> String {
    format!("{x:.digits$}").replace('.', ",")
}

/// "0,4 BB": one decimal from 0,1 on, two below (as the UI's `bbText`).
fn bb(x: f64) -> String {
    let v = x.abs();
    format!("{} BB", num(v, if v >= 0.095 || v == 0.0 { 1 } else { 2 }))
}

fn times(n: usize, what: &str) -> String {
    if n == 1 { format!("einmal {what}") } else { format!("{n}-mal {what}") }
}

/// "3 von 40 deiner Entscheidungen hier waren ungenau oder schlechter (…)."
fn flawed_line(flawed: usize, decisions: usize, detail: &str) -> String {
    let verb = if flawed == 1 { "war" } else { "waren" };
    format!("{flawed} von {decisions} deiner Entscheidungen hier {verb} ungenau oder schlechter ({detail}).")
}

fn cost_line(mean_bb: f64, n: usize, what: &str) -> String {
    if mean_bb < 0.005 {
        format!("kostet dich so gut wie nichts, {}", times(n, what))
    } else {
        format!("kostet dich im Schnitt etwa {} pro Mal, {}", bb(mean_bb), times(n, what))
    }
}

fn lage_article(lage: Lage, seat: Option<Seat>) -> &'static str {
    match lage {
        Lage::FirstIn if matches!(seat, Some(Seat::Cutoff | Seat::Button | Seat::SmallBlind)) => "steal",
        Lage::FirstIn => "pfr",
        Lage::PushFold | Lage::VsAllIn => "push-fold",
        Lage::VsLimp | Lage::VsOpen => "range",
        Lage::VsThreeBet => "fold-three-bet",
        Lage::CBet => "cbet",
        Lage::VsCBet => "fold-cbet",
        Lage::FlopBet | Lage::FlopVsBet => "pot-odds",
        Lage::TurnBet | Lage::TurnVsBet => "equity",
        Lage::RiverBet | Lage::RiverVsBet => "value-bluff",
    }
}

fn lage_term(lage: Lage) -> Option<&'static str> {
    match lage {
        Lage::VsThreeBet => Some("three_bet"),
        Lage::CBet | Lage::VsCBet => Some("cbet"),
        _ => None,
    }
}

fn spot_article(spot: SpotType) -> &'static str {
    match spot {
        SpotType::Open => "pfr",
        SpotType::Steal => "steal",
        SpotType::ThreeBet => "three-bet",
        SpotType::VsOpen | SpotType::VsLimp => "range",
        SpotType::VsThreeBet => "fold-three-bet",
        SpotType::Shove | SpotType::CallShove => "push-fold",
        SpotType::CBet => "cbet",
        SpotType::VsCBet => "fold-cbet",
        SpotType::Flop => "pot-odds",
        SpotType::Turn => "equity",
        SpotType::River => "value-bluff",
    }
}

fn stat_article(stat: StatId) -> Option<&'static str> {
    Some(match stat {
        StatId::Vpip => "vpip",
        StatId::Pfr => "pfr",
        StatId::ThreeBet => "three-bet",
        StatId::FoldToThreeBet => "fold-three-bet",
        StatId::Steal => "steal",
        StatId::FoldToSteal => "fold-steal",
        StatId::Cbet => "cbet",
        StatId::FoldToCbet => "fold-cbet",
        StatId::Wtsd => "wtsd",
        StatId::Wsd => "wsd",
        StatId::Af => "af",
        StatId::Hands | StatId::Afq => return None,
    })
}

/// The exercise that trains the decision behind a leak, where Ringside has one.
fn stat_exercise(c: &Check) -> Option<Target> {
    let short = c.game == GameKind::Tournament && c.band <= 1;
    let (kind, level) = match c.stat {
        StatId::Steal | StatId::Pfr | StatId::Vpip if short => ("push_fold_open", 1),
        StatId::FoldToSteal if short => ("push_fold_call", 1),
        StatId::FoldToCbet | StatId::Wtsd => ("pot_odds", 3),
        _ => return None,
    };
    Some(Target::Skill { kind: kind.into(), level })
}

fn spot(g: &Group, now: Option<i64>) -> NextStep {
    let e = (g.estimate_loss_pct / 100.0).clamp(0.0, 1.0);
    let trust = 1.0 - e + e * g.estimate_reliability.unwrap_or(UNKNOWN_RELIABILITY);
    let fresh = freshness(&g.last_played_at, now);
    let band = g.detail.split(" · ").next().unwrap_or(&g.detail);
    NextStep {
        id: format!("spot:{}", g.key),
        kind: StepKind::Spot,
        title: g.title.clone(),
        why: flawed_line(g.flawed, g.decisions, &g.detail),
        number: cost_line(g.mean_loss_bb, g.decisions, "gespielt"),
        term: lage_term(g.lage),
        per_time_bb: Some(g.mean_loss_bb),
        times: Some(g.decisions),
        total_bb: Some(g.loss_bb),
        trust,
        fresh,
        estimate: g.unsure,
        score: g.loss_bb * trust * fresh,
        spot_key: Some(g.key.clone()),
        spots: g.lage.spots().to_vec(),
        hand_id: None,
        practice: Some(Target::Spot { spot: g.key.clone() }),
        hands: Some(Hands::Spot { key: g.key.clone(), label: format!("{}, {band}", g.title) }),
        read: Some(lage_article(g.lage, g.seat)),
    }
}

/// A deviation as a sentence about your play, so a casual player reads it without the stat's name.
fn plain_leak(stat: StatId, high: bool) -> &'static str {
    match (stat, high) {
        (StatId::Vpip, true) => "Du spielst zu viele Hände",
        (StatId::Vpip, false) => "Du spielst zu wenige Hände",
        (StatId::Pfr, true) => "Du erhöhst vor dem Flop zu oft",
        (StatId::Pfr, false) => "Du erhöhst vor dem Flop zu selten",
        (StatId::ThreeBet, true) => "Du erhöhst zu oft noch einmal (3-Bet)",
        (StatId::ThreeBet, false) => "Du erhöhst zu selten noch einmal (3-Bet)",
        (StatId::FoldToThreeBet, true) => "Du gibst nach einer 3-Bet zu oft auf",
        (StatId::FoldToThreeBet, false) => "Du gibst nach einer 3-Bet zu selten auf",
        (StatId::Steal, true) => "Du greifst die Blinds zu oft an",
        (StatId::Steal, false) => "Du greifst die Blinds zu selten an",
        (StatId::FoldToSteal, true) => "Du gibst deine Blinds zu oft her",
        (StatId::FoldToSteal, false) => "Du verteidigst deine Blinds zu oft",
        (StatId::Cbet, true) => "Du setzt am Flop zu oft nach",
        (StatId::Cbet, false) => "Du setzt am Flop zu selten nach",
        (StatId::FoldToCbet, true) => "Du gibst am Flop zu oft auf",
        (StatId::FoldToCbet, false) => "Du gibst am Flop zu selten auf",
        (StatId::Wtsd, true) => "Du gehst zu oft bis zum Showdown",
        (StatId::Wtsd, false) => "Du gehst zu selten bis zum Showdown",
        (StatId::Wsd, true) => "Du gewinnst am Showdown auffällig oft",
        (StatId::Wsd, false) => "Du verlierst am Showdown zu oft",
        (StatId::Af | StatId::Afq, true) => "Du spielst nach dem Flop zu aggressiv",
        (StatId::Af | StatId::Afq, false) => "Du spielst nach dem Flop zu passiv",
        (StatId::Hands, _) => "Zu wenige Hände",
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// "am Button", "im Small Blind und im Big Blind", or "" for all seats.
fn seats(positions: &[&str]) -> String {
    let mut phrases: Vec<&str> = positions.iter().filter_map(|p| Seat::of(p)).map(Seat::phrase).collect();
    phrases.dedup();
    phrases.join(" und ")
}

fn leak(c: &Check) -> NextStep {
    let id = stat_id(c.stat);
    let high = c.direction == Some(Direction::TooHigh);
    let said = plain_leak(c.stat, high);
    let at = seats(&c.positions);
    let title = if at.is_empty() { said.to_string() } else { format!("{}: {said}", capitalize(&at)) };
    let ratio = c.stat == StatId::Af;
    let value = |x: f64| if ratio { num(x, 1) } else { format!("{} %", num(x, 0)) };
    let range = if ratio { format!("{}–{}", num(c.low, 1), num(c.high, 1)) } else { format!("{}–{} %", num(c.low, 0), num(c.high, 0)) };
    let game = if c.game == GameKind::Tournament { "Turnier" } else { "Cash" };
    let band = BANDS[c.band as usize];
    let why = format!("Du liegst bei {}, üblich sind {range} ({band}, {game}).", value(c.value));
    let trust = LEAK_TRUST * c.confidence;
    let (number, per_time, times, total, base) = match &c.cost {
        Some(cost) => {
            let per = cost.total_bb / cost.hands.max(1) as f64;
            let line = format!("kostet dich im Schnitt etwa {} pro Hand, {} Hände in dieser Lage", bb(per), cost.hands);
            let sure_gap = (cost.base_bb100 - cost.spot_bb100 - cost.margin_bb100).max(0.0);
            (line, Some(per), Some(cost.hands as usize), Some(cost.total_bb), sure_gap * cost.hands as f64 / 100.0)
        }
        None => {
            let line = format!("{} Gelegenheiten, zu {} % sicher", c.opp, num((c.confidence * 100.0).min(99.0), 0));
            (line, None, Some(c.opp as usize), None, LEAK_PLAIN_BB * c.score.min(LEAK_SCORE_CAP))
        }
    };
    let made = if c.stat == StatId::Af { None } else { Some(high) };
    let where_label = if c.positions.is_empty() { "alle".to_string() } else { c.positions.join(", ") };
    NextStep {
        id: format!("leak:{id}|{}|{}|{}", c.game.as_str(), c.positions.join(","), c.band),
        kind: StepKind::Leak,
        title,
        why,
        number,
        term: Some(id),
        per_time_bb: per_time,
        times,
        total_bb: total,
        trust,
        fresh: 1.0,
        estimate: true,
        score: base * trust,
        spot_key: None,
        spots: vec![],
        hand_id: None,
        practice: stat_exercise(c),
        hands: Some(Hands::Stat {
            stat: c.stat,
            made,
            positions: c.positions.clone(),
            band: c.band,
            game: c.game,
            label: format!("{} · {where_label} · {band}", word(id)),
        }),
        read: stat_article(c.stat),
    }
}

/// The first sentence of a text.
fn first_sentence(text: &str) -> &str {
    let t = text.trim();
    match t.find(". ") {
        Some(i) => &t[..=i],
        None => t,
    }
}

fn decision(d: &CostlyDecision, now: Option<i64>) -> Option<NextStep> {
    let b = &d.decision;
    let loss = b.ev_loss_bb.filter(|l| *l >= MIN_DECISION_BB)?;
    let best = b.best.as_deref().filter(|_| b.status == Status::Rated)?;
    let estimate = b.source == Some(Source::Estimate);
    let trust = ONE_OFF * if estimate { b.reliability.unwrap_or(UNKNOWN_RELIABILITY) } else { 1.0 };
    let fresh = freshness(&d.played_at, now);
    let why = match first_sentence(&b.why) {
        "" => "Diese eine Entscheidung hat besonders viel gekostet.".to_string(),
        s => s.to_string(),
    };
    Some(NextStep {
        id: format!("decision:{}:{}", d.hand_id, b.action_index),
        kind: StepKind::Decision,
        title: format!("Teure Hand: {} statt {best}", b.played),
        why,
        number: format!("hat dich etwa {} gekostet, eine einzelne Hand", bb(loss)),
        term: None,
        per_time_bb: Some(loss),
        times: Some(1),
        total_bb: Some(loss),
        trust,
        fresh,
        estimate,
        score: loss * trust * fresh,
        spot_key: None,
        spots: vec![b.spot],
        hand_id: Some(d.hand_id),
        practice: None,
        hands: Some(Hands::Decision { hand_id: d.hand_id, action_index: b.action_index }),
        read: Some(spot_article(b.spot)),
    })
}

fn concept(c: &Concept) -> Option<NextStep> {
    let base = c.mistakes as f64 * CONCEPT_MISTAKE_BB + c.questionable as f64 * CONCEPT_QUESTION_BB;
    if base <= 0.0 {
        return None;
    }
    let mut parts = Vec::new();
    if c.mistakes > 0 {
        parts.push(format!("{} Fehler", c.mistakes));
    }
    if c.questionable > 0 {
        let what = if c.questionable == 1 { "fragliche Entscheidung" } else { "fragliche Entscheidungen" };
        parts.push(format!("{} {what}", c.questionable));
    }
    let tournaments = if c.tournaments == 1 { "einem Turnier".to_string() } else { format!("{} Turnieren", c.tournaments) };
    Some(NextStep {
        id: format!("concept:{}", c.id),
        kind: StepKind::Concept,
        title: c.name.clone(),
        why: "Der KI-Coach hat das in deinen Turnier-Berichten als Schwäche angemerkt.".into(),
        number: format!("{} in {tournaments}, laut KI-Coach", parts.join(" und ")),
        term: None,
        per_time_bb: None,
        times: Some(c.mistakes + c.questionable),
        total_bb: None,
        trust: COACH_TRUST,
        fresh: 1.0,
        estimate: true,
        score: base * COACH_TRUST,
        spot_key: None,
        spots: vec![],
        hand_id: None,
        practice: c.exercise.as_ref().map(|(kind, level)| Target::Skill { kind: kind.clone(), level: *level }),
        hands: None,
        read: c.article,
    })
}

fn review(r: &Review) -> NextStep {
    let shaky = r.mean_score.is_some_and(|m| m < SHAKY);
    let pct = r.mean_score.map(|m| num(m * 100.0, 0));
    let why = match (&pct, shaky) {
        (Some(p), true) => format!("Zuletzt nur {p} % richtig – eine kurze Runde festigt es."),
        _ => {
            let days = r.days.floor().max(1.0) as i64;
            let ago = if days == 1 { "einem Tag".to_string() } else { format!("{days} Tagen") };
            format!("Zuletzt vor {ago} geübt – eine kurze Runde, damit es sitzt.")
        }
    };
    let base = if shaky { REVIEW_SHAKY_BB } else { REVIEW_BB };
    NextStep {
        id: format!("review:{}", r.kind),
        kind: StepKind::Review,
        title: format!("Wiederholen: {}", r.name),
        why,
        number: pct.map_or_else(|| "fällig zum Wiederholen".to_string(), |p| format!("{p} % richtig in den letzten Übungen")),
        term: None,
        per_time_bb: None,
        times: None,
        total_bb: None,
        trust: 1.0,
        fresh: 1.0,
        estimate: false,
        score: base,
        spot_key: None,
        spots: vec![],
        hand_id: None,
        practice: Some(Target::Skill { kind: r.kind.clone(), level: r.level }),
        hands: None,
        read: None,
    }
}

/// A kind of spot of one tournament that the whole history does not rank.
fn rare_kind(k: &KindLoss, part: &str) -> Option<NextStep> {
    let kind = SpotKind::parse(&k.key)?;
    let mean = k.loss_bb / k.decisions.max(1) as f64;
    let band = k.detail.split(" · ").next().unwrap_or(&k.detail);
    Some(NextStep {
        id: format!("spot:{}", k.key),
        kind: StepKind::Spot,
        title: k.title.clone(),
        why: flawed_line(k.flawed, k.decisions, &k.detail),
        number: cost_line(mean, k.decisions, part),
        term: lage_term(kind.lage),
        per_time_bb: Some(mean),
        times: Some(k.decisions),
        total_bb: Some(k.loss_bb),
        trust: 1.0,
        fresh: 1.0,
        estimate: false,
        score: k.loss_bb,
        spot_key: Some(k.key.clone()),
        spots: kind.lage.spots().to_vec(),
        hand_id: None,
        practice: Some(Target::Spot { spot: k.key.clone() }),
        hands: Some(Hands::Spot { key: k.key.clone(), label: format!("{}, {band}", k.title) }),
        read: Some(lage_article(kind.lage, kind.seat)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_flawed_decision_is_singular() {
        assert_eq!(flawed_line(1, 16, "ab 40 BB"), "1 von 16 deiner Entscheidungen hier war ungenau oder schlechter (ab 40 BB).");
        assert!(flawed_line(3, 16, "ab 40 BB").contains(" hier waren "));
    }
    use crate::db::{DecisionBrief, Rating};
    use crate::leaks::{Cost, SOURCES};
    use crate::model::Street;

    fn group(key: &str, decisions: usize, loss_bb: f64, last: &str) -> Group {
        let kind = SpotKind::parse(key).expect("key");
        Group {
            key: key.into(),
            lage: kind.lage,
            seat: kind.seat,
            band: kind.band,
            table: kind.table,
            title: kind.title(),
            detail: kind.detail(),
            decisions,
            flawed: decisions / 3,
            error_pct: 33.0,
            loss_bb,
            mean_loss_bb: loss_bb / decisions as f64,
            open: 0,
            sources: vec![(Source::Model, decisions)],
            computed_pct: 100.0,
            estimate_loss_pct: 0.0,
            estimate_reliability: None,
            unsure: false,
            few: false,
            rank: Some(1),
            last_played_at: last.into(),
            change: None,
        }
    }

    fn estimated(mut g: Group, reliability: f64) -> Group {
        g.estimate_loss_pct = 100.0;
        g.estimate_reliability = Some(reliability);
        g.unsure = true;
        g
    }

    fn leak_check(cost: Option<f64>) -> Check {
        Check {
            stat: StatId::Steal,
            game: GameKind::Tournament,
            positions: vec!["BTN"],
            band: 1,
            made: 70,
            opp: 100,
            value: 70.0,
            ci_low: 60.0,
            ci_high: 78.0,
            low: 33.0,
            high: 48.0,
            direction: Some(Direction::TooHigh),
            deviation: 22.0,
            confidence: 0.99,
            uncertain: false,
            score: 1.4,
            cost: cost.map(|margin_bb100| Cost { hands: 400, spot_bb100: 5.0, base_bb100: 8.0, margin_bb100, total_bb: 12.0 }),
            cites: vec![],
            note: None,
        }
    }

    fn costly(hand_id: u64, loss: f64, played_at: &str) -> CostlyDecision {
        CostlyDecision {
            hand_id,
            played_at: played_at.into(),
            hero_cards: "Ah Kd".into(),
            position: Some("BB".into()),
            decision: DecisionBrief {
                action_index: 4,
                street: Street::Preflop,
                spot: SpotType::CallShove,
                status: Status::Rated,
                played: "Fold".into(),
                best: Some("Call".into()),
                ev_loss_bb: Some(loss),
                rating: Some(Rating::Blunder),
                source: Some(Source::Equilibrium),
                reason: None,
                why: "Gegen diesen All-in liegt deine Hand vorne. Ein Fold verschenkt das.".into(),
                pot_bb: Some(20.0),
                reliability: None,
            },
        }
    }

    fn leaks(list: Vec<Check>) -> LeakReport {
        LeakReport { leaks: list, fine: vec![], too_small: 0, bands: BANDS, sources: SOURCES }
    }

    fn profile(groups: Vec<Group>) -> Profile {
        Profile { decisions: groups.iter().map(|g| g.decisions).sum(), loss_bb: 0.0, open: 0, min_decisions: 10, groups }
    }

    struct Data {
        profile: Profile,
        leaks: LeakReport,
        costliest: Vec<CostlyDecision>,
        concepts: Vec<Concept>,
        reviews: Vec<Review>,
    }

    impl Data {
        fn rank(&self) -> Vec<NextStep> {
            rank(&Inputs {
                profile: &self.profile,
                leaks: &self.leaks,
                costliest: &self.costliest,
                concepts: &self.concepts,
                reviews: &self.reviews,
            })
        }
    }

    fn only(groups: Vec<Group>) -> Data {
        Data { profile: profile(groups), leaks: leaks(vec![]), costliest: vec![], concepts: vec![], reviews: vec![] }
    }

    fn sample() -> Data {
        Data {
            profile: profile(vec![
                group("vs_open|BB|1|9", 31, 12.4, "2026/11/17 21:00:00"),
                estimated(group("river_vs_bet|-|3|6", 23, 14.6, "2026/11/17 22:00:00"), 0.73),
                group("push_fold|BTN|0|9", 40, 6.0, "2026/11/10 20:00:00"),
            ]),
            leaks: leaks(vec![leak_check(Some(2.0)), Check { positions: vec!["CO"], ..leak_check(None) }]),
            costliest: vec![costly(7, 9.0, "2026/11/17 20:30:00"), costly(8, 0.5, "2026/11/17 20:40:00")],
            concepts: vec![Concept {
                id: "pot_odds".into(),
                name: "Pot Odds".into(),
                exercise: Some(("pot_odds".into(), 2)),
                article: Some("pot-odds"),
                mistakes: 2,
                questionable: 1,
                tournaments: 2,
            }],
            reviews: vec![
                Review { kind: "outs".into(), name: "Outs zählen".into(), level: 2, days: 4.0, mean_score: Some(0.8) },
                Review { kind: "pot_odds".into(), name: "Pot Odds".into(), level: 2, days: 4.0, mean_score: Some(0.4) },
            ],
        }
    }

    #[test]
    fn the_list_is_the_same_in_any_order_of_the_input() {
        let a = sample();
        let mut b = sample();
        b.profile.groups.reverse();
        b.leaks.leaks.reverse();
        b.costliest.reverse();
        b.reviews.reverse();
        let (x, y) = (a.rank(), b.rank());
        assert_eq!(x, y);
        assert_eq!(x, a.rank(), "twice the same");
        // Equal scores fall back to kind and id.
        let c = only(vec![group("vs_open|SB|1|9", 20, 5.0, "2026/11/17 21:00:00"), group("vs_open|BB|1|9", 20, 5.0, "2026/11/17 21:00:00")]);
        let ids: Vec<String> = c.rank().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["spot:vs_open|BB|1|9", "spot:vs_open|SB|1|9"]);
    }

    #[test]
    fn cost_frequency_trust_and_recency_weigh_in() {
        let list = sample().rank();
        let order: Vec<&str> = list.iter().map(|s| s.id.as_str()).collect();
        // 12,4 BB computed beats 14,6 BB of estimates trusted at 73 %.
        assert_eq!(order[0], "spot:vs_open|BB|1|9", "{order:?}");
        assert_eq!(order[1], "spot:river_vs_bet|-|3|6");
        let river = &list[1];
        assert!((river.score - 14.6 * 0.73).abs() < 1e-9 && river.estimate);
        // A single decision counts as a third of a habit; below 1 BB it is no step.
        assert!(order.contains(&"decision:7:4") && !order.contains(&"decision:8:4"));
        // The coach concept trains pot odds, so the due review of pot odds is folded into it.
        assert!(order.contains(&"concept:pot_odds") && !order.contains(&"review:pot_odds"));
        assert_eq!(order.last(), Some(&"leak:steal|tournament|CO|1"));
        // A leak's cost weighs the sure part of its gap: (8 − 5 − 2) BB/100 over 400 hands.
        let steal = list.iter().find(|s| s.id == "leak:steal|tournament|BTN|1").unwrap();
        assert!((steal.score - 4.0 * 0.25 * 0.99).abs() < 1e-9, "{}", steal.score);
        assert_eq!(steal.number, "kostet dich im Schnitt etwa 0,03 BB pro Hand, 400 Hände in dieser Lage");

        // Same mean loss, more times: heavier.
        let more = only(vec![group("vs_open|BB|1|9", 20, 4.0, "2026/11/17 21:00:00"), group("vs_open|SB|1|9", 40, 8.0, "2026/11/17 21:00:00")]);
        assert_eq!(more.rank()[0].id, "spot:vs_open|SB|1|9");

        // Same cost, but last seen 60 days before the newest hand: a quarter of the weight.
        let old = only(vec![group("vs_open|BB|1|9", 20, 8.0, "2026/09/18 21:00:00"), group("vs_open|SB|1|9", 20, 7.0, "2026/11/17 21:00:00")]);
        let ranked = old.rank();
        assert_eq!(ranked[0].id, "spot:vs_open|SB|1|9");
        let stale = ranked.iter().find(|s| s.id == "spot:vs_open|BB|1|9").unwrap();
        assert!((stale.fresh - 0.25).abs() < 1e-9, "{}", stale.fresh);
    }

    #[test]
    fn every_entry_says_what_its_number_means_and_where_to_go() {
        for s in sample().rank() {
            assert!(!s.title.is_empty() && s.why.ends_with('.'), "{s:?}");
            assert!(!s.number.is_empty(), "{s:?}");
            assert!(s.practice.is_some() || s.hands.is_some() || s.read.is_some(), "{s:?}");
        }
        let first = &sample().rank()[0];
        assert_eq!(first.title, "Gegen eine Erhöhung im Big Blind");
        assert_eq!(first.number, "kostet dich im Schnitt etwa 0,4 BB pro Mal, 31-mal gespielt");
        assert_eq!(first.practice, Some(Target::Spot { spot: "vs_open|BB|1|9".into() }));
        let steal = sample().rank().into_iter().find(|s| s.id == "leak:steal|tournament|BTN|1").unwrap();
        assert_eq!(steal.title, "Am Button: Du greifst die Blinds zu oft an");
        assert_eq!(steal.why, "Du liegst bei 70 %, üblich sind 33–48 % (10–20 BB, Turnier).");
        assert_eq!(steal.practice, Some(Target::Skill { kind: "push_fold_open".into(), level: 1 }));
    }

    #[test]
    fn a_tournament_keeps_the_order_of_the_whole_list() {
        let list = sample().rank();
        let kind = |key: &str, loss_bb: f64| {
            let k = SpotKind::parse(key).unwrap();
            KindLoss { key: key.into(), title: k.title(), detail: k.detail(), lage: k.lage, decisions: 3, flawed: 1, loss_bb }
        };
        // The tournament lost most in the river spot, but the whole history ranks vs_open first.
        let kinds = vec![kind("river_vs_bet|-|3|6", 5.0), kind("vs_open|BB|1|9", 1.0), kind("vs_limp|SB|3|6", 0.8), kind("c_bet|-|2|6", 0.0)];
        let hands: HashSet<u64> = [7].into();
        let t = within(&list, &kinds, &hands, "in diesem Turnier");
        let ids: Vec<&str> = t.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["spot:vs_open|BB|1|9", "spot:river_vs_bet|-|3|6", "decision:7:4", "spot:vs_limp|SB|3|6"]);
        assert_eq!(t[0], list[0], "the same first entry as every other page");
        assert_eq!(t[3].number, "kostet dich im Schnitt etwa 0,3 BB pro Mal, 3-mal in diesem Turnier");
        assert!(within(&list, &[], &HashSet::new(), "in diesem Turnier").is_empty());
    }

    #[test]
    fn a_session_keeps_the_first_entry_when_it_played_that_spot() {
        let list = sample().rank();
        // The session's own profile: few decisions per kind, so none of them is ranked there.
        let part = profile(vec![
            Group { rank: None, few: true, ..group("river_vs_bet|-|3|6", 4, 3.0, "2026/11/17 22:00:00") },
            Group { rank: None, few: true, ..group("vs_open|BB|1|9", 2, 0.4, "2026/11/17 21:00:00") },
            Group { rank: None, few: true, ..group("c_bet|-|2|6", 3, 0.0, "2026/11/17 21:00:00") },
        ]);
        let kinds = kind_losses(&part);
        assert_eq!(kinds.iter().map(|k| k.key.as_str()).collect::<Vec<_>>(), ["river_vs_bet|-|3|6", "vs_open|BB|1|9"]);
        let s = within(&list, &kinds, &HashSet::new(), "in dieser Session");
        assert_eq!(s[0], list[0], "the overview's first step");
        assert_eq!(s.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["spot:vs_open|BB|1|9", "spot:river_vs_bet|-|3|6"]);
    }

    /// Every article an entry links to exists in „Wissen“.
    #[test]
    fn linked_articles_exist() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/lib/learn/wissen/articles.ts");
        let ts = std::fs::read_to_string(path).expect("articles of the UI");
        let ids: HashSet<&str> =
            regex::Regex::new(r#"(?m)^\s*id: "([\w-]+)""#).unwrap().captures_iter(&ts).map(|c| c.get(1).unwrap().as_str()).collect();
        let mut linked: Vec<&str> = Lage::ALL
            .into_iter()
            .flat_map(|l| [lage_article(l, None), lage_article(l, Some(Seat::Button))])
            .chain(SpotType::ALL.into_iter().map(spot_article))
            .chain(StatId::ALL.into_iter().filter_map(stat_article))
            .collect();
        linked.dedup();
        for a in linked {
            assert!(ids.contains(a), "article „{a}“ is missing in articles.ts");
        }
    }

    #[test]
    fn days_count_from_the_hand_history_date() {
        assert_eq!(day_number("1970/01/01 00:00:00"), Some(0));
        assert_eq!(day_number("2026/11/18 10:00:00").unwrap() - day_number("2026/10/19 09:00:00").unwrap(), 30);
        assert_eq!(day_number("kaputt"), None);
    }
}
