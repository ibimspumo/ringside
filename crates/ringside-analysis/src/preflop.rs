//! Preflop decisions: push/fold spots against the equilibrium tables, everything else
//! against the preflop model at the nearest precomputed configuration. Walking the
//! hand's preflop line through the model also yields the ranges that reach the flop.

use std::sync::{Arc, Mutex, PoisonError};

use ringside_core::db::{Choice, Decision, Source, SpotType};
use ringside_core::model::{Action, ActionKind, Hand, Street};
use ringside_core::stats::position_labels;
use ringside_engine::preflop::{Kind, Solution, Step, taken};
use ringside_engine::preflop_table::{self, Ante, PLAYERS, STACKS};
use ringside_engine::pushfold_table::{BB_ANTE_BB, CLASSIC_ANTE_BB};
use ringside_engine::range::NUM_CLASSES;

use crate::fmt::bb;
use crate::seats::{Table, ante_name, nearest, table};
use crate::{ChartKind, ChartTarget, hero_actions, pushfold, rated, unrated};

type Key = (Ante, usize, u64);
const CACHE_LIMIT: usize = 24;
static CACHE: Mutex<Vec<(Key, Option<Arc<Solution>>)>> = Mutex::new(Vec::new());

pub(crate) fn model(ante: Ante, players: usize, stack: f64) -> Option<Arc<Solution>> {
    let key = (ante, players, stack.to_bits());
    if let Some((_, hit)) = CACHE.lock().unwrap_or_else(PoisonError::into_inner).iter().find(|(k, _)| *k == key) {
        return hit.clone();
    }
    let sol = preflop_table::chart(ante, players, stack).map(Arc::new);
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if !cache.iter().any(|(k, _)| *k == key) {
        cache.push((key, sol.clone()));
        if cache.len() > CACHE_LIMIT {
            cache.remove(0);
        }
    }
    sol
}

/// The model configuration and the line in model seats that the flop ranges follow from,
/// so a spot can be named without its ranges.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpotModel {
    pub ante: Ante,
    pub players: usize,
    pub stack_bb: f64,
    pub line: Vec<Step>,
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
    pub model: SpotModel,
}

pub struct Preflop {
    pub decisions: Vec<Decision>,
    /// Ranges for a heads-up flop, or why there are none.
    pub flop: Result<Flop, String>,
    /// Chart spots of the rated decisions.
    pub targets: Vec<ChartTarget>,
}

pub fn analyze(hand: &Hand, hero: &str) -> Preflop {
    let indices: Vec<usize> =
        hero_actions(hand, hero).filter(|(_, a)| a.street == Street::Preflop).map(|(i, _)| i).collect();
    let fail = |reason: &str| Preflop {
        decisions: indices.iter().map(|&i| unrated(hand, i, SpotType::VsOpen, reason)).collect(),
        flop: Err(reason.to_string()),
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
        return Preflop { decisions, flop: Err("Push/Fold-Hand".into()), targets: target.into_iter().collect() };
    }
    walk(hand, &t, &indices)
}

/// Seat of the model for real seat `i`: smaller tables take the model's last seats, so
/// the missing early positions count as folded.
fn model_seat(t: &Table, players: usize, i: usize) -> usize {
    players - t.players() + i
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
        0 if played == Kind::AllIn => SpotType::Shove,
        0 if line.iter().any(|s| s.action.kind == Kind::Limp) => SpotType::VsOpen,
        0 if matches!(hero_position, "CO" | "BTN" | "SB") => SpotType::Steal,
        0 => SpotType::Open,
        _ if last_all_in => SpotType::CallShove,
        1 if matches!(played, Kind::Raise | Kind::AllIn) => SpotType::ThreeBet,
        1 => SpotType::VsOpen,
        _ => SpotType::VsThreeBet,
    }
}

fn walk(hand: &Hand, t: &Table, indices: &[usize]) -> Preflop {
    let n = t.players();
    let Some(&players) = PLAYERS.iter().find(|&&p| p >= n) else {
        return unrated_all(hand, indices, "Für diese Tischgröße gibt es kein Preflop-Modell.");
    };
    // Equal stacks: the hero's stack against the deepest opponent dealt in.
    let deepest = (0..n).filter(|&i| i != t.hero).map(|i| t.stack_bb(i)).fold(0.0, f64::max);
    let effective = t.stack_bb(t.hero).min(deepest);
    let stack = nearest(&STACKS, effective);
    let Some(sol) = model(t.ante, players, stack) else {
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
            let reason = "Diese Linie kommt im Preflop-Modell nicht vor (z. B. Limp außerhalb des Small Blinds oder ein zweiter Caller).";
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
                "Limp: das Preflop-Modell kennt Limps nur aus dem Small Blind."
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
            let reach: f64 = node.freq.iter().map(|f| f[class]).sum();
            if weights[seat][class] <= 0.0 || reach <= 0.0 {
                let reason = format!(
                    "Das Modell kommt mit {} nicht an diese Stelle (eine frühere Aktion spielt es mit dieser Hand nie).",
                    t.class()
                );
                decisions.push(unrated(hand, index, spot, reason));
            } else {
                let strategy = node
                    .actions
                    .iter()
                    .enumerate()
                    .map(|(k, x)| Choice {
                        label: kind_name(x.kind, x.to_bb),
                        freq: node.freq[k][class] / reach,
                        ev_bb: Some(node.ev[k][class]),
                        played: k == ai,
                    })
                    .collect();
                let mut note = config.clone();
                for s in size_notes.iter().chain(&size_note) {
                    note.push_str("; ");
                    note.push_str(s);
                }
                note.push('.');
                let pos = position(&a.player);
                let hand_class = t.class();
                targets.push(ChartTarget {
                    action_index: index,
                    kind: ChartKind::Preflop,
                    ante: t.ante,
                    players,
                    stack_bb: stack,
                    seat,
                    shover: None,
                    callers: Vec::new(),
                    node: Some(v),
                    hand: hand_class.to_string(),
                });
                decisions.push(rated(hand, index, spot, Source::Model, strategy, note, |d| {
                    let best = d.strategy.iter().max_by(|x, y| x.freq.total_cmp(&y.freq));
                    match best {
                        Some(b) => format!(
                            "Mit {hand_class} auf {pos} spielt das Modell bei {} Tiefe hier zu {} % {}.",
                            bb(stack),
                            (b.freq * 100.0).round(),
                            b.label
                        ),
                        None => String::new(),
                    }
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
                    model: SpotModel { ante: t.ante, players, stack_bb: stack, line: line.clone() },
                })
            }
        }
    };
    Preflop { decisions, flop, targets }
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
        targets: Vec::new(),
    }
}
