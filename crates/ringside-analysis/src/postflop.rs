//! Heads-up postflop decisions. Analysing a hand only builds the job: the solver spot
//! from the flop (ranges from the preflop model along the hand's line, board, pot and
//! stacks from the hand, standard bet sizes plus the sizes actually used, snapped to a
//! grid) and the hand's postflop actions. [`solve_job`] solves it later in the queue and
//! rates every hero decision at the node the real line leads to.

use ringside_core::db::{Choice, Decision, SpotType, Status};
use ringside_core::model::{ActionKind, Hand, Street};
use ringside_engine::{Card, Combo, HandClass, Range};
use ringside_solver::{
    ActionKind as TreeAction, Error as SolveError, NodeKind, Player, Progress, Settings, Spot, Step, StreetSizes, estimate,
    solve,
};
use serde::{Deserialize, Serialize};

use crate::fmt::{bb, num};
use crate::preflop::Flop;
use crate::seats::hero_combo;
use crate::{Source, base, hero_actions, is_raise, score};

/// Bet sizes a real bet is snapped to, in percent of the pot.
const GRID: [f64; 11] = [10.0, 25.0, 33.0, 50.0, 66.0, 75.0, 100.0, 125.0, 150.0, 200.0, 300.0];
/// Range weights are rounded to this step, as in the spot library.
const WEIGHT_STEP: f64 = 0.05;
/// Solves stop here: precise enough to rate decisions in BB, quick enough for a queue.
pub const TARGET_PCT: f64 = 1.0;
pub const MAX_ITERATIONS: u32 = 300;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobAction {
    pub index: usize,
    pub street: Street,
    pub oop: bool,
    pub kind: ActionKind,
    /// Bet or raise to this amount on the street, BB.
    pub to_bb: f64,
    pub all_in: bool,
    /// For the hero's actions: spot type and what was played.
    pub hero: Option<(SpotType, String)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PostflopJob {
    pub hand_id: u64,
    pub spot: Spot,
    pub hero_oop: bool,
    pub hero_combo: String,
    pub board: Vec<String>,
    pub actions: Vec<JobAction>,
    pub positions: [String; 2],
    pub range_notes: [String; 2],
    pub players: usize,
    pub note: String,
    /// Final pot in BB: big pots are solved first.
    pub pot_bb: f64,
}

/// Spot type of a hero action after the flop.
fn spot_type(hand: &Hand, index: usize, hero: &str) -> SpotType {
    let a = &hand.actions[index];
    match a.street {
        Street::Turn => return SpotType::Turn,
        Street::River => return SpotType::River,
        _ => {}
    }
    let aggressor = hand.actions.iter().rfind(|x| x.street == Street::Preflop && x.kind == ActionKind::Raise).map(|x| x.player.as_str());
    let before: Vec<_> = hand.actions[..index].iter().filter(|x| x.street == Street::Flop).collect();
    let first_bet = before.iter().find(|x| is_raise(x.kind));
    match (aggressor, first_bet) {
        (Some(p), None) if p == hero && !before.iter().any(|x| x.player == hero) => SpotType::CBet,
        (Some(p), Some(b)) if b.player == p && p != hero && before.iter().filter(|x| is_raise(x.kind)).count() == 1 => {
            SpotType::VsCBet
        }
        _ => SpotType::Flop,
    }
}

fn pending(hand: &Hand, index: usize, spot: SpotType) -> Decision {
    let a = &hand.actions[index];
    let mut d = base(hand.id, index, a.street, spot, crate::fmt::real_action(hand, a));
    d.status = Status::Pending;
    d.source = Some(Source::Solver);
    d.reason = Some("Wartet auf den Postflop-Solver.".into());
    d
}

fn snap(pct: f64) -> f64 {
    crate::seats::nearest(&GRID, pct)
}

fn range_text(weights: &[f64], add: Option<Combo>) -> (String, bool) {
    let mut r = Range::empty();
    for c in HandClass::all() {
        let w = (weights[c.index()] / WEIGHT_STEP).round() * WEIGHT_STEP;
        if w > 0.0 {
            r.set_class(c, w.min(1.0) as f32);
        }
    }
    let mut added = false;
    if let Some(combo) = add
        && r.weight(combo) <= 0.0
    {
        r.set_combo(combo, WEIGHT_STEP as f32);
        added = true;
    }
    (r.to_string(), added)
}

/// Pending decisions for the hero's postflop actions and the job that rates them, or
/// unrated decisions with the reason.
pub(crate) fn prepare(hand: &Hand, hero: &str, flop: Result<&Flop, &str>) -> (Vec<Decision>, Option<PostflopJob>) {
    let indices: Vec<usize> = hero_actions(hand, hero).filter(|(_, a)| a.street != Street::Preflop).map(|(i, _)| i).collect();
    if indices.is_empty() {
        return (Vec::new(), None);
    }
    let fail = |reason: &str| -> (Vec<Decision>, Option<PostflopJob>) {
        let list = indices.iter().map(|&i| crate::unrated(hand, i, spot_type(hand, i, hero), reason)).collect();
        (list, None)
    };
    let flop = match flop {
        Ok(f) => f,
        Err(reason) => return fail(reason),
    };
    if hand.board.len() < 3 {
        return fail("Board unvollständig.");
    }
    let Some(combo) = hero_combo(hand) else { return fail("Deine Karten fehlen.") };
    let big = hand.big_blind as f64;
    let Some(first) = hand.actions.iter().find(|a| a.street == Street::Flop) else { return fail("Keine Aktion am Flop.") };
    let names = [first.player.clone(), flop.players.iter().map(|p| &p.0).find(|n| **n != first.player).cloned().unwrap_or_default()];
    let side = |name: &str| names.iter().position(|n| n == name);
    let info = |name: &str| flop.players.iter().find(|p| p.0 == name);
    let (Some(oop), Some(ip)) = (info(&names[0]), info(&names[1])) else {
        return fail("Am Flop handeln andere Spieler als erwartet.");
    };
    let hero_oop = names[0] == hero;
    for (name, _, weights, _) in [oop, ip] {
        if name != hero && weights.iter().all(|&w| w * 20.0 < 0.5) {
            return fail("Der Gegner hat vor dem Flop etwas gespielt, das das Modell mit keiner Hand spielt; es gibt keine Range.");
        }
    }

    let pre: f64 = hand.actions.iter().filter(|a| a.street == Street::Preflop).map(|a| a.amount as f64).sum::<f64>() / big;
    let behind = |name: &str| {
        let seat = hand.seats.iter().find(|s| s.name == name).map_or(0, |s| s.stack) as f64;
        let put: f64 = hand.actions.iter().filter(|a| a.street == Street::Preflop && a.player == name).map(|a| a.amount as f64).sum();
        (seat - put) / big
    };
    let stack = behind(&names[0]).min(behind(&names[1]));
    if stack <= 0.0 {
        return fail("Kein Stack mehr dahinter.");
    }

    // Standard sizes of the spot library, plus every size actually used.
    let three_bet = flop.raises >= 2;
    let base_sizes = |bet: &[f64], raise: &[f64]| StreetSizes { bet: bet.to_vec(), raise: raise.to_vec() };
    let mut sizes = if three_bet {
        [base_sizes(&[33.0], &[]), base_sizes(&[66.0], &[]), base_sizes(&[75.0], &[])]
    } else {
        [base_sizes(&[33.0], &[60.0]), base_sizes(&[75.0], &[]), base_sizes(&[75.0], &[])]
    };
    let mut allin_threshold: f64 = 150.0;
    let mut notes: Vec<String> = Vec::new();
    let mut actions = Vec::new();
    let mut pot = pre;
    for (street_no, street) in [Street::Flop, Street::Turn, Street::River].into_iter().enumerate() {
        let mut bets = [0.0f64; 2];
        for (index, a) in hand.actions.iter().enumerate().filter(|(_, a)| a.street == street) {
            let Some(s) = side(&a.player) else { return fail("Am Flop handeln andere Spieler als erwartet.") };
            let mut to = 0.0;
            match a.kind {
                ActionKind::Bet | ActionKind::Raise => {
                    to = a.to.unwrap_or(a.amount) as f64 / big;
                    let facing = bets[1 - s];
                    let (pct, list, what) = if a.kind == ActionKind::Bet || facing == 0.0 {
                        (100.0 * to / pot, &mut sizes[street_no].bet, "Bet")
                    } else {
                        let pot_after_call = pot + bets[0] + bets[1] + (facing - bets[s]);
                        (100.0 * (to - facing) / pot_after_call, &mut sizes[street_no].raise, "Raise")
                    };
                    if a.all_in {
                        allin_threshold = allin_threshold.max(pct.ceil() + 1.0);
                    } else {
                        let snapped = snap(pct);
                        if !list.contains(&snapped) && list.len() < ringside_solver::MAX_SIZES {
                            list.push(snapped);
                        }
                        if (snapped / pct).ln().abs() > 0.1 {
                            notes.push(format!("{what} von {} % Pot als {} % gerechnet", pct.round(), num(snapped)));
                        }
                    }
                    bets[s] = to;
                }
                ActionKind::Call => bets[s] = bets[1 - s],
                _ => {}
            }
            let hero_info = (a.player == hero).then(|| (spot_type(hand, index, hero), crate::fmt::real_action(hand, a)));
            actions.push(JobAction { index, street, oop: s == 0, kind: a.kind, to_bb: to, all_in: a.all_in, hero: hero_info });
        }
        pot += bets[0] + bets[1];
    }
    for s in &mut sizes {
        s.bet.sort_by(f64::total_cmp);
        s.raise.sort_by(f64::total_cmp);
    }

    let (oop_range, oop_added) = range_text(&oop.2, hero_oop.then_some(combo));
    let (ip_range, ip_added) = range_text(&ip.2, (!hero_oop).then_some(combo));
    if oop_added || ip_added {
        notes.push(format!("{} liegt nicht in deiner Modell-Range und wurde mit kleinem Gewicht ergänzt", combo.class()));
    }
    let size_text = |s: &StreetSizes| {
        let mut t = s.bet.iter().map(|x| format!("{} %", num(*x))).collect::<Vec<_>>().join("/");
        if !s.raise.is_empty() {
            t.push_str(&format!(", Raise {} %", s.raise.iter().map(|x| num(*x)).collect::<Vec<_>>().join("/")));
        }
        t
    };
    let mut note = format!(
        "Postflop-Solver heads-up. Ranges aus dem {}. Einsatzgrößen Flop {}, Turn {}, River {}, All-in ab {} % Pot",
        flop.note,
        size_text(&sizes[0]),
        size_text(&sizes[1]),
        size_text(&sizes[2]),
        num(allin_threshold)
    );
    for n in &notes {
        note.push_str("; ");
        note.push_str(n);
    }
    note.push('.');

    let job = PostflopJob {
        hand_id: hand.id,
        spot: Spot {
            oop_range,
            ip_range,
            board: hand.board[..3].join(""),
            pot: (pre * 100.0).round() / 100.0,
            stack: (stack * 100.0).round() / 100.0,
            sizes,
            allin_threshold,
        },
        hero_oop,
        hero_combo: combo.to_string(),
        board: hand.board.clone(),
        actions,
        positions: [oop.1.clone(), ip.1.clone()],
        range_notes: [oop.3.clone(), ip.3.clone()],
        players: hand.active_players().count(),
        note,
        pot_bb: hand.total_pot as f64 / big,
    };
    let decisions = indices.iter().map(|&i| pending(hand, i, spot_type(hand, i, hero))).collect();
    (decisions, Some(job))
}

pub enum JobError {
    /// Stopped from outside (the client started); the job stays queued.
    Cancelled,
    /// The job cannot be solved; its decisions are stored as not rated with this reason.
    Failed(String),
}

fn combo_of(text: &str) -> Option<Combo> {
    let t: String = text.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if t.len() != 4 {
        return None;
    }
    let a: Card = t[..2].parse().ok()?;
    let b: Card = t[2..].parse().ok()?;
    Some(Combo::new(a, b))
}

fn label(kind: TreeAction, amount: f64) -> String {
    match kind {
        TreeAction::Fold => "Fold".into(),
        TreeAction::Check => "Check".into(),
        TreeAction::Call => "Call".into(),
        TreeAction::Bet => format!("Bet {}", bb(amount)),
        TreeAction::Raise => format!("Raise auf {}", bb(amount)),
        TreeAction::AllIn => "All-in".into(),
    }
}

/// The tree action closest to a real one, with a note when the size differs.
fn map_action(a: &JobAction, actions: &[ringside_solver::ActionInfo]) -> Option<(usize, Option<String>)> {
    let find = |k: TreeAction| actions.iter().position(|x| x.kind == k);
    match a.kind {
        ActionKind::Fold => find(TreeAction::Fold).map(|i| (i, None)),
        ActionKind::Check => find(TreeAction::Check).map(|i| (i, None)),
        ActionKind::Call => find(TreeAction::Call).map(|i| (i, None)),
        ActionKind::Bet | ActionKind::Raise => {
            if a.all_in
                && let Some(i) = find(TreeAction::AllIn)
            {
                return Some((i, None));
            }
            let i = actions
                .iter()
                .enumerate()
                .filter(|(_, x)| matches!(x.kind, TreeAction::Bet | TreeAction::Raise | TreeAction::AllIn))
                .min_by(|(_, x), (_, y)| (x.amount / a.to_bb).ln().abs().total_cmp(&(y.amount / a.to_bb).ln().abs()))?
                .0;
            let m = &actions[i];
            let note = ((m.amount / a.to_bb).ln().abs() > 0.1)
                .then(|| format!("Einsatz von {} gerechnet als {}", bb(a.to_bb), label(m.kind, m.amount)));
            Some((i, note))
        }
        _ => None,
    }
}

/// What to open in the solver: the flop spot as the solver page takes it, plus the path
/// to the decision.
fn solver_link(job: &PostflopJob, path: &[Step]) -> String {
    let side = |i: usize, range: &str| serde_json::json!({ "position": job.positions[i], "range": range, "note": job.range_notes[i] });
    let cards: Vec<String> = combo_of(&job.hero_combo).map(|c| {
        let (a, b) = c.cards();
        vec![a.to_string(), b.to_string()]
    }).unwrap_or_default();
    serde_json::json!({
        "board": job.board[..3],
        "pot": job.spot.pot,
        "stack": job.spot.stack,
        "oop": side(0, &job.spot.oop_range),
        "ip": side(1, &job.spot.ip_range),
        "hero": { "player": if job.hero_oop { "oop" } else { "ip" }, "cards": cards },
        "players": job.players,
        "source": format!("Hand #{}, Analyse", job.hand_id),
        "sizes": job.spot.sizes,
        "allin_threshold": job.spot.allin_threshold,
        "path": path,
    })
    .to_string()
}

/// Solves a job and rates every hero decision on the real line.
pub fn solve_job(
    job: &PostflopJob,
    threads: usize,
    memory_limit: u64,
    on_progress: impl FnMut(&Progress),
    stop: impl Fn() -> bool,
) -> Result<Vec<Decision>, JobError> {
    let failed = |e: SolveError| match e {
        SolveError::Cancelled => JobError::Cancelled,
        e => JobError::Failed(e.to_string()),
    };
    let est = estimate(&job.spot, memory_limit, threads).map_err(failed)?;
    if !est.fits {
        return Err(failed(SolveError::TooLarge { needed: est.memory_bytes, limit: memory_limit }));
    }
    let settings = Settings { target_pct: TARGET_PCT, max_iterations: MAX_ITERATIONS, threads };
    let mut sol = solve(&job.spot, &settings, memory_limit, on_progress, stop).map_err(failed)?;
    let hero = combo_of(&job.hero_combo).ok_or_else(|| JobError::Failed("Ungültige Karten.".into()))?;
    let accuracy = format!(
        " Gelöst bis {} % des Pots Ausbeutbarkeit ({} Iterationen).",
        num(sol.summary.exploitability_pct),
        sol.summary.iterations
    );

    let mut out = Vec::new();
    let mut path: Vec<Step> = Vec::new();
    let mut street = Street::Flop;
    let mut broken: Option<String> = None;
    let mut size_notes: Vec<String> = Vec::new();
    for a in &job.actions {
        let make = |spot: SpotType, played: &str| base(job.hand_id, a.index, a.street, spot, played.to_string());
        if broken.is_none() && a.street != street {
            street = a.street;
            let card = match street {
                Street::Turn => job.board.get(3),
                _ => job.board.get(4),
            };
            match card {
                Some(c) => path.push(Step::Card(c.clone())),
                None => broken = Some("Board unvollständig.".into()),
            }
        }
        if broken.is_none() {
            match sol.node(&path) {
                Ok(node) if node.kind == NodeKind::Action && node.player == Some(if a.oop { Player::Oop } else { Player::Ip }) => {
                    match map_action(a, &node.actions) {
                        Some((i, size_note)) => {
                            if let Some((spot, played)) = &a.hero {
                                let d = make(*spot, played);
                                match node.hands.iter().find(|h| combo_of(&h.combo) == Some(hero) && h.weight > 0.0) {
                                    Some(h) => {
                                        let strategy = node
                                            .actions
                                            .iter()
                                            .enumerate()
                                            .map(|(k, x)| Choice {
                                                label: label(x.kind, x.amount),
                                                freq: h.strategy[k],
                                                ev_bb: h.evs.get(k).copied(),
                                                played: k == i,
                                            })
                                            .collect();
                                        let mut note = job.note.clone();
                                        for n in size_notes.iter().chain(&size_note) {
                                            note.push_str(&format!(" {n}."));
                                        }
                                        note.push_str(&accuracy);
                                        let equity = h.equity;
                                        let mut d = score(d, Source::Solver, strategy, note, |d| {
                                            let best = d.strategy.iter().max_by(|x, y| x.freq.total_cmp(&y.freq));
                                            format!(
                                                "Deine Hand hat hier gegen die Range des Gegners etwa {} % Equity; der Solver spielt sie zu {} % {}.",
                                                (equity * 100.0).round(),
                                                best.map_or(0.0, |b| (b.freq * 100.0).round()),
                                                best.map_or("?", |b| b.label.as_str())
                                            )
                                        });
                                        d.solver = Some(solver_link(job, &path));
                                        out.push(d);
                                    }
                                    None => {
                                        let mut d = d;
                                        d.reason = Some("Deine Hand erreicht diesen Knoten im Solver nicht (frühere Aktion nie gespielt).".into());
                                        out.push(d);
                                    }
                                }
                            }
                            if let Some(n) = size_note {
                                size_notes.push(n);
                            }
                            path.push(Step::Action(i));
                            continue;
                        }
                        None => broken = Some("Diese Aktion gibt es im Spielbaum nicht.".into()),
                    }
                }
                Ok(_) => broken = Some("Die Linie der Hand weicht vom Spielbaum ab (z. B. nach einem All-in).".into()),
                Err(e) => broken = Some(e.to_string()),
            }
        }
        if let Some((spot, played)) = &a.hero {
            let mut d = make(*spot, played);
            d.reason = broken.clone();
            out.push(d);
        }
    }
    Ok(out)
}
