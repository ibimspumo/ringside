//! The estimate: a decision rated by equity against the likely ranges, pot odds and fold
//! equity, for every spot the model and the solver don't cover (pots of three or more
//! players after the flop, lines outside the model, spots the solver can't take).
//!
//! - **Ranges.** Every opponent starts with the model's range where the hand's line
//!   was in the model, otherwise with all hands; each action narrows it by how strong a
//!   hand is: before the flop by all-in equity against a random hand (limps by the
//!   model's typical limping range), after the flop by equity against a random hand on
//!   the board. After the flop each hand keeps the share the solver takes the move with
//!   at its strength within the range (drawing hands one share of their own), per
//!   street, kind of move and size class: a bet of the last aggressor (c-bet, barrel) or
//!   of anyone else (lead, probe), a raise, a call, a check with others still to act or
//!   one behind.
//! - **Equity** of the hero's two cards against all opponents at once, sampled with card
//!   removal.
//! - **EV of each option**, in BB from now on: fold 0; check `R × equity × pot`, heads-up
//!   out of position with what the opponent's bet after it brings; call
//!   `R × equity × (pot + call) − call`; a bet or raise wins the pot when everyone folds
//!   and otherwise plays against what continues. Opponents continue with the strongest
//!   part of their range, as much as the pot odds of the bet ask them to defend (the
//!   minimum defence frequency times a fitted factor); all-ins are played out. `R`
//!   (equity realization) is 1 on the river and all-in; before that less out of
//!   position, with more opponents and with a deeper stack-to-pot ratio, more for the
//!   aggressor.
//! - **Kinds, not sizes.** The formulas favour the biggest bet far more than solvers
//!   do, so the estimate rates the kind of move: every bet or raise counts as much as
//!   the best one, as does every check or call. An all-in counts with its own EV.
//! - **Lines are mixed.** With enough equity to go on, how a hand goes on (check or
//!   bet, call or raise) is what equilibrium strategies mix and what the formulas can't
//!   rank; there no option loses more than a fitted small share of the pot. Folding a
//!   hand that can go on, or going on with one that can't, is graded by the EVs.
//!
//! The free numbers are fitted per street, and how often its grades agree with the
//! solver's is measured on solved spots (`analysis-calibrate`) and stored with every
//! estimated decision.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::eval::Hand as Cards;
use ringside_engine::preflop::limp_shape;
use ringside_engine::{Card, CardSet, Combo, HandClass, PreflopMatrix};
use ringside_play::grade::{Grade, judge};
use serde::{Deserialize, Serialize};

pub const COMBOS: usize = 1326;
const EQUITY_SAMPLES: usize = 4000;
const STRENGTH_SAMPLES: usize = 48;

/// Weight per combo (`Combo::index`).
#[derive(Clone, Debug, PartialEq)]
pub struct Range(pub Vec<f32>);

impl Range {
    pub fn all() -> Range {
        Range(vec![1.0; COMBOS])
    }

    /// From weights per hand class.
    pub fn from_classes(w: &[f64]) -> Range {
        Range(Combo::all().map(|c| w[c.class().index()] as f32).collect())
    }

    pub fn parse(text: &str) -> Option<Range> {
        let r = ringside_engine::Range::parse(text).ok()?;
        Some(Range(Combo::all().map(|c| r.weight(c)).collect()))
    }

    /// Weighted combos not touching `dead`.
    pub fn live(&self, dead: CardSet) -> f64 {
        Combo::all().filter(|c| c.mask().0 & dead.0 == 0).map(|c| self.0[c.index()] as f64).sum()
    }
}

/// A combo is a draw when its equity against a random hand is this much above the share
/// of random hands it is ahead of now.
const DRAW_GAP: f32 = 0.12;

/// How strong every combo is on a board.
#[derive(Clone, Debug)]
pub struct Strength {
    /// All-in equity against a random hand, before the flop from the class table, after
    /// it sampled (0 for combos on the board).
    pub equity: Vec<f32>,
    /// On the flop and the turn: combos that win far more often than they are ahead now.
    pub draw: Vec<bool>,
}

/// The [`Strength`] of every combo on `board`.
pub fn strength(board: &[Card], rng: &mut ChaCha8Rng) -> Strength {
    let equity = equity_vs_random(board, rng);
    let draw = match board.len() {
        3 | 4 => {
            let ahead = ahead_now(board);
            (0..COMBOS).map(|i| equity[i] - ahead[i] >= DRAW_GAP).collect()
        }
        _ => vec![false; COMBOS],
    };
    Strength { equity, draw }
}

