//! Solving a catalog spot for the library, and checking a stored one.
//!
//! # Check
//! `postflop-solver` measures the exploitability of the strategy profile a game holds and
//! can lock a node's strategy, but its best response keeps to the locks of its own
//! player too, and CFR never trains the parts of the tree a locked strategy avoids. So a
//! stored flop strategy is measured the way it is meant: for each player, a game in which
//! only that player's flop decisions are locked to the stored bytes is solved, the other
//! player free everywhere and the turn and river below played optimally. The opponent's
//! best-response value in that game is what the stored flop strategy concedes; half the
//! sum over both players is its exploitability, comparable to a solve's (an exact
//! equilibrium concedes nothing, and the measure is an upper bound up to the solve's own
//! precision). A third game with both flops locked derives the rest of the file
//! (equities, EVs, the turn layer), so the check returns the spot rebuilt from its own
//! solves, flop bytes unchanged, and only that is kept. Three solves is why the cloud
//! takes catalog spots only.

use postflop_solver::{Action, PostFlopGame, compute_exploitability, compute_mes_ev, finalize, solve_step};

use super::catalog::{Catalog, SpotId};
use super::format::{Child, Stored, encode, hands};
use crate::node::ActionKind;
use crate::solve::{Settings, Solution, Summary};
use crate::spot::{bb, build, parse_board};
use crate::{Error, Progress, solve};

/// Solves `id` to the catalog's target and stores it.
pub fn solve_spot(
    catalog: &Catalog,
    id: SpotId,
    threads: usize,
    memory_limit: u64,
    on_progress: impl FnMut(&Progress),
    stop: impl Fn() -> bool,
) -> Result<Stored, Error> {
    let scenario = &catalog.scenarios[id.scenario];
    let flop = &catalog.flops[id.flop];
    let settings = Settings { target_pct: catalog.target_pct, max_iterations: catalog.max_iterations, threads };
    let mut solution = solve(&scenario.spot(flop), &settings, memory_limit, on_progress, stop)?;
    encode(&mut solution, catalog, scenario, flop)
}

/// The outcome of [`check`].
#[derive(Debug)]
pub struct Checked {
    /// Of the stored flop strategies, in percent of the pot.
    pub exploitability_pct: f64,
    /// Summed over the three solves.
    pub iterations: u32,
    /// The spot as this check derived it, with the stored flop strategies.
    pub rebuilt: Stored,
}

fn solver_action(a: &Action) -> (ActionKind, i32) {
    match *a {
        Action::Fold => (ActionKind::Fold, 0),
        Action::Check => (ActionKind::Check, 0),
        Action::Call => (ActionKind::Call, 0),
        Action::Bet(x) => (ActionKind::Bet, x),
        Action::Raise(x) => (ActionKind::Raise, x),
        Action::AllIn(x) => (ActionKind::AllIn, x),
        Action::None | Action::Chance(_) => (ActionKind::Fold, -1),
    }
}

struct Locker<'a> {
    game: &'a mut PostFlopGame,
    stored: &'a Stored,
    /// Per player: solver hand index of each stored-order combo.
    order: [Vec<usize>; 2],
    next: usize,
    /// Whose flop decisions get locked.
    players: [bool; 2],
}

impl Locker<'_> {
    fn lock(&mut self, history: &mut Vec<usize>, alive: [Vec<bool>; 2]) -> Result<(), Error> {
        let node = self.stored.nodes.get(self.next).ok_or_else(|| Error::Invalid("missing flop node".into()))?;
        self.next += 1;
        self.game.apply_history(history);
        let p = node.player as usize;
        if self.game.is_chance_node() || self.game.is_terminal_node() || self.game.current_player() != p {
            return Err(Error::Invalid("flop tree differs from the catalog".into()));
        }
        let actions = self.game.available_actions();
        let same = actions.len() == node.actions.len()
            && actions.iter().zip(&node.actions).all(|(a, s)| solver_action(a) == (s.kind, s.amount));
        if !same {
            return Err(Error::Invalid("actions differ from the catalog".into()));
        }
        let k = actions.len();
        let n = self.game.private_cards(p).len();
        let mut strategy = vec![0.0f32; k * n];
        let mut per_hand: Vec<Option<&[u8]>> = vec![None; alive[p].len()];
        let mut stored_hand = 0;
        for (h, &is_alive) in alive[p].iter().enumerate() {
            if !is_alive {
                continue;
            }
            let q = &node.strategy[stored_hand * k..(stored_hand + 1) * k];
            stored_hand += 1;
            for (a, &x) in q.iter().enumerate() {
                strategy[a * n + self.order[p][h]] = x as f32 / 255.0;
            }
            per_hand[h] = Some(q);
        }
        if stored_hand != node.equity.len() {
            return Err(Error::Invalid("hand count differs".into()));
        }
        if self.players[p] {
            self.game.lock_current_strategy(&strategy);
        }

        for a in 0..k {
            history.push(a);
            self.game.apply_history(history);
            let child = if self.game.is_terminal_node() {
                Child::End
            } else if self.game.is_chance_node() {
                let stacks = self.game.tree_config().effective_stack - self.game.total_bet_amount().into_iter().max().unwrap_or(0);
                if stacks <= 0 { Child::Runout } else { Child::Turn }
            } else {
                Child::Node
            };
            if child != node.children[a] {
                history.pop();
                return Err(Error::Invalid("flop tree differs from the catalog".into()));
            }
            if child == Child::Node {
                let mut next = alive.clone();
                for (h, q) in per_hand.iter().enumerate() {
                    if let Some(q) = q {
                        next[p][h] = q[a] > 0;
                    }
                }
                self.lock(history, next)?;
            }
            history.pop();
        }
        Ok(())
    }
}

