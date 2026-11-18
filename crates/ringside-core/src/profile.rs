//! The hero's playing profile: every rated decision grouped by the kind of spot it was
//! (situation, seat, stack depth and table size), with how often it comes up, how often it
//! goes wrong and what it costs. The kind of spot follows from the situation alone, never
//! from what the hero did, so a fold and a 3-bet against the same open land in one group.
//!
//! Groups are ranked by what they cost in total (how often × how much); a group needs
//! [`MIN_DECISIONS`] rated decisions to be ranked, and a group whose loss comes mostly
//! from the estimate is marked as unsure.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::db::{Choice, Rating, Source, SpotType, Status};
use crate::leaks::{BANDS, band_of};

/// Rated decisions a group needs to be ranked as a weakness.
pub const MIN_DECISIONS: usize = 10;
/// Rated decisions before and after the first practice needed to show a change.
pub const MIN_PROGRESS: usize = 10;
/// Share of the loss from the estimate above which a group is marked unsure.
const UNSURE_SHARE: f64 = 0.5;
/// Losses are summed in millionths of a big blind, so sums and ties do not depend on the
/// order of the rows.
const MICRO: f64 = 1e6;

fn micro(bb: f64) -> i64 {
    (bb.max(0.0) * MICRO).round() as i64
}

/// The situation of a decision, from the spot type of the analysis and whether the hero
/// faced a bet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lage {
    /// First in with 20 BB or more.
    FirstIn,
    /// First in below 20 BB: shove or fold.
    PushFold,
    VsLimp,
    VsOpen,
    VsThreeBet,
    VsAllIn,
    CBet,
    VsCBet,
    FlopBet,
    FlopVsBet,
    TurnBet,
    TurnVsBet,
    RiverBet,
    RiverVsBet,
}

impl Lage {
    pub const ALL: [Lage; 14] = [
        Lage::FirstIn,
        Lage::PushFold,
        Lage::VsLimp,
        Lage::VsOpen,
        Lage::VsThreeBet,
        Lage::VsAllIn,
        Lage::CBet,
        Lage::VsCBet,
        Lage::FlopBet,
        Lage::FlopVsBet,
        Lage::TurnBet,
        Lage::TurnVsBet,
        Lage::RiverBet,
        Lage::RiverVsBet,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Lage::FirstIn => "first_in",
            Lage::PushFold => "push_fold",
            Lage::VsLimp => "vs_limp",
            Lage::VsOpen => "vs_open",
            Lage::VsThreeBet => "vs_three_bet",
            Lage::VsAllIn => "vs_all_in",
            Lage::CBet => "c_bet",
            Lage::VsCBet => "vs_c_bet",
            Lage::FlopBet => "flop_bet",
            Lage::FlopVsBet => "flop_vs_bet",
            Lage::TurnBet => "turn_bet",
            Lage::TurnVsBet => "turn_vs_bet",
            Lage::RiverBet => "river_bet",
            Lage::RiverVsBet => "river_vs_bet",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Lage::FirstIn => "Eröffnen",
            Lage::PushFold => "Shove oder Fold als Erster",
            Lage::VsLimp => "Gegen Limper",
            Lage::VsOpen => "Gegen Open",
            Lage::VsThreeBet => "Gegen 3-Bet",
            Lage::VsAllIn => "Gegen All-in",
            Lage::CBet => "C-Bet am Flop",
            Lage::VsCBet => "Gegen C-Bet am Flop",
            Lage::FlopBet => "Flop: setzen oder checken",
            Lage::FlopVsBet => "Flop gegen Bet",
            Lage::TurnBet => "Turn: setzen oder checken",
            Lage::TurnVsBet => "Turn gegen Bet",
            Lage::RiverBet => "River: setzen oder checken",
            Lage::RiverVsBet => "River gegen Bet",
        }
    }

    pub fn preflop(self) -> bool {
        matches!(self, Lage::FirstIn | Lage::PushFold | Lage::VsLimp | Lage::VsOpen | Lage::VsThreeBet | Lage::VsAllIn)
    }

    /// The spot types of the analysis this situation is made of, for the hand filter.
    pub fn spots(self) -> &'static [SpotType] {
        match self {
            Lage::FirstIn | Lage::PushFold => &[SpotType::Open, SpotType::Steal, SpotType::Shove],
            Lage::VsLimp => &[SpotType::VsLimp],
            Lage::VsOpen => &[SpotType::VsOpen, SpotType::ThreeBet],
            Lage::VsThreeBet => &[SpotType::VsThreeBet],
            Lage::VsAllIn => &[SpotType::CallShove],
            Lage::CBet => &[SpotType::CBet],
            Lage::VsCBet => &[SpotType::VsCBet],
            Lage::FlopBet | Lage::FlopVsBet => &[SpotType::Flop],
            Lage::TurnBet | Lage::TurnVsBet => &[SpotType::Turn],
            Lage::RiverBet | Lage::RiverVsBet => &[SpotType::River],
        }
    }

    fn parse(s: &str) -> Option<Lage> {
        Lage::ALL.into_iter().find(|l| l.key() == s)
    }
}

