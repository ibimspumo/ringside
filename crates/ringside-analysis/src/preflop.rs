//! Preflop decisions: push/fold spots against the equilibrium tables, everything else
//! against the preflop model at the nearest precomputed configuration. Walking the
//! hand's preflop line through the model also yields the ranges that reach the flop.

use std::sync::{Arc, Mutex, PoisonError};

use ringside_core::db::{Choice, Decision, Source, SpotType};
use ringside_core::model::{Action, ActionKind, Hand, Street};
use ringside_core::stats::position_labels;
use ringside_engine::preflop::{Kind, Node, Solution, Step, limps_by_choice, taken};
use ringside_engine::preflop_table::{self, Ante, Family, LIMP_PLAYERS, ModelLine, STACKS};
use ringside_engine::pushfold_table::{BB_ANTE_BB, CLASSIC_ANTE_BB};
use ringside_engine::range::{HandClass, NUM_CLASSES};

use crate::fmt::bb;
use crate::replay::FlopRanges;
use crate::seats::{Table, ante_name, nearest, table};
use crate::{ChartKind, ChartTarget, base_at, hero_actions, pot_before, pushfold, score, unrated, why};

type Key = (Family, Ante, usize, u64);
const CACHE_LIMIT: usize = 24;
static CACHE: Mutex<Vec<(Key, Option<Arc<Solution>>)>> = Mutex::new(Vec::new());

/// The plain table, as the charts and the range comparison show it.
pub(crate) fn model(ante: Ante, players: usize, stack: f64) -> Option<Arc<Solution>> {
    cached(Family::Plain, ante, players, stack)
}

fn cached(family: Family, ante: Ante, players: usize, stack: f64) -> Option<Arc<Solution>> {
    let key = (family, ante, players, stack.to_bits());
    if let Some((_, hit)) = CACHE.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(k, _)| *k == key) {
        return hit.clone();
    }
    let sol = preflop_table::lookup(&family.params(ante, players, stack)).map(Arc::new);
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if !cache.iter().any(|(k, _)| *k == key) {
        cache.push((key, sol.clone()));
        if cache.len() > CACHE_LIMIT {
            cache.remove(0);
        }
    }
    sol
}

/// What the postflop analysis needs from the preflop line.
#[derive(Clone, Debug)]
pub struct Flop {
    /// The two players who see the flop: name, position, range weights per class and how
    /// the range came about.
    pub players: Vec<(String, String, Vec<f64>, String)>,
    /// Sized raises before the flop (1: single-raised pot, 2+: 3-bet pot).
    pub raises: usize,
    /// Model configuration and deviations, for the postflop note.
    pub note: String,
    /// The table and line the flop ranges follow from.
    pub model: ModelLine,
}

pub struct Preflop {
    pub decisions: Vec<Decision>,
    /// Ranges for a heads-up flop, or why there are none.
    pub flop: Result<Flop, String>,
    /// The model's range of every player who sees the flop, when the line is in the model.
    pub ranges: Option<FlopRanges>,
    /// Chart spots of the rated decisions.
    pub targets: Vec<ChartTarget>,
}

pub fn analyze(hand: &Hand, hero: &str) -> Preflop {
    analyze_in(hand, hero, Family::Limps)
}

/// [`analyze`] against the model tables of `family`: the learning spots take the plain
/// tables of the charts and the trainer.
pub(crate) fn analyze_in(hand: &Hand, hero: &str, family: Family) -> Preflop {
    let indices: Vec<usize> =
        hero_actions(hand, hero).filter(|(_, a)| a.street == Street::Preflop).map(|(i, _)| i).collect();
    let fail = |reason: &str| Preflop {
        decisions: indices.iter().map(|&i| unrated(hand, i, SpotType::VsOpen, reason)).collect(),
        flop: Err(reason.to_string()),
        ranges: None,
        targets: Vec::new(),
    };
    let t = match table(hand, hero) {
        Ok(t) => t,
        Err(reason) => return fail(&reason),
    };
    if let Some(own) = pushfold::spot(hand) {
        let (first, target) = pushfold::evaluate(hand, &own);
        let decisions = indices
            .iter()
            .map(|&i| if i == first.action_index { first.clone() } else { unrated(hand, i, first.spot, "Nach dem Push/Fold-Spot nicht bewertet.") })
            .collect();
        return Preflop { decisions, flop: Err("Push/Fold-Hand".into()), ranges: None, targets: target.into_iter().collect() };
    }
    walk(hand, &t, &indices, family)
}

