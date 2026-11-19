//! The tournament report: what Ringside's own numbers say about one tournament, without
//! a language model. The result against the all-in expectation, a score of the decisions
//! against the hero's average, where the loss came from (situation, seat, street, stack),
//! the costliest and the best decisions, the starting hands, the stack over the hands and
//! what to practise next.
//!
//! Invariants (see the tests): every rated decision counts exactly once in every
//! breakdown, so the losses of the groups of a breakdown add up to the total loss; luck is
//! the result minus the all-in expectation; the costliest mistakes are the costliest.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::db::{Choice, Rating, Source, SpotType, Status};
use crate::leaks::{BANDS, band_of};
use crate::model::Street;
use crate::profile::{Lage, ProfileRow, Seat, kind_of};

/// Losses are summed in millionths of a big blind, so sums do not depend on row order.
const MICRO: f64 = 1e6;
/// Costliest mistakes and best decisions shown.
pub const MOMENTS: usize = 3;
/// Earlier tournaments listed for comparison.
pub const HISTORY: usize = 5;
/// Share of a group's loss from the estimate above which the report says so.
const UNSURE_SHARE: f64 = 0.5;

fn micro(bb: f64) -> i64 {
    (bb.max(0.0) * MICRO).round() as i64
}

fn bb_of(micro: i64) -> f64 {
    micro as f64 / MICRO
}

/// One hand of the tournament in play order.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportHand {
    pub hand_id: u64,
    pub played_at: String,
    /// "Ah Kd".
    pub hero_cards: String,
    /// "AKo"; None when the cards are unknown.
    pub class: Option<String>,
    pub position: Option<String>,
    /// Chips won or lost.
    pub net: i64,
    pub big_blind: i64,
    /// Expected result in chips when the hero was all-in with every hand shown.
    pub ev: Option<i64>,
    /// The hero's chips at the start of the hand.
    pub stack: Option<i64>,
}

/// One stored decision with what the report shows of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReportDecision {
    pub row: ProfileRow,
    pub street: Street,
    pub played: String,
    pub best: Option<String>,
    pub why: String,
    pub strategy: Vec<Choice>,
    pub pot_bb: Option<f64>,
}