/// Per combo: the share of the other live combos it beats with the board as it is.
fn ahead_now(board: &[Card]) -> Vec<f32> {
    let dead = CardSet::from_cards(board);
    let cards = Cards::from_cards(board.iter().copied());
    let ranks: Vec<Option<ringside_engine::eval::HandRank>> = Combo::all()
        .map(|c| {
            let (a, b) = c.cards();
            (c.mask().0 & dead.0 == 0).then(|| cards.with(a).with(b).rank())
        })
        .collect();
    let mut sorted: Vec<_> = ranks.iter().flatten().copied().collect();
    sorted.sort();
    let n = sorted.len().max(1) as f32;
    ranks
        .iter()
        .map(|r| {
            r.map_or(0.0, |r| {
                let (below, upto) = (sorted.partition_point(|x| *x < r), sorted.partition_point(|x| *x <= r));
                (below as f32 + (upto - below) as f32 / 2.0) / n
            })
        })
        .collect()
}

fn equity_vs_random(board: &[Card], rng: &mut ChaCha8Rng) -> Vec<f32> {
    let m = PreflopMatrix::embedded();
    if board.is_empty() {
        let by_class: Vec<f32> = HandClass::all().map(|c| m.equity_vs_random(c) as f32).collect();
        return Combo::all().map(|c| by_class[c.class().index()]).collect();
    }
    let dead = CardSet::from_cards(board);
    let open: Vec<Card> = ringside_engine::cards::deck().into_iter().filter(|c| dead.0 & c.mask() == 0).collect();
    let board_cards = Cards::from_cards(board.iter().copied());
    Combo::all()
        .map(|combo| {
            if combo.mask().0 & dead.0 != 0 {
                return 0.0;
            }
            let (a, b) = combo.cards();
            let mut won = 0.0;
            for _ in 0..STRENGTH_SAMPLES {
                let mut used = combo.mask().0;
                let mut draw = || loop {
                    let c = open[rng.random_range(0..open.len())];
                    if used & c.mask() == 0 {
                        used |= c.mask();
                        break c;
                    }
                };
                let (x, y) = (draw(), draw());
                let mut rest = board_cards;
                for _ in board.len()..5 {
                    rest = rest.with(draw());
                }
                let me = rest.with(a).with(b).rank();
                let them = rest.with(x).with(y).rank();
                won += if me > them { 1.0 } else if me == them { 0.5 } else { 0.0 };
            }
            won / STRENGTH_SAMPLES as f32
        })
        .collect()
}

/// What an action says about a range, by strength percentile within it (0 weakest).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Signal {
    /// A limp before the flop: the typical limping range.
    Limp,
    /// First raise before the flop, `seats` players still to act behind.
    Open { behind: u8 },
    /// Call of a raise before the flop.
    CallRaise,
    /// Re-raise before the flop (3-bet), or more (4-bet).
    ThreeBet,
    FourBet,
    /// All-in before the flop with this many BB, or a call of one.
    Shove { bb: u16 },
    CallShove { bb: u16 },
    /// The big blind's check.
    Check,
    /// After the flop, with the size in percent of the pot ([`size_pct`]). A bet says
    /// whether the bettor made the last bet or raise before the street (a c-bet or a
    /// barrel) or not (a lead into the aggressor, a probe).
    Bet { size: u16, aggressor: bool },
    Raise(u16),
    Call(u16),
    /// A check after the flop; `behind` when nobody is left to act after it.
    PostCheck { behind: bool },
}

/// The size of a move after the flop as its [`Signal`] carries it: a bet or raise by what
/// it adds over a call against the pot after that call, a call by its price against the
/// pot before the bet. `pot` is everything in the middle before the move, bets in front
/// included; `facing` the highest bet in front, `own` the mover's, `to` its total after.
pub fn size_pct(pot: f64, facing: f64, own: f64, to: f64) -> u16 {
    let call = (facing - own).max(0.0);
    let size = if to > facing + 1e-9 { (to - facing) / (pot + call) } else { call / (pot - call).max(1e-9) };
    (size * 100.0).round().clamp(0.0, u16::MAX as f64) as u16
}

