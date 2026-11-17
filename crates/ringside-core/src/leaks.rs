//! Leak finder: the hero's stats per position and stack depth against published
//! reference ranges. The ranges are guide values from the sources named with each,
//! not a correct strategy; a leak is a value outside its range, ranked by how sure
//! the sample makes it times how far off it is.

use serde::Serialize;

use crate::model::GameKind;
use crate::stats::{Counters, StatId};

/// Stack depth bands in big blinds (effective stack): below 10, 10–20, 20–40, from 40.
pub const BANDS: [&str; 4] = ["unter 10 BB", "10–20 BB", "20–40 BB", "ab 40 BB"];
const BAND_EDGES: [f64; 3] = [10.0, 20.0, 40.0];

pub fn band_of(stack_bb: f64) -> u8 {
    BAND_EDGES.iter().filter(|&&e| stack_bb >= e).count() as u8
}

/// The same banding in SQL, for a column holding the stack in BB.
pub fn band_sql(col: &str) -> String {
    format!(
        "(CASE WHEN {col} IS NULL THEN NULL WHEN {col} < {} THEN 0 WHEN {col} < {} THEN 1 WHEN {col} < {} THEN 2 ELSE 3 END)",
        BAND_EDGES[0], BAND_EDGES[1], BAND_EDGES[2]
    )
}

/// Counter columns of a stat in `player_hands`: (made, opportunities). For AF, the
/// columns of aggressive actions and calls.
pub fn columns(stat: StatId) -> Option<(&'static str, &'static str)> {
    Some(match stat {
        StatId::Vpip => ("vpip", "vpip_opp"),
        StatId::Pfr => ("pfr", "vpip_opp"),
        StatId::ThreeBet => ("three_bet", "three_bet_opp"),
        StatId::FoldToThreeBet => ("fold_to_three_bet", "fold_to_three_bet_opp"),
        StatId::Steal => ("steal", "steal_opp"),
        StatId::FoldToSteal => ("fold_to_steal", "fold_to_steal_opp"),
        StatId::Cbet => ("cbet", "cbet_opp"),
        StatId::FoldToCbet => ("fold_to_cbet", "fold_to_cbet_opp"),
        StatId::Wtsd => ("wtsd", "saw_flop"),
        StatId::Wsd => ("wsd", "wtsd"),
        StatId::Af => ("postflop_aggr", "postflop_calls"),
        StatId::Hands | StatId::Afq => return None,
    })
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Source {
    pub id: &'static str,
    pub title: &'static str,
    pub publisher: &'static str,
    pub url: Option<&'static str>,
    /// What the values apply to, as the source says.
    pub scope: &'static str,
}

pub const SOURCES: &[Source] = &[
    Source {
        id: "pcg",
        title: "HUD Stats Glossary: VPIP, PFR, 3-Bet% and 20 More Explained",
        publisher: "Poker Club Games",
        url: Some("https://pokerclubgames.com/poker-news/hud-stats-glossary-vpip-pfr-3-bet-and-20-more-explained/"),
        scope: "Grobe Richtwerte für einen soliden Regular, Online 6-max No-Limit Cash",
    },
    Source {
        id: "pokercoaching",
        title: "How to Use Poker HUD Stats to Exploit Weak Players",
        publisher: "PokerCoaching.com",
        url: Some("https://pokercoaching.com/blog/poker-stats/"),
        scope: "Typische Werte guter Spieler, 6-max und Full Ring; VPIP auch für 6-max-Turniere",
    },
    Source {
        id: "blackrain79",
        title: "What Are The Best Poker HUD Stats?",
        publisher: "BlackRain79 (Nathan Williams)",
        url: Some("https://www.blackrain79.com/2017/10/what-are-the-best-poker-hud-stats.html"),
        scope: "Faustregeln für Cash NL2–NL25, 6-max und Full Ring",
    },
    Source {
        id: "bbz",
        title: "Preflop Strategy for MTTs: Ranges by Position at 40BB",
        publisher: "BBZ Poker",
        url: Some("https://bbzpoker.com/preflop-strategy-tournament-poker/"),
        scope: "Eröffnungsranges je Position, Turnier mit 8 Spielern, 40 BB",
    },
    Source {
        id: "rangerpro",
        title: "Short stack MTT strategy: push-fold and open ranges",
        publisher: "RangerPro",
        url: Some("https://rangerpro.fr/poker-guide/short-stack"),
        scope: "Eröffnungsbreite je Position bei 15–25 BB im Turnier",
    },
    Source {
        id: "nash",
        title: "Push/Fold-Gleichgewicht (Chip-EV)",
        publisher: "Ringside-Push/Fold-Tabellen",
        url: None,
        scope: "Nash-Gleichgewicht für 8 Spieler mit Big-Blind-Ante bei 5, 7,5 und 10 BB; im Bereich Push/Fold nachrechenbar",
    },
];

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Cite {
    pub source: &'static str,
    /// The value as the source gives it.
    pub says: &'static str,
}

/// A guide range for one stat. `positions` empty means all positions together;
/// every band in `bands` is checked on its own.
#[derive(Debug, Clone, Copy)]
pub struct Reference {
    pub stat: StatId,
    pub game: GameKind,
    pub positions: &'static [&'static str],
    pub bands: &'static [u8],
    pub low: f64,
    pub high: f64,
    pub cites: &'static [Cite],
    pub note: Option<&'static str>,
}

