//! Preflop model for deeper stacks (about 15–100 BB) at one table of 2–9 players:
//! open, limp, call, 3-bet, 4-bet and all-in with fixed sizes, solved per hand class.
//!
//! This is a **model**, not an equilibrium of the full game. Everything after the
//! flop is replaced by a formula (see [`Realization`]): a pot that sees a flop is
//! split by equity weighted with position, initiative, hand playability and stack
//! depth. All-in pots are exact (169×169 equity and tie tables). What the solver
//! measures — [`Solution::exploitability`] — is how far the result is from an
//! equilibrium *of this model game*.
//!
//! # Game tree
//! Seats are in preflop action order (first after the big blind … small blind, big
//! blind; heads-up the small blind is first). Sizes come from [`Sizes`]:
//! - Everyone folds to a player, who folds, opens to `open_bb` (`sb_open_bb` from the
//!   small blind), limps (small blind only, with [`Params::limps`]) or goes all in.
//! - After an open every later seat folds, calls, 3-bets or goes all in. Only one
//!   player may flat the open, except that the big blind may still call behind one
//!   caller: at most three players see a flop. A 3-bet behind a caller is a squeeze
//!   and grows by `squeeze_per_caller` opens per caller.
//! - After a sized 3-bet the remaining seats fold; the opener, then the caller, fold,
//!   call, 4-bet or go all in, and the first one to continue plays the 3-bettor
//!   heads-up (everyone else folds). A 4-bet is called, folded or answered all in.
//! - After an all-in every player still to act may call or fold; the first call ends
//!   the hand heads-up (all-in pots have two players).
//! - Against a small-blind limp the big blind checks, raises to `iso_bb` or goes all
//!   in; the small blind then folds, calls or goes all in.
//!
//! A sized raise that would put in at least [`Sizes::commit`] of the stack is replaced
//! by all-in, so short stacks degenerate to push/fold.
//!
//! [`Sizes::standard`] follows common solver and coaching practice: opens of 2–2.2 BB
//! below 60 BB and 2.5 BB deeper, 3 BB from the small blind (GTO Wizard mixes about
//! 3× at 30 BB, preflopwizard.app); 3-bets of about 2.5–3× in position and 3–4× out of
//! position, smaller when shallow (table attributed to GTO Wizard on poker.pro; MTT
//! Poker School, "3 Simple Rules to Choose the Right Preflop Raise Size"; Red Chip's
//! 20 BB charts assume 2.2 BB opens and 3-bets all in); 4-bets of 2.2× (the usual
//! 2.2–2.5×).
//!
//! # Postflop model
//! Equity realization (EQR = EV / (pot × equity)) published from solver reports:
//! button versus big blind in a single-raised pot 1.15 / 0.80 (9-max MTT, 20 BB,
//! 1 BB ante) and 1.17 / 0.78 over 184 flops; lojack versus big blind 1.14 / 0.87
//! (6-max cash, 100 BB); a limped pot 1.14 / 0.85 for the player in position
//! (poker.academy, "How to understand equity realisation"); an out-of-position opener
//! against an in-position caller 0.96 / 1.047 (GTO Wizard blog, "How ICM Quietly
//! Shapes Postflop Strategy", UTG versus BTN, 100 BB, chip EV); by hand type small
//! pairs 105–120 %, suited connectors 105–115 %, offsuit broadways 75–90 %, offsuit
//! junk 60–80 % (deepfold.co, rough guide values, not single solves).
//! [`Realization::default`] is fitted to these numbers; the ranges the model realizes
//! in its most frequent flops are reported in [`Solution::realized`] (button versus
//! big blind about 1.16 / 0.73–0.78 from 20 to 100 BB). The model knows nothing about
//! boards, so it can't reproduce how realization depends on the flop, and the hand
//! factors are estimates.
//!
//! # Card removal and solving
//! As in [`multiway`](crate::multiway): strategies are per hand class, and every
//! player sees each opponent's class through the exact two-class removal against its
//! own class and through how likely that class makes every other player's actions
//! (a chain of pairwise terms). Three-way pots use the simulated three-class showdown
//! tables of `multiway`. CFR+ (regret matching+, alternating updates, linear
//! averaging weighted by the player's own reach) runs until the summed best-response
//! gains of all players (NashConv), checked every few iterations, fall below
//! [`Params::target_exploitability`]. With more than two players CFR has no
//! convergence guarantee and the average can stall while the current strategies
//! converge, so both are checked; the best checked profile is returned.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::multiway::{INFEASIBLE, TRIPLES, WEAK3_ORDERS, conditional, simulate_triple, triple_classes, triple_index};
use crate::preflop_matrix::PreflopMatrix;
use crate::range::{HandClass, NUM_CLASSES, class_pair_count};

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 9;
const C: usize = NUM_CLASSES;
const CHECK_EVERY: u32 = 10;
/// Frequencies this close to 0 or 1 in the average profile are reported as pure.
const SNAP: f64 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Fold,
    Check,
    Call,
    Limp,
    Raise,
    AllIn,
}

/// An action and the chips its actor has in front afterwards (blind included, antes
/// not), in BB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub kind: Kind,
    pub to_bb: f64,
}

/// One non-fold action on the way to a node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub seat: usize,
    pub action: Action,
}

/// Bet sizes of the tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sizes {
    /// Open raise to, BB, from every seat but the small blind.
    pub open_bb: f64,
    pub sb_open_bb: f64,
    /// The big blind's raise over a small-blind limp, to BB.
    pub iso_bb: f64,
    /// 3-bet to, in multiples of the open, by a player in / out of position after the flop.
    pub three_bet_ip: f64,
    pub three_bet_oop: f64,
    /// Added to a 3-bet per caller of the open (squeeze), in opens.
    pub squeeze_per_caller: f64,
    /// 4-bet to, in multiples of the 3-bet.
    pub four_bet: f64,
    /// All-in is offered as an open (or over a limp) / 3-bet / 4-bet while the actor's
    /// stack is at most this many BB; a 5-bet is always all in.
    pub open_jam_max_bb: f64,
    pub three_bet_jam_max_bb: f64,
    pub four_bet_jam_max_bb: f64,
    /// A sized raise that would put in at least this share of the stack becomes all-in.
    pub commit: f64,
}

impl Sizes {
    /// Common sizes by stack depth: opens shrink and 3-bets get smaller as stacks get
    /// shorter, 3-bets out of position are larger (sources in the module docs).
    pub fn standard(stack_bb: f64) -> Sizes {
        let (open_bb, sb_open_bb, iso_bb) = if stack_bb < 25.0 {
            (2.0, 2.5, 3.0)
        } else if stack_bb < 60.0 {
            (2.2, 3.0, 3.5)
        } else {
            (2.5, 3.0, 3.5)
        };
        let (three_bet_ip, three_bet_oop) = if stack_bb < 30.0 {
            (2.5, 3.0)
        } else if stack_bb < 60.0 {
            (2.75, 3.75)
        } else {
            (3.0, 4.0)
        };
        Sizes {
            open_bb,
            sb_open_bb,
            iso_bb,
            three_bet_ip,
            three_bet_oop,
            squeeze_per_caller: 1.0,
            four_bet: 2.2,
            open_jam_max_bb: 25.0,
            three_bet_jam_max_bb: 50.0,
            four_bet_jam_max_bb: 60.0,
            commit: 0.4,
        }
    }