/// The kinds of moves after the flop whose ranges the estimate tells apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoveKind {
    /// A bet of the last aggressor.
    Bet,
    /// A bet of anyone else.
    Lead,
    Raise,
    Call,
    /// A check with others still to act after it, and one that ends the betting.
    Check,
    CheckBehind,
}

/// Lower ends of the size classes after the first, percent of the pot.
pub const SIZE_CLASSES: [u16; 3] = [45, 90, 160];
pub const SIZES: usize = SIZE_CLASSES.len() + 1;
/// Edges of the strength bins of a [`Curve`], by the share of the range at least as strong.
pub const BIN_EDGES: [f32; 13] = [0.0, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0];
pub const BINS: usize = BIN_EDGES.len() - 1;

pub fn size_class(pct: u16) -> usize {
    SIZE_CLASSES.iter().take_while(|&&c| pct >= c).count()
}

impl Signal {
    /// Kind and size class of a move after the flop.
    pub fn postflop(self) -> Option<(MoveKind, usize)> {
        Some(match self {
            Signal::Bet { size, aggressor: true } => (MoveKind::Bet, size_class(size)),
            Signal::Bet { size, aggressor: false } => (MoveKind::Lead, size_class(size)),
            Signal::Raise(s) => (MoveKind::Raise, size_class(s)),
            Signal::Call(s) => (MoveKind::Call, size_class(s)),
            Signal::PostCheck { behind: false } => (MoveKind::Check, 0),
            Signal::PostCheck { behind: true } => (MoveKind::CheckBehind, 0),
            _ => return None,
        })
    }
}

/// How often the hands of a range take a move after the flop, by their strength: what the
/// solver plays on the fitting half of `analysis-calibrate`, per street, kind of move and
/// size class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Curve {
    /// Board cards: 3, 4 or 5.
    pub cards: usize,
    pub kind: MoveKind,
    pub size: usize,
    /// Share of the hands in each strength bin that take the move; drawing hands
    /// ([`Strength::draw`]) take it `draw` of the time, whatever their bin.
    pub keep: Vec<f32>,
    pub draw: f32,
    /// Solver hands the curve rests on; 0 where it is borrowed from the nearest class.
    pub samples: usize,
}

impl Curve {
    /// For a combo at `top` (share of the range at least as strong).
    pub fn of(&self, top: f32, draw: bool) -> f32 {
        if draw { self.draw } else { self.at(top) }
    }

    /// At `top`, linear between the bin centres.
    pub fn at(&self, top: f32) -> f32 {
        let centre = |i: usize| (BIN_EDGES[i] + BIN_EDGES[i + 1]) / 2.0;
        let i = (0..BINS).take_while(|&i| centre(i) <= top).count();
        if i == 0 {
            return self.keep[0];
        }
        if i == BINS {
            return self.keep[BINS - 1];
        }
        let t = (top - centre(i - 1)) / (centre(i) - centre(i - 1));
        self.keep[i - 1] + t * (self.keep[i] - self.keep[i - 1])
    }
}

fn keep(signal: Signal, p: f32) -> f32 {
    let top = 1.0 - p;
    match signal {
        Signal::Limp | Signal::Check => unreachable!("handled by class"),
        Signal::Open { behind } => {
            let share = match behind {
                0..=1 => 0.42,
                2 => 0.40,
                3 => 0.27,
                4 => 0.20,
                5 => 0.17,
                6 => 0.15,
                _ => 0.13,
            };
            if top <= share { 1.0 } else if top <= share + 0.05 { 0.3 } else { 0.0 }
        }
        Signal::CallRaise => {
            if top <= 0.03 {
                0.3
            } else if top <= 0.30 {
                1.0
            } else if top <= 0.40 {
                0.3
            } else {
                0.02
            }
        }
        Signal::ThreeBet => {
            if top <= 0.07 {
                1.0
            } else if top <= 0.14 {
                0.25
            } else {
                0.02
            }
        }
        Signal::FourBet => {
            if top <= 0.035 { 1.0 } else if top <= 0.06 { 0.2 } else { 0.0 }
        }
        Signal::Shove { bb } => {
            let share = (3.0 / (bb.max(1) as f32)).clamp(0.03, 0.6);
            if top <= share { 1.0 } else if top <= share * 1.3 { 0.3 } else { 0.0 }
        }
        Signal::CallShove { bb } => {
            let share = (1.8 / (bb.max(1) as f32)).clamp(0.02, 0.4);
            if top <= share { 1.0 } else if top <= share * 1.3 { 0.3 } else { 0.0 }
        }
        Signal::Bet { .. } | Signal::Raise(_) | Signal::Call(_) | Signal::PostCheck { .. } => unreachable!("handled by curve"),
    }
}