const fn cite(source: &'static str, says: &'static str) -> Cite {
    Cite { source, says }
}

const CASH: GameKind = GameKind::Cash;
const MTT: GameKind = GameKind::Tournament;
/// Cash references are for 100 BB games.
const DEEP: &[u8] = &[3];
/// Postflop references come from cash sources; in tournaments they apply from 20 BB.
const MTT_DEEP: &[u8] = &[2, 3];
const ALL: &[&str] = &[];
const FROM_CASH: Option<&str> = Some("Aus Cash-Quellen übernommen; im Turnier erst ab 20 BB geprüft.");
const AROUND: Option<&str> = Some("Eine Angabe „um X“ ist hier als X ± 10 Punkte gelesen.");
const CHART: Option<&str> = Some("Chartwerte sind Eröffnungen, wenn alle vor dir gefoldet haben; der Bereich lässt einige Punkte Spiel.");

pub const REFERENCES: &[Reference] = &[
    // Cash, 100 BB.
    Reference { stat: StatId::Vpip, game: CASH, positions: ALL, bands: DEEP, low: 22.0, high: 28.0,
        cites: &[cite("pcg", "22–28 %"), cite("pokercoaching", "6-max Cash: 22 bis 28")], note: None },
    Reference { stat: StatId::Pfr, game: CASH, positions: ALL, bands: DEEP, low: 17.0, high: 23.0,
        cites: &[cite("pcg", "17–23 %"), cite("pokercoaching", "6-max: 17 bis 22")], note: None },
    Reference { stat: StatId::ThreeBet, game: CASH, positions: ALL, bands: DEEP, low: 6.0, high: 10.0,
        cites: &[cite("pcg", "6–10 %"), cite("pokercoaching", "5 bis 10 %")], note: None },
    Reference { stat: StatId::FoldToThreeBet, game: CASH, positions: ALL, bands: DEEP, low: 40.0, high: 55.0,
        cites: &[cite("pcg", "40–55 %"), cite("pokercoaching", "um 50 %")], note: None },
    Reference { stat: StatId::Steal, game: CASH, positions: &["CO", "BTN", "SB"], bands: DEEP, low: 30.0, high: 45.0,
        cites: &[cite("pcg", "ATS 30–45 %"), cite("pokercoaching", "ATS um 35 (6-max)"), cite("blackrain79", "ATS 35 (6-max)")],
        note: None },
    Reference { stat: StatId::FoldToSteal, game: CASH, positions: &["BB"], bands: DEEP, low: 30.0, high: 60.0,
        cites: &[cite("pcg", "Big Blind 30–45 %"), cite("pokercoaching", "um 60")],
        note: Some("Die Quellen liegen weit auseinander; der Bereich umfasst beide.") },
    Reference { stat: StatId::Cbet, game: CASH, positions: ALL, bands: DEEP, low: 50.0, high: 70.0,
        cites: &[cite("pcg", "Flop 50–65 %"), cite("pokercoaching", "um 70"), cite("blackrain79", "Flop 70")], note: None },
    Reference { stat: StatId::FoldToCbet, game: CASH, positions: ALL, bands: DEEP, low: 40.0, high: 60.0,
        cites: &[cite("pcg", "Flop 40–55 %"), cite("pokercoaching", "um 60"), cite("blackrain79", "Flop 60")], note: None },
    Reference { stat: StatId::Wtsd, game: CASH, positions: ALL, bands: DEEP, low: 24.0, high: 30.0,
        cites: &[cite("pcg", "24–30 %"), cite("pokercoaching", "um 30"), cite("blackrain79", "27 (6-max)")], note: None },
    Reference { stat: StatId::Wsd, game: CASH, positions: ALL, bands: DEEP, low: 48.0, high: 55.0,
        cites: &[cite("pcg", "48–55 %")], note: None },
    Reference { stat: StatId::Af, game: CASH, positions: ALL, bands: DEEP, low: 2.0, high: 4.0,
        cites: &[cite("pcg", "2–4"), cite("pokercoaching", "um 3"), cite("blackrain79", "3")], note: None },
    // Tournaments.
    Reference { stat: StatId::Vpip, game: MTT, positions: ALL, bands: MTT_DEEP, low: 15.0, high: 30.0,
        cites: &[cite("pokercoaching", "6-max-Turnier: 20 bis 30, je nach Stacktiefe; Full Ring: 15 bis 20")],
        note: Some("Der Bereich umfasst Tische mit 6 bis 9 Spielern.") },
    Reference { stat: StatId::Pfr, game: MTT, positions: ALL, bands: MTT_DEEP, low: 10.0, high: 22.0,
        cites: &[cite("pokercoaching", "6-max: 17 bis 22; Full Ring: 10 bis 15")],
        note: Some("Der Bereich umfasst Tische mit 6 bis 9 Spielern.") },
    Reference { stat: StatId::ThreeBet, game: MTT, positions: ALL, bands: &[1, 2, 3], low: 5.0, high: 10.0,
        cites: &[cite("pokercoaching", "5 bis 10 %")], note: None },
    Reference { stat: StatId::FoldToThreeBet, game: MTT, positions: ALL, bands: MTT_DEEP, low: 40.0, high: 55.0,
        cites: &[cite("pcg", "40–55 %"), cite("pokercoaching", "um 50 %")], note: FROM_CASH },
    Reference { stat: StatId::Steal, game: MTT, positions: &["CO"], bands: &[0], low: 30.0, high: 45.0,
        cites: &[cite("nash", "Shove 33 % bei 10 BB, 35 % bei 7,5 BB, 42 % bei 5 BB")], note: None },
    Reference { stat: StatId::Steal, game: MTT, positions: &["BTN"], bands: &[0], low: 38.0, high: 52.0,
        cites: &[cite("nash", "Shove 41 % bei 10 BB, 46 % bei 7,5 BB, 49 % bei 5 BB")], note: None },
    Reference { stat: StatId::Steal, game: MTT, positions: &["SB"], bands: &[0], low: 68.0, high: 86.0,
        cites: &[cite("nash", "Shove 72 % bei 10 BB, 77 % bei 7,5 BB, 84 % bei 5 BB")], note: None },
    Reference { stat: StatId::Steal, game: MTT, positions: &["CO"], bands: &[1], low: 25.0, high: 37.0,
        cites: &[cite("rangerpro", "Cutoff ~28 % bei 15 BB, ~34 % bei 20 BB")], note: CHART },
    Reference { stat: StatId::Steal, game: MTT, positions: &["BTN"], bands: &[1], low: 33.0, high: 48.0,
        cites: &[cite("rangerpro", "Button ~36 % bei 15 BB, ~45 % bei 20 BB")], note: CHART },
    Reference { stat: StatId::Steal, game: MTT, positions: &["CO"], bands: &[2], low: 30.0, high: 42.0,
        cites: &[cite("rangerpro", "Cutoff 36,8 % bei 25 BB"), cite("bbz", "Cutoff ~36 % bei 40 BB")], note: CHART },
    Reference { stat: StatId::Steal, game: MTT, positions: &["BTN"], bands: &[2], low: 42.0, high: 56.0,
        cites: &[cite("rangerpro", "Button 47,8 % bei 25 BB"), cite("bbz", "Button ~51 % bei 40 BB")], note: CHART },
    Reference { stat: StatId::Steal, game: MTT, positions: &["CO"], bands: &[3], low: 30.0, high: 42.0,
        cites: &[cite("bbz", "Cutoff ~36 % bei 40 BB")], note: CHART },
    Reference { stat: StatId::Steal, game: MTT, positions: &["BTN"], bands: &[3], low: 45.0, high: 57.0,
        cites: &[cite("bbz", "Button ~51 % bei 40 BB")], note: CHART },
    Reference { stat: StatId::FoldToSteal, game: MTT, positions: &["SB"], bands: &[0], low: 55.0, high: 77.0,
        cites: &[cite("nash", "Small Blind gegen Cutoff/Button: Fold 72–77 % bei 10 BB, 66–71 % bei 7,5 BB, 55–58 % bei 5 BB")],
        note: None },
    Reference { stat: StatId::FoldToSteal, game: MTT, positions: &["SB", "BB"], bands: &[1, 2, 3], low: 50.0, high: 70.0,
        cites: &[cite("pokercoaching", "um 60 (Turnier-Wert)")], note: AROUND },
    Reference { stat: StatId::Cbet, game: MTT, positions: ALL, bands: MTT_DEEP, low: 50.0, high: 70.0,
        cites: &[cite("pcg", "Flop 50–65 %"), cite("pokercoaching", "um 70"), cite("blackrain79", "Flop 70")], note: FROM_CASH },
    Reference { stat: StatId::FoldToCbet, game: MTT, positions: ALL, bands: MTT_DEEP, low: 40.0, high: 60.0,
        cites: &[cite("pcg", "Flop 40–55 %"), cite("pokercoaching", "um 60"), cite("blackrain79", "Flop 60")], note: FROM_CASH },
    Reference { stat: StatId::Wtsd, game: MTT, positions: ALL, bands: MTT_DEEP, low: 24.0, high: 30.0,
        cites: &[cite("pcg", "24–30 %"), cite("pokercoaching", "um 30"), cite("blackrain79", "27 (6-max)")], note: FROM_CASH },
    Reference { stat: StatId::Wsd, game: MTT, positions: ALL, bands: MTT_DEEP, low: 48.0, high: 55.0,
        cites: &[cite("pcg", "48–55 %")], note: FROM_CASH },
    Reference { stat: StatId::Af, game: MTT, positions: ALL, bands: MTT_DEEP, low: 2.0, high: 4.0,
        cites: &[cite("pcg", "2–4"), cite("pokercoaching", "um 3"), cite("blackrain79", "3")], note: FROM_CASH },
];