    fn values(&self) -> [f64; 11] {
        [
            self.open_bb,
            self.sb_open_bb,
            self.iso_bb,
            self.three_bet_ip,
            self.three_bet_oop,
            self.squeeze_per_caller,
            self.four_bet,
            self.open_jam_max_bb,
            self.three_bet_jam_max_bb,
            self.four_bet_jam_max_bb,
            self.commit,
        ]
    }

    fn from_values(v: [f64; 11]) -> Sizes {
        Sizes {
            open_bb: v[0],
            sb_open_bb: v[1],
            iso_bb: v[2],
            three_bet_ip: v[3],
            three_bet_oop: v[4],
            squeeze_per_caller: v[5],
            four_bet: v[6],
            open_jam_max_bb: v[7],
            three_bet_jam_max_bb: v[8],
            four_bet_jam_max_bb: v[9],
            commit: v[10],
        }
    }
}

/// The postflop model. A pot that sees a flop goes to its players in proportion to
/// `equity^γ × weight`, where a player's weight is
/// `(position^k × initiative^[last aggressor] × playability(class))^f(SPR)`,
/// `k` is the number of players acting before it after the flop,
/// `f(SPR) = 1 − exp(−SPR / spr_scale)` and `γ = 1 + (sharpness − 1) × f(SPR)`, so at
/// SPR 0 the pot is split by plain equity. Equity is the all-in equity of the hand
/// classes (three-way from simulated deals). The split always hands out the whole pot,
/// so the model game stays zero-sum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Realization {
    pub position: f64,
    pub initiative: f64,
    pub pair: f64,
    pub suited: f64,
    pub offsuit: f64,
    /// Extra playability of connected non-pairs: all of it without a gap, half with one.
    pub connected: f64,
    /// Lost playability of non-pairs with three or more ranks between their cards.
    pub gapped: f64,
    /// Exponent on equity: above 1 strong hands realize more and weak ones less.
    pub sharpness: f64,
    pub spr_scale: f64,
}

impl Default for Realization {
    /// Fitted to the published realization figures in the module docs: position and
    /// initiative to the button/big blind and opener/caller ratios, the hand factors to
    /// the spread between hand types, sharpness and depth scale so that range figures
    /// stay in the published band from 20 to 100 BB.
    fn default() -> Realization {
        Realization {
            position: 1.2,
            initiative: 1.1,
            pair: 1.0,
            suited: 1.10,
            offsuit: 0.88,
            connected: 0.08,
            gapped: 0.06,
            sharpness: 1.5,
            spr_scale: 1.5,
        }
    }
}

impl Realization {
    fn values(&self) -> [f64; 9] {
        [self.position, self.initiative, self.pair, self.suited, self.offsuit, self.connected, self.gapped, self.sharpness, self.spr_scale]
    }

    fn from_values(v: [f64; 9]) -> Realization {
        Realization {
            position: v[0],
            initiative: v[1],
            pair: v[2],
            suited: v[3],
            offsuit: v[4],
            connected: v[5],
            gapped: v[6],
            sharpness: v[7],
            spr_scale: v[8],
        }
    }

    /// Playability of a class.
    pub fn playability(&self, class: HandClass) -> f64 {
        if class.is_pair() {
            return self.pair;
        }
        let (hi, lo) = class.ranks();
        let gap = hi - lo - 1;
        let base = if class.is_suited() { self.suited } else { self.offsuit };
        base * match gap {
            0 => 1.0 + self.connected,
            1 => 1.0 + self.connected / 2.0,
            2 => 1.0,
            _ => 1.0 - self.gapped,
        }
    }

    /// Equity exponent at depth factor `f`: plain equity at SPR 0.
    pub fn sharpness_at(&self, f: f64) -> f64 {
        1.0 + (self.sharpness - 1.0) * f
    }