/// Narrows `range` after `signal`; `strength` is per combo, `dead` the visible cards.
/// After the flop by the stored [`Curve`]s.
pub fn narrow(range: &mut Range, signal: Signal, strength: &Strength, dead: CardSet) {
    narrow_by(range, signal, strength, dead, &crate::calibration::stored().moves);
}

/// [`narrow`] with the curves after the flop given.
pub fn narrow_by(range: &mut Range, signal: Signal, strength: &Strength, dead: CardSet, curves: &[Curve]) {
    match signal {
        Signal::Limp => {
            let shape = limp_shape();
            for c in Combo::all() {
                range.0[c.index()] *= shape[c.class().index()] as f32;
            }
            return;
        }
        Signal::Check => {
            let top = top_share(range, &strength.equity, dead);
            for c in Combo::all() {
                if top[c.index()] <= 0.05 {
                    range.0[c.index()] *= 0.3;
                }
            }
            return;
        }
        _ => {}
    }
    let top = top_share(range, &strength.equity, dead);
    if let Some((kind, size)) = signal.postflop() {
        let cards = dead.0.count_ones() as usize;
        let Some(curve) = curves.iter().find(|c| c.cards == cards && c.kind == kind && c.size == size) else { return };
        for i in 0..COMBOS {
            range.0[i] *= curve.of(top[i], strength.draw[i]);
        }
        return;
    }
    for c in Combo::all() {
        let i = c.index();
        range.0[i] *= keep(signal, 1.0 - top[i]);
    }
}

/// Per live combo, also those the range holds no weight of: the share of the range (by
/// weight) at least as strong.
pub fn top_share(range: &Range, strength: &[f32], dead: CardSet) -> Vec<f32> {
    let mut order: Vec<usize> = (0..COMBOS).filter(|&i| Combo::from_index(i).mask().0 & dead.0 == 0).collect();
    order.sort_by(|&a, &b| strength[b].total_cmp(&strength[a]));
    let total: f32 = order.iter().map(|&i| range.0[i]).sum();
    let mut out = vec![1.0; COMBOS];
    let mut above = 0.0;
    for &i in &order {
        out[i] = (above + range.0[i] / 2.0) / total.max(1e-9);
        above += range.0[i];
    }
    out
}

/// The opponents' part of the range that continues against a bet: the strongest
/// `share` of it.
fn continuing(range: &Range, strength: &[f32], dead: CardSet, share: f32) -> Range {
    let top = top_share(range, strength, dead);
    Range((0..COMBOS).map(|i| if top[i] <= share { range.0[i] } else { 0.0 }).collect())
}

/// Hero's pot share against all `ranges` at once on `board`, sampled.
pub fn equity(hero: Combo, ranges: &[&Range], board: &[Card], rng: &mut ChaCha8Rng) -> f64 {
    let dead = CardSet(hero.mask().0 | CardSet::from_cards(board).0);
    let lists: Vec<(Vec<usize>, Vec<f64>)> = ranges
        .iter()
        .map(|r| {
            let mut idx = Vec::new();
            let mut cum = Vec::new();
            let mut total = 0.0;
            for c in Combo::all() {
                let w = r.0[c.index()] as f64;
                if w > 0.0 && c.mask().0 & dead.0 == 0 {
                    total += w;
                    idx.push(c.index());
                    cum.push(total);
                }
            }
            (idx, cum)
        })
        .collect();
    if lists.iter().any(|(i, _)| i.is_empty()) {
        return 1.0;
    }
    let deck: Vec<Card> = ringside_engine::cards::deck();
    let board_cards = Cards::from_cards(board.iter().copied());
    let (ha, hb) = hero.cards();
    let mut share = 0.0;
    let mut done = 0;
    let mut tries = 0;
    while done < EQUITY_SAMPLES && tries < EQUITY_SAMPLES * 20 {
        tries += 1;
        let mut used = dead.0;
        let mut theirs = Vec::with_capacity(lists.len());
        let mut clash = false;
        for (idx, cum) in &lists {
            let x = rng.random::<f64>() * cum[cum.len() - 1];
            let k = cum.partition_point(|&c| c < x).min(idx.len() - 1);
            let combo = Combo::from_index(idx[k]);
            if combo.mask().0 & used != 0 {
                clash = true;
                break;
            }
            used |= combo.mask().0;
            theirs.push(combo);
        }
        if clash {
            continue;
        }
        let mut rest = board_cards;
        for _ in board.len()..5 {
            let c = loop {
                let c = deck[rng.random_range(0..52)];
                if used & c.mask() == 0 {
                    break c;
                }
            };
            used |= c.mask();
            rest = rest.with(c);
        }
        let best = rest.with(ha).with(hb).rank();
        let mut ties = 1;
        let mut lost = false;
        for c in &theirs {
            let (a, b) = c.cards();
            let r = rest.with(a).with(b).rank();
            if r > best {
                lost = true;
                break;
            }
            if r == best {
                ties += 1;
            }
        }
        if !lost {
            share += 1.0 / ties as f64;
        }
        done += 1;
    }
    if done == 0 { 0.5 } else { share / done as f64 }
}

