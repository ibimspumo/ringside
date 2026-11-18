//! The session review: the hero's last block of play (tournaments side by side and one
//! after another, cash hands) and what it says about the preflop play and about luck.
//!
//! Invariants (see the tests):
//! - a tournament never spans two sessions; hands less than [`GAP_MINUTES`] apart share one;
//! - the share of hands played counts the entry decisions (the hero's first preflop
//!   decision of a hand, with folding possible) that were not folds;
//! - the model's share is the mean of its play frequency at exactly those decisions, so it
//!   is weighted by seat, stack, situation and cards as they came;
//! - the per-class gaps add up to the difference of both;
//! - below [`FEW`] compared hands there are neither patterns nor a verdict;
//! - a decision rated right that lost is bad luck and never a mistake; one rated wrong that
//!   won is good luck;
//! - while PokerStars runs, [`SessionReview::locked`] keeps results only.

use std::collections::BTreeMap;

use ringside_engine::HandClass;
use serde::Serialize;

use crate::db::{Rating, Source, Status};
use crate::model::Street;
use crate::profile::{Seat, kind_of};
use crate::ranges::History;
use crate::report::{Report, ReportDecision, margin};

/// A pause longer than this ends a session. Tournament breaks at PokerStars last five
/// minutes and registering for the next tournament takes a few more; half an hour without
/// a hand means the player got up.
pub const GAP_MINUTES: f64 = 30.0;
/// Below this many compared hands the review shows the numbers only.
pub const FEW: usize = 30;
/// From this many compared hands the share played is more than a rough picture.
pub const SOLID: usize = 300;
/// Earlier sessions offered for choice.
pub const LISTED_SESSIONS: usize = 6;
/// Patterns named, classes listed per direction, examples of luck kept per side.
const PATTERNS: usize = 3;
const CLASSES: usize = 5;
const SWINGS: usize = 10;
/// A pattern needs at least this many decisions and this many hands of difference.
const PATTERN_MIN_DECISIONS: usize = 8;
const PATTERN_MIN_GAP: f64 = 2.0;
/// ... and a difference this many standard deviations off what the model would play.
const PATTERN_Z: f64 = 2.0;
/// A wrong decision counts as luck from this cost on; below it winning says nothing.
const LUCK_MIN_BB: f64 = 0.5;
/// A class counts as too loose or too tight from this many hands of difference.
const CLASS_GAP: f64 = 0.5;

/// Session index per hand, counting from 0. `hands` in play order as (minutes since any
/// fixed point, tournament). A tournament keeps its session open until its last hand.
pub fn split(hands: &[(f64, Option<u64>)]) -> Vec<usize> {
    let mut last: BTreeMap<u64, f64> = BTreeMap::new();
    for &(t, id) in hands {
        if let Some(id) = id {
            let e = last.entry(id).or_insert(t);
            *e = e.max(t);
        }
    }
    let mut session = 0;
    let mut open_until: Option<f64> = None;
    hands
        .iter()
        .map(|&(t, id)| {
            let reach = id.and_then(|i| last.get(&i).copied()).unwrap_or(t).max(t);
            open_until = Some(match open_until {
                Some(end) if t - end <= GAP_MINUTES => end.max(reach),
                Some(_) => {
                    session += 1;
                    reach
                }
                None => reach,
            });
            session
        })
        .collect()
}

/// One session: first and last hand, as in the hand history.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SessionSpan {
    pub start: String,
    pub end: String,
    pub hands: usize,
    /// In order of their first hand.
    pub tournaments: Vec<u64>,
    pub cash_hands: usize,
}

/// What the range charts store about one preflop decision (see [`crate::ranges`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PreflopFact {
    pub hand_id: u64,
    pub action_index: usize,
    /// `HandClass::index`.
    pub class: u8,
    pub position: String,
    pub vs_position: Option<String>,
    pub history: History,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sample {
    /// Below [`FEW`]: numbers only.
    Few,
    /// A rough picture.
    Rough,
    Solid,
}

/// One hand class: how often the hero played it at entry and how often the model would.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassGap {
    pub class: u8,
    /// "A9o".
    pub name: String,
    pub hands: usize,
    pub played: usize,
    /// Hands the model would have played, the sum of its frequencies.
    pub model: f64,
    pub loss_bb: f64,
}

impl ClassGap {
    /// Above zero the hero played it more often than the model.
    pub fn gap(&self) -> f64 {
        self.played as f64 - self.model
    }
}

/// A situation where the hero plays clearly more or fewer hands than the model.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pattern {
    /// "Gegen eine Erhöhung aus früher Position gehst du zu oft mit – 9 von 12 Mal, besser etwa 3."
    pub text: String,
    /// The hero plays too many hands here (else too few).
    pub wide: bool,
    pub decisions: usize,
    pub played: usize,
    pub model: f64,
    pub loss_bb: f64,
    /// The situation for "Deine Ranges": the commonest seat and opponent of the pattern.
    pub history: History,
    pub position: Option<String>,
    pub vs_position: Option<String>,
    /// Key of the playing profile's commonest kind of spot here, for "Lernen".
    pub practice: Option<String>,
}