    /// How fully the postflop weights apply at a stack-to-pot ratio.
    pub fn depth_factor(&self, spr: f64) -> f64 {
        if self.spr_scale <= 0.0 { 1.0 } else { 1.0 - (-spr.max(0.0) / self.spr_scale).exp() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Params {
    pub players: usize,
    /// Equal stack of every seat before posting, BB.
    pub stack_bb: f64,
    pub small_blind_bb: f64,
    /// Ante paid by every player.
    pub ante_bb: f64,
    /// Ante paid by the big blind alone.
    pub bb_ante_bb: f64,
    pub sizes: Sizes,
    /// The small blind may limp when everyone folded to it.
    pub limps: bool,
    pub realization: Realization,
    /// `false` treats all players' classes as independent.
    pub card_removal: bool,
    /// Stop once NashConv is below this, in BB per hand.
    pub target_exploitability: f64,
    pub max_iterations: u32,
    /// Monte Carlo deals per class triple for three-way pots.
    pub multiway_samples: u32,
}

impl Params {
    /// Chip EV, small blind 0.5 BB, no antes, standard sizes.
    pub fn new(players: usize, stack_bb: f64) -> Params {
        Params {
            players,
            stack_bb,
            small_blind_bb: 0.5,
            ante_bb: 0.0,
            bb_ante_bb: 0.0,
            sizes: Sizes::standard(stack_bb),
            limps: true,
            realization: Realization::default(),
            card_removal: true,
            target_exploitability: 0.01,
            max_iterations: 4000,
            multiway_samples: 1024,
        }
    }

    /// The numbers stored with a table entry, in a fixed order.
    pub(crate) fn numbers(&self) -> Vec<f64> {
        let mut v = vec![self.stack_bb, self.small_blind_bb, self.ante_bb, self.bb_ante_bb, self.target_exploitability];
        v.extend(self.sizes.values());
        v.extend(self.realization.values());
        v
    }

    pub(crate) fn from_numbers(
        players: usize,
        v: &[f64],
        limps: bool,
        card_removal: bool,
        max_iterations: u32,
        multiway_samples: u32,
    ) -> Option<Params> {
        if v.len() != 5 + 11 + 9 {
            return None;
        }
        Some(Params {
            players,
            stack_bb: v[0],
            small_blind_bb: v[1],
            ante_bb: v[2],
            bb_ante_bb: v[3],
            target_exploitability: v[4],
            sizes: Sizes::from_values(v[5..16].try_into().ok()?),
            realization: Realization::from_values(v[16..25].try_into().ok()?),
            limps,
            card_removal,
            max_iterations,
            multiway_samples,
        })
    }

    fn validate(&self) -> Result<(), Error> {
        let bad = |m: &str| Err(Error::InvalidInput(m.into()));
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&self.players) {
            return bad("the preflop model needs 2 to 9 players");
        }
        if self.numbers().iter().any(|x| !x.is_finite()) {
            return bad("all numbers must be finite");
        }
        if self.small_blind_bb <= 0.0 || self.small_blind_bb > 1.0 {
            return bad("small blind must be in (0, 1] BB");
        }
        if self.ante_bb < 0.0 || self.bb_ante_bb < 0.0 {
            return bad("antes must not be negative");
        }
        if self.stack_bb - self.ante_bb - self.bb_ante_bb <= 1.0 {
            return bad("the big blind must cover its blind and antes");
        }
        let s = &self.sizes;
        if s.open_bb <= 1.0 || s.sb_open_bb <= 1.0 || s.iso_bb <= 1.0 {
            return bad("opens must be larger than the big blind");
        }
        if s.three_bet_ip <= 1.0 || s.three_bet_oop <= 1.0 || s.four_bet <= 1.0 || s.squeeze_per_caller < 0.0 {
            return bad("raises must be larger than the bet they raise");
        }
        if !(0.0..=1.0).contains(&s.commit) {
            return bad("commit must be a share of the stack");
        }
        let r = &self.realization;
        if [r.position, r.initiative, r.pair, r.suited, r.offsuit].iter().any(|&x| x <= 0.0) || r.connected < 0.0 || !(0.0..1.0).contains(&r.gapped) || r.sharpness < 1.0 {
            return bad("realization weights must be positive");
        }
        if self.max_iterations == 0 {
            return bad("max_iterations must be positive");
        }
        if self.multiway_samples == 0 || self.multiway_samples > u16::MAX as u32 {
            return bad("multiway_samples must be in 1..=65535");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Solution {
    pub players: usize,
    pub stack_bb: f64,
    /// Every decision node in tree order; the root is node 0.
    pub nodes: Vec<Node>,
    /// Expected result per seat, BB against the starting stack.
    pub ev: Vec<f64>,
    /// What each seat could gain per hand with a best response in the model game.
    pub gain: Vec<f64>,
    /// NashConv: the sum of `gain`.
    pub exploitability: f64,
    pub iterations: u32,
    pub converged: bool,
    /// Σ EV over seats (0 in an exact zero-sum evaluation): the inconsistency of the
    /// pairwise card-removal chain.
    pub ev_sum_error: f64,
    /// Equity realization the model produces in the most frequent heads-up flops.
    pub realized: Vec<Realized>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub seat: usize,
    /// Non-fold actions before this node; every seat not in it that acted folded.
    pub line: Vec<Step>,
    pub actions: Vec<Action>,
    /// Probability that a hand reaches this node.
    pub reach: f64,
    /// `freq[a][class]`: frequency of action `a` per class (`HandClass::index`).
    pub freq: Vec<Vec<f64>>,
    /// `ev[a][class]`: expected result of action `a`, BB against the starting stack,
    /// given the node is reached with the class.
    pub ev: Vec<Vec<f64>>,
}

/// A heads-up flop of the solution: how much of its equity each player realizes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Realized {
    /// The actions that led to the flop.
    pub line: Vec<Step>,
    /// The two players, first to act after the flop first.
    pub seats: Vec<usize>,
    pub reach: f64,
    pub pot_bb: f64,
    pub spr: f64,
    /// All-in equity of each player's range against the other's.
    pub equity: Vec<f64>,
    /// Pot share in the model divided by equity.
    pub realization: Vec<f64>,
}

impl Solution {
    /// The node where `seat` acts after the non-fold actions `line`.
    pub fn node(&self, seat: usize, line: &[Step]) -> Option<usize> {
        self.nodes.iter().position(|n| n.seat == seat && n.line == line)
    }

    /// The open node of `seat` (everyone before folded).
    pub fn open_node(&self, seat: usize) -> Option<usize> {
        self.node(seat, &[])
    }

    /// Share of all combos that take action `a` at node `v`, in percent.
    pub fn action_pct(&self, v: usize, a: usize) -> f64 {
        100.0
            * HandClass::all()
                .map(|c| self.nodes[v].freq[a][c.index()] * c.combo_count() as f64 / 1326.0)
                .sum::<f64>()
    }

    /// Share of all combos that do anything but fold at node `v`, in percent.
    pub fn play_pct(&self, v: usize) -> f64 {
        let n = &self.nodes[v];
        (0..n.actions.len()).filter(|&a| n.actions[a].kind != Kind::Fold).map(|a| self.action_pct(v, a)).sum()
    }

    /// Share of all combos that take an action of `kind` at node `v`, in percent.
    pub fn kind_pct(&self, v: usize, kind: Kind) -> f64 {
        let n = &self.nodes[v];
        (0..n.actions.len()).filter(|&a| n.actions[a].kind == kind).map(|a| self.action_pct(v, a)).sum()
    }
}

/// Postflop order: 0 acts first after the flop.
pub fn postflop_rank(players: usize, seat: usize) -> usize {
    if players == 2 {
        return 1 - seat;
    }
    if seat == players - 2 {
        0
    } else if seat == players - 1 {
        1
    } else {
        seat + 2
    }
}

pub fn solve(params: &Params) -> Result<Solution, Error> {
    params.validate()?;
    let mut solver = Solver::new(params);
    Ok(solver.run())
}

/// Number of decision nodes and terminals of the tree for these parameters.
pub fn tree_size(params: &Params) -> Result<(usize, usize), Error> {
    params.validate()?;
    let t = Tree::build(params);
    Ok((t.nodes.len(), t.terms.len()))
}

/// The tree's node shapes (seat, line, actions) without solving; tables store only
/// the numbers and rebuild these.
/// A node's seat, line and actions.
pub(crate) type Shape = (usize, Vec<Step>, Vec<Action>);

pub(crate) fn skeleton(params: &Params) -> Result<Vec<Shape>, Error> {
    params.validate()?;
    let t = Tree::build(params);
    Ok(t.nodes.into_iter().map(|n| (n.seat, n.line, n.actions)).collect())
}

// ---------------------------------------------------------------------------------
// Tree.

#[derive(Clone, Copy)]
enum Child {
    Node(usize),
    Term(usize),
}

struct TreeNode {
    seat: usize,
    line: Vec<Step>,
    actions: Vec<Action>,
    children: Vec<Child>,
    /// Edge `edge0 + a` is action `a`.
    edge0: usize,
    /// Latest edge of each seat before this node.
    last: Vec<Option<usize>>,
}

#[derive(Clone, Copy, PartialEq)]
enum Outcome {
    /// Everyone else folded.
    Won(usize),
    Showdown,
    Postflop,
}

struct Terminal {
    outcome: Outcome,
    /// Seats contesting the pot; postflop in postflop order.
    involved: Vec<usize>,
    /// Card-independent result of each seat (chips put in, negative; a sole winner's
    /// pot included).
    base: Vec<f64>,
    /// The pot the involved seats share by their cards.
    pot: f64,
    spr: f64,
    aggressor: Option<usize>,
    last: Vec<Option<usize>>,
    line: Vec<Step>,
}

struct Tree {
    n: usize,
    nodes: Vec<TreeNode>,
    terms: Vec<Terminal>,
    /// (node, action) of each edge.
    edges: Vec<(usize, usize)>,
    nodes_of: Vec<Vec<usize>>,
    root: Child,
}

#[derive(Clone)]
struct Hand {
    contrib: Vec<f64>,
    last: Vec<Option<usize>>,
    line: Vec<Step>,
}

struct Builder<'a> {
    p: &'a Params,
    n: usize,
    live: Vec<f64>,
    ante: Vec<f64>,
    tree: Tree,
}

fn act(kind: Kind, to_bb: f64) -> Action {
    Action { kind, to_bb }
}

impl Tree {
    fn build(p: &Params) -> Tree {
        let n = p.players;
        let ante: Vec<f64> = (0..n).map(|s| p.ante_bb + if s == n - 1 { p.bb_ante_bb } else { 0.0 }).collect();
        let live: Vec<f64> = ante.iter().map(|a| p.stack_bb - a).collect();
        let mut contrib = vec![0.0; n];
        contrib[n - 2] = p.small_blind_bb;
        contrib[n - 1] = 1.0;
        let mut b = Builder {
            p,
            n,
            live,
            ante,
            tree: Tree { n, nodes: Vec::new(), terms: Vec::new(), edges: Vec::new(), nodes_of: vec![Vec::new(); n], root: Child::Term(0) },
        };
        let hand = Hand { contrib, last: vec![None; n], line: Vec::new() };
        let root = b.open(hand, 0);
        b.tree.root = root;
        b.tree
    }
}

impl Builder<'_> {
    fn push(&mut self, h: &Hand, seat: usize, actions: Vec<Action>) -> usize {
        let t = &mut self.tree;
        let v = t.nodes.len();
        let edge0 = t.edges.len();
        t.edges.extend((0..actions.len()).map(|a| (v, a)));
        t.nodes.push(TreeNode { seat, line: h.line.clone(), actions, children: Vec::new(), edge0, last: h.last.clone() });
        t.nodes_of[seat].push(v);
        v
    }

    fn after(&self, h: &Hand, v: usize, a: usize) -> Hand {
        let node = &self.tree.nodes[v];
        let action = node.actions[a];
        let mut h = h.clone();
        h.last[node.seat] = Some(node.edge0 + a);
        if action.kind != Kind::Fold {
            h.contrib[node.seat] = action.to_bb;
            h.line.push(Step { seat: node.seat, action });
        }
        h
    }

    /// The raise if it stays below the commitment share of the stack.
    fn sized(&self, seat: usize, to: f64) -> Option<f64> {
        (to < self.p.sizes.commit * self.live[seat]).then_some(to)
    }

    fn rank(&self, seat: usize) -> usize {
        postflop_rank(self.n, seat)
    }

    /// Builds the node and its children; `child(builder, hand after a, a)` builds each.
    fn node(&mut self, h: &Hand, seat: usize, actions: Vec<Action>, mut child: impl FnMut(&mut Self, Hand, Action) -> Child) -> Child {
        let v = self.push(h, seat, actions.clone());
        let children = (0..actions.len())
            .map(|a| {
                let next = self.after(h, v, a);
                child(self, next, actions[a])
            })
            .collect();
        self.tree.nodes[v].children = children;
        Child::Node(v)
    }

    fn terminal(&mut self, h: &Hand, outcome: Outcome, mut involved: Vec<usize>, aggressor: Option<usize>) -> Child {
        let n = self.n;
        let mut contrib = h.contrib.clone();
        if outcome == Outcome::Showdown {
            // The bigger all-in gets back what the other can't match.
            let m = involved.iter().map(|&s| contrib[s]).fold(f64::INFINITY, f64::min);
            for &s in &involved {
                contrib[s] = m;
            }
        }
        let dead: f64 = self.ante.iter().sum::<f64>() + contrib.iter().sum::<f64>();
        let mut base: Vec<f64> = (0..n).map(|s| -contrib[s] - self.ante[s]).collect();
        let mut pot = dead;
        if let Outcome::Won(w) = outcome {
            base[w] += dead;
            pot = 0.0;
            involved = vec![w];
        }
        if outcome == Outcome::Postflop {
            involved.sort_by_key(|&s| self.rank(s));
        }
        let behind = involved.iter().map(|&s| self.live[s] - contrib[s]).fold(f64::INFINITY, f64::min);
        let spr = if outcome == Outcome::Postflop { behind / pot } else { 0.0 };
        self.tree.terms.push(Terminal { outcome, involved, base, pot, spr, aggressor, last: h.last.clone(), line: h.line.clone() });
        Child::Term(self.tree.terms.len() - 1)
    }

    fn open(&mut self, h: Hand, s: usize) -> Child {
        let n = self.n;
        if s == n - 1 {
            return self.terminal(&h, Outcome::Won(n - 1), Vec::new(), None);
        }
        let params = self.p;
        let sz = &params.sizes;
        let sb = s == n - 2;
        let mut actions = vec![act(Kind::Fold, h.contrib[s])];
        if sb && self.p.limps {
            actions.push(act(Kind::Limp, 1.0));
        }
        let open = if sb && n > 2 { sz.sb_open_bb } else { sz.open_bb };
        let sized = self.sized(s, open);
        if let Some(to) = sized {
            actions.push(act(Kind::Raise, to));
        }
        if sized.is_none() || self.live[s] <= sz.open_jam_max_bb {
            actions.push(act(Kind::AllIn, self.live[s]));
        }
        self.node(&h, s, actions, |b, next, a| match a.kind {
            Kind::Fold => b.open(next, s + 1),
            Kind::Limp => b.vs_limp(next),
            Kind::Raise => b.vs_open(next, s, Vec::new(), s + 1),
            _ => b.vs_jam(next, s, (s + 1..n).collect(), 0),
        })
    }

    fn vs_open(&mut self, h: Hand, opener: usize, callers: Vec<usize>, s: usize) -> Child {
        let n = self.n;
        if s == n {
            if callers.is_empty() {
                return self.terminal(&h, Outcome::Won(opener), Vec::new(), None);
            }
            let mut involved = callers.clone();
            involved.push(opener);
            return self.terminal(&h, Outcome::Postflop, involved, Some(opener));
        }
        let params = self.p;
        let sz = &params.sizes;
        let bet = h.contrib[opener];
        let mut actions = vec![act(Kind::Fold, h.contrib[s])];
        if callers.is_empty() || (s == n - 1 && callers.len() == 1) {
            actions.push(act(Kind::Call, bet));
        }
        let mult = if self.rank(s) > self.rank(opener) { sz.three_bet_ip } else { sz.three_bet_oop };
        let sized = self.sized(s, bet * (mult + sz.squeeze_per_caller * callers.len() as f64));
        if let Some(to) = sized {
            actions.push(act(Kind::Raise, to));
        }
        if sized.is_none() || self.live[s] <= sz.three_bet_jam_max_bb {
            actions.push(act(Kind::AllIn, self.live[s]));
        }
        self.node(&h, s, actions, |b, next, a| match a.kind {
            Kind::Fold => b.vs_open(next, opener, callers.clone(), s + 1),
            Kind::Call => {
                let mut more = callers.clone();
                more.push(s);
                b.vs_open(next, opener, more, s + 1)
            }
            Kind::Raise => {
                let mut responders = vec![opener];
                responders.extend(&callers);
                b.vs_three_bet(next, responders, 0, s)
            }
            _ => {
                let mut responders: Vec<usize> = (s + 1..n).collect();
                responders.push(opener);
                responders.extend(&callers);
                b.vs_jam(next, s, responders, 0)
            }
        })
    }

    fn vs_three_bet(&mut self, h: Hand, responders: Vec<usize>, i: usize, t: usize) -> Child {
        let Some(&r) = responders.get(i) else {
            return self.terminal(&h, Outcome::Won(t), Vec::new(), None);
        };
        let params = self.p;
        let sz = &params.sizes;
        let bet = h.contrib[t];
        let mut actions = vec![act(Kind::Fold, h.contrib[r]), act(Kind::Call, bet)];
        let sized = self.sized(r, bet * sz.four_bet);
        if let Some(to) = sized {
            actions.push(act(Kind::Raise, to));
        }
        if sized.is_none() || self.live[r] <= sz.four_bet_jam_max_bb {
            actions.push(act(Kind::AllIn, self.live[r]));
        }
        self.node(&h, r, actions, |b, next, a| match a.kind {
            Kind::Fold => b.vs_three_bet(next, responders.clone(), i + 1, t),
            Kind::Call => b.terminal(&next, Outcome::Postflop, vec![r, t], Some(t)),
            Kind::Raise => b.vs_four_bet(next, r, t),
            _ => b.vs_jam(next, r, vec![t], 0),
        })
    }

    fn vs_four_bet(&mut self, h: Hand, r: usize, t: usize) -> Child {
        let actions = vec![act(Kind::Fold, h.contrib[t]), act(Kind::Call, h.contrib[r]), act(Kind::AllIn, self.live[t])];
        self.node(&h, t, actions, |b, next, a| match a.kind {
            Kind::Fold => b.terminal(&next, Outcome::Won(r), Vec::new(), None),
            Kind::Call => b.terminal(&next, Outcome::Postflop, vec![r, t], Some(r)),
            _ => b.vs_jam(next, t, vec![r], 0),
        })
    }

    fn vs_jam(&mut self, h: Hand, jammer: usize, responders: Vec<usize>, i: usize) -> Child {
        let Some(&s) = responders.get(i) else {
            return self.terminal(&h, Outcome::Won(jammer), Vec::new(), None);
        };
        let call = h.contrib[jammer].min(self.live[s]);
        let actions = vec![act(Kind::Fold, h.contrib[s]), act(Kind::Call, call)];
        self.node(&h, s, actions, |b, next, a| match a.kind {
            Kind::Fold => b.vs_jam(next, jammer, responders.clone(), i + 1),
            _ => b.terminal(&next, Outcome::Showdown, vec![jammer, s], None),
        })
    }

    fn vs_limp(&mut self, h: Hand) -> Child {
        let (sb, bb) = (self.n - 2, self.n - 1);
        let params = self.p;
        let sz = &params.sizes;
        let mut actions = vec![act(Kind::Check, 1.0)];
        let sized = self.sized(bb, sz.iso_bb);
        if let Some(to) = sized {
            actions.push(act(Kind::Raise, to));
        }
        if sized.is_none() || self.live[bb] <= sz.open_jam_max_bb {
            actions.push(act(Kind::AllIn, self.live[bb]));
        }
        self.node(&h, bb, actions, |b, next, a| match a.kind {
            Kind::Check => b.terminal(&next, Outcome::Postflop, vec![sb, bb], None),
            Kind::Raise => b.vs_iso(next),
            _ => b.vs_jam(next, bb, vec![sb], 0),
        })
    }

    fn vs_iso(&mut self, h: Hand) -> Child {
        let (sb, bb) = (self.n - 2, self.n - 1);
        let mut actions = vec![act(Kind::Fold, h.contrib[sb]), act(Kind::Call, h.contrib[bb])];
        if self.live[sb] <= self.p.sizes.three_bet_jam_max_bb {
            actions.push(act(Kind::AllIn, self.live[sb]));
        }
        self.node(&h, sb, actions, |b, next, a| match a.kind {
            Kind::Fold => b.terminal(&next, Outcome::Won(bb), Vec::new(), None),
            Kind::Call => b.terminal(&next, Outcome::Postflop, vec![sb, bb], Some(bb)),
            _ => b.vs_jam(next, sb, vec![bb], 0),
        })
    }
}

// ---------------------------------------------------------------------------------
// Pot shares.

/// All-in pot share of each player of every sorted class triple (`NaN` where the three
/// can't be dealt together), per sample count, shared by all solves of the process.
type TripleEquities = Arc<Vec<[f32; 3]>>;
static TRIPLE_EQUITY: Mutex<Vec<(u32, TripleEquities)>> = Mutex::new(Vec::new());

fn triple_equities(samples: u32) -> TripleEquities {
    let mut cache = TRIPLE_EQUITY.lock().expect("triple equity lock");
    if let Some((_, t)) = cache.iter().find(|(s, _)| *s == samples) {
        return t.clone();
    }
    let table: Vec<[f32; 3]> = (0..TRIPLES)
        .into_par_iter()
        .map(|i| {
            let row = simulate_triple(&triple_classes(i), samples);
            if row == INFEASIBLE {
                return [f32::NAN; 3];
            }
            let mut eq = [0.0f64; 3];
            for (&count, l) in row.iter().zip(&WEAK3_ORDERS.0) {
                let top = *l.iter().max().expect("three levels");
                let winners = l.iter().filter(|&&x| x == top).count() as f64;
                for (e, &li) in eq.iter_mut().zip(l) {
                    if li == top {
                        *e += count as f64 / samples as f64 / winners;
                    }
                }
            }
            eq.map(|e| e as f32)
        })
        .collect();
    let table = Arc::new(table);
    cache.push((samples, table.clone()));
    table
}

/// A table of one player's expected pot share against the other contestants'
/// classes, premultiplied by the card-removal weights: `[c][x]` heads-up,
/// `[c][x][y]` three-way (x, y in postflop order of the others).
struct ShareTable(Vec<f32>);

#[derive(Clone, PartialEq, Eq, Hash)]
enum TableKey {
    /// Heads-up all in: plain equity.
    Equity,
    /// Heads-up flop: observer weight / opponent weight and depth factor (f64 bits).
    Two(u64, u64),
    /// Three-way flop: weights in postflop order, depth factor, observer position.
    Three([u64; 3], u64, usize),
}

fn weights_of(r: &Realization) -> Vec<f64> {
    HandClass::all().map(|c| r.playability(c)).collect()
}

fn build_table(key: &TableKey, r: &Realization, cond: &[f64], samples: u32) -> ShareTable {
    let m = PreflopMatrix::embedded();
    let q = weights_of(r);
    match *key {
        TableKey::Equity => ShareTable(
            (0..C * C)
                .map(|i| {
                    let (c, x) = (i / C, i % C);
                    (cond[i] * m.equity(HandClass::from_index(c), HandClass::from_index(x))) as f32
                })
                .collect(),
        ),
        TableKey::Two(ratio, f) => {
            let (ratio, f) = (f64::from_bits(ratio), f64::from_bits(f));
            let rho = ratio.powf(f);
            let qf: Vec<f64> = q.iter().map(|x| x.powf(f)).collect();
            let gamma = r.sharpness_at(f);
            ShareTable(
                (0..C * C)
                    .map(|i| {
                        let (c, x) = (i / C, i % C);
                        if cond[i] == 0.0 {
                            return 0.0;
                        }
                        let e = m.equity(HandClass::from_index(c), HandClass::from_index(x));
                        let mine = e.powf(gamma) * qf[c] * rho;
                        let share = mine / (mine + (1.0 - e).powf(gamma) * qf[x]);
                        (cond[i] * share) as f32
                    })
                    .collect(),
            )
        }
        TableKey::Three(w, f, me) => {
            let f = f64::from_bits(f);
            let w = w.map(|x| f64::from_bits(x).powf(f));
            let qf: Vec<f64> = q.iter().map(|x| x.powf(f)).collect();
            let gamma = r.sharpness_at(f);
            let eq = triple_equities(samples);
            let others: Vec<usize> = (0..3).filter(|&k| k != me).collect();
            let mut g = vec![0f32; C * C * C];
            g.par_chunks_mut(C * C).enumerate().for_each(|(c, gc)| {
                let row = &cond[c * C..][..C];
                for x in 0..C {
                    for y in 0..C {
                        let weight = row[x] * row[y];
                        if weight == 0.0 {
                            continue;
                        }
                        let mut classes = [0u8; 3];
                        classes[me] = c as u8;
                        classes[others[0]] = x as u8;
                        classes[others[1]] = y as u8;
                        let mut order = [0usize, 1, 2];
                        order.sort_by_key(|&k| classes[k]);
                        let sorted = [classes[order[0]], classes[order[1]], classes[order[2]]];
                        let row_eq = eq[triple_index(&sorted)];
                        let share = if row_eq[0].is_nan() {
                            1.0 / 3.0
                        } else {
                            let mut e = [0.0; 3];
                            for (j, &k) in order.iter().enumerate() {
                                e[k] = row_eq[j] as f64;
                            }
                            let part = |k: usize| e[k].powf(gamma) * w[k] * qf[classes[k] as usize];
                            let total = part(0) + part(1) + part(2);
                            if total > 0.0 { part(me) / total } else { 1.0 / 3.0 }
                        };
                        gc[x * C + y] = (weight * share) as f32;
                    }
                }
            });
            ShareTable(g)
        }
    }
}

/// Heads-up pot share of `mine` against `other` in the model.
fn share_two(r: &Realization, e: f64, mine: HandClass, other: HandClass, ratio: f64, f: f64) -> f64 {
    let g = r.sharpness_at(f);
    let a = e.powf(g) * (ratio * r.playability(mine)).powf(f);
    a / (a + (1.0 - e).powf(g) * r.playability(other).powf(f))
}

fn dot32(a: &[f32], b: &[f32]) -> f64 {
    // Eight partial sums keep the f32 dot product vectorized and accurate.
    let mut acc = [0f32; 8];
    let (ac, ar) = a.as_chunks::<8>();
    let (bc, br) = b.as_chunks::<8>();
    for (x, y) in ac.iter().zip(bc) {
        for i in 0..8 {
            acc[i] += x[i] * y[i];
        }
    }
    let tail: f64 = ar.iter().zip(br).map(|(x, y)| *x as f64 * *y as f64).sum();
    acc.iter().map(|&x| x as f64).sum::<f64>() + tail
}

// ---------------------------------------------------------------------------------
// Solver.

struct Profile {
    /// `sigma[v][a * C + c]`.
    sigma: Vec<Vec<f64>>,
    /// Per edge: likelihood of the actor's actions up to and including it, per class.
    pi: Vec<Vec<f64>>,
    /// Per edge: the same seen by another player holding each class.
    seen: Vec<Vec<f64>>,
}

/// Values one observer records in a tree walk.
struct Records {
    /// Per own node: on-policy value of each action (without the prefix), per class.
    child: Vec<Option<Vec<Vec<f64>>>>,
    /// Per own node: the opponents' reach given the observer's class.
    prefix: Vec<Option<Vec<f64>>>,
    /// Per terminal: the opponents' reach (for terminals the observer is in).
    term_prefix: Vec<Option<Vec<f64>>>,
}

/// What a tree walk for one observer reads.
#[derive(Clone, Copy)]
struct WalkCtx<'a> {
    observer: usize,
    prof: &'a Profile,
    /// Action probabilities of the other seats' nodes given the observer's class.
    view: &'a [Vec<f64>],
    /// Terminal values for the observer.
    tv: &'a [Vec<f64>],
}

struct Checked {
    sigma: Vec<Vec<f64>>,
    ev: Vec<f64>,
    gain: Vec<f64>,
    records: Vec<Records>,
}

struct Solver<'p> {
    p: &'p Params,
    tree: Tree,
    prior: Vec<f64>,
    cond: Vec<f64>,
    tables: Vec<ShareTable>,
    /// Per terminal, per involved position: index into `tables`.
    term_tables: Vec<Vec<usize>>,
}

