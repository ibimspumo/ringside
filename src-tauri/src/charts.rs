//! Push/fold and preflop charts: the precomputed solutions shipped with the engine,
//! and a live solve for everything off their grid where that stays quick. Locked
//! while the PokerStars client runs, like the learning area.

use std::sync::{Arc, Mutex};

use ringside_engine::multiway::{MAX_PLAYERS, Solution, seat_name, solve};
use ringside_engine::preflop::{self, Kind, Realization, Sizes};
use ringside_engine::preflop_table;
use ringside_engine::pushfold_table::{Ante, MIN_PLAYERS, lookup, params, stacks};
use serde::Serialize;
use tauri::AppHandle;

use crate::{CmdResult, blocking};

/// Deeper stacks barely shove anymore, and solves get slow.
const MAX_LIVE_STACK_BB: f64 = 100.0;

#[derive(Serialize)]
pub struct PushFoldGrid {
    min_players: usize,
    max_players: usize,
    antes: Vec<GridAnte>,
}

#[derive(Serialize)]
struct GridAnte {
    ante: Ante,
    /// Ante per player and of the big blind alone, in BB.
    ante_bb: f64,
    bb_ante_bb: f64,
    /// Stacks with a precomputed chart.
    stacks: Vec<f64>,
}

#[derive(Serialize)]
pub struct PushFoldChart {
    players: usize,
    stack_bb: f64,
    ante: Ante,
    /// `false`: solved live because the grid has no such chart.
    precomputed: bool,
    converged: bool,
    exploitability_bb: f64,
    /// Probability that a hand ends in an all-in of three or more players.
    multiway_probability: f64,
    nodes: Vec<ChartNode>,
}

#[derive(Serialize)]
struct ChartNode {
    seat: usize,
    position: &'static str,
    /// `None`: everyone before folded and the seat may shove.
    shover: Option<usize>,
    /// Seats that called the shove before this one.
    callers: Vec<usize>,
    /// Probability that a hand reaches this node.
    reach: f64,
    /// Share of all combos played here, in percent.
    act_pct: f64,
    /// Shove/call frequency per hand class (`HandClass::index`), 0–255.
    freq: Vec<u8>,
    /// Shove/call EV per hand class and fold EV, in hundredths of a BB.
    ev_act: Vec<i32>,
    ev_fold: i32,
}

fn centi(bb: f64) -> i32 {
    (bb * 100.0).round() as i32
}

fn chart(ante: Ante, players: usize, stack_bb: f64, precomputed: bool, sol: &Solution) -> PushFoldChart {
    let nodes = sol
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| ChartNode {
            seat: n.seat,
            position: seat_name(players, n.seat),
            shover: n.shover,
            callers: n.callers.clone(),
            reach: n.reach,
            act_pct: sol.act_pct(i),
            freq: n.act.iter().map(|&f| (f.clamp(0.0, 1.0) * 255.0).round() as u8).collect(),
            ev_act: n.ev_act.iter().map(|&e| centi(e)).collect(),
            ev_fold: centi(n.ev_fold.first().copied().unwrap_or(0.0)),
        })
        .collect();
    PushFoldChart {
        players,
        stack_bb,
        ante,
        precomputed,
        converged: sol.converged,
        exploitability_bb: sol.exploitability,
        multiway_probability: sol.approximation.multiway_probability,
        nodes,
    }
}

#[tauri::command]
pub async fn pushfold_grid(app: AppHandle) -> CmdResult<PushFoldGrid> {
    blocking(app, |_| {
        let antes = Ante::ALL
            .into_iter()
            .map(|ante| {
                let p = params(ante, MIN_PLAYERS, 10.0);
                GridAnte { ante, ante_bb: p.ante_bb, bb_ante_bb: p.bb_ante_bb, stacks: stacks(ante).to_vec() }
            })
            .collect();
        Ok(PushFoldGrid { min_players: MIN_PLAYERS, max_players: MAX_PLAYERS, antes })
    })
    .await
}