/// Seat of the model for real seat `i`: smaller tables take the model's last seats, so
/// the missing early positions count as folded.
fn model_seat(t: &Table, players: usize, i: usize) -> usize {
    players - t.players() + i
}

pub(crate) fn kind_key(kind: Kind) -> &'static str {
    match kind {
        Kind::Fold => "fold",
        Kind::Check => "check",
        Kind::Call => "call",
        Kind::Limp => "limp",
        Kind::Raise => "raise",
        Kind::AllIn => "all_in",
    }
}

fn kind_name(kind: Kind, to_bb: f64) -> String {
    match kind {
        Kind::Fold => "Fold".into(),
        Kind::Check => "Check".into(),
        Kind::Call => "Call".into(),
        Kind::Limp => "Limp".into(),
        Kind::Raise => format!("Raise auf {}", bb(to_bb)),
        Kind::AllIn => "All-in".into(),
    }
}

/// The model action closest to the real one, with a note when the size differs.
fn map_action(a: &Action, actions: &[ringside_engine::preflop::Action], big: f64, stack: f64) -> Option<(usize, Option<String>)> {
    let find = |k: Kind| actions.iter().position(|x| x.kind == k);
    match a.kind {
        ActionKind::Fold => find(Kind::Fold).map(|i| (i, None)),
        ActionKind::Check => find(Kind::Check).map(|i| (i, None)),
        ActionKind::Call if a.all_in => find(Kind::Call).or(find(Kind::AllIn)).map(|i| (i, None)),
        ActionKind::Call => find(Kind::Call).or(find(Kind::Limp)).map(|i| (i, None)),
        ActionKind::Raise | ActionKind::Bet => {
            let to = a.to.unwrap_or(a.amount) as f64 / big;
            let shove = a.all_in || to >= 0.9 * stack;
            let (first, second) = if shove { (Kind::AllIn, Kind::Raise) } else { (Kind::Raise, Kind::AllIn) };
            let i = find(first).or(find(second))?;
            let m = actions[i];
            let note = match m.kind {
                Kind::Raise if shove => Some(format!("All-in als Raise auf {} gerechnet", bb(m.to_bb))),
                Kind::AllIn if !shove => Some(format!("Raise auf {} als All-in gerechnet", bb(to))),
                Kind::Raise if (to / m.to_bb).ln().abs() > 0.15 => {
                    Some(format!("Raise auf {} als Modellgröße {} gerechnet", bb(to), bb(m.to_bb)))
                }
                _ => None,
            };
            Some((i, note))
        }
        _ => None,
    }
}

fn spot_type(line: &[Step], played: Kind, hero_position: &str) -> SpotType {
    let raises: Vec<&Step> = line.iter().filter(|s| matches!(s.action.kind, Kind::Raise | Kind::AllIn)).collect();
    let last_all_in = raises.last().is_some_and(|s| s.action.kind == Kind::AllIn);
    match raises.len() {
        0 if line.iter().any(|s| s.action.kind == Kind::Limp) => SpotType::VsLimp,
        0 if played == Kind::AllIn => SpotType::Shove,
        0 if matches!(hero_position, "CO" | "BTN" | "SB") => SpotType::Steal,
        0 => SpotType::Open,
        _ if last_all_in => SpotType::CallShove,
        1 if matches!(played, Kind::Raise | Kind::AllIn) => SpotType::ThreeBet,
        1 => SpotType::VsOpen,
        _ => SpotType::VsThreeBet,
    }
}