/// The preflop play at a glance.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Verdict {
    /// 0 strong … 3 many mistakes.
    pub level: u8,
    pub title: String,
    pub line: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PreflopReview {
    /// Hands with a preflop decision of the hero.
    pub hands: usize,
    /// Of those, hands whose entry decision the model or the equilibrium rates.
    pub compared: usize,
    /// Compared hands the hero played.
    pub played: usize,
    /// Compared hands the model would have played.
    pub model_played: f64,
    /// Compared hands rated by the push/fold equilibrium (the rest by the model).
    pub equilibrium: usize,
    pub you_pct: Option<f64>,
    pub model_pct: Option<f64>,
    /// Every class with a compared hand, by class index.
    pub classes: Vec<ClassGap>,
    /// Classes played more (`wide`) or less (`tight`) than the model, most important first.
    pub wide: Vec<u8>,
    pub tight: Vec<u8>,
    pub patterns: Vec<Pattern>,
    pub sample: Option<Sample>,
    /// Every compared preflop decision, entries and later ones.
    pub decisions: usize,
    /// Of those, rated "good" or better.
    pub good: usize,
    pub loss_bb: f64,
    pub verdict: Option<Verdict>,
}

/// A decision of the session that went the other way than it deserved.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Swing {
    pub hand_id: u64,
    pub action_index: usize,
    pub tournament_id: Option<u64>,
    pub hero_cards: String,
    pub position: Option<String>,
    pub street: Street,
    pub played: String,
    pub best: Option<String>,
    pub rating: Option<Rating>,
    pub source: Option<Source>,
    pub why: String,
    /// The hand's result, BB.
    pub net_bb: f64,
    /// For all-ins: the result the cards give on average, and the difference to it.
    pub ev_bb: Option<f64>,
    pub luck_bb: Option<f64>,
    /// What the decision cost against the best action, BB (bad decisions).
    pub loss_bb: f64,
    /// What the played action gains over the best other one, by the strategy's EVs.
    pub margin_bb: Option<f64>,
    pub alternative: Option<String>,
    /// Against the cards shown, once the money was in: the hero's share of the pots.
    pub equity_pct: Option<f64>,
    pub opponents: Vec<Vec<String>>,
    pub board: Vec<String>,
}

/// One hand of the session as luck needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct SwingHand {
    pub hand_id: u64,
    pub tournament_id: Option<u64>,
    pub hero_cards: String,
    pub position: Option<String>,
    pub net: i64,
    /// All-in expectation in chips, when the money went in before the river.
    pub ev: Option<i64>,
    pub big_blind: i64,
    pub showdown: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SessionReview {
    pub span: SessionSpan,
    /// Result, luck, analysis state; the advice in it is empty while locked.
    pub report: Report,
    pub preflop: PreflopReview,
    /// Right decisions that lost, strongest first.
    pub bad_luck: Vec<Swing>,
    /// Wrong decisions that won, costliest first.
    pub good_luck: Vec<Swing>,
    /// Advice is left out: PokerStars runs.
    pub locked: bool,
}

impl SessionReview {
    /// The review without anything that rates or advises: results, luck in chips and the
    /// share of hands played stay.
    pub fn locked(self) -> SessionReview {
        let p = self.preflop;
        let preflop = PreflopReview {
            hands: p.hands,
            compared: p.compared,
            played: p.played,
            you_pct: p.you_pct,
            sample: p.sample,
            ..Default::default()
        };
        let r = self.report;
        let report = Report {
            hands: r.hands,
            unanalyzed: r.unanalyzed,
            pending: r.pending,
            unrated: r.unrated,
            luck: r.luck,
            ..Default::default()
        };
        SessionReview { span: self.span, report, preflop, bad_luck: Vec::new(), good_luck: Vec::new(), locked: true }
    }
}

fn played_kind(d: &ReportDecision) -> Option<&str> {
    d.strategy.iter().find(|c| c.played).map(|c| c.kind.as_str())
}

/// The model's play frequency at a preflop decision it rates where folding was possible,
/// and whether the hero played.
fn compared(d: &ReportDecision) -> Option<(f64, bool)> {
    if d.street != Street::Preflop || d.row.status != Status::Rated {
        return None;
    }
    if !matches!(d.row.source, Some(Source::Model | Source::Equilibrium)) {
        return None;
    }
    if !d.strategy.iter().any(|c| c.kind == "fold") {
        return None;
    }
    let fold: f64 = d.strategy.iter().filter(|c| c.kind == "fold").map(|c| c.freq).sum();
    Some(((1.0 - fold).clamp(0.0, 1.0), played_kind(d)? != "fold"))
}

fn is_good(r: Option<Rating>) -> bool {
    r.is_some_and(|r| r <= Rating::Good)
}

fn is_flawed(r: Option<Rating>) -> bool {
    r.is_some_and(|r| r >= Rating::Inaccuracy)
}

