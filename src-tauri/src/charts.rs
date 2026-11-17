//! Push/fold charts: the precomputed equilibria shipped with the engine, and a live
//! solve for everything off their grid. Locked while the PokerStars client runs, like
//! the learning area.

use ringside_engine::multiway::{MAX_PLAYERS, Solution, seat_name, solve};
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