fn walk(hand: &Hand, t: &Table, indices: &[usize], family: Family) -> Preflop {
    let n = t.players();
    let Some(&players) = LIMP_PLAYERS.iter().find(|&&p| p >= n) else {
        return unrated_all(hand, indices, "Für diese Tischgröße gibt es kein Preflop-Modell.");
    };
    // Equal stacks: the hero's stack against the deepest opponent dealt in.
    let deepest = (0..n).filter(|&i| i != t.hero).map(|i| t.stack_bb(i)).fold(0.0, f64::max);
    let effective = t.stack_bb(t.hero).min(deepest);
    // Far below the shallowest table the model's lines (sized raises, flops) mean nothing.
    if effective < STACKS[0] * 2.0 / 3.0 {
        return unrated_all(hand, indices, "Zu wenig Chips für das Preflop-Modell und kein reiner Push/Fold-Spot.");
    }
    let stack = nearest(&STACKS, effective);
    let Some(sol) = cached(family, t.ante, players, stack) else {
        return unrated_all(hand, indices, "Diese Modell-Konfiguration ist nicht vorgerechnet.");
    };

    let mut config = vec![format!("Preflop-Modell (kein Gleichgewicht des echten Spiels): {players} Spieler, alle mit {}, {}", bb(stack), ante_name(t.ante))];
    if n != players {
        config.push(format!("am Tisch {n} Spieler, die ersten {} Plätze des Modells gelten als gefoldet", players - n));
    }
    if (stack - effective).abs() >= 0.5 {
        config.push(format!("nächste vorgerechnete Tiefe: {} statt {}", bb(stack), bb(effective)));
    }
    let (table_ante, real_ante) = match t.ante {
        Ante::Classic => (CLASSIC_ANTE_BB, t.ante_bb),
        Ante::BigBlind => (BB_ANTE_BB, t.bb_ante_bb),
        Ante::None => (0.0, 0.0),
    };
    if (real_ante - table_ante).abs() > 0.1 * table_ante.max(0.01) {
        config.push(format!("Ante gerechnet mit {} statt {}", bb(table_ante), bb(real_ante)));
    }
    if (t.small_blind_bb - 0.5).abs() > 0.01 {
        config.push(format!("Small Blind gerechnet mit 0,5 BB statt {}", bb(t.small_blind_bb)));
    }
    let config = config.join("; ");

    let positions = position_labels(hand);
    let position = |name: &str| positions.iter().find(|(p, _)| p == name).map_or("?", |(_, l)| *l);
    let class = t.class().index();
    let mut line: Vec<Step> = Vec::new();
    let mut weights: Vec<Vec<f64>> = vec![vec![1.0; NUM_CLASSES]; players];
    let mut folded = vec![false; n];
    let mut broken: Option<String> = None;
    let mut decisions = Vec::new();
    let mut targets = Vec::new();
    let mut size_notes: Vec<String> = Vec::new();
    let mut raises = 0;

    for (index, a) in hand.actions.iter().enumerate().filter(|(_, a)| a.street == Street::Preflop && !a.kind.is_post()) {
        let is_hero = a.player == t.order[t.hero].name;
        if let Some(reason) = &broken {
            if is_hero {
                decisions.push(unrated(hand, index, SpotType::VsOpen, reason.clone()));
            }
            continue;
        }
        let Some(real) = t.seat_of(&a.player) else {
            if a.kind == ActionKind::Fold {
                continue;
            }
            broken = Some("Aktion eines Spielers ohne Platz am Tisch.".into());
            continue;
        };
        let seat = model_seat(t, players, real);
        let Some(v) = sol.node(seat, &line) else {
            // The model folds the seats still to act behind a 3-bet or an isolation raise
            // without asking them, so their real folds keep the line.
            if a.kind == ActionKind::Fold {
                folded[real] = true;
                if is_hero {
                    let reason = "Hier lässt das Preflop-Modell diesen Platz nur folden (z. B. hinter einer 3-Bet).";
                    decisions.push(unrated(hand, index, spot_type(&line, Kind::Fold, position(&a.player)), reason));
                }
                continue;
            }
            let reason = "Diese Linie kommt im Preflop-Modell nicht vor (z. B. ein dritter Limper, ein Call hinter einer Isolation oder ein zweiter Caller).";
            broken = Some(reason.into());
            if is_hero {
                decisions.push(unrated(hand, index, SpotType::VsOpen, reason));
            }
            continue;
        };
        let node = &sol.nodes[v];
        let Some((ai, size_note)) = map_action(a, &node.actions, t.big_blind, t.stack_bb(real)) else {
            let limp = a.kind == ActionKind::Call && !line.iter().any(|s| matches!(s.action.kind, Kind::Raise | Kind::AllIn));
            let reason = if limp {
                "Ein weiterer Limp: das Preflop-Modell rechnet mit höchstens zwei Limpern."
            } else {
                "Diese Aktion bietet das Preflop-Modell hier nicht an."
            };
            broken = Some(reason.into());
            if is_hero {
                decisions.push(unrated(hand, index, SpotType::VsOpen, reason));
            }
            continue;
        };
        let m = node.actions[ai];
        if is_hero {
            let spot = spot_type(&line, m.kind, position(&a.player));
            let own = own_moves(node, players);
            let reach: f64 = own.freq.iter().map(|f| f[class]).sum();
            if m.kind == Kind::Limp && !limps_by_choice(players, seat) {
                let reason = "Das Preflop-Modell limpt von hier nicht selbst: Seine Limps stehen für die anderen Spieler am Tisch.";
                decisions.push(unrated(hand, index, spot, reason));
            } else if weights[seat][class] <= 0.0 || reach <= 0.0 {
                let reason = format!(
                    "Das Modell kommt mit {} nicht an diese Stelle (eine frühere Aktion spielt es mit dieser Hand nie).",
                    t.class()
                );
                decisions.push(unrated(hand, index, spot, reason));
            } else {
                let strategy = choices(&own, class, Some(m));
                let mut note = config.clone();
                for s in size_notes.iter().chain(&size_note) {
                    note.push_str("; ");
                    note.push_str(s);
                }
                note.push('.');
                let pos = position(&a.player);
                // The charts show the plain tables: the same spot there, if the line has one.
                if let Some(node) = preflop_table::PLAYERS
                    .contains(&players)
                    .then(|| cached(Family::Plain, t.ante, players, stack))
                    .flatten()
                    .and_then(|plain| plain.node(seat, &line))
                {
                    targets.push(ChartTarget {
                        action_index: index,
                        kind: ChartKind::Preflop,
                        ante: t.ante,
                        players,
                        stack_bb: stack,
                        seat,
                        shover: None,
                        callers: Vec::new(),
                        node: Some(node),
                        hand: t.class().to_string(),
                    });
                }
                let pot = pot_before(hand, index);
                let facts = ranked(&own, class, &t.class().to_string(), format!("Als {pos} bei {} Tiefe", bb(stack)));
                decisions.push(score(base_at(hand, index, spot), Source::Model, strategy, pot, note, |d| {
                    why::ranked(&facts, &d.strategy, d.pot_bb.unwrap_or(1.0))
                }));
            }
        }
        if let Some(s) = size_note {
            size_notes.push(s);
        }
        for c in 0..NUM_CLASSES {
            let f = node.freq[ai][c];
            weights[seat][c] *= if taken(f) { f } else { 0.0 };
        }
        match m.kind {
            Kind::Fold => folded[real] = true,
            Kind::Raise | Kind::AllIn => raises += 1,
            _ => {}
        }
        if m.kind != Kind::Fold {
            line.push(Step { seat, action: m });
        }
    }

    let ranges: Option<FlopRanges> = broken.is_none().then(|| {
        (0..n).filter(|&i| !folded[i]).map(|i| (t.order[i].name.clone(), weights[model_seat(t, players, i)].clone())).collect()
    });
    let flop = match broken {
        Some(reason) => Err(format!("Preflop-Linie außerhalb des Modells, daher keine Ranges: {reason}")),
        None => {
            let left: Vec<usize> = (0..n).filter(|&i| !folded[i]).collect();
            if left.len() != 2 {
                Err(format!("Multiway-Pot ({} Spieler am Flop) wird nicht bewertet.", left.len()))
            } else if line.iter().any(|s| s.action.kind == Kind::AllIn) {
                Err("Schon vor dem Flop all-in.".into())
            } else {
                let mut note = config;
                for s in &size_notes {
                    note.push_str("; ");
                    note.push_str(s);
                }
                Ok(Flop {
                    players: left
                        .iter()
                        .map(|&i| {
                            let name = t.order[i].name.clone();
                            let pos = position(&name).to_string();
                            let how = range_note(&line, model_seat(t, players, i), &pos);
                            (name, pos, weights[model_seat(t, players, i)].clone(), how)
                        })
                        .collect(),
                    raises,
                    note,
                    model: ModelLine { family, ante: t.ante, players, stack_bb: stack, line: line.clone() },
                })
            }
        }
    };
    Preflop { decisions, flop, ranges, targets }
}