impl<'p> Solver<'p> {
    fn new(p: &'p Params) -> Solver<'p> {
        let tree = Tree::build(p);
        let prior: Vec<f64> = HandClass::all().map(|c| c.combo_count() as f64 / 1326.0).collect();
        let cond = conditional(p.card_removal);
        let r = &p.realization;
        let mut keys: Vec<TableKey> = Vec::new();
        let mut index: HashMap<TableKey, usize> = HashMap::new();
        let mut term_tables = Vec::with_capacity(tree.terms.len());
        for t in &tree.terms {
            let k = t.involved.len();
            let mut ids = Vec::new();
            if k >= 2 {
                let f = r.depth_factor(t.spr);
                let w: Vec<f64> = t
                    .involved
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| r.position.powi(i as i32) * if t.aggressor == Some(s) { r.initiative } else { 1.0 })
                    .collect();
                for m in 0..k {
                    let key = match (t.outcome, k) {
                        (Outcome::Showdown, _) => TableKey::Equity,
                        (_, 2) => TableKey::Two((w[m] / w[1 - m]).to_bits(), f.to_bits()),
                        _ => TableKey::Three([w[0].to_bits(), w[1].to_bits(), w[2].to_bits()], f.to_bits(), m),
                    };
                    let id = *index.entry(key.clone()).or_insert_with(|| {
                        keys.push(key);
                        keys.len() - 1
                    });
                    ids.push(id);
                }
            }
            term_tables.push(ids);
        }
        let tables = keys.iter().map(|k| build_table(k, r, &cond, p.multiway_samples)).collect();
        Solver { p, tree, prior, cond, tables, term_tables }
    }