fn seat_from(s: Seat) -> &'static str {
    match s {
        Seat::Early => "aus früher Position",
        Seat::Middle => "aus mittlerer Position",
        Seat::Cutoff => "vom Cutoff",
        Seat::Button => "vom Button",
        Seat::SmallBlind => "aus dem Small Blind",
        Seat::BigBlind => "aus dem Big Blind",
    }
}

fn pattern_text(history: History, seat: Option<Seat>, wide: bool, played: usize, n: usize, model: f64) -> String {
    let at = seat.map(Seat::phrase).map(|s| format!(" {s}")).unwrap_or_default();
    let from = seat.map(seat_from).map(|s| format!(" {s}")).unwrap_or_default();
    let what = match (history, wide) {
        (History::Unopened, true) => format!("Wenn alle vor dir folden, spielst du{at} zu viele Hände"),
        (History::Unopened, false) => format!("Wenn alle vor dir folden, spielst du{at} zu wenige Hände"),
        (History::Limped, true) => format!("Nach einem Limp spielst du{at} zu oft mit"),
        (History::Limped, false) => format!("Nach einem Limp spielst du{at} zu selten mit"),
        (History::VsOpen | History::Squeeze, true) => format!("Gegen eine Erhöhung{from} gehst du zu oft mit"),
        (History::VsOpen | History::Squeeze, false) => format!("Gegen eine Erhöhung{from} gibst du zu oft auf"),
        (History::VsThreeBet, true) => "Wird dein Erhöhen erneut erhöht, gehst du zu oft mit".to_string(),
        (History::VsThreeBet, false) => "Wird dein Erhöhen erneut erhöht, gibst du zu oft auf".to_string(),
        (History::VsShove, true) => format!("Gegen ein All-in{from} callst du zu oft"),
        (History::VsShove, false) => format!("Gegen ein All-in{from} callst du zu selten"),
    };
    let best = model.round() as usize;
    let note = if history == History::Unopened && seat == Some(Seat::Button) && !wide { " – dabei ist das die stärkste Position" } else { "" };
    format!("{what}{note}: {played} von {n} Mal, besser etwa {best}.")
}

/// Whose seat a situation is about: the opponent's when someone raised or shoved first.
fn pattern_seat(history: History, position: &str, vs: Option<&str>) -> Option<Seat> {
    match history {
        History::VsOpen | History::Squeeze | History::VsShove => vs.and_then(Seat::of),
        History::VsThreeBet => None,
        History::Unopened | History::Limped => Seat::of(position),
    }
}

fn most<K: Ord + Clone>(m: &BTreeMap<K, usize>) -> Option<K> {
    m.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map(|(k, _)| k.clone())
}

#[derive(Default)]
struct Group {
    n: usize,
    played: usize,
    model: f64,
    var: f64,
    loss: f64,
    positions: BTreeMap<String, usize>,
    vs: BTreeMap<String, usize>,
    practice: BTreeMap<String, usize>,
}