/// The moves of the model at `node`: without the other players' limps first in, whose EVs
/// are not those of a move the model makes.
pub(crate) fn own_moves(node: &Node, players: usize) -> Node {
    let keep: Vec<usize> =
        (0..node.actions.len()).filter(|&k| node.actions[k].kind != Kind::Limp || limps_by_choice(players, node.seat)).collect();
    Node {
        seat: node.seat,
        line: node.line.clone(),
        actions: keep.iter().map(|&k| node.actions[k]).collect(),
        reach: node.reach,
        freq: keep.iter().map(|&k| node.freq[k].clone()).collect(),
        ev: keep.iter().map(|&k| node.ev[k].clone()).collect(),
    }
}

/// The choices of `class` at `node` with the frequencies among its moves there; `played`
/// marks the hero's.
pub(crate) fn choices(node: &Node, class: usize, played: Option<ringside_engine::preflop::Action>) -> Vec<Choice> {
    let reach: f64 = node.freq.iter().map(|f| f[class]).sum();
    node.actions
        .iter()
        .enumerate()
        .map(|(k, x)| Choice {
            label: kind_name(x.kind, x.to_bb),
            freq: node.freq[k][class] / reach,
            ev_bb: Some(node.ev[k][class]),
            played: Some(*x) == played,
            kind: kind_key(x.kind).into(),
        })
        .collect()
}