    fn matvec(&self, w: &[f64]) -> Vec<f64> {
        (0..C)
            .map(|c| self.cond[c * C..][..C].iter().zip(w).map(|(a, b)| a * b).sum())
            .collect()
    }

    /// Likelihoods of every edge for `sigma`.
    fn profile(&self, sigma: Vec<Vec<f64>>) -> Profile {
        let mut prof = Profile { sigma, pi: vec![Vec::new(); self.tree.edges.len()], seen: vec![Vec::new(); self.tree.edges.len()] };
        self.update_edges(&mut prof, None);
        prof
    }

    /// Recomputes the edges of `seat` (or all).
    fn update_edges(&self, prof: &mut Profile, seat: Option<usize>) {
        let ones = vec![1.0; C];
        let mut changed = Vec::new();
        for (v, node) in self.tree.nodes.iter().enumerate() {
            if seat.is_some_and(|s| s != node.seat) {
                continue;
            }
            for a in 0..node.actions.len() {
                let e = node.edge0 + a;
                let before = node.last[node.seat].map_or(&ones, |u| &prof.pi[u]);
                let s = &prof.sigma[v][a * C..][..C];
                prof.pi[e] = before.iter().zip(s).map(|(b, s)| b * s).collect();
                changed.push(e);
            }
        }
        let seen: Vec<Vec<f64>> = changed.par_iter().map(|&e| self.matvec(&prof.pi[e])).collect();
        for (e, s) in changed.into_iter().zip(seen) {
            prof.seen[e] = s;
        }
    }