/// A rated decision of an earlier tournament, summed per tournament.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PastScore {
    pub tournament_id: u64,
    pub started_at: String,
    pub name: Option<String>,
    pub decisions: usize,
    pub good: usize,
    pub loss_bb: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Luck {
    /// Chips won or lost over the hands.
    pub net: i64,
    /// The same with every all-in at its expectation.
    pub ev: i64,
    /// `net - ev`: above zero the cards ran well.
    pub luck: i64,
    pub net_bb: f64,
    pub ev_bb: f64,
    pub luck_bb: f64,
    /// Hands with an all-in and every hand shown.
    pub all_ins: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Score {
    /// Rated decisions.
    pub decisions: usize,
    /// Best, mixed or good.
    pub good: usize,
    pub inaccuracies: usize,
    /// Mistakes and blunders.
    pub mistakes: usize,
    /// Rated decisions per rating, best first.
    pub ratings: Vec<usize>,
    pub loss_bb: f64,
    /// Loss per 100 rated decisions, BB.
    pub per100: Option<f64>,
    pub good_pct: Option<f64>,
    /// Share of the loss from the estimate, percent.
    pub estimate_loss_pct: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CostGroup {
    pub key: String,
    pub title: String,
    pub decisions: usize,
    pub good: usize,
    /// Inaccuracies or worse.
    pub flawed: usize,
    pub loss_bb: f64,
    /// Share of the group's loss from the estimate, percent.
    pub estimate_loss_pct: f64,
    /// Most of the loss rests on the estimate.
    pub unsure: bool,
    /// Spot types of the group, for the hand list.
    pub spots: Vec<SpotType>,
}

/// One way to split the decisions; every rated decision is in exactly one group.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Breakdown {
    pub key: &'static str,
    pub title: &'static str,
    /// Costliest first.
    pub groups: Vec<CostGroup>,
}

/// A decision worth showing: one of the costliest mistakes or of the best decisions.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Moment {
    pub hand_id: u64,
    pub action_index: usize,
    /// 1-based hand number in the tournament.
    pub n: usize,
    pub played_at: String,
    pub hero_cards: String,
    pub position: Option<String>,
    pub street: Street,
    pub spot: SpotType,
    /// The situation in words, e.g. "Gegen eine Erhöhung".
    pub situation: String,
    pub stack_bb: Option<f64>,
    pub played: String,
    pub best: Option<String>,
    pub why: String,
    pub rating: Option<Rating>,
    pub source: Option<Source>,
    pub loss_bb: f64,
    /// For a good decision: what the best other action would have cost, BB.
    pub margin_bb: Option<f64>,
    /// That other action.
    pub alternative: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StartingHand {
    /// "AKo".
    pub class: String,
    pub hands: usize,
    pub net_bb: f64,
    pub loss_bb: f64,
    /// Rated decisions with an inaccuracy or worse.
    pub flawed: usize,
    pub hand_ids: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StackPoint {
    pub n: usize,
    pub hand_id: u64,
    /// Chips at the start of the hand.
    pub stack: Option<i64>,
    pub big_blind: i64,
    pub net: i64,
    /// Loss of the decisions in this hand, BB.
    pub loss_bb: f64,
    /// 1-based rank among the costliest mistakes when one of them is in this hand.
    pub mistake: Option<usize>,
}

/// What one kind of spot cost in this tournament.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Practice {
    /// Key of the playing profile's group, for "Lernen".
    pub key: String,
    pub title: String,
    pub detail: String,
    pub lage: Lage,
    pub decisions: usize,
    pub flawed: usize,
    pub loss_bb: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PastTournament {
    pub tournament_id: u64,
    pub started_at: String,
    pub name: Option<String>,
    pub decisions: usize,
    pub per100: Option<f64>,
    pub good_pct: Option<f64>,
}

/// The pooled score of the earlier tournaments.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Average {
    pub tournaments: usize,
    pub decisions: usize,
    pub per100: Option<f64>,
    pub good_pct: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Report {
    pub hands: usize,
    /// Hands not analysed yet.
    pub unanalyzed: usize,
    /// Decisions waiting for a postflop solve: the report is preliminary meanwhile.
    pub pending: usize,
    /// Decisions outside what the engine rates.
    pub unrated: usize,
    pub luck: Luck,
    pub score: Score,
    pub breakdowns: Vec<Breakdown>,
    pub worst: Vec<Moment>,
    pub best: Vec<Moment>,
    pub starting: Vec<StartingHand>,
    pub timeline: Vec<StackPoint>,
    /// Loss per kind of spot in this tournament, costliest first.
    pub kinds: Vec<Practice>,
    /// „Was als Nächstes?“ for this tournament: the one list of the whole history, filtered to
    /// this tournament ([`crate::next::within`]). Filled by the caller, which knows the
    /// whole history.
    pub next: Vec<crate::next::NextStep>,
    /// The latest earlier tournaments, newest first.
    pub history: Vec<PastTournament>,
    pub average: Option<Average>,
}

fn per100(loss: i64, decisions: usize) -> Option<f64> {
    (decisions > 0).then(|| 100.0 * bb_of(loss) / decisions as f64)
}

fn pct(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| 100.0 * part as f64 / whole as f64)
}

fn is_good(r: Option<Rating>) -> bool {
    r.is_some_and(|r| r <= Rating::Good)
}

fn is_flawed(r: Option<Rating>) -> bool {
    r.is_some_and(|r| r >= Rating::Inaccuracy)
}

fn loss_of(d: &ReportDecision) -> i64 {
    micro(d.row.ev_loss_bb.unwrap_or(0.0))
}

fn rated(d: &ReportDecision) -> bool {
    d.row.status == Status::Rated
}

#[derive(Default)]
struct Acc {
    title: String,
    decisions: usize,
    good: usize,
    flawed: usize,
    loss: i64,
    estimate_loss: i64,
    spots: Vec<SpotType>,
}

impl Acc {
    fn add(&mut self, d: &ReportDecision) {
        let loss = loss_of(d);
        self.decisions += 1;
        self.good += is_good(d.row.rating) as usize;
        self.flawed += is_flawed(d.row.rating) as usize;
        self.loss += loss;
        if d.row.source == Some(Source::Estimate) {
            self.estimate_loss += loss;
        }
        if !self.spots.contains(&d.row.spot) {
            self.spots.push(d.row.spot);
        }
    }

    fn group(self, key: String) -> CostGroup {
        let share = if self.loss > 0 { self.estimate_loss as f64 / self.loss as f64 } else { 0.0 };
        CostGroup {
            key,
            title: self.title,
            decisions: self.decisions,
            good: self.good,
            flawed: self.flawed,
            loss_bb: bb_of(self.loss),
            estimate_loss_pct: 100.0 * share,
            unsure: share > UNSURE_SHARE,
            spots: self.spots,
        }
    }
}

fn breakdown(
    key: &'static str,
    title: &'static str,
    decisions: &[&ReportDecision],
    of: impl Fn(&ReportDecision) -> (String, String),
) -> Breakdown {
    let mut acc: BTreeMap<String, Acc> = BTreeMap::new();
    for d in decisions {
        let (k, t) = of(d);
        let a = acc.entry(k).or_insert_with(|| Acc { title: t, ..Default::default() });
        a.add(d);
    }
    let mut groups: Vec<CostGroup> = acc.into_iter().map(|(k, a)| a.group(k)).collect();
    groups.sort_by(|a, b| b.loss_bb.total_cmp(&a.loss_bb).then(b.decisions.cmp(&a.decisions)).then(a.key.cmp(&b.key)));
    Breakdown { key, title, groups }
}

fn situation(d: &ReportDecision) -> (String, String) {
    match kind_of(&d.row) {
        Some(k) => (k.lage.key().to_string(), k.lage.name().to_string()),
        None => ("-".into(), "Ohne Einordnung".into()),
    }
}

fn seat(d: &ReportDecision) -> (String, String) {
    match d.row.position.as_deref().and_then(Seat::of) {
        Some(s) => (s.key().to_string(), seat_name(s).to_string()),
        None => ("-".into(), "Position unbekannt".into()),
    }
}

fn seat_name(s: Seat) -> &'static str {
    match s {
        Seat::Early => "Frühe Position",
        Seat::Middle => "Mittlere Position",
        Seat::Cutoff => "Cutoff",
        Seat::Button => "Button",
        Seat::SmallBlind => "Small Blind",
        Seat::BigBlind => "Big Blind",
    }
}