/// Where class `class` stands among the hands that reach `node`: the moves other than the
/// passive one (fold, else check or call), how much of the range takes them, and the share
/// of the range for which they bring more over the passive move than for the hero's hand.
fn ranked(node: &Node, class: usize, name: &str, place: String) -> why::Ranked {
    let kinds: Vec<why::Move> = node.actions.iter().map(|a| why::Move::of(kind_key(a.kind))).collect();
    let base = kinds.iter().position(|&m| m == why::Move::Fold).or_else(|| kinds.iter().position(|m| !m.aggressive())).unwrap_or(0);
    let others: Vec<usize> = (0..kinds.len()).filter(|&k| k != base).collect();
    let mut moves: Vec<why::Move> = others.iter().map(|&k| kinds[k]).collect();
    moves.dedup();
    let act = match moves.as_slice() {
        [one] => Some(*one),
        _ if kinds[base] == why::Move::Fold => None,
        _ => Some(why::Move::Raise),
    };
    let combos = |c: usize| HandClass::from_index(c).combo_count() as f64;
    let reach = |c: usize| node.freq.iter().map(|f| f[c]).sum::<f64>();
    let margin = |c: usize| others.iter().map(|&k| node.ev[k][c]).fold(f64::NEG_INFINITY, f64::max) - node.ev[base][c];
    let total: f64 = (0..NUM_CLASSES).map(|c| combos(c) * reach(c)).sum();
    let share = (0..NUM_CLASSES).map(|c| combos(c) * others.iter().map(|&k| node.freq[k][c]).sum::<f64>()).sum::<f64>() / total.max(1e-9);
    let mine = margin(class);
    let above: f64 = (0..NUM_CLASSES)
        .filter(|&c| reach(c) > 0.0)
        .map(|c| combos(c) * reach(c) * if c == class { 0.5 } else if margin(c) > mine { 1.0 } else { 0.0 })
        .sum();
    why::Ranked { chart: why::Chart::Model, place, act, share, all_hands: false, class: name.to_string(), rank: above / total.max(1e-9) }
}