    /// Class weights of `seat` given every action in `last` (each seat's latest edge),
    /// for an observer at `observer`: its own actions exactly, the others' through the
    /// pairwise chain. Card removal against the observer comes from `cond`.
    fn belief(&self, prof: &Profile, last: &[Option<usize>], seat: usize, observer: usize) -> Vec<f64> {
        let mut b = match last[seat] {
            Some(e) => prof.pi[e].clone(),
            None => vec![1.0; C],
        };
        for (t, e) in last.iter().enumerate() {
            if t == seat || t == observer {
                continue;
            }
            if let Some(e) = e {
                for (x, s) in b.iter_mut().zip(&prof.seen[*e]) {
                    *x *= s;
                }
            }
        }
        b
    }

    /// Per node of another seat: probability of each action given the observer's
    /// class, `[a * C + c]`.
    fn view(&self, observer: usize, prof: &Profile) -> Vec<Vec<f64>> {
        self.tree
            .nodes
            .par_iter()
            .enumerate()
            .map(|(v, node)| {
                if node.seat == observer {
                    return Vec::new();
                }
                let beta = self.belief(prof, &node.last, node.seat, observer);
                let k = node.actions.len();
                let mut out = vec![0.0; k * C];
                for a in 0..k {
                    let s = &prof.sigma[v][a * C..][..C];
                    let w: Vec<f64> = beta.iter().zip(s).map(|(b, s)| b * s).collect();
                    out[a * C..][..C].copy_from_slice(&self.matvec(&w));
                }
                for c in 0..C {
                    let den: f64 = (0..k).map(|a| out[a * C + c]).sum();
                    for a in 0..k {
                        out[a * C + c] = if den > 0.0 { out[a * C + c] / den } else { 1.0 / k as f64 };
                    }
                }
                out
            })
            .collect()
    }