fn street(d: &ReportDecision) -> (String, String) {
    let (k, t) = match d.street {
        Street::Preflop => ("0", "Vor dem Flop"),
        Street::Flop => ("1", "Flop"),
        Street::Turn => ("2", "Turn"),
        Street::River => ("3", "River"),
    };
    (k.into(), t.into())
}

fn band(d: &ReportDecision) -> (String, String) {
    match d.row.stack_bb.filter(|s| s.is_finite()).map(band_of) {
        Some(b) => (b.to_string(), BANDS[b as usize].to_string()),
        None => ("-".into(), "Stack unbekannt".into()),
    }
}

/// What the played action gained over the best other action, by the strategy's EVs.
pub(crate) fn margin(d: &ReportDecision) -> Option<(f64, String)> {
    let played = d.strategy.iter().find(|c| c.played)?.ev_bb?;
    let other = d
        .strategy
        .iter()
        .filter(|c| !c.played)
        .filter_map(|c| c.ev_bb.map(|ev| (ev, c.label.clone())))
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    Some((played - other.0, other.1))
}

/// Builds the report from the hands of one tournament in play order and their stored
/// decisions (forced moves included or not: they never count). `past` holds the scores
/// of the other tournaments; only those started before `started_at` are compared.
pub fn report(hands: &[ReportHand], decisions: &[ReportDecision], unanalyzed: usize, started_at: &str, past: &[PastScore]) -> Report {
    let index: BTreeMap<u64, usize> = hands.iter().enumerate().map(|(i, h)| (h.hand_id, i)).collect();
    let decisions: Vec<&ReportDecision> =
        decisions.iter().filter(|d| index.contains_key(&d.row.hand_id) && kind_of(&d.row).is_some()).collect();
    let rated_list: Vec<&ReportDecision> = decisions.iter().copied().filter(|d| rated(d)).collect();

    let mut out = Report {
        hands: hands.len(),
        unanalyzed,
        pending: decisions.iter().filter(|d| d.row.status == Status::Pending).count(),
        unrated: decisions.iter().filter(|d| d.row.status == Status::Unrated).count(),
        ..Default::default()
    };

    // Luck: chips exactly, BB per hand at its own big blind.
    for h in hands {
        let ev = h.ev.unwrap_or(h.net);
        out.luck.net += h.net;
        out.luck.ev += ev;
        out.luck.all_ins += h.ev.is_some() as usize;
        if h.big_blind > 0 {
            out.luck.net_bb += h.net as f64 / h.big_blind as f64;
            out.luck.ev_bb += ev as f64 / h.big_blind as f64;
        }
    }
    out.luck.luck = out.luck.net - out.luck.ev;
    out.luck.luck_bb = out.luck.net_bb - out.luck.ev_bb;

    // Score.
    let mut total = 0i64;
    let mut estimate = 0i64;
    out.score.ratings = vec![0; Rating::ALL.len()];
    for d in &rated_list {
        let loss = loss_of(d);
        total += loss;
        if d.row.source == Some(Source::Estimate) {
            estimate += loss;
        }
        out.score.decisions += 1;
        out.score.good += is_good(d.row.rating) as usize;
        match d.row.rating {
            Some(Rating::Inaccuracy) => out.score.inaccuracies += 1,
            Some(Rating::Mistake | Rating::Blunder) => out.score.mistakes += 1,
            _ => {}
        }
        if let Some(r) = d.row.rating {
            out.score.ratings[r.level() as usize] += 1;
        }
    }
    out.score.loss_bb = bb_of(total);
    out.score.per100 = per100(total, out.score.decisions);
    out.score.good_pct = pct(out.score.good, out.score.decisions);
    out.score.estimate_loss_pct = if total > 0 { 100.0 * estimate as f64 / total as f64 } else { 0.0 };

    out.breakdowns = vec![
        breakdown("situation", "Spot-Art", &rated_list, situation),
        breakdown("seat", "Position", &rated_list, seat),
        breakdown("street", "Straße", &rated_list, street),
        breakdown("stack", "Stacktiefe", &rated_list, band),
    ];

    // Moments.
    let moment = |d: &ReportDecision| {
        let i = index[&d.row.hand_id];
        let h = &hands[i];
        let m = margin(d);
        Moment {
            hand_id: d.row.hand_id,
            action_index: d.row.action_index,
            n: i + 1,
            played_at: h.played_at.clone(),
            hero_cards: h.hero_cards.clone(),
            position: h.position.clone(),
            street: d.street,
            spot: d.row.spot,
            situation: situation(d).1,
            stack_bb: d.row.stack_bb,
            played: d.played.clone(),
            best: d.best.clone(),
            why: d.why.clone(),
            rating: d.row.rating,
            source: d.row.source,
            loss_bb: bb_of(loss_of(d)),
            margin_bb: m.as_ref().map(|x| x.0),
            alternative: m.map(|x| x.1),
        }
    };
    let mut flawed: Vec<&ReportDecision> =
        rated_list.iter().copied().filter(|d| is_flawed(d.row.rating) && loss_of(d) > 0).collect();
    flawed.sort_by(|a, b| {
        loss_of(b).cmp(&loss_of(a)).then(a.row.hand_id.cmp(&b.row.hand_id)).then(a.row.action_index.cmp(&b.row.action_index))
    });
    out.worst = flawed.iter().take(MOMENTS).map(|d| moment(d)).collect();

    let mut good: Vec<(&ReportDecision, Option<f64>)> = rated_list
        .iter()
        .copied()
        .filter(|d| matches!(d.row.rating, Some(Rating::Best | Rating::Mix)))
        .map(|d| (d, margin(d).map(|m| m.0)))
        .filter(|(_, m)| m.is_none_or(|m| m > 0.0))
        .collect();
    good.sort_by(|(a, ma), (b, mb)| {
        let key = |m: &Option<f64>, d: &ReportDecision| (m.is_some(), m.unwrap_or(0.0), d.pot_bb.unwrap_or(0.0));
        let (x, y) = (key(ma, a), key(mb, b));
        y.0.cmp(&x.0)
            .then(y.1.total_cmp(&x.1))
            .then(y.2.total_cmp(&x.2))
            .then(a.row.hand_id.cmp(&b.row.hand_id))
            .then(a.row.action_index.cmp(&b.row.action_index))
    });
    out.best = good.iter().take(MOMENTS).map(|(d, _)| moment(d)).collect();

    // Starting hands.
    let mut loss_by_hand: BTreeMap<u64, (i64, usize)> = BTreeMap::new();
    for d in &rated_list {
        let e = loss_by_hand.entry(d.row.hand_id).or_default();
        e.0 += loss_of(d);
        e.1 += is_flawed(d.row.rating) as usize;
    }
    let mut classes: BTreeMap<String, (usize, f64, i64, usize, Vec<u64>)> = BTreeMap::new();
    for h in hands {
        let Some(class) = &h.class else { continue };
        let e = classes.entry(class.clone()).or_default();
        let (loss, fl) = loss_by_hand.get(&h.hand_id).copied().unwrap_or_default();
        e.0 += 1;
        if h.big_blind > 0 {
            e.1 += h.net as f64 / h.big_blind as f64;
        }
        e.2 += loss;
        e.3 += fl;
        e.4.push(h.hand_id);
    }
    out.starting = classes
        .into_iter()
        .map(|(class, (n, net_bb, loss, fl, ids))| StartingHand { class, hands: n, net_bb, loss_bb: bb_of(loss), flawed: fl, hand_ids: ids })
        .collect();

    // Timeline.
    out.timeline = hands
        .iter()
        .enumerate()
        .map(|(i, h)| StackPoint {
            n: i + 1,
            hand_id: h.hand_id,
            stack: h.stack,
            big_blind: h.big_blind,
            net: h.net,
            loss_bb: bb_of(loss_by_hand.get(&h.hand_id).map_or(0, |x| x.0)),
            mistake: out.worst.iter().position(|m| m.hand_id == h.hand_id).map(|p| p + 1),
        })
        .collect();

    // Loss per kind of spot.
    let mut kinds: BTreeMap<String, (crate::profile::SpotKind, usize, usize, i64)> = BTreeMap::new();
    for d in &rated_list {
        let Some(k) = kind_of(&d.row) else { continue };
        let e = kinds.entry(k.key()).or_insert((k, 0, 0, 0));
        e.1 += 1;
        e.2 += is_flawed(d.row.rating) as usize;
        e.3 += loss_of(d);
    }
    let mut kinds: Vec<_> = kinds.into_iter().filter(|(_, e)| e.3 > 0).collect();
    kinds.sort_by(|a, b| b.1.3.cmp(&a.1.3).then(b.1.1.cmp(&a.1.1)).then(a.0.cmp(&b.0)));
    out.kinds = kinds
        .into_iter()
        .map(|(key, (k, n, fl, loss))| Practice {
            key,
            title: k.title(),
            detail: k.detail(),
            lage: k.lage,
            decisions: n,
            flawed: fl,
            loss_bb: bb_of(loss),
        })
        .collect();

    (out.history, out.average) = compare(started_at, past);
    out
}

