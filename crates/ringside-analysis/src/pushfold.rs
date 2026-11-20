//! Push/fold decisions against the bundled equilibrium tables.
//!
//! Exact solves of the real table (uneven stacks, dead money) take seconds each, too
//! slow for a whole history, so the nearest table entry stands in: the same number of
//! players, every stack equal to the hero's effective stack (rounded to the grid) and
//! the closest ante structure. The note says what differs.

use std::sync::{Arc, Mutex, PoisonError};

use ringside_core::model::Hand;
use ringside_core::db::{Choice, Decision, Source, SpotType};
use ringside_engine::multiway::Solution;
use ringside_engine::pushfold_table::{self, Ante, CLASSIC_ANTE_BB, BB_ANTE_BB};
use ringside_learn::{Answer, Kind, OwnSpot, Prompt, TableSpot, detect_spots};

use crate::fmt::{bb, num};
use crate::seats::{ante_name, nearest};
use crate::{ChartKind, ChartTarget, base_at, pot_before, score, unrated, why};

type Key = (Ante, usize, u64);
const CACHE_LIMIT: usize = 64;
static CACHE: Mutex<Vec<(Key, Option<Arc<Solution>>)>> = Mutex::new(Vec::new());

pub(crate) fn table(ante: Ante, players: usize, stack: f64) -> Option<Arc<Solution>> {
    let key = (ante, players, stack.to_bits());
    if let Some((_, hit)) = CACHE.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(k, _)| *k == key) {
        return hit.clone();
    }
    let sol = pushfold_table::chart(ante, players, stack).map(Arc::new);
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if !cache.iter().any(|(k, _)| *k == key) {
        cache.push((key, sol.clone()));
        if cache.len() > CACHE_LIMIT {
            cache.remove(0);
        }
    }
    sol
}

/// The hero's push/fold decision of `hand`, if it has one.
pub fn spot(hand: &Hand) -> Option<OwnSpot> {
    detect_spots(hand, Some(Kind::PushFoldOpen)).pop().or_else(|| detect_spots(hand, Some(Kind::PushFoldCall)).pop())
}