/// Seats grouped as a player thinks of them; every early and middle seat alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Seat {
    #[serde(rename = "EP")]
    Early,
    #[serde(rename = "MP")]
    Middle,
    #[serde(rename = "CO")]
    Cutoff,
    #[serde(rename = "BTN")]
    Button,
    #[serde(rename = "SB")]
    SmallBlind,
    #[serde(rename = "BB")]
    BigBlind,
}

impl Seat {
    const ALL: [Seat; 6] = [Seat::Early, Seat::Middle, Seat::Cutoff, Seat::Button, Seat::SmallBlind, Seat::BigBlind];

    pub fn of(position: &str) -> Option<Seat> {
        Some(match position {
            "BB" => Seat::BigBlind,
            "SB" => Seat::SmallBlind,
            "BTN" => Seat::Button,
            "CO" => Seat::Cutoff,
            "LJ" | "HJ" => Seat::Middle,
            p if p.starts_with("UTG") => Seat::Early,
            _ => return None,
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            Seat::Early => "EP",
            Seat::Middle => "MP",
            Seat::Cutoff => "CO",
            Seat::Button => "BTN",
            Seat::SmallBlind => "SB",
            Seat::BigBlind => "BB",
        }
    }

    /// The seats of `position_labels` in this group, for the hand filter.
    pub fn positions(self) -> &'static [&'static str] {
        match self {
            Seat::Early => &["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4"],
            Seat::Middle => &["LJ", "HJ"],
            Seat::Cutoff => &["CO"],
            Seat::Button => &["BTN"],
            Seat::SmallBlind => &["SB"],
            Seat::BigBlind => &["BB"],
        }
    }