/// Hero's hands of one game, position and stack band, summed.
#[derive(Debug, Clone, Default)]
pub struct Cell {
    pub game: Option<GameKind>,
    pub position: Option<String>,
    pub band: Option<u8>,
    pub counters: Counters,
    /// Sum and sum of squares of the result per hand in BB, for BB/100 and its spread.
    pub net_bb: f64,
    pub net_bb_sq: f64,
}

/// Fewer chances than this and a value is not checked at all.
pub const MIN_CHECKED: i64 = 10;
/// Fewer chances than this and a leak counts as uncertain however far off it is.
pub const MIN_SURE: i64 = 30;
/// Below this one-sided confidence a leak counts as uncertain.
pub const SURE: f64 = 0.9;
const Z95: f64 = 1.959_964;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    TooHigh,
    TooLow,
}

/// What a leak costs, roughly: the hero's BB/100 in its spot against their own
/// BB/100 at the same positions over all stack depths. Only given when the spot is
/// a real part of that and the gap is clear at 95 %.
#[derive(Debug, Clone, Serialize)]
pub struct Cost {
    pub hands: i64,
    pub spot_bb100: f64,
    pub base_bb100: f64,
    /// 95 % margin of `spot_bb100`.
    pub margin_bb100: f64,
    /// `(base − spot) × hands / 100`, in BB.
    pub total_bb: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub stat: StatId,
    pub game: GameKind,
    /// Empty for all positions.
    pub positions: Vec<&'static str>,
    pub band: u8,
    pub made: i64,
    pub opp: i64,
    /// Percent, or the plain ratio for AF.
    pub value: f64,
    /// 95 % Wilson interval, same unit.
    pub ci_low: f64,
    pub ci_high: f64,
    pub low: f64,
    pub high: f64,
    /// Outside the range, or `None` inside.
    pub direction: Option<Direction>,
    /// Distance to the range, same unit.
    pub deviation: f64,
    /// One-sided probability that the true value is outside the range on this side.
    pub confidence: f64,
    /// A leak with fewer than [`MIN_SURE`] chances or below [`SURE`] confidence.
    pub uncertain: bool,
    /// Confidence × deviation relative to the range width.
    pub score: f64,
    pub cost: Option<Cost>,
    pub cites: Vec<Cite>,
    pub note: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeakReport {
    /// Values outside their range, surest and largest first; uncertain ones last.
    pub leaks: Vec<Check>,
    /// Values inside their range.
    pub fine: Vec<Check>,
    /// Spots with a reference but fewer than [`MIN_CHECKED`] chances.
    pub too_small: usize,
    pub bands: [&'static str; 4],
    pub sources: &'static [Source],
}

/// Wilson score interval for `k` of `n` at normal quantile `z`, as fractions.
pub fn wilson(k: i64, n: i64, z: f64) -> (f64, f64) {
    if n <= 0 {
        return (0.0, 1.0);
    }
    let n = n as f64;
    let p = k as f64 / n;
    let z2 = z * z;
    let centre = p + z2 / (2.0 * n);
    let spread = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    let d = 1.0 + z2 / n;
    (((centre - spread) / d).max(0.0), ((centre + spread) / d).min(1.0))
}

/// Standard normal distribution function (Abramowitz–Stegun 7.1.26, error below 1e-7).
fn phi(z: f64) -> f64 {
    let x = z.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-x * x).exp();
    if z >= 0.0 { 0.5 * (1.0 + erf) } else { 0.5 * (1.0 - erf) }
}

/// How sure `k` of `n` puts the true rate above `edge` (or below it with `above` false):
/// Φ(z) for the z at which the Wilson bound meets the edge.
pub fn confidence(k: i64, n: i64, edge: f64, above: bool) -> f64 {
    let p = k as f64 / n.max(1) as f64;
    if n <= 0 || (above && p <= edge) || (!above && p >= edge) {
        return 0.0;
    }
    let bound = |z: f64| {
        let (lo, hi) = wilson(k, n, z);
        if above { lo } else { hi }
    };
    let (mut lo, mut hi) = (0.0, 12.0);
    if (above && bound(hi) > edge) || (!above && bound(hi) < edge) {
        return 1.0;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let outside = if above { bound(mid) > edge } else { bound(mid) < edge };
        if outside { lo = mid } else { hi = mid }
    }
    phi(lo)
}

/// Frequencies are k of n; AF = aggressive ÷ calls becomes a frequency of aggressive
/// actions among both, so its value and range map through `x / (1 + x)`.
fn to_rate(stat: StatId, x: f64) -> f64 {
    if stat == StatId::Af { x / (1.0 + x) } else { x / 100.0 }
}

fn from_rate(stat: StatId, p: f64) -> f64 {
    if stat == StatId::Af {
        if p >= 1.0 { f64::INFINITY } else { p / (1.0 - p) }
    } else {
        100.0 * p
    }
}

/// Made and chances of a stat on the frequency scale.
fn trials(stat: StatId, c: &Counters) -> (i64, i64) {
    let (made, opp) = stat.parts(c);
    if stat == StatId::Af { (made, made + opp) } else { (made, opp) }
}

fn check(r: &Reference, band: u8, cell: &Cell, base: &Cell) -> Option<Check> {
    let (k, n) = trials(r.stat, &cell.counters);
    if n < MIN_CHECKED {
        return None;
    }
    let p = k as f64 / n as f64;
    let (lo, hi) = (to_rate(r.stat, r.low), to_rate(r.stat, r.high));
    let (ci_lo, ci_hi) = wilson(k, n, Z95);
    let (made, opp) = r.stat.parts(&cell.counters);
    let value = r.stat.value(&cell.counters).unwrap_or_else(|| from_rate(r.stat, p));
    let (direction, conf, rel) = if p > hi {
        (Some(Direction::TooHigh), confidence(k, n, hi, true), (p - hi) / (hi - lo))
    } else if p < lo {
        (Some(Direction::TooLow), confidence(k, n, lo, false), (lo - p) / (hi - lo))
    } else {
        (None, 0.0, 0.0)
    };
    let deviation = match direction {
        Some(Direction::TooHigh) => value - r.high,
        Some(Direction::TooLow) => r.low - value,
        None => 0.0,
    };
    Some(Check {
        stat: r.stat,
        game: r.game,
        positions: r.positions.to_vec(),
        band,
        made,
        opp,
        value,
        ci_low: from_rate(r.stat, ci_lo),
        ci_high: from_rate(r.stat, ci_hi),
        low: r.low,
        high: r.high,
        direction,
        deviation,
        confidence: conf,
        uncertain: direction.is_some() && (n < MIN_SURE || conf < SURE),
        score: conf * rel,
        cost: direction.and_then(|_| cost(cell, base)),
        cites: r.cites.to_vec(),
        note: r.note,
    })
}

/// Hands needed in a spot before its result is compared at all.
const MIN_COST_HANDS: i64 = 200;

fn cost(cell: &Cell, base: &Cell) -> Option<Cost> {
    let n = cell.counters.hands;
    let m = base.counters.hands;
    if n < MIN_COST_HANDS || n >= m {
        return None;
    }
    let mean = cell.net_bb / n as f64;
    let var = (cell.net_bb_sq / n as f64 - mean * mean).max(0.0) * n as f64 / (n - 1) as f64;
    let margin = Z95 * (var / n as f64).sqrt() * 100.0;
    let spot = 100.0 * mean;
    let base_rate = 100.0 * base.net_bb / m as f64;
    (spot + margin < base_rate).then(|| Cost {
        hands: n,
        spot_bb100: spot,
        base_bb100: base_rate,
        margin_bb100: margin,
        total_bb: (base_rate - spot) * n as f64 / 100.0,
    })
}

fn sum<'a>(cells: impl Iterator<Item = &'a Cell>) -> Cell {
    let mut out = Cell::default();
    for c in cells {
        out.counters.add(&c.counters);
        out.net_bb += c.net_bb;
        out.net_bb_sq += c.net_bb_sq;
    }
    out
}