fn range_note(line: &[Step], seat: usize, position: &str) -> String {
    let own: Vec<String> = line.iter().filter(|s| s.seat == seat).map(|s| kind_name(s.action.kind, s.action.to_bb)).collect();
    if own.is_empty() {
        format!("{position}: Check im Big Blind")
    } else {
        format!("{position}: {}", own.join(", dann "))
    }
}

fn unrated_all(hand: &Hand, indices: &[usize], reason: &str) -> Preflop {
    Preflop {
        decisions: indices.iter().map(|&i| unrated(hand, i, SpotType::VsOpen, reason)).collect(),
        flop: Err(reason.to_string()),
        ranges: None,
        targets: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringside_engine::preflop::{Params, limp_shape};

    /// Chips in the pot after the non-fold actions `line`: antes, blinds and every raise.
    fn pot_at(p: &Params, line: &[Step]) -> f64 {
        let n = p.players;
        let mut contrib = vec![0.0; n];
        contrib[n - 2] = p.small_blind_bb;
        contrib[n - 1] = 1.0;
        for s in line {
            contrib[s.seat] = s.action.to_bb;
        }
        contrib.iter().sum::<f64>() + p.ante_bb * n as f64 + p.bb_ante_bb
    }

    /// The model's moves the analysis rates with, at every node of every table with limps
    /// and for every hand there: a limp is only offered where the model limps by choice
    /// (the small blind first in, limping along), never earns more than the best other
    /// move unless the model limps the hand above its minimum share, and no move wins
    /// more than the pot plus a stack or loses more than a stack. AA and KK raise for at
    /// least as much as they limp, except where the small blind limps them by choice: a
    /// trap against the big blind, mostly at 15 to 25 BB, worth up to a third of a big
    /// blind more than the raise.
    #[test]
    fn the_models_moves_keep_limps_below_raises() {
        // Two EV steps of the tables plus what the solve leaves over.
        const TOL: f64 = 0.03;
        let shape = limp_shape();
        let premium: Vec<usize> = ["AA", "KK"].iter().map(|h| h.parse::<HandClass>().unwrap().index()).collect();
        let mut checked = 0usize;
        for ante in Ante::ALL {
            for &n in &LIMP_PLAYERS {
                for (p, sol) in preflop_table::embedded_entries(Family::Limps, ante, n) {
                    let what = format!("{} {n} players {} BB", ante.key(), p.stack_bb);
                    let reach = sol.own_reach();
                    for (v, node) in sol.nodes.iter().enumerate() {
                        let own = own_moves(node, n);
                        let pot = pot_at(&p, &node.line);
                        for c in (0..NUM_CLASSES).filter(|&c| reach[v][c] > 0.0) {
                            let strategy = choices(&own, c, None);
                            for x in &strategy {
                                let ev = x.ev_bb.unwrap();
                                assert!(ev >= -p.stack_bb - 1e-9 && ev <= pot + p.stack_bb, "{what}, node {v}, class {c}: {} for {ev} BB", x.label);
                            }
                            let Some(limp) = strategy.iter().position(|x| x.kind == "limp") else { continue };
                            assert!(limps_by_choice(n, node.seat), "{what}, seat {}: offers the other players' limp", node.seat);
                            let ev = |k: usize| strategy[k].ev_bb.unwrap();
                            let others = || (0..strategy.len()).filter(|&k| k != limp);
                            let best = others().map(ev).fold(f64::MIN, f64::max);
                            let raise = others().filter(|&k| matches!(strategy[k].kind.as_str(), "raise" | "all_in")).map(ev).fold(f64::MIN, f64::max);
                            let by_choice = own.freq[limp][c] > p.open_limps * shape[c] + 0.01;
                            assert!(by_choice || ev(limp) <= best + TOL, "{what}, seat {}: class {c} limps for {:+.2} BB, best other move {best:+.2} BB", node.seat, ev(limp));
                            if premium.contains(&c) && !by_choice {
                                assert!(ev(limp) <= raise + TOL, "{what}, seat {}: class {c} limps for {:+.2} BB, raises for {raise:+.2} BB", node.seat, ev(limp));
                            }
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 10_000, "only {checked} limps checked");
    }
}
