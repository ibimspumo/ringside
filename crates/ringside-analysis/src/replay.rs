//! A hand played up to one of the hero's decisions, as the estimate needs it: chips,
//! who is still in, and every opponent's range narrowed by what they did.

use std::collections::HashMap;

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_core::db::{Choice, Decision, Source, SpotType};
use ringside_core::model::{Action, ActionKind, Hand, Street};
use ringside_engine::preflop::postflop_rank;
use ringside_engine::{Card, CardSet};

use crate::calibration::Calibration;
use crate::estimate::{Curve, Opponent, Opt, Range, Signal, Spot, estimate, narrow_by, size_pct, strength};
use crate::fmt::bb;
use crate::seats::hero_combo;
use crate::{base, fmt, pot_before, score, why};

/// Ranges the hand's line in the model gives at the flop, by player.
pub type FlopRanges = HashMap<String, Vec<f64>>;

fn street_cards(street: Street) -> usize {
    match street {
        Street::Preflop => 0,
        Street::Flop => 3,
        Street::Turn => 4,
        Street::River => 5,
    }
}

/// The model's ranges at the flop along the hand's preflop line, when it is in the model.
pub fn flop_ranges(hand: &Hand, hero: &str) -> Option<FlopRanges> {
    crate::preflop::analyze(hand, hero).ranges
}