/// Checks every reference against the hero's cells.
pub fn find(cells: &[Cell]) -> LeakReport {
    let mut leaks = Vec::new();
    let mut fine = Vec::new();
    let mut too_small = 0;
    for r in REFERENCES {
        let at = |c: &&Cell| {
            c.game == Some(r.game)
                && (r.positions.is_empty() || c.position.as_deref().is_some_and(|p| r.positions.contains(&p)))
        };
        let base = sum(cells.iter().filter(at));
        for &band in r.bands {
            let cell = sum(cells.iter().filter(at).filter(|c| c.band == Some(band)));
            match check(r, band, &cell, &base) {
                Some(c) if c.direction.is_some() => leaks.push(c),
                Some(c) => fine.push(c),
                None if r.stat.parts(&cell.counters).1 > 0 || cell.counters.hands > 0 => too_small += 1,
                None => {}
            }
        }
    }
    leaks.sort_by(|a: &Check, b: &Check| a.uncertain.cmp(&b.uncertain).then(b.score.total_cmp(&a.score)));
    LeakReport { leaks, fine, too_small, bands: BANDS, sources: SOURCES }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from Wilson's formula worked by hand.
    #[test]
    fn wilson_matches_hand_calculation() {
        // 20 of 100: centre 0.2 + 1.9208/100 = 0.219208; spread 1.96 × √(0.0016 + 0.0000960) = 0.080721;
        // divisor 1.038415 → 0.13342 … 0.28883.
        let (lo, hi) = wilson(20, 100, Z95);
        assert!((lo - 0.13342).abs() < 1e-4 && (hi - 0.28883).abs() < 1e-4, "{lo} {hi}");
        // 0 of 10 still has an upper bound: z² / (n + z²) = 3.8415 / 13.8415.
        let (lo, hi) = wilson(0, 10, Z95);
        assert_eq!(lo, 0.0);
        assert!((hi - 0.27753).abs() < 1e-4, "{hi}");
    }

    #[test]
    fn confidence_grows_with_the_sample() {
        assert!((phi(Z95) - 0.975).abs() < 1e-6);
        // 40 % against an upper edge of 30 %.
        let small = confidence(4, 10, 0.30, true);
        let large = confidence(400, 1000, 0.30, true);
        assert!(small > 0.5 && small < 0.8, "{small}");
        assert!(large > 0.9999, "{large}");
        // Where the 95 % Wilson bound meets the edge, the one-sided confidence is 97.5 %.
        let (lo, _) = wilson(40, 100, Z95);
        assert!((confidence(40, 100, lo, true) - 0.975).abs() < 1e-3);
        assert_eq!(confidence(20, 100, 0.30, true), 0.0);
    }

    fn cell(game: GameKind, pos: &str, band: u8, c: Counters, hands: i64, net_bb: f64) -> Cell {
        Cell {
            game: Some(game),
            position: Some(pos.into()),
            band: Some(band),
            counters: Counters { hands, ..c },
            net_bb,
            net_bb_sq: net_bb * net_bb / hands as f64 + hands as f64 * 4.0,
        }
    }

    #[test]
    fn finds_leaks_ranks_them_and_marks_small_samples() {
        let cells = vec![
            // Button at 10–20 BB steals 70 of 100 (range 33–48): a sure leak.
            cell(MTT, "BTN", 1, Counters { steal: 70, steal_opp: 100, ..Default::default() }, 400, -40.0),
            // Cutoff at 10–20 BB steals 8 of 12 (range 25–37): outside, but a small sample.
            cell(MTT, "CO", 1, Counters { steal: 8, steal_opp: 12, ..Default::default() }, 300, 0.0),
            // Cutoff at 20–40 BB steals 36 of 100: inside 30–42.
            cell(MTT, "CO", 2, Counters { steal: 36, steal_opp: 100, ..Default::default() }, 300, 0.0),
            // Button deep, lots of hands winning: the base for the button's cost.
            cell(MTT, "BTN", 3, Counters::default(), 2000, 400.0),
        ];
        let r = find(&cells);
        let btn = &r.leaks[0];
        assert_eq!((btn.stat, btn.positions.as_slice(), btn.band), (StatId::Steal, &["BTN"][..], 1));
        assert_eq!(btn.direction, Some(Direction::TooHigh));
        assert!((btn.deviation - 22.0).abs() < 1e-9 && !btn.uncertain && btn.confidence > 0.99);
        let co = r.leaks.iter().find(|l| l.positions == ["CO"]).expect("cutoff leak");
        assert!(co.uncertain);
        assert!(r.leaks.last().unwrap().uncertain);
        assert!(r.fine.iter().any(|c| c.stat == StatId::Steal && c.positions == ["CO"] && c.band == 2));
        // Button: −10 BB/100 in the spot against (−40 + 400) / 2400 hands = +15 BB/100 overall.
        let cost = btn.cost.as_ref().expect("clear gap");
        assert!((cost.spot_bb100 + 10.0).abs() < 1e-9 && (cost.base_bb100 - 15.0).abs() < 1e-9);
        assert!((cost.total_bb - 100.0).abs() < 1e-9);
        // A spot that is the whole base has nothing to be compared with.
        assert!(co.cost.is_none());
    }

    #[test]
    fn af_is_checked_as_a_share_of_aggressive_actions() {
        // 30 bets/raises, 30 calls: AF 1 against 2–4.
        let c = Counters { postflop_aggr: 30, postflop_calls: 30, ..Default::default() };
        let r = find(&[cell(CASH, "BTN", 3, c, 500, 0.0)]);
        let af = r.leaks.iter().find(|l| l.stat == StatId::Af).expect("AF leak");
        assert_eq!(af.direction, Some(Direction::TooLow));
        assert!((af.value - 1.0).abs() < 1e-9 && (af.deviation - 1.0).abs() < 1e-9);
        assert!(af.ci_low < 1.0 && af.ci_high > 1.0);
    }

    #[test]
    fn bands_split_at_their_edges() {
        assert_eq!([band_of(9.99), band_of(10.0), band_of(19.9), band_of(20.0), band_of(40.0)], [0, 1, 1, 2, 3]);
    }
}
