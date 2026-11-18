//! The estimate as the grade source of "Hände spielen" where no solver plays after the
//! flop: the same ranges, EVs, grades and reliability as for a real hand
//! ([`crate::replay::rate`]), from what the practice table knows of the hand.

use std::sync::Mutex;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_core::db::Choice;
use ringside_core::model::Street as HandStreet;
use ringside_engine::preflop::postflop_rank;
use ringside_engine::{CardSet, Combo};
use ringside_play::assess::{Assess, Assessment, Situation};
use ringside_play::hand::Street;

use crate::estimate::{Opponent, Opt, Range, Signal, Spot, estimate, narrow, strength, strategy};
use crate::why;

/// Lessons are asked for more than once per decision (to show it, to grade it, to look
/// ahead); the last few answers are kept.
const CACHE_LIMIT: usize = 8;

#[derive(Default)]
pub struct Estimator {
    cache: Mutex<Vec<(String, Option<Assessment>)>>,
}

impl Estimator {
    pub fn new() -> Estimator {
        Estimator::default()
    }
}

fn hand_street(s: Street) -> HandStreet {
    match s {
        Street::Preflop => HandStreet::Preflop,
        Street::Flop => HandStreet::Flop,
        Street::Turn => HandStreet::Turn,
        Street::River => HandStreet::River,
    }
}

fn cards(s: Street) -> usize {
    match s {
        Street::Preflop => 0,
        Street::Flop => 3,
        Street::Turn => 4,
        Street::River => 5,
    }
}

fn signal(kind: &str) -> Option<Signal> {
    Some(match kind {
        "bet" => Signal::Bet,
        "raise" | "all_in" => Signal::Raise,
        "call" => Signal::Call,
        "check" => Signal::PostCheck,
        _ => return None,
    })
}

fn opt(c: &ringside_play::hand::Choice) -> Opt {
    match c.kind {
        "fold" => Opt::Fold,
        "check" => Opt::Check,
        "call" => Opt::Call,
        "bet" => Opt::Bet(c.to_bb),
        "raise" => Opt::Raise(c.to_bb),
        _ => Opt::AllIn,
    }
}

/// The opponents' ranges at the decision: the model's range from the flop on, narrowed by
/// every move after it as the analysis narrows a real hand's.
fn ranges(s: &Situation) -> Vec<Range> {
    let mut rng = ChaCha8Rng::seed_from_u64(0x5eed);
    let mut strengths: Vec<(Street, Vec<f32>)> = Vec::new();
    let mut out: Vec<Range> = s.ranges.iter().map(|r| r.as_deref().map_or_else(Range::all, Range::from_classes)).collect();
    for m in &s.moves {
        let Some(k) = s.opponents.iter().position(|&o| o == m.seat) else { continue };
        let Some(sig) = signal(m.kind) else { continue };
        let board = &s.board[..cards(m.street).min(s.board.len())];
        if !strengths.iter().any(|(st, _)| *st == m.street) {
            strengths.push((m.street, strength(board, &mut rng)));
        }
        let st = &strengths.iter().find(|(st, _)| *st == m.street).expect("just added").1;
        narrow(&mut out[k], sig, st, CardSet::from_cards(board));
    }
    out
}

fn compute(s: &Situation, seed: u64) -> Option<Assessment> {
    let street = hand_street(s.street);
    let me = s.hero_seat;
    let rank = |seat: usize| postflop_rank(s.players, seat);
    let opponents: Vec<Opponent> = ranges(s)
        .into_iter()
        .zip(&s.opponents)
        .map(|(range, &o)| Opponent { range, behind: s.stacks[o], street_in: s.bets[o] })
        .collect();
    let spot = Spot {
        preflop: false,
        river: s.street == Street::River,
        board: s.board.to_vec(),
        hero: Combo::new(s.hero[0], s.hero[1]),
        opponents,
        pot: s.pot,
        hero_in: s.bets[me],
        hero_behind: s.stacks[me],
        in_position: s.opponents.iter().all(|&o| rank(o) < rank(me)),
        aggressor: s.aggressor == Some(me),
    };
    let opts: Vec<Opt> = s.choices.iter().map(opt).collect();
    let calibration = crate::calibration::stored();
    let t = calibration.tuning(street);
    let est = estimate(&spot, &opts, seed, &t);
    let freq = strategy(&est, s.pot, &t);
    let choices: Vec<Choice> = opts
        .iter()
        .enumerate()
        .map(|(i, o)| Choice { label: o.key().into(), freq: freq[i], ev_bb: Some(est.evs[i]), played: false, kind: o.key().into() })
        .collect();
    let top = (0..opts.len()).max_by(|&a, &b| est.evs[a].total_cmp(&est.evs[b]));
    let behind = spot.opponents.iter().map(|o| o.behind).fold(0.0, f64::max).min(spot.hero_behind);
    let facts = why::Estimated {
        equity: est.equity,
        opponents: s.opponents.len(),
        need: est.pot_odds,
        fold: top.and_then(|k| est.folds[k]),
        spr: (est.pot_odds.is_none() && s.pot > 0.0).then(|| behind / s.pot),
    };
    Some(Assessment {
        strategy: freq,
        evs: Some(est.evs.clone()),
        equity: Some(est.equity),
        reliability: Some(calibration.reliability(street)),
        why: Some(why::estimate(&facts, &choices, s.pot)),
    })
}

impl Assess for Estimator {
    /// After the flop only: before it the estimate is measured against the preflop model,
    /// which the practice table has already left there.
    fn assess(&self, s: &Situation) -> Option<Assessment> {
        if s.street == Street::Preflop || s.choices.len() < 2 {
            return None;
        }
        let key = format!("{:?}|{:?}|{}|{:?}|{:?}|{:?}|{}", s.hero, s.board, s.pot, s.bets, s.stacks, s.opponents, s.moves.len());
        if let Some((_, hit)) = self.cache.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
            return hit.clone();
        }
        let seed = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
        let found = compute(s, seed);
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.push((key, found.clone()));
        if cache.len() > CACHE_LIMIT {
            cache.remove(0);
        }
        found
    }
}