/// The chart for equal stacks of `stack_bb`; precomputed when on the grid, else solved.
#[tauri::command]
pub async fn pushfold_chart(ante: Ante, players: usize, stack_bb: f64, app: AppHandle) -> CmdResult<PushFoldChart> {
    blocking(app, move |_| {
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&players) {
            return Err(format!("Push/Fold gibt es für {MIN_PLAYERS} bis {MAX_PLAYERS} Spieler."));
        }
        let p = params(ante, players, stack_bb);
        // The big blind has to cover its blind and antes.
        let min = 1.0 + p.ante_bb + p.bb_ante_bb;
        if !(stack_bb.is_finite() && (min..=MAX_LIVE_STACK_BB).contains(&stack_bb)) {
            return Err(format!("Der Stack muss zwischen {min} und {MAX_LIVE_STACK_BB} BB liegen."));
        }
        match lookup(&p) {
            Some(sol) => Ok(chart(ante, players, stack_bb, true, &sol)),
            None => {
                let sol = solve(&p).map_err(|e| e.to_string())?;
                Ok(chart(ante, players, stack_bb, false, &sol))
            }
        }
    })
    .await
}

// ---------------------------------------------------------------------------------
// Preflop model (deeper stacks).

/// Live solves stay within a minute or two on a normal PC only at six-handed tables.
const PREFLOP_LIVE_MAX_PLAYERS: usize = 6;
const PREFLOP_LIVE_STACKS: (f64, f64) = (10.0, 100.0);

#[derive(Serialize)]
pub struct PreflopGrid {
    players: Vec<usize>,
    stacks: Vec<f64>,
    bb_ante_bb: f64,
    live_max_players: usize,
    live_min_stack_bb: f64,
    live_max_stack_bb: f64,
    target_exploitability_bb: f64,
}

#[derive(Serialize)]
pub struct PreflopSpots {
    players: usize,
    /// Position names by seat.
    positions: Vec<&'static str>,
    stack_bb: f64,
    bb_ante_bb: f64,
    precomputed: bool,
    converged: bool,
    exploitability_bb: f64,
    iterations: u32,
    ev_sum_error_bb: f64,
    sizes: Sizes,
    realization: Realization,
    realized: Vec<PreflopRealized>,
    nodes: Vec<PreflopSpot>,
}

#[derive(Serialize)]
struct PreflopStep {
    seat: usize,
    position: &'static str,
    kind: Kind,
    to_bb: f64,
}

#[derive(Serialize)]
struct PreflopAction {
    kind: Kind,
    to_bb: f64,
    /// Share of all combos that take it, in percent.
    pct: f64,
}

#[derive(Serialize)]
struct PreflopSpot {
    id: usize,
    seat: usize,
    position: &'static str,
    line: Vec<PreflopStep>,
    actions: Vec<PreflopAction>,
    /// Probability that a hand reaches this spot.
    reach: f64,
}

#[derive(Serialize)]
struct PreflopRealized {
    line: Vec<PreflopStep>,
    positions: Vec<&'static str>,
    reach: f64,
    pot_bb: f64,
    spr: f64,
    equity: Vec<f64>,
    realization: Vec<f64>,
}

#[derive(Serialize)]
pub struct PreflopNode {
    id: usize,
    /// Frequency per action and hand class (`HandClass::index`), 0–255.
    freq: Vec<Vec<u8>>,
    /// EV per action and hand class, in hundredths of a BB against the starting stack.
    ev: Vec<Vec<i32>>,
}

/// Players, stack bits, precomputed, solution.
type CachedPreflop = (usize, u64, bool, Arc<preflop::Solution>);

/// The last solutions asked for, so that browsing nodes doesn't decode or solve again.
static PREFLOP_CACHE: Mutex<Vec<CachedPreflop>> = Mutex::new(Vec::new());

fn preflop_solution(players: usize, stack_bb: f64) -> Result<(bool, Arc<preflop::Solution>), String> {
    let key = (players, stack_bb.to_bits());
    if let Some(hit) = PREFLOP_CACHE.lock().unwrap().iter().find(|e| (e.0, e.1) == key) {
        return Ok((hit.2, hit.3.clone()));
    }
    if !preflop_table::PLAYERS.contains(&players) {
        return Err("Das Preflop-Modell gibt es für 6, 8 und 9 Spieler.".into());
    }
    let p = preflop_table::params(players, stack_bb);
    let (precomputed, sol) = match preflop_table::lookup(&p) {
        Some(sol) => (true, sol),
        None => {
            let (lo, hi) = PREFLOP_LIVE_STACKS;
            if players > PREFLOP_LIVE_MAX_PLAYERS {
                return Err(format!(
                    "Für {players} Spieler gibt es nur die vorgerechneten Tiefen. Live würde diese Tiefe auf einem normalen PC zu lange dauern."
                ));
            }
            if !(stack_bb.is_finite() && (lo..=hi).contains(&stack_bb)) {
                return Err(format!("Der Stack muss zwischen {lo} und {hi} BB liegen."));
            }
            (false, preflop::solve(&p).map_err(|e| e.to_string())?)
        }
    };
    let sol = Arc::new(sol);
    let mut cache = PREFLOP_CACHE.lock().unwrap();
    cache.retain(|e| (e.0, e.1) != key);
    cache.insert(0, (players, stack_bb.to_bits(), precomputed, sol.clone()));
    cache.truncate(4);
    Ok((precomputed, sol))
}