/// A game of the stored spot with the flop decisions of `players` locked, solved to the
/// catalog's target (or `max_iterations`).
struct Limits<'a> {
    pool: rayon::ThreadPool,
    threads: usize,
    memory_limit: u64,
    max_iterations: u32,
    stop: &'a dyn Fn() -> bool,
}

fn locked_solve(stored: &Stored, catalog: &Catalog, players: [bool; 2], limits: &Limits) -> Result<(Solution, f64), Error> {
    let Limits { pool, threads, memory_limit, max_iterations, stop } = limits;
    let (threads, memory_limit, max_iterations) = (*threads, *memory_limit, *max_iterations);
    let scenario = &catalog.scenarios[catalog.scenario(&stored.scenario).ok_or_else(|| Error::Invalid("scenario not in the catalog".into()))?];
    catalog.flop(&stored.flop).ok_or_else(|| Error::Invalid("flop not in the catalog".into()))?;
    let spot = scenario.spot(&stored.flop);
    let built = pool.install(|| build(&spot, memory_limit, threads))?;
    let estimate = built.estimate;
    if !estimate.fits {
        return Err(Error::TooLarge { needed: estimate.memory_bytes, limit: memory_limit });
    }
    let mut game = built.game;
    pool.install(|| game.allocate_memory(estimate.compressed));

    let flop = parse_board(&stored.flop)?;
    let mut order = [Vec::new(), Vec::new()];
    for (p, range) in [&scenario.oop_range, &scenario.ip_range].into_iter().enumerate() {
        let cards = game.private_cards(p);
        for (combo, _) in hands(range, &flop)? {
            let (a, b) = combo.cards();
            let pair = (a.index().min(b.index()), a.index().max(b.index()));
            let i = cards
                .iter()
                .position(|&(x, y)| (x.min(y), x.max(y)) == pair)
                .ok_or_else(|| Error::Invalid("hand not in the game".into()))?;
            order[p].push(i);
        }
    }
    let alive = [vec![true; order[0].len()], vec![true; order[1].len()]];
    let mut locker = Locker { game: &mut game, stored, order, next: 0, players };
    locker.lock(&mut Vec::new(), alive)?;
    if locker.next != stored.nodes.len() {
        return Err(Error::Invalid("extra flop nodes".into()));
    }
    game.back_to_root();

    let pot = game.tree_config().starting_pot as f64;
    let target = catalog.target_pct / 100.0 * pot;
    let mut expl = pool.install(|| compute_exploitability(&game)) as f64;
    let mut iteration = 0;
    while iteration < max_iterations && expl > target {
        if stop() {
            return Err(Error::Cancelled);
        }
        pool.install(|| solve_step(&game, iteration));
        iteration += 1;
        if iteration.is_multiple_of(10) || iteration == max_iterations {
            expl = pool.install(|| compute_exploitability(&game)) as f64;
        }
    }
    let summary = Summary {
        iterations: iteration,
        exploitability: bb(expl),
        exploitability_pct: expl / pot * 100.0,
        converged: expl <= target,
        elapsed_ms: 0,
        memory_bytes: estimate.memory_bytes,
        compressed: estimate.compressed,
        threads,
    };
    Ok((Solution { game, spot, summary }, pot))
}

/// Checks `stored` (see the module docs); each of the three solves stops at the catalog's
/// target or after `max_iterations`. Fails for a spot that is malformed or not in the
/// catalog.
pub fn check(
    stored: &Stored,
    catalog: &Catalog,
    threads: usize,
    memory_limit: u64,
    max_iterations: u32,
    stop: impl Fn() -> bool,
) -> Result<Checked, Error> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .map_err(|e| Error::Invalid(format!("threads: {e}")))?;
    let limits = Limits { pool, threads, memory_limit, max_iterations, stop: &stop };
    let pool = &limits.pool;
    let mut conceded = 0.0;
    let mut iterations = 0;
    for p in 0..2 {
        let mut players = [false; 2];
        players[p] = true;
        let (solution, pot) = locked_solve(stored, catalog, players, &limits)?;
        // Values are zero-sum around half the pot, so the opponent's best-response value
        // is directly what it wins beyond an even split; the equilibrium values cancel
        // over both players.
        let best = pool.install(|| compute_mes_ev(&solution.game));
        conceded += best[1 - p] as f64 / pot * 100.0;
        iterations += solution.summary.iterations;
    }
    let exploitability_pct = conceded / 2.0;

    let (mut solution, _) = locked_solve(stored, catalog, [true, true], &limits)?;
    iterations += solution.summary.iterations;
    pool.install(|| finalize(&mut solution.game));
    let scenario = &catalog.scenarios[catalog.scenario(&stored.scenario).expect("checked in locked_solve")];
    let rebuilt = encode(&mut solution, catalog, scenario, &stored.flop)?;
    let same_flop = rebuilt.nodes.len() == stored.nodes.len()
        && rebuilt.nodes.iter().zip(&stored.nodes).all(|(a, b)| a.strategy == b.strategy && a.actions == b.actions);
    if !same_flop {
        return Err(Error::Invalid("rebuilt flop strategies differ from the stored ones".into()));
    }
    Ok(Checked { exploitability_pct, iterations, rebuilt })
}