/// The preflop part of the review from the session's decisions and the stored preflop
/// facts (`hero_preflop`), joined by hand and action.
pub fn preflop_review(facts: &[PreflopFact], decisions: &[ReportDecision]) -> PreflopReview {
    let by_key: BTreeMap<(u64, usize), &ReportDecision> =
        decisions.iter().map(|d| ((d.row.hand_id, d.row.action_index), d)).collect();
    let mut entry: BTreeMap<u64, &PreflopFact> = BTreeMap::new();
    for f in facts {
        let e = entry.entry(f.hand_id).or_insert(f);
        if f.action_index < e.action_index {
            *e = f;
        }
    }
    let mut out = PreflopReview { hands: entry.len(), ..Default::default() };
    let mut classes: BTreeMap<u8, ClassGap> = BTreeMap::new();
    for f in entry.values() {
        let Some(d) = by_key.get(&(f.hand_id, f.action_index)) else { continue };
        let Some((p, played)) = compared(d) else { continue };
        out.compared += 1;
        out.played += played as usize;
        out.model_played += p;
        out.equilibrium += (d.row.source == Some(Source::Equilibrium)) as usize;
        let c = classes.entry(f.class).or_insert_with(|| ClassGap {
            class: f.class,
            name: HandClass::from_index(f.class as usize).to_string(),
            hands: 0,
            played: 0,
            model: 0.0,
            loss_bb: 0.0,
        });
        c.hands += 1;
        c.played += played as usize;
        c.model += p;
        c.loss_bb += d.row.ev_loss_bb.unwrap_or(0.0);
    }
    if out.compared > 0 {
        out.you_pct = Some(100.0 * out.played as f64 / out.compared as f64);
        out.model_pct = Some(100.0 * out.model_played / out.compared as f64);
    }
    out.classes = classes.into_values().collect();
    let ranked = |wide: bool| {
        let mut list: Vec<&ClassGap> =
            out.classes.iter().filter(|c| if wide { c.gap() >= CLASS_GAP } else { -c.gap() >= CLASS_GAP }).collect();
        list.sort_by(|a, b| b.loss_bb.total_cmp(&a.loss_bb).then(b.gap().abs().total_cmp(&a.gap().abs())).then(a.class.cmp(&b.class)));
        list.into_iter().take(CLASSES).map(|c| c.class).collect::<Vec<_>>()
    };
    out.wide = ranked(true);
    out.tight = ranked(false);

    // Every compared decision for patterns and the score, entries and later ones.
    let mut groups: BTreeMap<(History, Option<Seat>), Group> = BTreeMap::new();
    let mut loss = 0.0;
    for f in facts {
        let Some(d) = by_key.get(&(f.hand_id, f.action_index)) else { continue };
        let Some((p, played)) = compared(d) else { continue };
        out.decisions += 1;
        out.good += is_good(d.row.rating) as usize;
        loss += d.row.ev_loss_bb.unwrap_or(0.0);
        let g = groups.entry((f.history, pattern_seat(f.history, &f.position, f.vs_position.as_deref()))).or_default();
        g.n += 1;
        g.played += played as usize;
        g.model += p;
        g.var += p * (1.0 - p);
        g.loss += d.row.ev_loss_bb.unwrap_or(0.0);
        *g.positions.entry(f.position.clone()).or_default() += 1;
        if let Some(v) = &f.vs_position {
            *g.vs.entry(v.clone()).or_default() += 1;
        }
        if let Some(k) = kind_of(&d.row) {
            *g.practice.entry(k.key()).or_default() += 1;
        }
    }
    out.loss_bb = loss;
    out.sample = match out.compared {
        0 => None,
        n if n < FEW => Some(Sample::Few),
        n if n < SOLID => Some(Sample::Rough),
        _ => Some(Sample::Solid),
    };
    if out.sample.is_none_or(|s| s == Sample::Few) {
        return out;
    }

    let mut patterns: Vec<Pattern> = groups
        .into_iter()
        .filter_map(|((history, seat), g)| {
            let gap = g.played as f64 - g.model;
            let clear = g.n >= PATTERN_MIN_DECISIONS && gap.abs() >= PATTERN_MIN_GAP && gap.abs() >= PATTERN_Z * g.var.sqrt();
            clear.then(|| Pattern {
                text: pattern_text(history, seat, gap > 0.0, g.played, g.n, g.model),
                wide: gap > 0.0,
                decisions: g.n,
                played: g.played,
                model: g.model,
                loss_bb: g.loss,
                history,
                position: most(&g.positions),
                vs_position: most(&g.vs),
                practice: most(&g.practice),
            })
        })
        .collect();
    patterns.sort_by(|a, b| {
        let gap = |p: &Pattern| (p.played as f64 - p.model).abs();
        gap(b).total_cmp(&gap(a)).then(b.loss_bb.total_cmp(&a.loss_bb)).then(a.text.cmp(&b.text))
    });
    patterns.truncate(PATTERNS);
    out.patterns = patterns;
    out.verdict = Some(verdict(&out));
    out
}

/// One of four steps by how many of ten preflop decisions the model rates good or better
/// (the number the line says), and in one line whether the hero plays too many or too few hands.
fn verdict(p: &PreflopReview) -> Verdict {
    let share = if p.decisions > 0 { p.good as f64 / p.decisions as f64 } else { 0.0 };
    let tens = (share * 10.0).round() as usize;
    let (level, title) = match tens {
        10 => (0, "Stark gespielt"),
        9 => (1, "Solide gespielt"),
        8 => (2, "Ausbaufähig"),
        _ => (3, "Viele Fehler vor dem Flop"),
    };
    let diff = p.you_pct.unwrap_or(0.0) - p.model_pct.unwrap_or(0.0);
    let direction = match diff {
        d if d >= 3.0 => "Du spielst etwas zu viele Hände.",
        d if d <= -3.0 => "Du spielst etwas zu wenige Hände.",
        _ => "Wie viele Hände du spielst, passt.",
    };
    Verdict { level, title: title.to_string(), line: format!("{tens} von 10 Entscheidungen vor dem Flop passen. {direction}") }
}

fn invested(d: &ReportDecision) -> bool {
    played_kind(d).is_some_and(|k| k != "fold" && k != "check")
}

/// The action the recommendation takes most often.
fn best_kind(d: &ReportDecision) -> Option<&str> {
    d.strategy.iter().max_by(|a, b| a.freq.total_cmp(&b.freq)).map(|c| c.kind.as_str())
}

fn trusted(s: Option<Source>) -> bool {
    matches!(s, Some(Source::Equilibrium | Source::Model | Source::Solver))
}