/// An option of the hero, sizes as "to" amounts on the street in BB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Opt {
    Fold,
    Check,
    Call,
    Bet(f64),
    Raise(f64),
    AllIn,
}

impl Opt {
    pub fn key(self) -> &'static str {
        match self {
            Opt::Fold => "fold",
            Opt::Check => "check",
            Opt::Call => "call",
            Opt::Bet(_) => "bet",
            Opt::Raise(_) => "raise",
            Opt::AllIn => "all_in",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Opponent {
    pub range: Range,
    /// Chips behind, BB.
    pub behind: f64,
    /// Put in on this street, BB.
    pub street_in: f64,
}

/// A decision as the estimate sees it.
#[derive(Clone, Debug)]
pub struct Spot {
    pub preflop: bool,
    pub river: bool,
    pub board: Vec<Card>,
    pub hero: Combo,
    pub opponents: Vec<Opponent>,
    /// Chips in the middle before the hero acts, BB.
    pub pot: f64,
    pub hero_in: f64,
    pub hero_behind: f64,
    /// The hero acts after every opponent left after the flop.
    pub in_position: bool,
    /// The hero made the last raise before (the aggressor).
    pub aggressor: bool,
}

impl Spot {
    /// Every opponent is all in: nobody can bet or call any more.
    pub fn nobody_can_bet(&self) -> bool {
        self.opponents.iter().all(|o| o.behind <= 1e-9)
    }

    pub fn to_call(&self) -> f64 {
        let top = self.opponents.iter().map(|o| o.street_in).fold(self.hero_in, f64::max);
        (top - self.hero_in).min(self.hero_behind).max(0.0)
    }
}

#[derive(Clone, Debug)]
pub struct Estimate {
    /// EV of each option in BB, from now on.
    pub evs: Vec<f64>,
    pub equity: f64,
    pub pot_odds: Option<f64>,
    /// For each bet or raise: how often everyone folds to the best one of its kind.
    pub folds: Vec<Option<f64>>,
}

/// The free numbers of the estimate, fitted by `analysis-calibrate` against the solver
/// (after the flop) and the preflop model (before it).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tuning {
    /// Share of each EV gap to the best option that counts: the formulas leave out how
    /// the later streets even out a weaker choice, so their gaps come out too wide.
    pub shrink: f64,
    /// How much realization grows with equity, `× (1 + strength × (equity − ½))`:
    /// strong hands win bigger pots later, weak ones give up.
    pub strength: f64,
    /// Opponents continue with this multiple of the minimum defence frequency.
    pub defend: f64,
    /// Options this close to the best, in percent of the pot, are as good within what
    /// the estimate can tell apart: they share the recommendation like a mix.
    #[serde(default = "default_tie")]
    pub tie_pct: f64,
    /// A hand with at least the pot odds' equity less this margin can go on, and how it
    /// goes on (check or bet, call or raise) is what equilibrium strategies mix and the
    /// estimate can't rank: there no such option loses more than `line_pct` of the pot.
    /// Hands above `line_top` equity are left out, a near-nut hand misplayed is a
    /// mistake the estimate does see. A margin of −1 leaves every line to the EVs.
    pub line_margin: f64,
    pub line_pct: f64,
    pub line_top: f64,
}

fn default_tie() -> f64 {
    ringside_play::grade::BEST_PCT
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            shrink: 1.0,
            strength: 0.0,
            defend: 1.1,
            tie_pct: default_tie(),
            line_margin: -1.0,
            line_pct: 100.0,
            line_top: 1.0,
        }
    }
}