/// The latest earlier tournaments with rated decisions, newest first, and their pooled
/// score. Tournaments started at or after `started_at` are not earlier.
pub fn compare(started_at: &str, past: &[PastScore]) -> (Vec<PastTournament>, Option<Average>) {
    let mut earlier: Vec<&PastScore> = past.iter().filter(|p| p.decisions > 0 && p.started_at.as_str() < started_at).collect();
    earlier.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(b.tournament_id.cmp(&a.tournament_id)));
    let history = earlier
        .iter()
        .take(HISTORY)
        .map(|p| PastTournament {
            tournament_id: p.tournament_id,
            started_at: p.started_at.clone(),
            name: p.name.clone(),
            decisions: p.decisions,
            per100: per100(micro(p.loss_bb), p.decisions),
            good_pct: pct(p.good, p.decisions),
        })
        .collect();
    let average = (!earlier.is_empty()).then(|| {
        let decisions = earlier.iter().map(|p| p.decisions).sum();
        let loss = earlier.iter().map(|p| micro(p.loss_bb)).sum();
        Average {
            tournaments: earlier.len(),
            decisions,
            per100: per100(loss, decisions),
            good_pct: pct(earlier.iter().map(|p| p.good).sum(), decisions),
        }
    });
    (history, average)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so the invariants run over many made-up tournaments.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn pick<T: Clone>(&mut self, xs: &[T]) -> T {
            xs[self.next() as usize % xs.len()].clone()
        }
    }

    const POSITIONS: [&str; 9] = ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB", "??"];
    const CARDS: [&str; 6] = ["Ah Kd", "7c 2d", "Qs Qh", "Jh Th", "9s 8s", ""];
    const STATUSES: [Status; 5] = [Status::Rated, Status::Rated, Status::Rated, Status::Pending, Status::Forced];

    fn class_of(cards: &str) -> Option<String> {
        let v: Vec<String> = cards.split_whitespace().map(String::from).collect();
        crate::stats::hand_class(&v)
    }

    /// Anonymised made-up tournament: hands with chips, all-ins and decisions of every kind.
    fn tournament(seed: u64, n_hands: usize) -> (Vec<ReportHand>, Vec<ReportDecision>) {
        let mut g = Lcg(seed);
        let mut stack = 1500i64;
        let mut hands = Vec::new();
        let mut decisions = Vec::new();
        for i in 0..n_hands {
            let hand_id = 5000 + i as u64 * 7;
            let big_blind = g.pick(&[20, 40, 100, 200]);
            let net = g.next() as i64 % 600 - 300;
            let cards = g.pick(&CARDS);
            let ev = g.next().is_multiple_of(5).then(|| net + (g.next() as i64 % 200 - 100));
            let position = Some(g.pick(&POSITIONS).to_string());
            hands.push(ReportHand {
                hand_id,
                played_at: format!("2026/11/18 20:{:02}:{:02}", i / 60, i % 60),
                hero_cards: cards.to_string(),
                class: class_of(cards),
                position: position.clone(),
                net,
                big_blind,
                ev,
                stack: Some(stack),
            });
            stack += net;
            for a in 0..(g.next() % 4) as usize {
                let status = g.pick(&STATUSES);
                let source = match status {
                    Status::Forced => Source::Forced,
                    Status::Pending => Source::Solver,
                    _ => g.pick(&[Source::Equilibrium, Source::Model, Source::Solver, Source::Estimate]),
                };
                let rating = (status == Status::Rated).then(|| g.pick(&Rating::ALL));
                let street = g.pick(&[Street::Preflop, Street::Flop, Street::Turn, Street::River]);
                let played_ev = (g.next() % 500) as f64 / 100.0 - 2.0;
                let strategy = vec![
                    Choice { label: "Fold".into(), freq: 0.3, ev_bb: Some(0.0), played: a % 2 == 0, kind: "fold".into() },
                    Choice { label: "Call".into(), freq: 0.7, ev_bb: Some(played_ev), played: a % 2 == 1, kind: "call".into() },
                ];
                decisions.push(ReportDecision {
                    row: ProfileRow {
                        hand_id,
                        action_index: a * 3 + 2,
                        played_at: hands.last().unwrap().played_at.clone(),
                        spot: g.pick(&SpotType::ALL),
                        status,
                        rating,
                        ev_loss_bb: rating.map(|_| (g.next() % 900) as f64 / 100.0),
                        source: Some(source),
                        reliability: (source == Source::Estimate).then_some(0.8),
                        facing_bet: g.next().is_multiple_of(2),
                        position: position.clone(),
                        stack_bb: (!g.next().is_multiple_of(9)).then(|| (g.next() % 8000) as f64 / 100.0),
                        players: 2 + (g.next() % 8) as usize,
                    },
                    street,
                    played: "Call 2 BB".into(),
                    best: Some("Fold".into()),
                    why: String::new(),
                    strategy,
                    pot_bb: Some((g.next() % 3000) as f64 / 100.0),
                });
            }
        }
        (hands, decisions)
    }

    fn counted(decisions: &[ReportDecision]) -> Vec<&ReportDecision> {
        decisions.iter().filter(|d| d.row.status == Status::Rated && d.row.source != Some(Source::Forced)).collect()
    }

    fn micro_sum(xs: impl Iterator<Item = f64>) -> i64 {
        xs.map(|x| (x * MICRO).round() as i64).sum()
    }

    #[test]
    fn the_groups_of_every_breakdown_add_up_to_the_total_loss() {
        for seed in 0..200 {
            let (hands, decisions) = tournament(seed, 40);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            let total = micro(r.score.loss_bb);
            assert_eq!(r.breakdowns.len(), 4);
            for b in &r.breakdowns {
                assert_eq!(micro_sum(b.groups.iter().map(|g| g.loss_bb)), total, "seed {seed}, {}", b.key);
            }
        }
    }

    #[test]
    fn every_rated_decision_counts_exactly_once() {
        for seed in 0..200 {
            let (hands, decisions) = tournament(seed, 40);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            let want = counted(&decisions).len();
            assert_eq!(r.score.decisions, want, "seed {seed}");
            assert_eq!(r.score.ratings.iter().sum::<usize>(), want, "seed {seed}");
            for b in &r.breakdowns {
                assert_eq!(b.groups.iter().map(|g| g.decisions).sum::<usize>(), want, "seed {seed}, {}", b.key);
                let keys: std::collections::BTreeSet<&str> = b.groups.iter().map(|g| g.key.as_str()).collect();
                assert_eq!(keys.len(), b.groups.len(), "seed {seed}: a group twice in {}", b.key);
            }
            let total_loss = micro_sum(counted(&decisions).iter().map(|d| d.row.ev_loss_bb.unwrap_or(0.0)));
            assert_eq!(micro(r.score.loss_bb), total_loss, "seed {seed}");
        }
    }

    #[test]
    fn decisions_of_other_hands_do_not_count() {
        let (hands, mut decisions) = tournament(7, 20);
        let before = report(&hands, &decisions, 0, "2026/11/18", &[]);
        let mut stranger = decisions[0].clone();
        stranger.row.hand_id = 1;
        stranger.row.status = Status::Rated;
        stranger.row.rating = Some(Rating::Blunder);
        stranger.row.ev_loss_bb = Some(50.0);
        decisions.push(stranger);
        assert_eq!(report(&hands, &decisions, 0, "2026/11/18", &[]), before);
    }

    #[test]
    fn luck_is_the_result_minus_the_all_in_expectation() {
        for seed in 0..200 {
            let (hands, decisions) = tournament(seed, 60);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            let net: i64 = hands.iter().map(|h| h.net).sum();
            let ev: i64 = hands.iter().map(|h| h.ev.unwrap_or(h.net)).sum();
            assert_eq!(r.luck.net, net);
            assert_eq!(r.luck.ev, ev);
            assert_eq!(r.luck.luck, net - ev);
            assert!((r.luck.luck_bb - (r.luck.net_bb - r.luck.ev_bb)).abs() < 1e-9);
            assert_eq!(r.luck.all_ins, hands.iter().filter(|h| h.ev.is_some()).count());
            // Without an all-in there is no luck to speak of.
            let calm: Vec<ReportHand> = hands.iter().cloned().map(|h| ReportHand { ev: None, ..h }).collect();
            let c = report(&calm, &decisions, 0, "2026/11/18", &[]);
            assert_eq!(c.luck.luck, 0);
            assert!(c.luck.luck_bb.abs() < 1e-9);
        }
    }

    #[test]
    fn the_costliest_mistakes_are_the_costliest() {
        for seed in 0..200 {
            let (hands, decisions) = tournament(seed, 50);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            assert!(r.worst.len() <= MOMENTS);
            assert!(r.worst.windows(2).all(|w| w[0].loss_bb >= w[1].loss_bb), "seed {seed}");
            let shown: Vec<(u64, usize)> = r.worst.iter().map(|m| (m.hand_id, m.action_index)).collect();
            let flawed: Vec<&ReportDecision> = counted(&decisions)
                .into_iter()
                .filter(|d| d.row.rating.is_some_and(|x| x >= Rating::Inaccuracy) && d.row.ev_loss_bb.unwrap_or(0.0) > 0.0)
                .collect();
            assert_eq!(r.worst.len(), flawed.len().min(MOMENTS), "seed {seed}");
            if let Some(last) = r.worst.last() {
                for d in flawed.iter().filter(|d| !shown.contains(&(d.row.hand_id, d.row.action_index))) {
                    assert!(micro(d.row.ev_loss_bb.unwrap()) <= micro(last.loss_bb), "seed {seed}: a costlier mistake left out");
                }
            }
            for m in &r.worst {
                assert!(m.rating.is_some_and(|x| x >= Rating::Inaccuracy));
                assert_eq!(hands[m.n - 1].hand_id, m.hand_id);
            }
        }
    }

    #[test]
    fn the_best_decisions_are_rated_best_and_gain_something() {
        for seed in 0..200 {
            let (hands, decisions) = tournament(seed, 50);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            assert!(r.best.len() <= MOMENTS);
            for m in &r.best {
                assert!(matches!(m.rating, Some(Rating::Best | Rating::Mix)), "seed {seed}");
                assert!(m.margin_bb.is_none_or(|x| x > 0.0), "seed {seed}");
            }
            assert!(r.best.windows(2).all(|w| w[0].margin_bb.unwrap_or(0.0) >= w[1].margin_bb.unwrap_or(0.0) || w[1].margin_bb.is_none()));
        }
    }

    #[test]
    fn starting_hands_and_the_timeline_cover_every_hand_once() {
        for seed in 0..100 {
            let (hands, decisions) = tournament(seed, 50);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            let with_class: Vec<&ReportHand> = hands.iter().filter(|h| h.class.is_some()).collect();
            assert_eq!(r.starting.iter().map(|s| s.hands).sum::<usize>(), with_class.len());
            let net_bb: f64 = with_class.iter().map(|h| h.net as f64 / h.big_blind as f64).sum();
            assert!((r.starting.iter().map(|s| s.net_bb).sum::<f64>() - net_bb).abs() < 1e-6);
            let ids: Vec<u64> = r.starting.iter().flat_map(|s| s.hand_ids.clone()).collect();
            assert_eq!(ids.len(), with_class.len());
            assert_eq!(r.timeline.len(), hands.len());
            assert!(r.timeline.iter().enumerate().all(|(i, p)| p.n == i + 1 && p.hand_id == hands[i].hand_id));
            assert_eq!(micro_sum(r.timeline.iter().map(|p| p.loss_bb)), micro(r.score.loss_bb), "seed {seed}");
            let marked = r.timeline.iter().filter(|p| p.mistake.is_some()).count();
            let distinct: std::collections::BTreeSet<u64> = r.worst.iter().map(|m| m.hand_id).collect();
            assert_eq!(marked, distinct.len());
        }
    }

    #[test]
    fn kinds_of_spot_hold_every_loss_costliest_first() {
        for seed in 0..100 {
            let (hands, decisions) = tournament(seed, 50);
            let r = report(&hands, &decisions, 0, "2026/11/18", &[]);
            let mut by_kind: BTreeMap<String, i64> = BTreeMap::new();
            for d in counted(&decisions) {
                *by_kind.entry(kind_of(&d.row).unwrap().key()).or_default() += micro(d.row.ev_loss_bb.unwrap_or(0.0));
            }
            by_kind.retain(|_, loss| *loss > 0);
            assert_eq!(r.kinds.len(), by_kind.len(), "seed {seed}");
            assert!(r.kinds.iter().all(|k| micro(k.loss_bb) == by_kind[&k.key]), "seed {seed}");
            assert!(r.kinds.windows(2).all(|w| w[0].loss_bb >= w[1].loss_bb), "seed {seed}");
        }
    }

    #[test]
    fn pending_decisions_make_the_report_preliminary() {
        let (hands, decisions) = tournament(3, 60);
        let r = report(&hands, &decisions, 4, "2026/11/18", &[]);
        assert_eq!(r.pending, decisions.iter().filter(|d| d.row.status == Status::Pending).count());
        assert!(r.pending > 0);
        assert_eq!(r.unanalyzed, 4);
    }

    #[test]
    fn the_comparison_takes_only_earlier_tournaments() {
        let past = |id: u64, at: &str, decisions: usize, good: usize, loss: f64| PastScore {
            tournament_id: id,
            started_at: at.into(),
            name: None,
            decisions,
            good,
            loss_bb: loss,
        };
        let all = [
            past(1, "2026/11/01 20:00:00", 100, 80, 10.0),
            past(2, "2026/11/05 20:00:00", 50, 45, 2.0),
            past(3, "2026/11/18 20:00:00", 40, 30, 8.0),
            past(4, "2026/11/20 20:00:00", 40, 30, 8.0),
            past(5, "2026/11/02 20:00:00", 0, 0, 0.0),
        ];
        let (history, avg) = compare("2026/11/18 20:00:00", &all);
        assert_eq!(history.iter().map(|h| h.tournament_id).collect::<Vec<_>>(), vec![2, 1]);
        let avg = avg.unwrap();
        assert_eq!((avg.tournaments, avg.decisions), (2, 150));
        assert!((avg.per100.unwrap() - 8.0).abs() < 1e-9);
        assert!((avg.good_pct.unwrap() - 125.0 / 150.0 * 100.0).abs() < 1e-9);
        assert_eq!(compare("2026/10/01", &all), (Vec::new(), None));
    }
}