    pub fn phrase(self) -> &'static str {
        match self {
            Seat::Early => "aus früher Position",
            Seat::Middle => "aus mittlerer Position",
            Seat::Cutoff => "im Cutoff",
            Seat::Button => "am Button",
            Seat::SmallBlind => "im Small Blind",
            Seat::BigBlind => "im Big Blind",
        }
    }

    fn parse(s: &str) -> Option<Seat> {
        Seat::ALL.into_iter().find(|x| x.key() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TableSize {
    HeadsUp,
    /// Three to five players, e.g. at a final table: the ranges differ a lot from 6-max.
    Few,
    Short,
    Full,
}

impl TableSize {
    const ALL: [TableSize; 4] = [TableSize::HeadsUp, TableSize::Few, TableSize::Short, TableSize::Full];

    pub fn of(players: usize) -> TableSize {
        match players {
            0..=2 => TableSize::HeadsUp,
            3..=5 => TableSize::Few,
            6 => TableSize::Short,
            _ => TableSize::Full,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            TableSize::HeadsUp => "2",
            TableSize::Few => "5",
            TableSize::Short => "6",
            TableSize::Full => "9",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            TableSize::HeadsUp => "Heads-up",
            TableSize::Few => "3–5 Spieler",
            TableSize::Short => "6 Spieler",
            TableSize::Full => "7–10 Spieler",
        }
    }

    /// Smallest and largest player count of the group.
    pub fn players(self) -> (usize, usize) {
        match self {
            TableSize::HeadsUp => (2, 2),
            TableSize::Few => (3, 5),
            TableSize::Short => (6, 6),
            TableSize::Full => (7, 10),
        }
    }

    fn parse(s: &str) -> Option<TableSize> {
        TableSize::ALL.into_iter().find(|x| x.key() == s)
    }
}

/// The kind of spot: situation, seat (before the flop only), stack band and table size.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpotKind {
    pub lage: Lage,
    pub seat: Option<Seat>,
    /// Index into [`BANDS`].
    pub band: Option<u8>,
    pub table: TableSize,
}

impl SpotKind {
    /// "vs_open|BB|1|9": situation, seat, band and table size; "-" for an unknown part.
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.lage.key(),
            self.seat.map_or("-", Seat::key),
            self.band.map_or("-".to_string(), |b| b.to_string()),
            self.table.key()
        )
    }

    pub fn parse(key: &str) -> Option<SpotKind> {
        let mut parts = key.split('|');
        let lage = Lage::parse(parts.next()?)?;
        let seat = match parts.next()? {
            "-" => None,
            s => Some(Seat::parse(s)?),
        };
        let band = match parts.next()? {
            "-" => None,
            b => Some(b.parse().ok().filter(|&b: &u8| (b as usize) < BANDS.len())?),
        };
        let table = TableSize::parse(parts.next()?)?;
        parts.next().is_none().then_some(SpotKind { lage, seat, band, table })
    }

    /// "Gegen Open im Big Blind".
    pub fn title(&self) -> String {
        match self.seat {
            Some(s) => format!("{} {}", self.lage.name(), s.phrase()),
            None => self.lage.name().to_string(),
        }
    }

    /// "10–20 BB · 7–10 Spieler".
    pub fn detail(&self) -> String {
        match self.band {
            Some(b) => format!("{} · {}", BANDS[b as usize], self.table.name()),
            None => self.table.name().to_string(),
        }
    }
}

/// One stored decision of the hero with what the profile needs of its hand.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRow {
    pub hand_id: u64,
    pub action_index: usize,
    /// "YYYY/MM/DD HH:MM:SS" as in the hand history.
    pub played_at: String,
    pub spot: SpotType,
    pub status: Status,
    pub rating: Option<Rating>,
    pub ev_loss_bb: Option<f64>,
    pub source: Option<Source>,
    pub reliability: Option<f64>,
    /// The hero had a bet to call (the recommendation offers a call).
    pub facing_bet: bool,
    /// Seat label of `stats::position_labels`.
    pub position: Option<String>,
    /// Effective stack at the start of the hand.
    pub stack_bb: Option<f64>,
    /// Players dealt in.
    pub players: usize,
}

/// Whether a recommendation was made facing a bet.
pub fn facing_bet(strategy: &[Choice]) -> bool {
    strategy.iter().any(|c| c.kind == "call")
}

/// The kind of spot of a decision; `None` for forced moves, which are no decisions.
pub fn kind_of(r: &ProfileRow) -> Option<SpotKind> {
    if r.status == Status::Forced || r.source == Some(Source::Forced) {
        return None;
    }
    let band = r.stack_bb.filter(|s| s.is_finite()).map(band_of);
    let first_in_short = band.is_some_and(|b| b < 2);
    let lage = match r.spot {
        SpotType::Open | SpotType::Steal | SpotType::Shove if first_in_short => Lage::PushFold,
        SpotType::Open | SpotType::Steal | SpotType::Shove => Lage::FirstIn,
        SpotType::VsLimp => Lage::VsLimp,
        SpotType::VsOpen | SpotType::ThreeBet => Lage::VsOpen,
        SpotType::VsThreeBet => Lage::VsThreeBet,
        SpotType::CallShove => Lage::VsAllIn,
        SpotType::CBet => Lage::CBet,
        SpotType::VsCBet => Lage::VsCBet,
        SpotType::Flop if r.facing_bet => Lage::FlopVsBet,
        SpotType::Flop => Lage::FlopBet,
        SpotType::Turn if r.facing_bet => Lage::TurnVsBet,
        SpotType::Turn => Lage::TurnBet,
        SpotType::River if r.facing_bet => Lage::RiverVsBet,
        SpotType::River => Lage::RiverBet,
    };
    let seat = if lage.preflop() { r.position.as_deref().and_then(Seat::of) } else { None };
    Some(SpotKind { lage, seat, band, table: TableSize::of(r.players) })
}