/// Equity realization on later streets.
fn realization(s: &Spot, spr: f64, eq: f64, t: &Tuning) -> f64 {
    if s.river || s.nobody_can_bet() {
        return 1.0;
    }
    let pos = if s.in_position { 1.0 } else { 0.86 };
    let crowd = 1.0 - 0.07 * (s.opponents.len().saturating_sub(1)) as f64;
    let depth = 1.0 - 0.04 * spr.clamp(0.0, 6.0);
    let lead = if s.aggressor { 1.04 } else { 1.0 };
    let strength = 1.0 + t.strength * (eq - 0.5);
    (pos * crowd * depth * lead * strength).clamp(0.4, 1.6)
}

/// EVs of `options` at `spot`; `curves` say how the opponents' ranges play after the flop.
pub fn estimate(spot: &Spot, options: &[Opt], seed: u64, t: &Tuning, curves: &[Curve]) -> Estimate {
    let strength = strength(&spot.board, &mut ChaCha8Rng::seed_from_u64(seed ^ 0x5eed));
    settle(&prepare(spot, options, seed, t, &strength, curves), t)
}

/// What an estimate needs before [`settle`], with the [`strength`] of the board's combos.
pub fn prepare(spot: &Spot, options: &[Opt], seed: u64, t: &Tuning, strength: &Strength, curves: &[Curve]) -> Prepared {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let dead = CardSet(spot.hero.mask().0 | CardSet::from_cards(&spot.board).0);
    let board_dead = CardSet::from_cards(&spot.board);
    let ranges: Vec<&Range> = spot.opponents.iter().map(|o| &o.range).collect();
    let eq = equity(spot.hero, &ranges, &spot.board, &mut rng);
    let call = spot.to_call();
    let behind_after_call = spot.hero_behind - call;
    let spr_now = behind_after_call / (spot.pot + call).max(0.01);
    let r = realization(spot, spr_now, eq, t);
    let pot_odds = (call > 0.0).then(|| call / (spot.pot + call));
    let ev_of = |opt: Opt, rng: &mut ChaCha8Rng| -> (f64, Option<f64>) {
        match opt {
            Opt::Fold => (0.0, None),
            Opt::Check => (r * checked(spot, eq, strength, board_dead, dead, curves, rng), None),
            Opt::Call => {
                let rr = if call >= spot.hero_behind - 1e-9 { 1.0 } else { r };
                (rr * eq * (spot.pot + call) - call, None)
            }
            Opt::Bet(to) | Opt::Raise(to) => aggression(spot, to.min(spot.hero_in + spot.hero_behind), &strength.equity, board_dead, dead, rng, t),
            Opt::AllIn => aggression(spot, spot.hero_in + spot.hero_behind, &strength.equity, board_dead, dead, rng, t),
        }
    };
    let (evs, folds): (Vec<f64>, Vec<Option<f64>>) = options.iter().map(|&o| ev_of(o, &mut rng)).unzip();
    Prepared { options: options.to_vec(), evs, folds, equity: eq, pot_odds, pot: spot.pot }
}

/// The part of an estimate that the free numbers after [`Tuning::defend`] and
/// [`Tuning::strength`] don't change, so they can be tried on it quickly.
#[derive(Clone, Debug)]
pub struct Prepared {
    options: Vec<Opt>,
    /// Unshrunk EV of each option, BB.
    evs: Vec<f64>,
    folds: Vec<Option<f64>>,
    equity: f64,
    pot_odds: Option<f64>,
    pot: f64,
}