    /// Value of terminal `z` for `observer` per class, given it is reached.
    fn term_value(&self, z: usize, observer: usize, prof: &Profile) -> Vec<f64> {
        let t = &self.tree.terms[z];
        let base = t.base[observer];
        let Some(m) = t.involved.iter().position(|&s| s == observer).filter(|_| t.involved.len() >= 2) else {
            return vec![base; C];
        };
        let others: Vec<usize> = t.involved.iter().copied().filter(|&s| s != observer).collect();
        let betas: Vec<Vec<f64>> = others.iter().map(|&s| self.belief(prof, &t.last, s, observer)).collect();
        let norms: Vec<Vec<f64>> = betas.iter().map(|b| self.matvec(b)).collect();
        let b32: Vec<Vec<f32>> = betas.iter().map(|b| b.iter().map(|&x| x as f32).collect()).collect();
        let g = &self.tables[self.term_tables[z][m]].0;
        let value = |c: usize| -> f64 {
            let den: f64 = norms.iter().map(|n| n[c]).product();
            if den <= 0.0 {
                return base;
            }
            let num = if others.len() == 1 {
                dot32(&g[c * C..][..C], &b32[0])
            } else {
                let gc = &g[c * C * C..][..C * C];
                (0..C)
                    .filter(|&x| b32[0][x] != 0.0)
                    .map(|x| b32[0][x] as f64 * dot32(&gc[x * C..][..C], &b32[1]))
                    .sum()
            };
            base + t.pot * num / den
        };
        if others.len() == 1 { (0..C).map(value).collect() } else { (0..C).into_par_iter().map(value).collect() }
    }

    /// Returns (on-policy value, best-response value) per class below `child`.
    fn walk(&self, ctx: &WalkCtx, child: Child, prefix: &[f64], rec: &mut Records) -> (Vec<f64>, Vec<f64>) {
        let WalkCtx { observer, prof, view, tv } = *ctx;
        let v = match child {
            Child::Term(z) => {
                if self.tree.terms[z].involved.contains(&observer) {
                    rec.term_prefix[z] = Some(prefix.to_vec());
                }
                return (tv[z].clone(), tv[z].clone());
            }
            Child::Node(v) => v,
        };
        let node = &self.tree.nodes[v];
        let k = node.actions.len();
        if node.seat == observer {
            let kids: Vec<(Vec<f64>, Vec<f64>)> = node
                .children
                .iter()
                .map(|&ch| self.walk(ctx, ch, prefix, rec))
                .collect();
            let s = &prof.sigma[v];
            let val = (0..C).map(|c| (0..k).map(|a| s[a * C + c] * kids[a].0[c]).sum()).collect();
            let br = (0..C).map(|c| kids.iter().map(|kid| kid.1[c]).fold(f64::NEG_INFINITY, f64::max)).collect();
            rec.child[v] = Some(kids.into_iter().map(|kid| kid.0).collect());
            rec.prefix[v] = Some(prefix.to_vec());
            return (val, br);
        }
        let probs = &view[v];
        let (mut val, mut br) = (vec![0.0; C], vec![0.0; C]);
        for (a, &ch) in node.children.iter().enumerate() {
            let pa = &probs[a * C..][..C];
            if pa.iter().zip(prefix).all(|(x, p)| x * p == 0.0) {
                continue;
            }
            let next: Vec<f64> = prefix.iter().zip(pa).map(|(p, x)| p * x).collect();
            let (kv, kb) = self.walk(ctx, ch, &next, rec);
            for c in 0..C {
                val[c] += pa[c] * kv[c];
                br[c] += pa[c] * kb[c];
            }
        }
        (val, br)
    }

    /// One observer's values: (EV, best-response EV, records).
    fn pass(&self, observer: usize, prof: &Profile) -> (f64, f64, Records) {
        let view = self.view(observer, prof);
        let tv: Vec<Vec<f64>> = (0..self.tree.terms.len())
            .into_par_iter()
            .map(|z| self.term_value(z, observer, prof))
            .collect();
        let mut rec = Records {
            child: vec![None; self.tree.nodes.len()],
            prefix: vec![None; self.tree.nodes.len()],
            term_prefix: vec![None; self.tree.terms.len()],
        };
        let ctx = WalkCtx { observer, prof, view: &view, tv: &tv };
        let (val, br) = self.walk(&ctx, self.tree.root, &[1.0; C], &mut rec);
        let ev = val.iter().zip(&self.prior).map(|(v, p)| v * p).sum();
        let best = br.iter().zip(&self.prior).map(|(v, p)| v * p).sum();
        (ev, best, rec)
    }

    fn evaluate(&self, sigma: Vec<Vec<f64>>) -> Checked {
        let prof = self.profile(sigma);
        let (mut ev, mut gain, mut records) = (Vec::new(), Vec::new(), Vec::new());
        for o in 0..self.tree.n {
            let (e, b, r) = self.pass(o, &prof);
            ev.push(e);
            gain.push((b - e).max(0.0));
            records.push(r);
        }
        Checked { sigma: prof.sigma, ev, gain, records }
    }

    /// Own reach of the actor at `v` per class.
    fn own_reach<'a>(&self, prof: &'a Profile, v: usize, ones: &'a [f64]) -> &'a [f64] {
        let node = &self.tree.nodes[v];
        node.last[node.seat].map_or(ones, |e| &prof.pi[e])
    }

    fn run(&mut self) -> Solution {
        let n = self.tree.n;
        let target = self.p.target_exploitability;
        let sigma: Vec<Vec<f64>> = self.tree.nodes.iter().map(|node| vec![1.0 / node.actions.len() as f64; node.actions.len() * C]).collect();
        let mut prof = self.profile(sigma);
        let mut regret: Vec<Vec<f64>> = prof.sigma.iter().map(|s| vec![0.0; s.len()]).collect();
        let mut sum: Vec<Vec<f64>> = regret.clone();
        let mut norm: Vec<Vec<f64>> = self.tree.nodes.iter().map(|_| vec![0.0; C]).collect();
        let ones = vec![1.0; C];
        let (mut iterations, mut converged) = (0, false);
        let mut best: Option<Checked> = None;
        while iterations < self.p.max_iterations && !converged {
            iterations += 1;
            let t = iterations as f64;
            for o in 0..n {
                let (_, _, rec) = self.pass(o, &prof);
                for &v in &self.tree.nodes_of[o] {
                    let (Some(kids), Some(pre)) = (&rec.child[v], &rec.prefix[v]) else { continue };
                    let k = self.tree.nodes[v].actions.len();
                    let own = self.own_reach(&prof, v, &ones).to_vec();
                    let (s, r) = (&mut prof.sigma[v], &mut regret[v]);
                    for c in 0..C {
                        let val: f64 = (0..k).map(|a| s[a * C + c] * kids[a][c]).sum();
                        let mut total = 0.0;
                        for a in 0..k {
                            let x = &mut r[a * C + c];
                            *x = (*x + pre[c] * (kids[a][c] - val)).max(0.0);
                            total += *x;
                        }
                        // With no positive regret any strategy keeps the RM+ bound.
                        if total > 0.0 {
                            for a in 0..k {
                                s[a * C + c] = r[a * C + c] / total;
                            }
                        }
                        for a in 0..k {
                            sum[v][a * C + c] += t * own[c] * s[a * C + c];
                        }
                        norm[v][c] += t * own[c];
                    }
                }
                self.update_edges(&mut prof, Some(o));
            }
            if iterations % CHECK_EVERY == 0 || iterations == self.p.max_iterations {
                // With more than two players the average need not approach an
                // equilibrium even when the current strategies do, so both are measured.
                let avg = self.average(&sum, &norm, &prof.sigma);
                for candidate in [avg, self.snap(prof.sigma.clone())] {
                    let checked = self.evaluate(candidate);
                    let total: f64 = checked.gain.iter().sum();
                    converged |= total < target;
                    if best.as_ref().is_none_or(|b| total < b.gain.iter().sum::<f64>()) {
                        best = Some(checked);
                    }
                }
            }
        }
        let checked = best.expect("the last iteration is always checked");
        self.solution(checked, iterations, converged)
    }