/// The spot of the hero's action `index`, or why there is none.
/// Opponents' ranges narrow after the flop by `curves`.
pub fn spot_at(hand: &Hand, hero: &str, index: usize, flop: Option<&FlopRanges>, curves: &[Curve]) -> Result<Spot, String> {
    let big = hand.big_blind.max(1) as f64;
    let combo = hero_combo(hand).ok_or("Deine Karten fehlen in der Hand-History.")?;
    let action = &hand.actions[index];
    let board: Vec<Card> = hand.board.iter().take(street_cards(action.street)).map(|c| c.parse::<Card>()).collect::<Result<_, _>>().map_err(|_| "Board unlesbar.")?;
    if board.len() < street_cards(action.street) {
        return Err("Board unvollständig.".into());
    }
    // Seats dealt in, in preflop order: first after the big blind, big blind last.
    let dealt: Vec<&str> = hand
        .seats
        .iter()
        .filter(|s| !s.sitting_out || hand.actions.iter().any(|a| a.player == s.name))
        .map(|s| s.name.as_str())
        .collect();
    let n = dealt.len();
    let bb_name = hand.actions.iter().find(|a| a.kind == ActionKind::PostBigBlind).map(|a| a.player.as_str());
    let at = bb_name.and_then(|b| dealt.iter().position(|&s| s == b)).unwrap_or(n - 1);
    let order: Vec<&str> = (1..=n).map(|k| dealt[(at + k) % n]).collect();
    let seat = |name: &str| order.iter().position(|&s| s == name);
    let stack = |name: &str| hand.seats.iter().find(|s| s.name == name).map_or(0.0, |s| s.stack as f64 / big);

    let mut rng = ChaCha8Rng::seed_from_u64(hand.id ^ (index as u64) << 40);
    let mut put: HashMap<&str, f64> = HashMap::new();
    let mut street_in: HashMap<&str, f64> = HashMap::new();
    let mut folded: Vec<&str> = Vec::new();
    let from_model = action.street != Street::Preflop && flop.is_some();
    let mut ranges: HashMap<&str, Range> = order.iter().map(|&p| (p, Range::all())).collect();
    if let Some(f) = flop.filter(|_| from_model) {
        for (name, w) in f {
            if let Some(r) = ranges.get_mut(name.as_str()) {
                *r = Range::from_classes(w);
            }
        }
    }
    let mut street = Street::Preflop;
    let mut strengths = strength(&[], &mut rng);
    let mut raises = 0;
    let mut aggressor: Option<&str> = None;
    let mut street_aggressor: Option<&str> = None;
    let rank = |p: &str| seat(p).map_or(0, |s| postflop_rank(n, s));
    for (i, a) in hand.actions[..index].iter().enumerate() {
        if a.street != street {
            street = a.street;
            street_aggressor = aggressor;
            street_in.clear();
            let cards: Vec<Card> = hand.board.iter().take(street_cards(street)).filter_map(|c| c.parse().ok()).collect();
            strengths = strength(&cards, &mut rng);
            raises = 0;
        }
        *put.entry(a.player.as_str()).or_default() += a.amount as f64 / big;
        if a.kind != ActionKind::PostAnte {
            *street_in.entry(a.player.as_str()).or_default() += a.amount as f64 / big;
        }
        if a.kind == ActionKind::Fold {
            folded.push(a.player.as_str());
            continue;
        }
        if a.kind.is_post() || a.player == hero || (street == Street::Preflop && from_model) {
            if a.kind.is_aggressive() {
                raises += 1;
                aggressor = Some(a.player.as_str());
            }
            continue;
        }
        let signal = if street == Street::Preflop {
            preflop_signal(hand, a, raises, &order, seat(&a.player), i)
        } else {
            let added = a.amount as f64 / big;
            let own = street_in.get(a.player.as_str()).copied().unwrap_or(0.0) - added;
            let facing = street_in.iter().filter(|(p, _)| **p != a.player.as_str()).map(|(_, v)| *v).fold(0.0, f64::max);
            let size = size_pct(pot_before(hand, i), facing, own, own + added);
            match a.kind {
                ActionKind::Bet => Some(Signal::Bet { size, aggressor: street_aggressor == Some(a.player.as_str()) }),
                ActionKind::Raise => Some(Signal::Raise(size)),
                ActionKind::Call => Some(Signal::Call(size)),
                ActionKind::Check => {
                    let behind = order.iter().filter(|&&p| p != a.player && !folded.contains(&p)).all(|&p| rank(p) < rank(&a.player));
                    Some(Signal::PostCheck { behind })
                }
                _ => None,
            }
        };
        if a.kind.is_aggressive() || (a.all_in && a.kind != ActionKind::Call) {
            raises += 1;
            aggressor = Some(a.player.as_str());
        }
        if let (Some(sig), Some(r)) = (signal, ranges.get_mut(a.player.as_str())) {
            let dead = CardSet::from_cards(&hand.board.iter().take(street_cards(street)).filter_map(|c| c.parse().ok()).collect::<Vec<Card>>());
            narrow_by(r, sig, &strengths, dead, curves);
        }
    }
    // The hero may be first to act on a new street.
    if action.street != street {
        street_in.clear();
    }
    let alive: Vec<&str> = order.iter().copied().filter(|p| !folded.contains(p) && *p != hero).collect();
    if alive.is_empty() {
        return Err("Kein Gegner mehr in der Hand.".into());
    }
    let hero_put = put.get(hero).copied().unwrap_or(0.0);
    let behind = |p: &str| (stack(p) - put.get(p).copied().unwrap_or(0.0)).max(0.0);
    let opponents: Vec<Opponent> = alive
        .iter()
        .map(|&p| Opponent { range: ranges.remove(p).unwrap_or_else(Range::all), behind: behind(p), street_in: street_in.get(p).copied().unwrap_or(0.0) })
        .collect();
    let hero_rank = rank(hero);
    Ok(Spot {
        preflop: action.street == Street::Preflop,
        river: action.street == Street::River,
        board,
        hero: combo,
        opponents,
        pot: pot_before(hand, index),
        hero_in: street_in.get(hero).copied().unwrap_or(0.0),
        hero_behind: (stack(hero) - hero_put).max(0.0),
        in_position: alive.iter().all(|&p| rank(p) < hero_rank),
        aggressor: aggressor == Some(hero),
    })
}