fn swing(h: &SwingHand, d: &ReportDecision) -> Swing {
    let bb = h.big_blind.max(1) as f64;
    let m = margin(d);
    Swing {
        hand_id: h.hand_id,
        action_index: d.row.action_index,
        tournament_id: h.tournament_id,
        hero_cards: h.hero_cards.clone(),
        position: h.position.clone(),
        street: d.street,
        played: d.played.clone(),
        best: d.best.clone(),
        rating: d.row.rating,
        source: d.row.source,
        why: d.why.clone(),
        net_bb: h.net as f64 / bb,
        ev_bb: h.ev.map(|ev| ev as f64 / bb),
        luck_bb: h.ev.map(|ev| (h.net - ev) as f64 / bb),
        loss_bb: d.row.ev_loss_bb.unwrap_or(0.0),
        margin_bb: m.as_ref().map(|x| x.0),
        alternative: m.map(|x| x.1),
        equity_pct: None,
        opponents: Vec::new(),
        board: Vec::new(),
    }
}

/// Right decisions that lost and wrong ones that won. Judged by the rating at the moment
/// of the decision (against the opponent's range), never by the cards shown afterwards.
///
/// Bad luck: the hand lost at showdown or below its all-in expectation, and every decision
/// in it is rated good or better by the equilibrium, the model or the solver; the decision
/// shown is the last one that put chips in. Good luck: the hand won and holds a decision
/// rated as an inaccuracy or worse by those sources that put chips in where folding or
/// checking was best, at a cost of [`LUCK_MIN_BB`] or more; the costliest one is shown. A too passive line that won is no luck.
pub fn swings(hands: &[SwingHand], decisions: &[ReportDecision]) -> (Vec<Swing>, Vec<Swing>) {
    let mut by_hand: BTreeMap<u64, Vec<&ReportDecision>> = BTreeMap::new();
    for d in decisions.iter().filter(|d| d.row.status != Status::Forced && d.row.source != Some(Source::Forced)) {
        by_hand.entry(d.row.hand_id).or_default().push(d);
    }
    let (mut bad, mut good) = (Vec::new(), Vec::new());
    for h in hands {
        let Some(ds) = by_hand.get(&h.hand_id) else { continue };
        let bb = h.big_blind.max(1) as f64;
        if h.net < 0 {
            let unlucky = h.showdown || h.ev.is_some_and(|ev| h.net < ev);
            let all_right = ds.iter().all(|d| d.row.status == Status::Rated && trusted(d.row.source) && is_good(d.row.rating));
            let credited = ds.iter().rev().find(|d| invested(d));
            if let (true, true, Some(d)) = (unlucky, all_right, credited) {
                let strength = h.ev.map_or(-h.net as f64, |ev| (ev - h.net) as f64) / bb;
                bad.push((strength, swing(h, d)));
            }
        } else if h.net > 0 {
            let worst = ds
                .iter()
                .filter(|d| d.row.status == Status::Rated && trusted(d.row.source) && is_flawed(d.row.rating))
                .filter(|d| invested(d) && best_kind(d).is_some_and(|k| k == "fold" || k == "check"))
                .filter(|d| d.row.ev_loss_bb.unwrap_or(0.0) >= LUCK_MIN_BB)
                .max_by(|a, b| a.row.ev_loss_bb.unwrap_or(0.0).total_cmp(&b.row.ev_loss_bb.unwrap_or(0.0)));
            if let Some(d) = worst {
                good.push((d.row.ev_loss_bb.unwrap_or(0.0), swing(h, d)));
            }
        }
    }
    let ranked = |mut list: Vec<(f64, Swing)>| {
        list.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.hand_id.cmp(&b.1.hand_id)));
        list.into_iter().take(SWINGS).map(|x| x.1).collect::<Vec<_>>()
    };
    (ranked(bad), ranked(good))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Choice, SpotType};
    use crate::profile::ProfileRow;
    use crate::report::report;

    /// A small deterministic generator for made-up, anonymous sessions.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn unit(&mut self) -> f64 {
            (self.next() % 10_001) as f64 / 10_000.0
        }
    }

    // ---- sessions -------------------------------------------------------------------

    #[test]
    fn a_pause_of_more_than_half_an_hour_starts_a_new_session() {
        let hands = [(0.0, None), (10.0, None), (40.0, None), (70.5, None), (71.0, None)];
        assert_eq!(split(&hands), [0, 0, 0, 1, 1]);
    }

    #[test]
    fn tournaments_side_by_side_share_a_session() {
        // Two tables interleave; a third tournament starts while the second still runs.
        let hands = [(0.0, Some(1)), (1.0, Some(2)), (2.0, Some(1)), (3.0, Some(2)), (50.0, Some(3)), (60.0, Some(2))];
        assert_eq!(split(&hands), [0; 6]);
    }

    #[test]
    fn a_tournament_never_spans_two_sessions() {
        let mut g = Lcg(7);
        for _ in 0..300 {
            let mut hands: Vec<(f64, Option<u64>)> = Vec::new();
            let mut t = 0.0;
            for _ in 0..60 {
                t += (g.next() % 90) as f64;
                let id = (!g.next().is_multiple_of(4)).then(|| g.next() % 5);
                hands.push((t, id));
            }
            let s = split(&hands);
            assert!(s.windows(2).all(|w| w[1] == w[0] || w[1] == w[0] + 1), "sessions count up");
            for (i, w) in hands.windows(2).enumerate() {
                if w[1].0 - w[0].0 <= GAP_MINUTES {
                    assert_eq!(s[i], s[i + 1], "close hands share a session");
                }
            }
            for id in 0..5 {
                let of: Vec<usize> = hands.iter().zip(&s).filter(|(h, _)| h.1 == Some(id)).map(|(_, &x)| x).collect();
                assert!(of.windows(2).all(|w| w[0] == w[1]), "tournament {id} split: {of:?}");
            }
        }
    }

    // ---- preflop --------------------------------------------------------------------

    fn class(name: &str) -> u8 {
        HandClass::all().find(|c| c.to_string() == name).unwrap().index() as u8
    }

    fn decision(hand_id: u64, action_index: usize, model_play: f64, played: bool, source: Source, rating: Rating, loss: f64) -> ReportDecision {
        ReportDecision {
            row: ProfileRow {
                hand_id,
                action_index,
                played_at: "2026/11/18 20:00:00".into(),
                spot: SpotType::VsOpen,
                status: Status::Rated,
                rating: Some(rating),
                ev_loss_bb: Some(loss),
                source: Some(source),
                reliability: None,
                facing_bet: true,
                position: Some("BB".into()),
                stack_bb: Some(30.0),
                players: 6,
            },
            street: Street::Preflop,
            played: if played { "Call 2 BB".into() } else { "Fold".into() },
            best: None,
            why: String::new(),
            strategy: vec![
                Choice { label: "Fold".into(), freq: 1.0 - model_play, ev_bb: Some(0.0), played: !played, kind: "fold".into() },
                Choice { label: "Call".into(), freq: model_play, ev_bb: Some(0.5), played, kind: "call".into() },
            ],
            pot_bb: Some(3.5),
        }
    }

    fn fact(hand_id: u64, action_index: usize, class: u8, history: History, position: &str, vs: Option<&str>) -> PreflopFact {
        PreflopFact { hand_id, action_index, class, position: position.into(), vs_position: vs.map(String::from), history }
    }

    /// Made-up session: one entry decision per hand, some with a later one.
    fn session(seed: u64, n: usize) -> (Vec<PreflopFact>, Vec<ReportDecision>) {
        let mut g = Lcg(seed);
        let (mut facts, mut ds) = (Vec::new(), Vec::new());
        let seats = ["UTG", "HJ", "CO", "BTN", "SB", "BB"];
        for h in 0..n as u64 {
            let c = (g.next() % 169) as u8;
            let p = if g.next().is_multiple_of(3) { 0.0 } else { g.unit() };
            let played = g.unit() < 0.3;
            let source = if g.next().is_multiple_of(4) { Source::Equilibrium } else { Source::Model };
            let history = [History::Unopened, History::VsOpen, History::Limped][(g.next() % 3) as usize];
            let pos = seats[(g.next() % 6) as usize];
            facts.push(fact(h, 3, c, history, pos, (history != History::Unopened).then_some("UTG")));
            // Some hands are not compared: estimate source or not analysed at all.
            match g.next() % 10 {
                0 => {}
                1 => ds.push(decision(h, 3, p, played, Source::Estimate, Rating::Good, 0.0)),
                _ => ds.push(decision(h, 3, p, played, source, Rating::ALL[(g.next() % 6) as usize], g.unit())),
            }
            if played && g.next().is_multiple_of(3) {
                facts.push(fact(h, 9, c, History::VsThreeBet, pos, Some("BTN")));
                ds.push(decision(h, 9, g.unit(), g.next().is_multiple_of(2), Source::Model, Rating::Good, 0.0));
            }
        }
        (facts, ds)
    }

    #[test]
    fn the_share_played_counts_the_entry_decisions_and_the_model_share_is_their_mean() {
        for seed in 0..100 {
            let (facts, ds) = session(seed, 150);
            let p = preflop_review(&facts, &ds);
            let entries: Vec<&ReportDecision> = ds.iter().filter(|d| d.row.action_index == 3 && compared(d).is_some()).collect();
            assert_eq!(p.compared, entries.len());
            assert_eq!(p.played, entries.iter().filter(|d| d.played != "Fold").count());
            let mean = entries.iter().map(|d| d.strategy[1].freq).sum::<f64>() / entries.len() as f64;
            assert!((p.model_pct.unwrap() - 100.0 * mean).abs() < 1e-9);
            assert!((p.you_pct.unwrap() - 100.0 * p.played as f64 / p.compared as f64).abs() < 1e-9);
            assert_eq!(p.hands, 150);
        }
    }

    #[test]
    fn the_model_share_is_weighted_as_played() {
        // Nine button hands the model plays half the time, one UTG hand it never plays.
        let mut facts = Vec::new();
        let mut ds = Vec::new();
        for h in 0..10u64 {
            let (pos, p) = if h < 9 { ("BTN", 0.5) } else { ("UTG", 0.0) };
            facts.push(fact(h, 2, class("K9o"), History::Unopened, pos, None));
            ds.push(decision(h, 2, p, false, Source::Model, Rating::Good, 0.0));
        }
        let p = preflop_review(&facts, &ds);
        assert!((p.model_pct.unwrap() - 45.0).abs() < 1e-9, "{:?}", p.model_pct);
    }

    #[test]
    fn too_loose_and_too_tight_add_up_to_the_difference() {
        for seed in 0..100 {
            let (facts, ds) = session(seed, 200);
            let p = preflop_review(&facts, &ds);
            let sum: f64 = p.classes.iter().map(ClassGap::gap).sum();
            assert!((sum - (p.played as f64 - p.model_played)).abs() < 1e-9);
            let hands: usize = p.classes.iter().map(|c| c.hands).sum();
            assert_eq!(hands, p.compared);
            assert!(p.wide.iter().all(|c| p.classes.iter().any(|x| x.class == *c && x.gap() >= CLASS_GAP)));
            assert!(p.tight.iter().all(|c| p.classes.iter().any(|x| x.class == *c && x.gap() <= -CLASS_GAP)));
        }
    }

    #[test]
    fn a_small_sample_gives_a_hint_instead_of_patterns() {
        // 20 hands, all of them calls against an early open the model always folds.
        let mut facts = Vec::new();
        let mut ds = Vec::new();
        for h in 0..20u64 {
            facts.push(fact(h, 2, class("72o"), History::VsOpen, "BB", Some("UTG")));
            ds.push(decision(h, 2, 0.0, true, Source::Model, Rating::Blunder, 1.0));
        }
        let p = preflop_review(&facts, &ds);
        assert_eq!(p.sample, Some(Sample::Few));
        assert!(p.patterns.is_empty() && p.verdict.is_none());

        // With 40 such hands the pattern is there, in plain words.
        for h in 20..40u64 {
            facts.push(fact(h, 2, class("72o"), History::VsOpen, "BB", Some("UTG")));
            ds.push(decision(h, 2, 0.0, true, Source::Model, Rating::Blunder, 1.0));
        }
        let p = preflop_review(&facts, &ds);
        assert_eq!(p.sample, Some(Sample::Rough));
        assert_eq!(p.patterns.len(), 1);
        assert_eq!(p.patterns[0].text, "Gegen eine Erhöhung aus früher Position gehst du zu oft mit: 40 von 40 Mal, besser etwa 0.");
        assert_eq!(p.patterns[0].vs_position.as_deref(), Some("UTG"));
        assert_eq!(p.verdict.as_ref().unwrap().level, 3);
    }

    #[test]
    fn noise_is_no_pattern() {
        // The hero plays exactly as often as the model expects, in every situation.
        let mut facts = Vec::new();
        let mut ds = Vec::new();
        for h in 0..400u64 {
            facts.push(fact(h, 2, class("AJo"), History::Unopened, "BTN", None));
            ds.push(decision(h, 2, 0.5, h % 2 == 0, Source::Model, Rating::Good, 0.0));
        }
        let p = preflop_review(&facts, &ds);
        assert_eq!(p.sample, Some(Sample::Solid));
        assert!(p.patterns.is_empty(), "{:?}", p.patterns);
        assert_eq!(p.verdict.unwrap().title, "Stark gespielt");
    }

    // ---- luck -----------------------------------------------------------------------

    fn hand(hand_id: u64, net: i64, ev: Option<i64>, showdown: bool) -> SwingHand {
        SwingHand {
            hand_id,
            tournament_id: Some(1),
            hero_cards: "Ah Qd".into(),
            position: Some("BTN".into()),
            net,
            ev,
            big_blind: 100,
            showdown,
        }
    }

    /// A shove the recommendation takes when rated good, and folds otherwise.
    fn shove(hand_id: u64, rating: Rating, source: Source) -> ReportDecision {
        let right = rating <= Rating::Good;
        let mut d = decision(hand_id, 4, if right { 1.0 } else { 0.0 }, true, source, rating, if right { 0.0 } else { 2.5 });
        d.played = "All-in 15 BB".into();
        d.strategy[1].kind = "all_in".into();
        d
    }

    #[test]
    fn a_right_decision_that_lost_is_bad_luck_and_never_a_mistake() {
        let hands = [hand(1, -1500, Some(400), true), hand(2, -200, None, false)];
        let ds = [shove(1, Rating::Best, Source::Equilibrium), decision(2, 4, 0.0, false, Source::Model, Rating::Best, 0.0)];
        let (bad, good) = swings(&hands, &ds);
        assert_eq!(bad.len(), 1, "a fold that lost the blind is no bad luck");
        assert_eq!((bad[0].hand_id, bad[0].luck_bb), (1, Some(-19.0)));
        assert!(good.is_empty());

        // Too passive with aces and won anyway: a mistake, but winning was no luck.
        let mut limp = shove(3, Rating::Blunder, Source::Model);
        limp.strategy[0].freq = 0.0;
        limp.strategy[1].freq = 1.0;
        limp.strategy[1].kind = "call".into();
        limp.strategy.push(Choice { label: "Raise".into(), freq: 0.0, ev_bb: Some(3.0), played: false, kind: "raise".into() });
        limp.strategy[1].freq = 0.0;
        limp.strategy[2].freq = 1.0;
        let (_, good) = swings(&[hand(3, 900, None, true)], &[limp]);
        assert!(good.is_empty(), "{good:?}");
        let report_hands: Vec<_> = hands
            .iter()
            .map(|h| crate::report::ReportHand {
                hand_id: h.hand_id,
                played_at: "2026/11/18 20:00:00".into(),
                hero_cards: h.hero_cards.clone(),
                class: None,
                position: h.position.clone(),
                net: h.net,
                big_blind: h.big_blind,
                ev: h.ev,
                stack: None,
            })
            .collect();
        let r = report(&report_hands, &ds, 0, "2026/11/18", &[]);
        assert!(r.worst.iter().all(|m| m.hand_id != 1), "listed as a mistake");
    }

    #[test]
    fn a_wrong_decision_that_won_is_good_luck() {
        let hands = [hand(1, 1500, Some(-300), true), hand(2, 900, None, true)];
        let ds = [shove(1, Rating::Mistake, Source::Equilibrium), shove(2, Rating::Blunder, Source::Estimate)];
        let (bad, good) = swings(&hands, &ds);
        assert!(bad.is_empty());
        assert_eq!(good.len(), 1, "estimates are not sure enough to call it luck");
        assert_eq!((good[0].hand_id, good[0].loss_bb), (1, 2.5));
    }

    #[test]
    fn luck_never_judges_a_hand_twice_or_one_with_a_mistake_as_bad_luck() {
        let mut g = Lcg(3);
        for _ in 0..200 {
            let mut hands = Vec::new();
            let mut ds = Vec::new();
            for h in 0..40u64 {
                let net = (g.next() % 3000) as i64 - 1500;
                hands.push(hand(h, net, g.next().is_multiple_of(3).then(|| (g.next() % 3000) as i64 - 1500), g.next().is_multiple_of(2)));
                for a in 0..(g.next() % 3) as usize {
                    let rating = Rating::ALL[(g.next() % 6) as usize];
                    let source = [Source::Equilibrium, Source::Model, Source::Solver, Source::Estimate][(g.next() % 4) as usize];
                    let mut d = shove(h, rating, source);
                    d.row.action_index = a;
                    ds.push(d);
                }
            }
            let (bad, good) = swings(&hands, &ds);
            for s in &bad {
                assert!(hands.iter().any(|h| h.hand_id == s.hand_id && h.net < 0));
                assert!(ds.iter().filter(|d| d.row.hand_id == s.hand_id).all(|d| is_good(d.row.rating) && trusted(d.row.source)));
                assert!(!good.iter().any(|x| x.hand_id == s.hand_id));
            }
            for s in &good {
                assert!(hands.iter().any(|h| h.hand_id == s.hand_id && h.net > 0));
                assert!(is_flawed(s.rating) && trusted(s.source));
            }
            assert!(!good.is_empty() || !bad.is_empty() || hands.len() < 40, "the generator gives both kinds now and then");
        }
    }

    #[test]
    fn locked_keeps_results_only() {
        let (facts, ds) = session(5, 200);
        let preflop = preflop_review(&facts, &ds);
        assert!(preflop.model_pct.is_some() && preflop.verdict.is_some());
        let hands = [hand(1, -1500, Some(400), true)];
        let (bad_luck, good_luck) = swings(&hands, &[shove(1, Rating::Best, Source::Equilibrium)]);
        let mut report = Report { hands: 200, ..Default::default() };
        report.luck.luck_bb = -19.0;
        report.worst = vec![];
        let review = SessionReview { span: SessionSpan::default(), report, preflop, bad_luck, good_luck, locked: false };
        let l = review.clone().locked();
        assert!(l.locked);
        assert_eq!((l.report.hands, l.report.luck.luck_bb), (200, -19.0), "results stay");
        assert_eq!((l.preflop.played, l.preflop.compared, l.preflop.you_pct), (review.preflop.played, review.preflop.compared, review.preflop.you_pct));
        assert!(l.preflop.model_pct.is_none() && l.preflop.verdict.is_none() && l.preflop.patterns.is_empty());
        assert!(l.preflop.classes.is_empty() && l.preflop.wide.is_empty() && l.preflop.tight.is_empty());
        assert!(l.bad_luck.is_empty() && l.good_luck.is_empty());
        assert!(l.report.worst.is_empty() && l.report.best.is_empty() && l.report.kinds.is_empty() && l.report.next.is_empty() && l.report.breakdowns.is_empty());
        assert_eq!(l.report.score.decisions, 0, "no grades");
        // Nothing advisory survives in what the page receives.
        let json = serde_json::to_string(&l).unwrap();
        for word in ["model_pct\":null", "\"verdict\":null", "\"worst\":[]", "\"bad_luck\":[]"] {
            assert!(json.contains(word), "{word} in {json}");
        }
    }
}