/// The estimate's EVs from what [`prepare`] found. The gaps to the best option are
/// shrunk, so the levels move with them; they are anchored again where folding is 0, so
/// every option reads as what it brings over giving up. Only the gaps grade.
pub fn settle(p: &Prepared, t: &Tuning) -> Estimate {
    let kind = |o: Opt| match o {
        Opt::Fold => 0,
        Opt::Check | Opt::Call => 1,
        Opt::Bet(_) | Opt::Raise(_) => 2,
        Opt::AllIn => 3,
    };
    let best_of_kind: Vec<usize> = p
        .options
        .iter()
        .map(|&o| (0..p.options.len()).filter(|&k| kind(p.options[k]) == kind(o)).max_by(|&a, &b| p.evs[a].total_cmp(&p.evs[b])).unwrap_or(0))
        .collect();
    let by_kind: Vec<f64> = best_of_kind.iter().map(|&k| p.evs[k]).collect();
    let top = by_kind.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let goes_on = p.equity >= p.pot_odds.unwrap_or(0.0) - t.line_margin && p.equity < t.line_top;
    let line_cap = t.line_pct / 100.0 * p.pot;
    let mut evs: Vec<f64> = p
        .options
        .iter()
        .zip(&by_kind)
        .map(|(&o, &e)| {
            let loss = t.shrink * (top - e);
            top - if goes_on && o != Opt::Fold { loss.min(line_cap) } else { loss }
        })
        .collect();
    if let Some(fold) = p.options.iter().position(|&o| o == Opt::Fold) {
        let base = evs[fold];
        evs.iter_mut().for_each(|e| *e -= base);
    }
    let folds = best_of_kind.iter().map(|&k| p.folds[k]).collect();
    Estimate { evs, equity: p.equity, pot_odds: p.pot_odds, folds }
}

/// How often the estimate plays each option: it has no mixed strategy, so the options
/// it can't tell apart from the best one share the frequency.
pub fn strategy(est: &Estimate, pot: f64, t: &Tuning) -> Vec<f64> {
    let top = est.evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let near: Vec<bool> = est.evs.iter().map(|&e| (top - e) / pot.max(0.01) * 100.0 <= t.tie_pct).collect();
    let share = 1.0 / near.iter().filter(|&&x| x).count().max(1) as f64;
    near.iter().map(|&x| if x { share } else { 0.0 }).collect()
}

/// The grade of every option, as the analysis and the practice give it.
pub fn grades(est: &Estimate, options: &[Opt], pot: f64, t: &Tuning) -> Vec<Grade> {
    let freq = strategy(est, pot, t);
    (0..options.len()).map(|k| judge(&freq, Some(&est.evs), k, pot).grade).collect()
}

/// Typical bets of each size class, share of the pot.
const CLASS_BETS: [f64; SIZES] = [0.33, 0.7, 1.2, 2.0];

/// What checking wins at showdown, before realization: the pot by the equity, and where
/// the only opponent acts after, what their bet brings. They bet the part of their range
/// the curve of the street's most common size gives; the hero calls when the equity
/// against it pays, and the bet adds to the pot then.
fn checked(spot: &Spot, eq: f64, strength: &Strength, board_dead: CardSet, dead: CardSet, curves: &[Curve], rng: &mut ChaCha8Rng) -> f64 {
    let plain = eq * spot.pot;
    let [o] = spot.opponents.as_slice() else { return plain };
    if spot.preflop || spot.in_position {
        return plain;
    }
    let cards = spot.board.len();
    let kind = if spot.aggressor { MoveKind::Lead } else { MoveKind::Bet };
    let Some(curve) = curves.iter().filter(|c| c.cards == cards && c.kind == kind).max_by_key(|c| c.samples) else { return plain };
    let bet = (CLASS_BETS[curve.size] * spot.pot).min(o.behind).min(spot.hero_behind);
    if bet <= 0.0 {
        return plain;
    }
    let top = top_share(&o.range, &strength.equity, board_dead);
    let bets = Range((0..COMBOS).map(|i| o.range.0[i] * curve.of(top[i], strength.draw[i]).clamp(0.0, 1.0)).collect());
    let checks = Range((0..COMBOS).map(|i| o.range.0[i] - bets.0[i]).collect());
    let q = bets.live(dead) / o.range.live(dead).max(1e-9);
    if q <= 0.0 || q >= 1.0 {
        return plain;
    }
    let eq_checks = equity(spot.hero, &[&checks], &spot.board, rng);
    let eq_bets = equity(spot.hero, &[&bets], &spot.board, rng);
    (1.0 - q) * eq_checks * spot.pot + q * (eq_bets * (spot.pot + 2.0 * bet) - bet).max(0.0)
}