fn preflop_signal(hand: &Hand, a: &Action, raises: usize, order: &[&str], seat: Option<usize>, index: usize) -> Option<Signal> {
    let big = hand.big_blind.max(1) as f64;
    let to = a.to.unwrap_or(a.amount) as f64 / big;
    let stack_bb = |name: &str| hand.seats.iter().find(|s| s.name == name).map_or(0.0, |s| s.stack as f64 / big);
    let pre_shove = hand.actions[..index].iter().any(|x| x.street == Street::Preflop && x.all_in && x.kind != ActionKind::Call && !x.kind.is_post());
    Some(match a.kind {
        ActionKind::Check => Signal::Check,
        ActionKind::Call if raises == 0 => Signal::Limp,
        ActionKind::Call if pre_shove => Signal::CallShove { bb: stack_bb(&a.player).round() as u16 },
        ActionKind::Call => Signal::CallRaise,
        ActionKind::Raise | ActionKind::Bet if a.all_in || to >= 0.9 * stack_bb(&a.player) => Signal::Shove { bb: stack_bb(&a.player).round() as u16 },
        ActionKind::Raise | ActionKind::Bet => match raises {
            0 => Signal::Open { behind: seat.map_or(3, |s| (order.len() - 1 - s) as u8) },
            1 => Signal::ThreeBet,
            _ => Signal::FourBet,
        },
        _ => return None,
    })
}

/// The hero's options at `spot` and the one played (index).
pub fn options(spot: &Spot, a: &Action, big: f64) -> (Vec<Opt>, usize) {
    let call = spot.to_call();
    let cap = spot.hero_in + spot.hero_behind;
    let top = spot.hero_in + call;
    let mut opts = Vec::new();
    if call > 0.0 {
        opts.push(Opt::Fold);
        opts.push(Opt::Call);
    } else {
        opts.push(Opt::Check);
    }
    let mut sizes: Vec<f64> = Vec::new();
    if spot.preflop {
        if top <= 1.0 + 1e-9 {
            // First in an open, behind limps the usual isolation size; the big blind
            // among the opponents is no limper.
            let blind = usize::from(spot.hero_in < 1.0 - 1e-9);
            let limpers = spot.opponents.iter().filter(|o| o.street_in >= 1.0 - 1e-9).count().saturating_sub(blind);
            sizes.push(if limpers == 0 { 2.5 } else { 3.0 + limpers as f64 });
        } else {
            sizes.push(top * 3.0);
        }
    } else if call > 0.0 {
        sizes.push(top + 2.0 * call + (spot.pot + call) * 0.5);
    } else {
        for pct in [0.33, 0.66, 1.0] {
            sizes.push(spot.hero_in + spot.pot * pct);
        }
    }
    let played_to = a.to.unwrap_or(a.amount) as f64 / big + if a.kind == ActionKind::Bet { spot.hero_in } else { 0.0 };
    let aggressive = a.kind.is_aggressive() && !(a.all_in || played_to >= cap - 1e-9);
    if aggressive && !sizes.iter().any(|s| (s / played_to).ln().abs() < 0.1) {
        sizes.push(played_to);
    }
    for s in sizes {
        if s < cap * 0.9 && s > top {
            opts.push(if call > 0.0 || (spot.preflop && top >= 1.0) { Opt::Raise(s) } else { Opt::Bet(s) });
        }
    }
    // As in the solver's trees: all in only when the stack is at most one and a half
    // pots, or when it was played.
    let shove_played = !matches!(a.kind, ActionKind::Fold | ActionKind::Check | ActionKind::Call) && (a.all_in || played_to >= cap - 1e-9);
    if spot.preflop || shove_played || spot.hero_behind - call <= 1.5 * (spot.pot + 2.0 * call) {
        opts.push(Opt::AllIn);
    }
    let played = match a.kind {
        ActionKind::Fold => opts.iter().position(|o| *o == Opt::Fold),
        ActionKind::Check => opts.iter().position(|o| *o == Opt::Check),
        ActionKind::Call if a.all_in && call >= spot.hero_behind - 1e-9 => opts.iter().position(|o| *o == Opt::Call),
        ActionKind::Call => opts.iter().position(|o| *o == Opt::Call),
        _ if a.all_in || played_to >= cap - 1e-9 => opts.iter().position(|o| *o == Opt::AllIn),
        _ => opts
            .iter()
            .enumerate()
            .filter_map(|(i, o)| match o {
                Opt::Bet(x) | Opt::Raise(x) => Some((i, (x / played_to).ln().abs())),
                _ => None,
            })
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|x| x.0),
    };
    // Calling all in for less than the bet is the same as calling.
    if call >= spot.hero_behind - 1e-9 {
        opts.retain(|o| *o != Opt::AllIn);
    }
    let played = played.unwrap_or(0).min(opts.len() - 1);
    (opts, played)
}