/// Error rate before and after the first practice of a kind of spot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Change {
    pub before: usize,
    pub before_error_pct: f64,
    pub after: usize,
    pub after_error_pct: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Group {
    pub key: String,
    pub lage: Lage,
    pub seat: Option<Seat>,
    pub band: Option<u8>,
    pub table: TableSize,
    /// "Gegen Open im Big Blind".
    pub title: String,
    /// "10–20 BB · 7–10 Spieler".
    pub detail: String,
    /// Rated decisions.
    pub decisions: usize,
    /// Inaccuracies or worse.
    pub flawed: usize,
    pub error_pct: f64,
    pub loss_bb: f64,
    pub mean_loss_bb: f64,
    /// Decisions still waiting for a solve or not rated.
    pub open: usize,
    /// Rated decisions per source.
    pub sources: Vec<(Source, usize)>,
    /// Share of rated decisions from equilibrium, solver or model, percent.
    pub computed_pct: f64,
    /// Share of the loss that comes from the estimate, percent.
    pub estimate_loss_pct: f64,
    /// How often the estimate agreed with the solver, weighted by loss; with estimate loss only.
    pub estimate_reliability: Option<f64>,
    /// Most of the loss rests on the estimate.
    pub unsure: bool,
    /// Fewer than [`MIN_DECISIONS`] rated decisions.
    pub few: bool,
    /// 1-based place among the ranked groups.
    pub rank: Option<usize>,
    pub last_played_at: String,
    pub change: Option<Change>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Profile {
    pub decisions: usize,
    pub loss_bb: f64,
    pub open: usize,
    pub min_decisions: usize,
    /// Ranked groups first, costliest first; then the rest the same way.
    pub groups: Vec<Group>,
}

#[derive(Default)]
struct Acc {
    decisions: usize,
    flawed: usize,
    loss: i64,
    open: usize,
    sources: BTreeMap<&'static str, (Source, usize)>,
    computed: usize,
    estimate_loss: i64,
    estimate_weighted: f64,
    last: String,
}

fn is_rated(r: &ProfileRow) -> bool {
    r.status == Status::Rated
}

fn flawed(r: &ProfileRow) -> bool {
    r.rating.is_some_and(|x| x >= Rating::Inaccuracy)
}

fn pct(part: f64, whole: f64) -> f64 {
    if whole > 0.0 { 100.0 * part / whole } else { 0.0 }
}

/// Groups `rows`, ranks the groups and adds what changed after practice: `practiced`
/// holds a group key with the last hand id stored before its first practice.
pub fn profile(rows: &[ProfileRow], practiced: &[(String, u64)]) -> Profile {
    let mut acc: BTreeMap<SpotKind, Acc> = BTreeMap::new();
    let mut out = Profile { min_decisions: MIN_DECISIONS, ..Default::default() };
    let mut total = 0i64;
    for r in rows {
        let Some(kind) = kind_of(r) else { continue };
        let a = acc.entry(kind).or_default();
        if r.played_at > a.last {
            a.last = r.played_at.clone();
        }
        if !is_rated(r) {
            a.open += 1;
            out.open += 1;
            continue;
        }
        let loss = micro(r.ev_loss_bb.unwrap_or(0.0));
        a.decisions += 1;
        a.flawed += flawed(r) as usize;
        a.loss += loss;
        out.decisions += 1;
        total += loss;
        if let Some(s) = r.source {
            a.sources.entry(s.key()).or_insert((s, 0)).1 += 1;
            match s {
                Source::Estimate => {
                    a.estimate_loss += loss;
                    a.estimate_weighted += loss as f64 * r.reliability.unwrap_or(0.0);
                }
                Source::Forced => {}
                _ => a.computed += 1,
            }
        }
    }
    let mut groups: Vec<Group> = acc
        .into_iter()
        .map(|(kind, a)| {
            let estimate_share = if a.loss > 0 { a.estimate_loss as f64 / a.loss as f64 } else { 0.0 };
            let loss_bb = a.loss as f64 / MICRO;
            Group {
                key: kind.key(),
                lage: kind.lage,
                seat: kind.seat,
                band: kind.band,
                table: kind.table,
                title: kind.title(),
                detail: kind.detail(),
                decisions: a.decisions,
                flawed: a.flawed,
                error_pct: pct(a.flawed as f64, a.decisions as f64),
                loss_bb,
                mean_loss_bb: if a.decisions > 0 { loss_bb / a.decisions as f64 } else { 0.0 },
                open: a.open,
                sources: a.sources.into_values().collect(),
                computed_pct: pct(a.computed as f64, a.decisions as f64),
                estimate_loss_pct: 100.0 * estimate_share,
                estimate_reliability: (a.estimate_loss > 0).then(|| a.estimate_weighted / a.estimate_loss as f64),
                unsure: estimate_share > UNSURE_SHARE,
                few: a.decisions < MIN_DECISIONS,
                rank: None,
                last_played_at: a.last,
                change: None,
            }
        })
        .collect();
    groups.sort_by(|a, b| {
        a.few
            .cmp(&b.few)
            .then(b.loss_bb.total_cmp(&a.loss_bb))
            .then(b.decisions.cmp(&a.decisions))
            .then(a.key.cmp(&b.key))
    });
    let mut rank = 0;
    for g in groups.iter_mut().filter(|g| !g.few && g.loss_bb > 0.0) {
        rank += 1;
        g.rank = Some(rank);
    }
    for (key, after_hand) in practiced {
        if let Some(g) = groups.iter_mut().find(|g| &g.key == key) {
            g.change = change(rows, key, *after_hand);
        }
    }
    out.loss_bb = total as f64 / MICRO;
    out.groups = groups;
    out
}

/// Error rate of the group's rated decisions in hands up to `after_hand` and after it;
/// `None` until both sides have [`MIN_PROGRESS`] of them.
pub fn change(rows: &[ProfileRow], key: &str, after_hand: u64) -> Option<Change> {
    let (mut before, mut after) = ((0, 0), (0, 0));
    for r in rows.iter().filter(|r| is_rated(r) && kind_of(r).is_some_and(|k| k.key() == key)) {
        let side = if r.hand_id > after_hand { &mut after } else { &mut before };
        side.0 += 1;
        side.1 += flawed(r) as usize;
    }
    (before.0 >= MIN_PROGRESS && after.0 >= MIN_PROGRESS).then(|| Change {
        before: before.0,
        before_error_pct: pct(before.1 as f64, before.0 as f64),
        after: after.0,
        after_error_pct: pct(after.1 as f64, after.0 as f64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small deterministic generator, so the properties run over many made-up profiles.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
            xs[self.next() as usize % xs.len()]
        }
    }

    const POSITIONS: [&str; 9] = ["UTG", "UTG+1", "LJ", "HJ", "CO", "BTN", "SB", "BB", "??"];
    const STATUSES: [Status; 4] = [Status::Rated, Status::Rated, Status::Pending, Status::Forced];

    fn rows(seed: u64, n: usize) -> Vec<ProfileRow> {
        made_up(seed, n, false)
    }

    /// With `coarse`, few kinds of spot, so groups grow large enough to be ranked.
    fn made_up(seed: u64, n: usize, coarse: bool) -> Vec<ProfileRow> {
        let mut g = Lcg(seed);
        (0..n)
            .map(|i| {
                let status = g.pick(&STATUSES);
                let source = match status {
                    Status::Forced => Source::Forced,
                    Status::Pending => Source::Solver,
                    _ => g.pick(&[Source::Equilibrium, Source::Model, Source::Solver, Source::Estimate]),
                };
                let rating = (status == Status::Rated).then(|| g.pick(&Rating::ALL));
                ProfileRow {
                    hand_id: 1000 + i as u64 / 3,
                    action_index: i % 3,
                    played_at: format!("2026/11/{:02} 20:{:02}:00", 1 + g.next() % 28, g.next() % 60),
                    spot: if coarse { g.pick(&[SpotType::VsOpen, SpotType::River, SpotType::Open]) } else { g.pick(&SpotType::ALL) },
                    status,
                    rating,
                    ev_loss_bb: rating.map(|_| (g.next() % 400) as f64 / 100.0),
                    source: Some(source),
                    reliability: (source == Source::Estimate).then(|| (g.next() % 100) as f64 / 100.0),
                    facing_bet: g.next().is_multiple_of(2),
                    position: Some(if coarse { g.pick(&["BB", "BTN"]) } else { g.pick(&POSITIONS) }.to_string()),
                    stack_bb: if coarse { Some(g.pick(&[15.0, 30.0])) } else { (!g.next().is_multiple_of(8)).then(|| (g.next() % 9000) as f64 / 100.0) },
                    players: if coarse { 9 } else { 2 + (g.next() % 8) as usize },
                }
            })
            .collect()
    }

    fn rated(rs: &[ProfileRow]) -> impl Iterator<Item = &ProfileRow> {
        rs.iter().filter(|r| r.status == Status::Rated)
    }

    #[test]
    fn group_losses_add_up_to_the_total() {
        for seed in 0..50 {
            let rs = made_up(seed, 300, seed % 2 == 0);
            let p = profile(&rs, &[]);
            let total: f64 = rated(&rs).map(|r| r.ev_loss_bb.unwrap_or(0.0)).sum();
            let groups: f64 = p.groups.iter().map(|g| g.loss_bb).sum();
            assert!((total - groups).abs() < 1e-6, "seed {seed}: {total} vs {groups}");
            assert!((p.loss_bb - total).abs() < 1e-6);
        }
    }

    #[test]
    fn every_rated_decision_lies_in_exactly_one_group() {
        for seed in 0..50 {
            let rs = rows(seed, 300);
            let p = profile(&rs, &[]);
            assert_eq!(p.groups.iter().map(|g| g.decisions).sum::<usize>(), rated(&rs).count());
            assert_eq!(p.decisions, rated(&rs).count());
            for r in rated(&rs) {
                let key = kind_of(r).expect("a rated decision has a kind").key();
                assert_eq!(p.groups.iter().filter(|g| g.key == key).count(), 1);
            }
            let forced = rs.iter().filter(|r| r.status == Status::Forced).count();
            let open = rs.iter().filter(|r| r.status == Status::Pending).count();
            assert_eq!(p.open, open);
            assert_eq!(p.groups.iter().map(|g| g.decisions + g.open).sum::<usize>(), rs.len() - forced);
        }
    }

    #[test]
    fn ranking_is_stable_and_ignores_input_order() {
        for seed in 0..60 {
            let rs = made_up(seed, 250, seed % 2 == 0);
            let a = profile(&rs, &[]);
            let mut shuffled = rs.clone();
            let mut g = Lcg(seed ^ 0xdead);
            for i in (1..shuffled.len()).rev() {
                shuffled.swap(i, g.next() as usize % (i + 1));
            }
            let b = profile(&shuffled, &[]);
            let keys = |p: &Profile| p.groups.iter().map(|g| (g.key.clone(), g.rank)).collect::<Vec<_>>();
            assert_eq!(keys(&a), keys(&b));
            for (x, y) in a.groups.iter().zip(&b.groups) {
                assert!((x.loss_bb - y.loss_bb).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn ranks_go_to_large_enough_groups_costliest_first() {
        let mut seen = 0;
        for seed in 0..30 {
            let p = profile(&made_up(seed, 400, true), &[]);
            let ranked: Vec<&Group> = p.groups.iter().filter(|g| g.rank.is_some()).collect();
            seen += ranked.len();
            for (i, g) in ranked.iter().enumerate() {
                assert_eq!(g.rank, Some(i + 1));
                assert!(g.decisions >= MIN_DECISIONS && !g.few);
            }
            for w in ranked.windows(2) {
                assert!(w[0].loss_bb >= w[1].loss_bb);
            }
            // Ranked groups come before every unranked one.
            let first_unranked = p.groups.iter().position(|g| g.rank.is_none()).unwrap_or(p.groups.len());
            assert!(p.groups[first_unranked..].iter().all(|g| g.rank.is_none()));
            for g in &p.groups {
                assert_eq!(g.few, g.decisions < MIN_DECISIONS);
                assert_eq!(g.unsure, g.estimate_loss_pct > 100.0 * UNSURE_SHARE);
            }
        }
        assert!(seen > 100, "the made-up profiles rank groups");
    }

    #[test]
    fn kind_depends_on_the_situation_not_on_the_action() {
        let base = ProfileRow {
            hand_id: 1,
            action_index: 5,
            played_at: "2026/11/17 21:00:00".into(),
            spot: SpotType::VsOpen,
            status: Status::Rated,
            rating: Some(Rating::Best),
            ev_loss_bb: Some(0.0),
            source: Some(Source::Model),
            reliability: None,
            facing_bet: true,
            position: Some("BB".into()),
            stack_bb: Some(16.0),
            players: 9,
        };
        let three_bet = ProfileRow { spot: SpotType::ThreeBet, ..base.clone() };
        assert_eq!(kind_of(&base), kind_of(&three_bet));
        let k = kind_of(&base).unwrap();
        assert_eq!(k.key(), "vs_open|BB|1|9");
        assert_eq!(k.title(), "Gegen Open im Big Blind");
        assert_eq!(k.detail(), "10–20 BB · 7–10 Spieler");
        assert_eq!(SpotKind::parse(&k.key()), Some(k));
        // Open, steal and shove as first in are one situation, split by depth only.
        let open = ProfileRow { spot: SpotType::Open, position: Some("UTG+1".into()), stack_bb: Some(50.0), ..base.clone() };
        let shove = ProfileRow { spot: SpotType::Shove, ..open.clone() };
        assert_eq!(kind_of(&open), kind_of(&shove));
        assert_eq!(kind_of(&open).unwrap().lage, Lage::FirstIn);
        assert_eq!(kind_of(&ProfileRow { stack_bb: Some(12.0), ..open }).unwrap().lage, Lage::PushFold);
        // After the flop the seat is left out.
        let river = ProfileRow { spot: SpotType::River, ..base.clone() };
        assert_eq!(kind_of(&river).unwrap().key(), "river_vs_bet|-|1|9");
        assert_eq!(kind_of(&ProfileRow { status: Status::Forced, source: Some(Source::Forced), ..base }), None);
    }

    #[test]
    fn every_key_parses_back() {
        for r in rows(7, 500) {
            if let Some(k) = kind_of(&r) {
                assert_eq!(SpotKind::parse(&k.key()), Some(k));
            }
        }
        assert_eq!(SpotKind::parse("vs_open|BB|9|9"), None);
        assert_eq!(SpotKind::parse("nope|BB|1|9"), None);
        // A final table of three to five players is its own group, apart from 6-max.
        let sizes: Vec<&str> = (2..=10).map(|n| TableSize::of(n).key()).collect();
        assert_eq!(sizes, ["2", "5", "5", "5", "6", "9", "9", "9", "9"]);
        for t in TableSize::ALL {
            assert_eq!(TableSize::parse(t.key()), Some(t));
        }
    }

    #[test]
    fn change_needs_enough_hands_on_both_sides() {
        let row = |hand_id: u64, rating: Rating| ProfileRow {
            hand_id,
            action_index: 0,
            played_at: String::new(),
            spot: SpotType::VsOpen,
            status: Status::Rated,
            rating: Some(rating),
            ev_loss_bb: Some(0.5),
            source: Some(Source::Model),
            reliability: None,
            facing_bet: true,
            position: Some("BB".into()),
            stack_bb: Some(30.0),
            players: 6,
        };
        let key = "vs_open|BB|2|6";
        let mut rs: Vec<ProfileRow> =
            (0..10).map(|i| row(i, if i < 4 { Rating::Mistake } else { Rating::Best })).collect();
        rs.extend((100..109).map(|i| row(i, if i < 102 { Rating::Mistake } else { Rating::Best })));
        assert_eq!(change(&rs, key, 50), None, "nine after the practice are not enough");
        rs.push(row(200, Rating::Best));
        let c = change(&rs, key, 50).unwrap();
        assert_eq!((c.before, c.after), (10, 10));
        assert!((c.before_error_pct - 40.0).abs() < 1e-9 && (c.after_error_pct - 20.0).abs() < 1e-9);
        let p = profile(&rs, &[(key.to_string(), 50)]);
        assert_eq!(p.groups[0].change, Some(c));
    }
}