/// EV of betting or raising to `to` on the street, and how often everyone folds to it.
fn aggression(spot: &Spot, to: f64, strength: &[f32], board_dead: CardSet, dead: CardSet, rng: &mut ChaCha8Rng, t: &Tuning) -> (f64, Option<f64>) {
    let add = (to - spot.hero_in).max(0.0);
    let all_in = add >= spot.hero_behind - 1e-9;
    let facing = spot.to_call() > 0.0;
    let mut shares = Vec::new();
    let mut continuing_ranges = Vec::new();
    for o in &spot.opponents {
        let need = (to - o.street_in).min(o.behind).max(0.0);
        if need <= 0.0 {
            continue;
        }
        // Minimum defence: the share that keeps a bluff from profiting at once.
        let mdf = spot.pot / (spot.pot + need);
        let mut share = (mdf * t.defend) as f32;
        if facing {
            share *= 0.8;
        }
        if all_in && !spot.river {
            share *= 0.85;
        }
        let share = share.clamp(0.02, 0.95);
        let live = o.range.live(dead);
        if live <= 0.0 {
            continue;
        }
        shares.push(share as f64);
        continuing_ranges.push(continuing(&o.range, strength, board_dead, share));
    }
    if shares.is_empty() {
        // Nobody can put in more: the bet is a call of what is there, the rest comes back.
        let call = spot.to_call();
        let eq = equity(spot.hero, &spot.opponents.iter().map(|o| &o.range).collect::<Vec<_>>(), &spot.board, rng);
        return (realization(spot, 0.0, eq, t) * eq * (spot.pot + call) - call, None);
    }
    let all_fold: f64 = shares.iter().map(|s| 1.0 - s).product();
    let callers: f64 = shares.iter().sum::<f64>() / (1.0 - all_fold).max(1e-9);
    // Against one likely caller the strongest continuing range stands in; against more,
    // all of them.
    let eq_called = if callers < 1.5 {
        let k = (0..shares.len()).max_by(|&a, &b| shares[a].total_cmp(&shares[b])).unwrap_or(0);
        equity(spot.hero, &[&continuing_ranges[k]], &spot.board, rng)
    } else {
        equity(spot.hero, &continuing_ranges.iter().collect::<Vec<_>>(), &spot.board, rng)
    };
    let called_pot = spot.pot + add + callers * add;
    let behind = spot.hero_behind - add;
    let rr = if all_in { 1.0 } else { realization(&Spot { aggressor: true, ..spot.clone() }, behind / called_pot.max(0.01), eq_called, t) };
    (all_fold * spot.pot + (1.0 - all_fold) * (rr * eq_called * called_pot - add), Some(all_fold))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared(options: Vec<Opt>, evs: Vec<f64>, equity: f64, pot_odds: Option<f64>) -> Prepared {
        let folds = vec![None; options.len()];
        Prepared { options, evs, folds, equity, pot_odds, pot: 20.0 }
    }

    /// Shrinking the gaps leaves folding at 0 and every gap as the shrink makes it.
    #[test]
    fn folding_brings_nothing_and_the_gaps_stay() {
        let p = prepared(vec![Opt::Fold, Opt::Call, Opt::Raise(30.0)], vec![0.0, 19.89, 20.11], 0.9, Some(0.28));
        for t in [Tuning::default(), Tuning { shrink: 0.18, line_margin: 0.05, line_pct: 1.5, ..Tuning::default() }] {
            let est = settle(&p, &t);
            assert_eq!(est.evs[0], 0.0, "{t:?}");
            let top = est.evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            // The gap to the best option (what the move costs) is the shrunk one, capped
            // for options that go on.
            let goes_on = 0.9 >= 0.28 - t.line_margin && 0.9 < t.line_top;
            for (k, &raw) in p.evs.iter().enumerate() {
                let loss = t.shrink * (20.11 - raw);
                let want = if goes_on && k > 0 { loss.min(t.line_pct / 100.0 * 20.0) } else { loss };
                assert!((top - est.evs[k] - want).abs() < 1e-9, "{t:?} option {k}");
            }
        }
        // Without a fold nothing is anchored.
        let check = prepared(vec![Opt::Check, Opt::Bet(6.0)], vec![5.0, 6.0], 0.6, None);
        assert_eq!(settle(&check, &Tuning::default()).evs, vec![5.0, 6.0]);
    }
}