fn steps(players: usize, line: &[preflop::Step]) -> Vec<PreflopStep> {
    line.iter()
        .map(|s| PreflopStep { seat: s.seat, position: seat_name(players, s.seat), kind: s.action.kind, to_bb: s.action.to_bb })
        .collect()
}

#[tauri::command]
pub async fn preflop_grid(app: AppHandle) -> CmdResult<PreflopGrid> {
    blocking(app, |_| {
        Ok(PreflopGrid {
            players: preflop_table::PLAYERS.to_vec(),
            stacks: preflop_table::STACKS.to_vec(),
            bb_ante_bb: preflop_table::BB_ANTE_BB,
            live_max_players: PREFLOP_LIVE_MAX_PLAYERS,
            live_min_stack_bb: PREFLOP_LIVE_STACKS.0,
            live_max_stack_bb: PREFLOP_LIVE_STACKS.1,
            target_exploitability_bb: preflop_table::TARGET_EXPLOITABILITY,
        })
    })
    .await
}

/// Every decision of the model at equal stacks of `stack_bb`, without the per-hand data.
#[tauri::command]
pub async fn preflop_spots(players: usize, stack_bb: f64, app: AppHandle) -> CmdResult<PreflopSpots> {
    blocking(app, move |_| {
        let (precomputed, sol) = preflop_solution(players, stack_bb)?;
        let p = preflop_table::params(players, stack_bb);
        let nodes = sol
            .nodes
            .iter()
            .enumerate()
            .map(|(id, n)| PreflopSpot {
                id,
                seat: n.seat,
                position: seat_name(players, n.seat),
                line: steps(players, &n.line),
                actions: n
                    .actions
                    .iter()
                    .enumerate()
                    .map(|(a, x)| PreflopAction { kind: x.kind, to_bb: x.to_bb, pct: sol.action_pct(id, a) })
                    .collect(),
                reach: n.reach,
            })
            .collect();
        let realized = sol
            .realized
            .iter()
            .map(|r| PreflopRealized {
                line: steps(players, &r.line),
                positions: r.seats.iter().map(|&s| seat_name(players, s)).collect(),
                reach: r.reach,
                pot_bb: r.pot_bb,
                spr: r.spr,
                equity: r.equity.clone(),
                realization: r.realization.clone(),
            })
            .collect();
        Ok(PreflopSpots {
            players,
            positions: ringside_engine::multiway::position_names(players),
            stack_bb,
            bb_ante_bb: p.bb_ante_bb,
            precomputed,
            converged: sol.converged,
            exploitability_bb: sol.exploitability,
            iterations: sol.iterations,
            ev_sum_error_bb: sol.ev_sum_error,
            sizes: p.sizes,
            realization: p.realization,
            realized,
            nodes,
        })
    })
    .await
}

/// Frequencies and EVs of one decision.
#[tauri::command]
pub async fn preflop_node(players: usize, stack_bb: f64, node: usize, app: AppHandle) -> CmdResult<PreflopNode> {
    blocking(app, move |_| {
        let (_, sol) = preflop_solution(players, stack_bb)?;
        let n = sol.nodes.get(node).ok_or("Diesen Spot gibt es nicht.")?;
        Ok(PreflopNode {
            id: node,
            freq: n.freq.iter().map(|f| f.iter().map(|&x| (x.clamp(0.0, 1.0) * 255.0).round() as u8).collect()).collect(),
            ev: n.ev.iter().map(|e| e.iter().map(|&x| centi(x)).collect()).collect(),
        })
    })
    .await
}