    /// Reach-weighted linear average; classes that never reached a node keep the
    /// current strategy there.
    fn average(&self, sum: &[Vec<f64>], norm: &[Vec<f64>], current: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let avg = (0..self.tree.nodes.len())
            .map(|v| {
                let k = self.tree.nodes[v].actions.len();
                (0..k * C)
                    .map(|i| {
                        let c = i % C;
                        if norm[v][c] > 0.0 { sum[v][i] / norm[v][c] } else { current[v][i] }
                    })
                    .collect()
            })
            .collect();
        self.snap(avg)
    }

    /// Frequencies below [`SNAP`] dropped and the rest renormalised.
    fn snap(&self, mut sigma: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
        for (v, s) in sigma.iter_mut().enumerate() {
            let k = self.tree.nodes[v].actions.len();
            for c in 0..C {
                let mut total = 0.0;
                for a in 0..k {
                    let x = &mut s[a * C + c];
                    if *x < SNAP {
                        *x = 0.0;
                    }
                    total += *x;
                }
                for a in 0..k {
                    s[a * C + c] = if total > 0.0 { s[a * C + c] / total } else { 1.0 / k as f64 };
                }
            }
        }
        sigma
    }

    fn solution(&self, checked: Checked, iterations: u32, converged: bool) -> Solution {
        let prof = self.profile(checked.sigma);
        let ones = vec![1.0; C];
        let nodes = self
            .tree
            .nodes
            .iter()
            .enumerate()
            .map(|(v, node)| {
                let rec = &checked.records[node.seat];
                let k = node.actions.len();
                let own = self.own_reach(&prof, v, &ones);
                let (ev, reach) = match (&rec.child[v], &rec.prefix[v]) {
                    (Some(kids), Some(pre)) => (kids.clone(), (0..C).map(|c| self.prior[c] * own[c] * pre[c]).sum()),
                    _ => (vec![vec![0.0; C]; k], 0.0),
                };
                Node {
                    seat: node.seat,
                    line: node.line.clone(),
                    actions: node.actions.clone(),
                    reach,
                    freq: (0..k).map(|a| prof.sigma[v][a * C..][..C].to_vec()).collect(),
                    ev,
                }
            })
            .collect();
        let realized = self.realized(&prof, &checked.records);
        let exploitability = checked.gain.iter().sum();
        Solution {
            players: self.tree.n,
            stack_bb: self.p.stack_bb,
            nodes,
            ev_sum_error: checked.ev.iter().sum(),
            ev: checked.ev,
            gain: checked.gain,
            exploitability,
            iterations,
            converged,
            realized,
        }
    }

    /// Range-level equity realization of the most frequent heads-up flops.
    fn realized(&self, prof: &Profile, records: &[Records]) -> Vec<Realized> {
        let r = &self.p.realization;
        let m = PreflopMatrix::embedded();
        let ones = vec![1.0; C];
        let mut out: Vec<Realized> = self
            .tree
            .terms
            .iter()
            .enumerate()
            .filter(|(_, t)| t.outcome == Outcome::Postflop && t.involved.len() == 2)
            .filter_map(|(z, t)| {
                let (a, b) = (t.involved[0], t.involved[1]);
                let pre = records[a].term_prefix[z].as_ref()?;
                let own = t.last[a].map_or(&ones, |e| &prof.pi[e]);
                let reach: f64 = (0..C).map(|c| self.prior[c] * own[c] * pre[c]).sum();
                let pa = t.last[a].map_or(&ones, |e| &prof.pi[e]);
                let pb = t.last[b].map_or(&ones, |e| &prof.pi[e]);
                let f = r.depth_factor(t.spr);
                let w = |s: usize, i: i32| r.position.powi(i) * if t.aggressor == Some(s) { r.initiative } else { 1.0 };
                let ratio = w(a, 0) / w(b, 1);
                let (mut total, mut eq, mut share) = (0.0, 0.0, 0.0);
                for x in HandClass::all() {
                    if pa[x.index()] == 0.0 {
                        continue;
                    }
                    for y in HandClass::all() {
                        let wt = class_pair_count(x, y) as f64 * pa[x.index()] * pb[y.index()];
                        if wt == 0.0 {
                            continue;
                        }
                        let e = m.equity(x, y);
                        total += wt;
                        eq += wt * e;
                        share += wt * share_two(r, e, x, y, ratio, f);
                    }
                }
                if total == 0.0 {
                    return None;
                }
                let (eq, share) = (eq / total, share / total);
                Some(Realized {
                    line: t.line.clone(),
                    seats: vec![a, b],
                    reach,
                    pot_bb: t.pot,
                    spr: t.spr,
                    equity: vec![eq, 1.0 - eq],
                    realization: vec![share / eq, (1.0 - share) / (1.0 - eq)],
                })
            })
            .collect();
        out.sort_by(|x, y| y.reach.total_cmp(&x.reach));
        out.truncate(16);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_shape() {
        let p = Params::new(9, 100.0);
        let t = Tree::build(&p);
        println!("9 players, 100 BB: {} nodes, {} terminals", t.nodes.len(), t.terms.len());
        // At most three players see a flop, all-in pots are heads-up.
        assert!(t.terms.iter().all(|z| z.involved.len() <= 3));
        assert!(t.terms.iter().filter(|z| z.outcome == Outcome::Showdown).all(|z| z.involved.len() == 2));
        // Every terminal hands out exactly what was put in.
        for z in &t.terms {
            let k = z.involved.len().max(1) as f64;
            let total: f64 = z.base.iter().sum::<f64>() + if z.involved.len() >= 2 { z.pot } else { 0.0 };
            assert!(total.abs() < 1e-9, "{total} over {k} players");
        }
        // Short stacks drop the sized raises once they commit the stack.
        let short = Tree::build(&Params::new(6, 4.0));
        assert!(short.nodes.iter().flat_map(|n| &n.actions).all(|a| a.kind != Kind::Raise || a.to_bb < 1.6));
    }

    #[test]
    fn playability_order() {
        let r = Realization::default();
        let q = |s: &str| r.playability(s.parse().unwrap());
        assert!(q("T9s") > q("T7s") && q("T7s") > q("T9o") && q("T9o") > q("T7o"));
        assert_eq!(r.depth_factor(0.0), 0.0);
        assert!(r.depth_factor(17.0) > 0.99);
    }
}