/// The decision, and the chart spot it was rated against when there is one.
pub fn evaluate(hand: &Hand, own: &OwnSpot) -> (Decision, Option<ChartTarget>) {
    let index = own.stop_before_action;
    let Prompt::PushFoldTable {
        hand: cards, stacks_bb, small_blind_bb, ante_bb, bb_ante_bb, dead_bb, hero, shover, callers, ..
    } = &own.exercise.prompt
    else {
        return (unrated(hand, index, SpotType::Shove, "Kein Push/Fold-Spot."), None);
    };
    let spot_type = if shover.is_some() { SpotType::CallShove } else { SpotType::Shove };
    let ts = TableSpot {
        stacks_bb: stacks_bb.clone(),
        small_blind_bb: *small_blind_bb,
        ante_bb: *ante_bb,
        bb_ante_bb: *bb_ante_bb,
        dead_bb: *dead_bb,
        hero: *hero,
        shover: *shover,
        callers: callers.clone(),
    };
    let ante = if *bb_ante_bb > 0.0 {
        Ante::BigBlind
    } else if *ante_bb > 0.0 {
        Ante::Classic
    } else {
        Ante::None
    };
    let players = ts.players();
    let effective = ts.effective_bb();
    let stack = nearest(pushfold_table::stacks(ante), effective);
    let Some(sol) = table(ante, players, stack) else {
        return (unrated(hand, index, spot_type, "Für diese Tischgröße gibt es keine Push/Fold-Tabelle."), None);
    };
    let class = ringside_engine::Combo::new(cards[0], cards[1]).class();
    let Some(s) = sol.spot(ts.hero, class, ts.shover, &ts.callers) else {
        return (unrated(hand, index, spot_type, "Diesen Spot (z. B. zu viele Caller) gibt es im Push/Fold-Modell nicht."), None);
    };
    let (act, kind) = if shover.is_some() { ("Call", "call") } else { ("All-in", "all_in") };
    let did_act = !matches!(&own.played, Some(Answer::Choice(c)) if c == "fold");
    let strategy = vec![
        Choice { label: act.into(), freq: s.freq, ev_bb: Some(s.ev_act), played: did_act, kind: kind.into() },
        Choice { label: "Fold".into(), freq: 1.0 - s.freq, ev_bb: Some(s.ev_fold), played: !did_act, kind: "fold".into() },
    ];

    let mut note = vec![format!(
        "Push/Fold-Gleichgewicht in Chips (ohne ICM): {players} Spieler, alle mit {}, {}",
        bb(stack),
        ante_name(ante)
    )];
    if (stack - effective).abs() >= 0.25 {
        note.push(format!("nächste vorgerechnete Tiefe: {} statt {} effektiv", bb(stack), bb(effective)));
    }
    let max = ts.stacks_bb.iter().copied().fold(0.0, f64::max);
    let min = ts.stacks_bb.iter().copied().fold(f64::INFINITY, f64::min);
    if max - min > 1.0 {
        note.push(format!("echte Stacks ungleich ({} bis {})", num(min), bb(max)));
    }
    let (table_ante, real_ante) = match ante {
        Ante::Classic => (CLASSIC_ANTE_BB, *ante_bb),
        Ante::BigBlind => (BB_ANTE_BB, *bb_ante_bb),
        Ante::None => (0.0, 0.0),
    };
    if (real_ante - table_ante).abs() > 0.1 * table_ante.max(0.01) {
        note.push(format!("Ante gerechnet mit {} statt {}", bb(table_ante), bb(real_ante)));
    }
    if (*small_blind_bb - 0.5).abs() > 0.01 {
        note.push(format!("Small Blind gerechnet mit 0,5 BB statt {}", bb(*small_blind_bb)));
    }
    if *dead_bb > 0.0 {
        note.push(format!("totes Geld von {} nicht berücksichtigt", bb(*dead_bb)));
    }
    if !sol.converged {
        note.push(format!("Lösung nicht voll konvergiert (bis {})", bb(sol.exploitability)));
    }
    let node = &sol.nodes[s.node];
    let margin = |c: usize| node.ev_act[c] - node.ev_fold[c];
    let mine = margin(class.index());
    let combos = |c: &ringside_engine::HandClass| c.combo_count() as f64 / 1326.0;
    let rank = ringside_engine::HandClass::all()
        .map(|c| if margin(c.index()) > mine { combos(&c) } else if c == class { combos(&c) / 2.0 } else { 0.0 })
        .sum::<f64>();
    // The position the rest of the app shows; the chart's table may leave out players
    // who were all in with the ante.
    let seat = hand
        .hero
        .as_deref()
        .and_then(|h| ringside_core::stats::position_labels(hand).into_iter().find(|(n, _)| n == h))
        .map_or_else(|| ringside_engine::multiway::seat_name(players, ts.hero).to_string(), |(_, p)| p.to_string());
    let facts = why::Ranked {
        chart: why::Chart::Equilibrium,
        place: if shover.is_some() { format!("Als {seat} gegen das All-in bei {} effektiv", bb(effective)) } else { format!("Als {seat} bei {} effektiv", bb(effective)) },
        act: Some(if shover.is_some() { why::Move::Call } else { why::Move::AllIn }),
        share: sol.act_pct(s.node) / 100.0,
        all_hands: true,
        class: class.to_string(),
        rank,
    };
    let target = ChartTarget {
        action_index: index,
        kind: ChartKind::Pushfold,
        ante,
        players,
        stack_bb: stack,
        seat: ts.hero,
        shover: ts.shover,
        callers: ts.callers.clone(),
        node: None,
        hand: class.to_string(),
    };
    (score(base_at(hand, index, spot_type), Source::Equilibrium, strategy, pot_before(hand, index), note.join("; ") + ".", |d| why::ranked(&facts, &d.strategy, d.pot_bb.unwrap_or(1.0))), Some(target))
}