fn opt_label(o: Opt) -> String {
    match o {
        Opt::Fold => "Fold".into(),
        Opt::Check => "Check".into(),
        Opt::Call => "Call".into(),
        Opt::Bet(x) => format!("Bet {}", bb(x)),
        Opt::Raise(x) => format!("Raise auf {}", bb(x)),
        Opt::AllIn => "All-in".into(),
    }
}

/// Measured agreement of the estimate's grades with the reference at `street`.
pub fn reliability(street: Street) -> f64 {
    crate::calibration::stored().reliability(street)
}

/// The hero's action `index` rated by the estimate, `why_not` saying why model and
/// solver don't cover it.
pub fn rate(hand: &Hand, hero: &str, index: usize, spot_type: SpotType, flop: Option<&FlopRanges>, why_not: &str) -> Result<Decision, String> {
    rate_with(hand, hero, index, spot_type, flop, why_not, crate::calibration::stored())
}

/// [`rate`] with the estimate's numbers given.
pub fn rate_with(
    hand: &Hand,
    hero: &str,
    index: usize,
    spot_type: SpotType,
    flop: Option<&FlopRanges>,
    why_not: &str,
    numbers: &Calibration,
) -> Result<Decision, String> {
    let spot = spot_at(hand, hero, index, flop, &numbers.moves)?;
    let tuning = &numbers.tuning(hand.actions[index].street);
    let a = &hand.actions[index];
    let big = hand.big_blind.max(1) as f64;
    let (opts, played) = options(&spot, a, big);
    let est = estimate(&spot, &opts, hand.id.wrapping_mul(31).wrapping_add(index as u64), tuning, &numbers.moves);
    let pot = spot.pot;
    let freq = crate::estimate::strategy(&est, pot, tuning);
    let strategy: Vec<Choice> = opts
        .iter()
        .enumerate()
        .map(|(i, &o)| Choice { label: opt_label(o), freq: freq[i], ev_bb: Some(est.evs[i]), played: i == played, kind: o.key().into() })
        .collect();
    let opponents = spot.opponents.len();
    let top = (0..opts.len()).max_by(|&x, &y| est.evs[x].total_cmp(&est.evs[y]));
    let behind = spot.opponents.iter().map(|o| o.behind).fold(0.0, f64::max).min(spot.hero_behind);
    let facts = why::Estimated {
        equity: est.equity,
        opponents,
        need: est.pot_odds,
        fold: top.and_then(|k| est.folds[k]),
        spr: (!spot.preflop && est.pot_odds.is_none() && spot.pot > 0.0).then(|| behind / spot.pot),
    };
    let preflop = spot.preflop;
    let rel = reliability(a.street);
    let mut note = format!(
        "Einschätzung (keine Lösung): {why_not} Ranges aus der Vorgeschichte{}, Equity gegen alle Gegner zugleich, Fold Equity aus den Pot Odds der Gegner; liegt ein Zug nahe an der Indifferenz, zählt er als Mix. In mindestens {} % der Prüf-Spots lag sie höchstens eine Stufe neben {}, bei guten Zügen wie bei Fehlern.",
        if flop.is_some() && !preflop { " (am Flop aus dem Preflop-Modell)" } else { "" },
        (rel * 100.0).round(),
        if preflop { "dem Preflop-Modell" } else { "dem Solver" }
    );
    let small = crate::calibration::stored().small(a.street);
    if !small.is_empty() {
        note.push_str(&format!(" Zu wenige Fälle, um mitzuzählen: {}.", small.join(", ")));
    }
    if opponents > 1 {
        note.push_str(" Gemessen ist das heads-up; mit mehreren Gegnern ist sie ungeprüft.");
    }
    let d = base(hand.id, index, a.street, spot_type, fmt::real_action(hand, a));
    let mut d = score(d, Source::Estimate, strategy, pot, note, |d| why::estimate(&facts, &d.strategy, pot));
    d.reliability = Some(rel);
    Ok(d)
}
