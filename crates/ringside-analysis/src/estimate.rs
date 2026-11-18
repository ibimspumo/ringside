//! The estimate: a decision rated by equity against the likely ranges, pot odds and fold
//! equity, for every spot the model and the solver don't cover (pots of three or more
//! players after the flop, lines outside the model, spots the solver can't take).
//!
//! - **Ranges.** Every opponent starts with the model's range where the hand's line
//!   was in the model, otherwise with all hands; each action narrows it by how strong a
//!   hand is: before the flop by all-in equity against a random hand (limps by the
//!   model's typical limping range), after the flop by equity against a random hand on
//!   the board. Bets and raises keep the strong part and some of the weakest (bluffs),
//!   calls the middle, checks most of it.
//! - **Equity** of the hero's two cards against all opponents at once, sampled with card
//!   removal.
//! - **EV of each option**, in BB from now on: fold 0; check `R × equity × pot`; call
//!   `R × equity × (pot + call) − call`; a bet or raise wins the pot when everyone folds
//!   and otherwise plays against what continues. Opponents continue with the strongest
//!   part of their range, as much as the pot odds of the bet ask them to defend (the
//!   minimum defence frequency, a little more after the flop, as real players call
//!   more); all-ins are played out. `R` (equity realization) is 1 on the river and
//!   all-in; before that less out of position, with more opponents and with a deeper
//!   stack-to-pot ratio, more for the aggressor.
//!
//! - **Kinds, not sizes.** The formulas favour the biggest bet far more than solvers
//!   do, so the estimate rates the kind of move only: every bet or raise (all-in
//!   included) counts as much as the best one, as does every check or call.
//!
//! How often its grades agree with the solver's is measured on solved spots
//! (`analysis-calibrate`) and stored with every estimated decision.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::eval::Hand as Cards;
use ringside_engine::preflop::limp_shape;
use ringside_engine::{Card, CardSet, Combo, HandClass, PreflopMatrix};
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

/// Showdown strength of every combo: all-in equity against a random hand, before the
/// flop from the class table, after it sampled on `board` (0 for combos on the board).
pub fn strength(board: &[Card], rng: &mut ChaCha8Rng) -> Vec<f32> {
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
    /// After the flop.
    Bet,
    Raise,
    Call,
    PostCheck,
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
        Signal::Bet => {
            if top <= 0.40 {
                1.0
            } else if p <= 0.25 {
                0.35
            } else {
                0.25
            }
        }
        Signal::Raise => {
            if top <= 0.20 {
                1.0
            } else if p <= 0.20 {
                0.15
            } else {
                0.1
            }
        }
        Signal::Call => {
            if top <= 0.10 {
                0.6
            } else if p >= 0.30 {
                1.0
            } else {
                0.1
            }
        }
        Signal::PostCheck => {
            if top <= 0.12 {
                0.5
            } else {
                1.0
            }
        }
    }
}

/// Narrows `range` after `signal`; `strength` is per combo, `dead` the visible cards.
pub fn narrow(range: &mut Range, signal: Signal, strength: &[f32], dead: CardSet) {
    match signal {
        Signal::Limp => {
            let shape = limp_shape();
            for c in Combo::all() {
                range.0[c.index()] *= shape[c.class().index()] as f32;
            }
            return;
        }
        Signal::Check => {
            let top = top_share(range, strength, dead);
            for c in Combo::all() {
                if top[c.index()] <= 0.05 {
                    range.0[c.index()] *= 0.3;
                }
            }
            return;
        }
        _ => {}
    }
    let top = top_share(range, strength, dead);
    for c in Combo::all() {
        let i = c.index();
        range.0[i] *= keep(signal, 1.0 - top[i]);
    }
}

/// Per combo: the share of the range (by weight) at least as strong.
fn top_share(range: &Range, strength: &[f32], dead: CardSet) -> Vec<f32> {
    let mut order: Vec<usize> = (0..COMBOS).filter(|&i| range.0[i] > 0.0 && Combo::from_index(i).mask().0 & dead.0 == 0).collect();
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
}

fn default_tie() -> f64 {
    ringside_play::grade::BEST_PCT
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning { shrink: 1.0, strength: 0.0, defend: 1.1, tie_pct: default_tie() }
    }
}

/// Equity realization on later streets.
fn realization(s: &Spot, spr: f64, eq: f64, t: &Tuning) -> f64 {
    if s.river {
        return 1.0;
    }
    let pos = if s.in_position { 1.0 } else { 0.86 };
    let crowd = 1.0 - 0.07 * (s.opponents.len().saturating_sub(1)) as f64;
    let depth = 1.0 - 0.04 * spr.clamp(0.0, 6.0);
    let lead = if s.aggressor { 1.04 } else { 1.0 };
    let strength = 1.0 + t.strength * (eq - 0.5);
    (pos * crowd * depth * lead * strength).clamp(0.4, 1.6)
}

/// EVs of `options` at `spot`.
pub fn estimate(spot: &Spot, options: &[Opt], seed: u64, t: &Tuning) -> Estimate {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let dead = CardSet(spot.hero.mask().0 | CardSet::from_cards(&spot.board).0);
    let board_dead = CardSet::from_cards(&spot.board);
    let ranges: Vec<&Range> = spot.opponents.iter().map(|o| &o.range).collect();
    let eq = equity(spot.hero, &ranges, &spot.board, &mut rng);
    let strength = strength(&spot.board, &mut rng);
    let call = spot.to_call();
    let behind_after_call = spot.hero_behind - call;
    let spr_now = behind_after_call / (spot.pot + call).max(0.01);
    let r = realization(spot, spr_now, eq, t);
    let pot_odds = (call > 0.0).then(|| call / (spot.pot + call));
    let ev_of = |opt: Opt, rng: &mut ChaCha8Rng| -> f64 {
        match opt {
            Opt::Fold => 0.0,
            Opt::Check => r * eq * spot.pot,
            Opt::Call => {
                let rr = if call >= spot.hero_behind - 1e-9 { 1.0 } else { r };
                rr * eq * (spot.pot + call) - call
            }
            Opt::Bet(to) | Opt::Raise(to) => aggression(spot, to.min(spot.hero_in + spot.hero_behind), &strength, board_dead, dead, rng, t),
            Opt::AllIn => aggression(spot, spot.hero_in + spot.hero_behind, &strength, board_dead, dead, rng, t),
        }
    };
    let raw: Vec<f64> = options.iter().map(|&o| ev_of(o, &mut rng)).collect();
    let family = |o: Opt| match o {
        Opt::Fold => 0,
        Opt::Check | Opt::Call => 1,
        _ => 2,
    };
    let by_kind: Vec<f64> = options
        .iter()
        .map(|&o| options.iter().zip(&raw).filter(|(x, _)| family(**x) == family(o)).map(|(_, &e)| e).fold(f64::NEG_INFINITY, f64::max))
        .collect();
    let top = by_kind.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let evs = by_kind.iter().map(|&e| top - t.shrink * (top - e)).collect();
    Estimate { evs, equity: eq, pot_odds }
}

/// EV of betting or raising to `to` on the street.
fn aggression(spot: &Spot, to: f64, strength: &[f32], board_dead: CardSet, dead: CardSet, rng: &mut ChaCha8Rng, t: &Tuning) -> f64 {
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
        let eq = equity(spot.hero, &spot.opponents.iter().map(|o| &o.range).collect::<Vec<_>>(), &spot.board, rng);
        return realization(spot, 0.0, eq, t) * eq * spot.pot;
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
    all_fold * spot.pot + (1.0 - all_fold) * (rr * eq_called * called_pot - add)
}
