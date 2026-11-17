//! Push/fold Nash equilibrium for one table of 2–9 players, in chip EV or ICM.
//!
//! # Game
//! Seats are given in preflop action order: the first player after the big blind,
//! …, small blind, big blind (heads-up the small blind is first). While everyone
//! before has folded, a player shoves or folds. After the first shove every later
//! player calls or folds, so a pot can have several callers, up to
//! [`Params::max_all_in`] players in all (3 by default, as in HoldemResources and
//! ICMIZER); once that many are in, everyone left folds. A call puts the caller's
//! whole stack in: nobody keeps chips behind with a decision left, every pot is
//! checked down and side pots settle exactly. There are no limps and no raises
//! below all-in; the big blind only acts after a shove.
//!
//! Antes and the big-blind ante are dead money for the main pot, blinds are live.
//! Results are chip EV (BB won or lost against the starting stack) or, with
//! payouts, Malmuth–Harville ICM equity of the final stacks.
//!
//! # Model
//! Strategies are per hand class. Card removal is a chain of pairwise terms: every
//! player evaluates its options with each other player's class weighted by the exact
//! two-class removal against its own class and by how likely that class makes every
//! earlier action (a fold makes weak classes likelier, a shove strong ones). Heads-up
//! this is the exact deal distribution, so two players reproduce
//! [`solve_push_fold`](crate::solve_push_fold). With three or more players it is an
//! **approximation**: the removal between two other players only enters through
//! those action likelihoods, and a player outside a showdown values it without its
//! own cards. [`Approximation::ev_sum_error`] shows the resulting inconsistency.
//!
//! Showdown outcomes:
//! - two players: exact, from the 169×169 equity and tie tables;
//! - three or more: **Monte Carlo**, `multiway_samples` deals per sorted class tuple
//!   with exact card removal inside the tuple and a seed derived from the tuple, so
//!   results are deterministic. Triples are kept for the whole process (21 MB per
//!   sample count). [`Approximation`] in the result says whether such pots were
//!   reachable and how likely they are.
//!
//! # Solving
//! CFR+ (regret matching+ with alternating updates and linear averaging) on the full
//! tree, warm-started from the same game capped at heads-up pots. A class keeps its
//! strategy while it has no positive regret, so the played ranges stay narrow;
//! multiway showdown tables are only built over them. Every few iterations the
//! average profile is checked with an exact best response on the same tree and
//! model; the solve stops once the summed gains of all players' best responses
//! (NashConv) are below `target_exploitability`. With three or more players CFR has
//! no convergence guarantee, so [`Solution::converged`] reports what was reached and
//! the best checked profile is returned.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{Arc, LazyLock, Mutex, RwLock};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cards::Card;
use crate::eval::Hand;
use crate::icm::icm;
use crate::preflop_matrix::{PreflopMatrix, TieMatrix};
use crate::range::{HandClass, NUM_CLASSES, class_pair_count};

pub const MAX_PLAYERS: usize = 9;
const C: usize = NUM_CLASSES;
const CHECK_EVERY: u32 = 8;
/// Frequencies this close to 0 or 1 in the average profile are reported as pure.
const SNAP: f64 = 1e-3;
/// Warm start: classes at least this strong against a random hand shove or call.
const START_SHOVE_EQUITY: f64 = 0.58;
const START_CALL_EQUITY: f64 = 0.64;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Params {
    /// Stack per seat in BB before posting, in preflop action order (see module docs).
    pub stacks_bb: Vec<f64>,
    pub small_blind_bb: f64,
    /// Ante paid by every player.
    #[serde(default)]
    pub ante_bb: f64,
    /// Ante paid by the big blind alone.
    #[serde(default)]
    pub bb_ante_bb: f64,
    /// Extra dead money in the pot.
    #[serde(default)]
    pub dead_bb: f64,
    /// Prize per finishing place, first place first. Empty means chip EV.
    #[serde(default)]
    pub payouts: Vec<f64>,
    /// `false` treats all players' classes as independent (for measuring the effect).
    #[serde(default = "default_card_removal")]
    pub card_removal: bool,
    /// Stop once NashConv is below this, in result units (BB, or prize units with ICM).
    pub target_exploitability: f64,
    pub max_iterations: u32,
    /// Monte Carlo deals per class tuple for showdowns of three or more players.
    #[serde(default = "default_samples")]
    pub multiway_samples: u32,
    /// Most players all in at once (2..=9); once that many are in, everyone left folds.
    /// 3 is the standard of HoldemResources and ICMIZER. Each extra player multiplies
    /// the showdown tables by a range size: four players at 10 BB take about two
    /// minutes instead of under two seconds, short stacks with antes far longer.
    /// [`Approximation::capped_probability`] shows how often the cap binds.
    #[serde(default = "default_max_all_in")]
    pub max_all_in: usize,
}

fn default_max_all_in() -> usize {
    3
}

fn default_card_removal() -> bool {
    true
}

fn default_samples() -> u32 {
    1024
}

impl Params {
    /// Chip EV, small blind 0.5 BB, no antes.
    pub fn new(stacks_bb: Vec<f64>) -> Params {
        Params {
            stacks_bb,
            small_blind_bb: 0.5,
            ante_bb: 0.0,
            bb_ante_bb: 0.0,
            dead_bb: 0.0,
            payouts: Vec::new(),
            card_removal: true,
            target_exploitability: 1e-3,
            max_iterations: 5_000,
            multiway_samples: default_samples(),
            max_all_in: default_max_all_in(),
        }
    }

    pub fn equal(players: usize, stack_bb: f64) -> Params {
        Params::new(vec![stack_bb; players])
    }

    /// ICM with these payouts; the target scales with the prize pool.
    pub fn with_payouts(mut self, payouts: Vec<f64>) -> Params {
        self.target_exploitability = 1e-5 * payouts.iter().sum::<f64>();
        self.payouts = payouts;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Solution {
    pub players: usize,
    /// EVs are ICM prize equity; otherwise BB won or lost against the starting stack.
    pub icm: bool,
    /// Every decision node, open shoves first (see [`Solution::node`]).
    pub nodes: Vec<Node>,
    /// Expected result per seat.
    pub ev: Vec<f64>,
    /// What each seat could gain per hand with a best response.
    pub gain: Vec<f64>,
    /// NashConv: the sum of `gain`.
    pub exploitability: f64,
    pub iterations: u32,
    pub converged: bool,
    pub approximation: Approximation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub seat: usize,
    /// `None` for an open node (everyone before folded), else the first shover.
    pub shover: Option<usize>,
    /// Seats that called the shove before `seat`, in order.
    pub callers: Vec<usize>,
    /// Probability that a hand reaches this node.
    pub reach: f64,
    /// Shove (open node) or call frequency per class, indexed by `HandClass::index`.
    pub act: Vec<f64>,
    /// Expected result of shoving/calling per class, given the node is reached.
    pub ev_act: Vec<f64>,
    /// Expected result of folding per class, given the node is reached.
    pub ev_fold: Vec<f64>,
}

/// What in the result is an approximation; see the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Approximation {
    /// Some all-in of three or more players has positive probability, so EVs contain
    /// Monte Carlo showdown values.
    pub multiway_monte_carlo: bool,
    pub multiway_samples: u32,
    /// Probability that a hand ends in an all-in of three or more players, from the
    /// product of the players' action frequencies.
    pub multiway_probability: f64,
    /// Probability that `max_all_in` players are in while someone is still to act and
    /// has to fold: how much the cap on pot size shapes this game.
    pub capped_probability: f64,
    /// Card removal between pairs of players (else none at all).
    pub pairwise_card_removal: bool,
    /// Σ EV over seats minus what the game conserves (0 chips; the prize pool with
    /// ICM): the inconsistency of the per-player card-removal model.
    pub ev_sum_error: f64,
}

/// One class at one node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    pub node: usize,
    pub freq: f64,
    pub ev_act: f64,
    pub ev_fold: f64,
    pub reach: f64,
}

impl Solution {
    /// The node where `seat` acts after `shover` (None: everyone folded to it) and
    /// `callers` (in seat order).
    pub fn node(&self, seat: usize, shover: Option<usize>, callers: &[usize]) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.seat == seat && n.shover == shover && n.callers == callers)
    }

    pub fn spot(
        &self,
        seat: usize,
        class: HandClass,
        shover: Option<usize>,
        callers: &[usize],
    ) -> Option<Spot> {
        let i = self.node(seat, shover, callers)?;
        let (n, c) = (&self.nodes[i], class.index());
        Some(Spot {
            node: i,
            freq: n.act[c],
            ev_act: n.ev_act[c],
            ev_fold: n.ev_fold[c],
            reach: n.reach,
        })
    }

    /// Share of all combos played at a node, in percent.
    pub fn act_pct(&self, node: usize) -> f64 {
        100.0
            * HandClass::all()
                .map(|c| self.nodes[node].act[c.index()] * c.combo_count() as f64 / 1326.0)
                .sum::<f64>()
    }
}

/// Table position name of `seat` in a game of `players` (action order).
pub fn seat_name(players: usize, seat: usize) -> &'static str {
    const FROM_BUTTON: [&str; 7] = ["BTN", "CO", "HJ", "LJ", "UTG+2", "UTG+1", "UTG"];
    if seat + 1 == players {
        "BB"
    } else if seat + 2 == players {
        "SB"
    } else {
        let names = &FROM_BUTTON[..players - 2];
        names[players - 3 - seat]
    }
}

pub fn solve(params: &Params) -> Result<Solution, Error> {
    let money = Money::new(params)?;
    let mut solver = Solver::new(params, money);
    Ok(solver.run())
}

/// Measures someone else's strategy: `act[v][c]` is the shove or call frequency at
/// node `v` (in the order of [`Solution::nodes`]) for class `c`. One best-response pass
/// per seat, so checking a downloaded solution costs a fraction of solving it.
/// `iterations` is 0 and `converged` says whether it is within the target.
pub fn evaluate(params: &Params, act: &[Vec<f64>]) -> Result<Solution, Error> {
    let money = Money::new(params)?;
    let mut solver = Solver::new(params, money);
    let bad = |m: String| Err(Error::InvalidInput(m));
    if act.len() != solver.tree.nodes.len() {
        return bad(format!("{} nodes given, the tree has {}", act.len(), solver.tree.nodes.len()));
    }
    if let Some(v) = act.iter().position(|a| a.len() != C || a.iter().any(|x| !(0.0..=1.0).contains(x))) {
        return bad(format!("node {v}: needs {C} frequencies between 0 and 1"));
    }
    let checked = solver.evaluate(solver.profile(act.to_vec()));
    let converged = checked.gain.iter().sum::<f64>() < params.target_exploitability;
    Ok(solver.solution(checked, 0, converged))
}

// ---------------------------------------------------------------------------------
// Money: posting, side pots, utilities.

struct Money {
    n: usize,
    stack: Vec<f64>,
    /// Chips left after antes, all of which go in on a shove or call.
    live: Vec<f64>,
    blind: Vec<f64>,
    dead: f64,
    payouts: Vec<f64>,
}

impl Money {
    fn new(p: &Params) -> Result<Money, Error> {
        let n = p.stacks_bb.len();
        let bad = |m: &str| Err(Error::InvalidInput(m.into()));
        if !(2..=MAX_PLAYERS).contains(&n) {
            return bad("push/fold needs 2 to 9 players");
        }
        let numbers = [
            p.small_blind_bb,
            p.ante_bb,
            p.bb_ante_bb,
            p.dead_bb,
            p.target_exploitability,
        ];
        if numbers.iter().chain(&p.stacks_bb).chain(&p.payouts).any(|x| !x.is_finite()) {
            return bad("all numbers must be finite");
        }
        if p.small_blind_bb <= 0.0 || p.small_blind_bb > 1.0 {
            return bad("small blind must be in (0, 1] BB");
        }
        if p.ante_bb < 0.0 || p.bb_ante_bb < 0.0 || p.dead_bb < 0.0 {
            return bad("antes and dead money must not be negative");
        }
        if p.payouts.len() > n || p.payouts.iter().any(|x| *x < 0.0) {
            return bad("need at most one non-negative payout per player");
        }
        if p.max_iterations == 0 {
            return bad("max_iterations must be positive");
        }
        if p.max_all_in < 2 {
            return bad("max_all_in must be at least 2");
        }
        if p.multiway_samples == 0 || p.multiway_samples > u16::MAX as u32 {
            return bad("multiway_samples must be in 1..=65535");
        }
        let mut blind = vec![0.0; n];
        blind[n - 2] = p.small_blind_bb;
        blind[n - 1] = 1.0;
        let mut live = Vec::with_capacity(n);
        let mut dead = p.dead_bb;
        for (s, &stack) in p.stacks_bb.iter().enumerate() {
            let ante = p.ante_bb + if s == n - 1 { p.bb_ante_bb } else { 0.0 };
            if stack - ante < blind[s] || stack - ante <= 0.0 {
                return bad("every stack must cover its antes and blind");
            }
            dead += ante;
            live.push(stack - ante);
        }
        Ok(Money {
            n,
            stack: p.stacks_bb.clone(),
            live,
            blind,
            dead,
            payouts: p.payouts.clone(),
        })
    }

    fn icm(&self) -> bool {
        !self.payouts.is_empty()
    }

    /// Final stacks when `involved` (seat order) are all in with showdown strengths
    /// `levels` (higher wins), every other seat folded and only lost its blind.
    fn settle(&self, involved: &[usize], levels: &[u8]) -> Vec<f64> {
        let n = self.n;
        let contrib: Vec<f64> = (0..n)
            .map(|s| {
                if involved.contains(&s) {
                    self.live[s]
                } else {
                    self.blind[s]
                }
            })
            .collect();
        let mut fin: Vec<f64> = (0..n).map(|s| self.live[s] - contrib[s]).collect();
        if involved.is_empty() {
            fin[n - 1] += self.dead + contrib.iter().sum::<f64>();
            return fin;
        }
        let mut tops: Vec<f64> = involved.iter().map(|&s| contrib[s]).collect();
        tops.sort_by(f64::total_cmp);
        tops.dedup();
        let (mut prev, mut main_winners) = (0.0, Vec::new());
        for (layer, &top) in tops.iter().enumerate() {
            let mut amount: f64 = contrib.iter().map(|&x| x.min(top) - x.min(prev)).sum();
            let eligible = involved.iter().enumerate().filter(|(_, s)| contrib[**s] >= top);
            let best = eligible.clone().map(|(m, _)| levels[m]).max().unwrap_or(0);
            let winners: Vec<usize> = eligible.filter(|(m, _)| levels[*m] == best).map(|(_, s)| *s).collect();
            if layer == 0 {
                amount += self.dead;
                main_winners = winners.clone();
            }
            for &w in &winners {
                fin[w] += amount / winners.len() as f64;
            }
            prev = top;
        }
        let orphan: f64 = contrib.iter().map(|&x| (x - prev).max(0.0)).sum();
        for &w in &main_winners {
            fin[w] += orphan / main_winners.len() as f64;
        }
        fin
    }

    fn utilities(&self, involved: &[usize], levels: &[u8]) -> Vec<f64> {
        let fin = self.settle(involved, levels);
        if self.icm() {
            icm(&fin, &self.payouts).expect("validated stacks and payouts").ev
        } else {
            fin.iter().zip(&self.stack).map(|(f, s)| f - s).collect()
        }
    }

    /// What the game conserves: 0 chips, or the prize pool.
    fn total(&self) -> f64 {
        if self.icm() {
            self.payouts.iter().sum::<f64>()
        } else {
            0.0
        }
    }
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
    shover: Option<usize>,
    callers: Vec<usize>,
    fold: Child,
    act: Child,
    /// Action nodes of the players already all in when this node is reached.
    involved_above: Vec<usize>,
    /// Every earlier decision on the way here: (node, acted).
    history: Vec<(usize, bool)>,
}

struct Terminal {
    /// Seats all in, in seat order.
    involved: Vec<usize>,
    /// Node at which each involved seat went all in.
    act_nodes: Vec<usize>,
}

struct Tree {
    /// Most players all in at once; later players fold.
    cap: usize,
    nodes: Vec<TreeNode>,
    terms: Vec<Terminal>,
    root: Child,
    nodes_of: Vec<Vec<usize>>,
}

impl Tree {
    fn build(n: usize, cap: usize) -> Tree {
        let mut t = Tree {
            cap,
            nodes: Vec::new(),
            terms: Vec::new(),
            root: Child::Term(0),
            nodes_of: vec![Vec::new(); n],
        };
        let (mut path, mut hist) = (Vec::new(), Vec::new());
        // Open nodes first so that node i < n - 1 is seat i's open node.
        for s in 0..n - 1 {
            let history = (0..s).map(|u| (u, false)).collect();
            t.push_node(s, None, Vec::new(), Vec::new(), history);
        }
        t.root = t.open(0, n, &mut path, &mut hist);
        t
    }

    fn push_node(
        &mut self,
        seat: usize,
        shover: Option<usize>,
        callers: Vec<usize>,
        above: Vec<usize>,
        history: Vec<(usize, bool)>,
    ) -> usize {
        self.nodes.push(TreeNode {
            seat,
            shover,
            callers,
            fold: Child::Term(0),
            act: Child::Term(0),
            involved_above: above,
            history,
        });
        self.nodes_of[seat].push(self.nodes.len() - 1);
        self.nodes.len() - 1
    }

    fn term(&mut self, path: &[(usize, usize)]) -> Child {
        self.terms.push(Terminal {
            involved: path.iter().map(|p| p.0).collect(),
            act_nodes: path.iter().map(|p| p.1).collect(),
        });
        Child::Term(self.terms.len() - 1)
    }

    /// `path`: (seat, node) of the players all in so far; `hist`: (node, acted) of
    /// every decision so far.
    fn open(&mut self, seat: usize, n: usize, path: &mut Vec<(usize, usize)>, hist: &mut Vec<(usize, bool)>) -> Child {
        if seat == n - 1 {
            return self.term(path);
        }
        let id = seat;
        hist.push((id, false));
        let fold = self.open(seat + 1, n, path, hist);
        hist.pop();
        path.push((seat, id));
        hist.push((id, true));
        let act = self.call(seat + 1, n, path, hist);
        hist.pop();
        path.pop();
        self.nodes[id].fold = fold;
        self.nodes[id].act = act;
        Child::Node(id)
    }

    fn call(&mut self, seat: usize, n: usize, path: &mut Vec<(usize, usize)>, hist: &mut Vec<(usize, bool)>) -> Child {
        if seat == n || path.len() == self.cap {
            return self.term(path);
        }
        let shover = Some(path[0].0);
        let callers = path[1..].iter().map(|p| p.0).collect();
        let above = path.iter().map(|p| p.1).collect();
        let id = self.push_node(seat, shover, callers, above, hist.clone());
        hist.push((id, false));
        let fold = self.call(seat + 1, n, path, hist);
        hist.pop();
        path.push((seat, id));
        hist.push((id, true));
        let act = self.call(seat + 1, n, path, hist);
        hist.pop();
        path.pop();
        self.nodes[id].fold = fold;
        self.nodes[id].act = act;
        Child::Node(id)
    }
}

// ---------------------------------------------------------------------------------
// Class tables and Monte Carlo showdowns.

/// `cond[c * C + x]`: probability that another player holds class x given class c.
fn conditional(card_removal: bool) -> Vec<f64> {
    let prior: Vec<f64> = HandClass::all().map(|c| c.combo_count() as f64 / 1326.0).collect();
    let mut m = vec![0.0; C * C];
    for c in HandClass::all() {
        for x in HandClass::all() {
            m[c.index() * C + x.index()] = if card_removal {
                class_pair_count(c, x) as f64 / (c.combo_count() as f64 * 1225.0)
            } else {
                prior[x.index()]
            };
        }
    }
    m
}

static CLASS_COMBOS: LazyLock<Vec<Vec<(u64, Hand)>>> = LazyLock::new(|| {
    HandClass::all()
        .map(|c| {
            c.combos()
                .iter()
                .map(|k| {
                    let (a, b) = k.cards();
                    (k.mask().0, Hand::from_cards([a, b]))
                })
                .collect()
        })
        .collect()
});

#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }

    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write_u128(&mut self, x: u128) {
        self.write_u64(x as u64);
        self.write_u64((x >> 64) as u64);
    }
}

type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<KeyHasher>>;

/// Sorted class tuple, one byte per class (+1 so that lengths differ).
fn tuple_key(sorted: &[u8]) -> u128 {
    sorted
        .iter()
        .enumerate()
        .fold(0u128, |k, (i, &c)| k | ((c as u128 + 1) << (8 * i)))
}

/// Strength level codes (4 bits per position, higher wins) with their probability.
type Outcome = Box<[(u64, f32)]>;

fn levels_code(levels: &[u8]) -> u64 {
    levels
        .iter()
        .enumerate()
        .fold(0u64, |k, (i, &l)| k | (l as u64) << (4 * i))
}

fn code_level(code: u64, i: usize) -> u8 {
    (code >> (4 * i) & 15) as u8
}

/// Whether the classes can be dealt to different players at once.
fn feasible(sorted: &[u8]) -> bool {
    fn dfs(classes: &[u8], used: u64) -> bool {
        let Some((&first, rest)) = classes.split_first() else {
            return true;
        };
        CLASS_COMBOS[first as usize]
            .iter()
            .any(|&(m, _)| m & used == 0 && dfs(rest, used | m))
    }
    let mut cards_per_rank = [0u8; 13];
    for &c in sorted {
        let (hi, lo) = HandClass::from_index(c as usize).ranks();
        cards_per_rank[hi as usize] += 1;
        cards_per_rank[lo as usize] += 1;
    }
    cards_per_rank.iter().all(|&n| n <= 4) && dfs(sorted, 0)
}

/// wyrand: the showdown simulation needs speed, not cryptographic quality.
struct Wy(u64);

impl Wy {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0xa076_1d64_78bd_642f);
        let t = self.0 as u128 * (self.0 ^ 0xe703_7ed1_a0b4_28db) as u128;
        (t >> 64) as u64 ^ t as u64
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.next() as u128 * n as u128) >> 64) as usize
    }
}

/// Deals `samples` showdowns of a sorted class tuple, uniformly over disjoint combo
/// tuples and boards, with a seed derived from the tuple, and passes each outcome's
/// level code (sorted-tuple order) to `record`. `false` if the classes can't be
/// dealt together.
fn simulate(sorted: &[u8], samples: u32, mut record: impl FnMut(u64)) -> bool {
    if !feasible(sorted) {
        return false;
    }
    let k = sorted.len();
    let key = tuple_key(sorted);
    let mut rng = Wy(key as u64 ^ ((key >> 64) as u64).rotate_left(32));
    let mut hands = [Hand::EMPTY; MAX_PLAYERS];
    for _ in 0..samples {
        let used = loop {
            let mut used = 0u64;
            let mut ok = true;
            for (h, &c) in hands.iter_mut().zip(sorted) {
                let combos = &CLASS_COMBOS[c as usize];
                let (m, hand) = combos[rng.below(combos.len())];
                ok &= m & used == 0;
                used |= m;
                *h = hand;
            }
            if ok {
                break used;
            }
        };
        let (mut board, mut taken, mut got) = (Hand::EMPTY, used, 0);
        let (mut bits, mut left) = (0u64, 0);
        while got < 5 {
            if left == 0 {
                (bits, left) = (rng.next(), 10);
            }
            let card = (bits & 63) as u8;
            (bits, left) = (bits >> 6, left - 1);
            if card < 52 && taken >> card & 1 == 0 {
                taken |= 1 << card;
                board = board.with(Card::from_index(card));
                got += 1;
            }
        }
        let mut ranks = [0u16; MAX_PLAYERS];
        for (r, h) in ranks.iter_mut().zip(&hands[..k]) {
            *r = h.combine(board).rank().0;
        }
        let mut distinct = ranks;
        let distinct = &mut distinct[..k];
        distinct.sort_unstable();
        let mut m = 0;
        for i in 0..k {
            if i == 0 || distinct[i] != distinct[m - 1] {
                distinct[m] = distinct[i];
                m += 1;
            }
        }
        let mut code = 0u64;
        for (i, r) in ranks[..k].iter().enumerate() {
            let level = distinct[..m].partition_point(|d| d < r) as u64;
            code |= level << (4 * i);
        }
        record(code);
    }
    true
}

/// Outcome distribution of a sorted tuple of four or more classes.
fn simulate_outcome(sorted: &[u8], samples: u32) -> Outcome {
    let mut counts: Vec<(u64, u32)> = Vec::new();
    let feasible = simulate(sorted, samples, |code| match counts.iter_mut().find(|e| e.0 == code) {
        Some(e) => e.1 += 1,
        None => counts.push((code, 1)),
    });
    if !feasible {
        return Box::new([]);
    }
    counts.sort_unstable();
    counts
        .into_iter()
        .map(|(code, n)| (code, n as f32 / samples as f32))
        .collect()
}

/// Number of sorted class triples (multisets of size 3 out of 169).
const TRIPLES: usize = C * (C + 1) * (C + 2) / 6;
/// Weak orders of three players.
const WEAK3: usize = 13;
/// Count row of a triple that can't be dealt (feasible rows sum to the sample count).
const INFEASIBLE: [u16; WEAK3] = [u16::MAX; WEAK3];

type TripleRow = [u16; WEAK3];

/// Level vectors of the 13 weak orders of three players, and their index by
/// `l0 * 9 + l1 * 3 + l2`.
static WEAK3_ORDERS: LazyLock<(Vec<[u8; 3]>, [u8; 27])> = LazyLock::new(|| {
    let mut orders = Vec::new();
    let mut index = [u8::MAX; 27];
    for (i, slot) in index.iter_mut().enumerate() {
        let l = [(i / 9) as u8, (i / 3 % 3) as u8, (i % 3) as u8];
        let top = *l.iter().max().expect("three levels");
        if (0..=top).all(|v| l.contains(&v)) {
            *slot = orders.len() as u8;
            orders.push(l);
        }
    }
    assert_eq!(orders.len(), WEAK3);
    (orders, index)
});

/// Inverse of [`triple_index`].
fn triple_classes(index: usize) -> [u8; 3] {
    let tet = |c: usize| c * (c + 1) * (c + 2) / 6;
    let c = (0..C).rev().find(|&c| tet(c) <= index).expect("index below TRIPLES");
    let rest = index - tet(c);
    let b = (0..=c).rev().find(|&b| b * (b + 1) / 2 <= rest).expect("b ≤ c");
    [(rest - b * (b + 1) / 2) as u8, b as u8, c as u8]
}

fn triple_index(sorted: &[u8]) -> usize {
    let (a, b, c) = (sorted[0] as usize, sorted[1] as usize, sorted[2] as usize);
    c * (c + 1) * (c + 2) / 6 + b * (b + 1) / 2 + a
}

fn simulate_triple(sorted: &[u8], samples: u32) -> TripleRow {
    let mut row = [0u16; WEAK3];
    let index = &WEAK3_ORDERS.1;
    let feasible = simulate(sorted, samples, |code| {
        let l = |i| code_level(code, i) as usize;
        row[index[l(0) * 9 + l(1) * 3 + l(2)] as usize] += 1;
    });
    if feasible { row } else { INFEASIBLE }
}

type TripleTable = Arc<RwLock<Vec<TripleRow>>>;

/// Simulated triples per sample count, shared by all solves of the process: a row of
/// zeros is not simulated yet. Results don't depend on what is cached because each
/// row is a function of the triple and the sample count alone.
static TRIPLE_TABLES: Mutex<Vec<(u32, TripleTable)>> = Mutex::new(Vec::new());

fn triple_table(samples: u32) -> TripleTable {
    let mut tables = TRIPLE_TABLES.lock().expect("triple table lock");
    if let Some((_, t)) = tables.iter().find(|(s, _)| *s == samples) {
        return t.clone();
    }
    let table = Arc::new(RwLock::new(vec![[0u16; WEAK3]; TRIPLES]));
    tables.push((samples, table.clone()));
    table
}

/// Read access to the showdown outcome distributions.
struct Showdowns<'a> {
    triples: &'a [TripleRow],
    samples: f64,
    larger: &'a FastMap<u128, Outcome>,
}

impl Showdowns<'_> {
    /// Calls `f(seat-order level code, probability)` for each outcome of the classes
    /// (seat order). Classes that can't be dealt together (possible only through the
    /// pairwise removal model) count as a split pot, so chips stay conserved; returns
    /// `false` for them.
    fn for_each(&self, classes: &[u8], mut f: impl FnMut(u64, f64)) -> bool {
        let k = classes.len();
        if k == 2 {
            let (a, b) = (
                HandClass::from_index(classes[0] as usize),
                HandClass::from_index(classes[1] as usize),
            );
            let eq = PreflopMatrix::embedded().equity(a, b);
            let tie = TieMatrix::embedded().tie(a, b);
            f(levels_code(&[1, 0]), (eq - tie / 2.0).clamp(0.0, 1.0));
            f(0, tie);
            f(levels_code(&[0, 1]), (1.0 - eq - tie / 2.0).clamp(0.0, 1.0));
            return true;
        }
        let mut order = [0usize; MAX_PLAYERS];
        let order = &mut order[..k];
        for (i, o) in order.iter_mut().enumerate() {
            *o = i;
        }
        order.sort_by_key(|&m| classes[m]);
        let mut sorted = [0u8; MAX_PLAYERS];
        for (s, &m) in sorted.iter_mut().zip(order.iter()) {
            *s = classes[m];
        }
        let sorted = &sorted[..k];
        if k == 3 {
            let row = &self.triples[triple_index(sorted)];
            if *row == INFEASIBLE {
                f(0, 1.0);
                return false;
            }
            for (&n, l) in row.iter().zip(&WEAK3_ORDERS.0) {
                if n > 0 {
                    f(reorder(levels_code(l), order), n as f64 / self.samples);
                }
            }
            return true;
        }
        let outcome = &self.larger[&tuple_key(sorted)];
        if outcome.is_empty() {
            f(0, 1.0);
            return false;
        }
        for &(code, p) in outcome.iter() {
            f(reorder(code, order), p as f64);
        }
        true
    }
}

// ---------------------------------------------------------------------------------
// Showdown tables per terminal.

/// Every seat's utility per outcome of one terminal, filled on first use.
struct Utilities {
    k: usize,
    /// Indexed by `Σ level_m · k^m` while that stays small.
    dense: Vec<Option<Box<[f64]>>>,
    sparse: FastMap<u64, Box<[f64]>>,
}

impl Utilities {
    fn new(k: usize) -> Utilities {
        let dense = if k <= 5 { k.pow(k as u32) } else { 0 };
        Utilities {
            k,
            dense: vec![None; dense],
            sparse: FastMap::default(),
        }
    }

    fn get(&mut self, money: &Money, involved: &[usize], code: u64) -> &[f64] {
        let compute = || {
            let levels: Vec<u8> = (0..involved.len()).map(|i| code_level(code, i)).collect();
            money.utilities(involved, &levels).into_boxed_slice()
        };
        if self.dense.is_empty() {
            return self.sparse.entry(code).or_insert_with(compute);
        }
        let index = (0..self.k)
            .rev()
            .fold(0, |i, m| i * self.k + code_level(code, m) as usize);
        self.dense[index].get_or_insert_with(compute)
    }
}

/// Values of one involved seat over its own class × the other involved seats'
/// classes within `supports`.
struct RoleBox {
    /// Positions of the other involved seats.
    others: Vec<usize>,
    supports: Vec<Vec<u8>>,
    /// `g[c * flat + f]`: removal weight × utility.
    g: Vec<f32>,
    flat: usize,
}

/// For seats outside the pot (ICM only): pairwise joint weight of the involved
/// classes and every seat's expected utility.
struct JointBox {
    supports: Vec<Vec<u8>>,
    jw: Vec<f64>,
    eu: Vec<f64>,
}

struct TermData {
    roles: Vec<Option<RoleBox>>,
    joint: Option<JointBox>,
    /// Utilities when nobody or one player is all in.
    fixed: Option<Vec<f64>>,
    utilities: Utilities,
}

/// Iterates the mixed-radix index over `supports`, calling `f(flat, classes)`.
fn for_each_tuple(supports: &[Vec<u8>], mut f: impl FnMut(usize, &[u8])) {
    let k = supports.len();
    if supports.iter().any(|s| s.is_empty()) {
        return;
    }
    let mut idx = vec![0usize; k];
    let mut cls: Vec<u8> = supports.iter().map(|s| s[0]).collect();
    let mut flat = 0;
    loop {
        f(flat, &cls);
        flat += 1;
        let mut d = k;
        loop {
            if d == 0 {
                return;
            }
            d -= 1;
            idx[d] += 1;
            if idx[d] < supports[d].len() {
                cls[d] = supports[d][idx[d]];
                break;
            }
            idx[d] = 0;
            cls[d] = supports[d][0];
        }
    }
}

/// `w[f] = Π_q weights_q(class)` over the mixed-radix index of `supports`.
fn product_weights(supports: &[Vec<u8>], weights: &[&[f64]]) -> Vec<f64> {
    let mut w = vec![1.0];
    for (s, wq) in supports.iter().zip(weights) {
        let mut next = Vec::with_capacity(w.len() * s.len());
        for &a in &w {
            for &x in s {
                next.push(a * wq[x as usize]);
            }
        }
        w = next;
    }
    w
}

fn reorder(code: u64, order: &[usize]) -> u64 {
    order
        .iter()
        .enumerate()
        .fold(0u64, |k, (j, &m)| k | (code_level(code, j) as u64) << (4 * m))
}

/// Classes a showdown table must cover. Frequencies below [`SNAP`] are left out:
/// the evaluated averages never have them, and while training they only shift the
/// rare multiway values by a negligible amount.
/// Class lists of the other involved seats a showdown table covers.
type Supports = Vec<Vec<u8>>;
/// A table to build: terminal, observer position (`None`: the seats outside), supports.
type Job = (usize, Option<usize>, Supports);

fn played(sigma: &[f64]) -> Vec<u8> {
    (0..C).filter(|&c| sigma[c] >= SNAP).map(|c| c as u8).collect()
}

fn covers(have: &[Vec<u8>], need: &[Vec<u8>]) -> bool {
    have.iter()
        .zip(need)
        .all(|(h, n)| n.iter().all(|c| h.binary_search(c).is_ok()))
}

/// Table supports for a rebuild: keep the old classes too unless the table would
/// stay much larger than needed, so that classes flickering in and out of a range
/// don't force a rebuild every iteration.
fn grow(old: Option<&[Vec<u8>]>, need: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let Some(old) = old else { return need };
    let size = |s: &[Vec<u8>]| s.iter().map(|x| x.len().max(1) as f64).product::<f64>();
    let union: Vec<Vec<u8>> = old
        .iter()
        .zip(&need)
        .map(|(o, n)| {
            let mut u: Vec<u8> = o.iter().chain(n).copied().collect();
            u.sort_unstable();
            u.dedup();
            u
        })
        .collect();
    if size(&union) <= 1.25 * size(&need) { union } else { need }
}

// ---------------------------------------------------------------------------------
// Solver.

struct Profile {
    sigma: Vec<Vec<f64>>,
    /// `amarg[v][c]`: probability that the actor at v acts, seen by a player with class c.
    amarg: Vec<Vec<f64>>,
}

/// One observer's view of the others: for each node of another seat, the actor's
/// class distribution given the observer's class and every earlier action
/// (`b`: likelihood of the history per actor class, `z`: its normaliser per observer
/// class) and the resulting action probability `a` per observer class.
struct View {
    a: Vec<Vec<f64>>,
    b: Vec<Vec<f64>>,
    z: Vec<Vec<f64>>,
}

struct Learner {
    regret: Vec<(f64, f64)>,
    sum: Vec<f64>,
}

#[derive(Default)]
struct PassOut {
    /// Per node of the observer: act and fold values without the prefix, the prefix,
    /// and the probability of the all-in players' actions before it.
    act: FastMap<usize, Vec<f64>>,
    fold: FastMap<usize, Vec<f64>>,
    prefix: FastMap<usize, Vec<f64>>,
    mass: FastMap<usize, Vec<f64>>,
}

/// What a tree walk for one observer reads.
struct WalkCtx<'a> {
    observer: usize,
    prof: &'a Profile,
    view: &'a View,
    /// Terminal values for the observer.
    tv: &'a [Vec<f64>],
}

/// A profile with its evaluation.
struct Checked {
    prof: Profile,
    ev: Vec<f64>,
    gain: Vec<f64>,
    outs: Vec<PassOut>,
}

enum Built {
    Role(RoleBox),
    Joint(JointBox),
}

struct Solver<'p> {
    params: &'p Params,
    money: Money,
    tree: Tree,
    prior: Vec<f64>,
    cond: Vec<f64>,
    terms: Vec<TermData>,
    triples: TripleTable,
    /// Simulated tuples of four or more classes.
    larger: FastMap<u128, Outcome>,
}

impl<'p> Solver<'p> {
    fn new(params: &'p Params, money: Money) -> Solver<'p> {
        let tree = Tree::build(money.n, params.max_all_in);
        let prior: Vec<f64> = HandClass::all().map(|c| c.combo_count() as f64 / 1326.0).collect();
        let cond = conditional(params.card_removal);
        let terms = tree
            .terms
            .iter()
            .map(|t| TermData {
                roles: (0..t.involved.len()).map(|_| None).collect(),
                joint: None,
                fixed: (t.involved.len() <= 1).then(|| money.utilities(&t.involved, &[0])),
                utilities: Utilities::new(t.involved.len()),
            })
            .collect();
        Solver {
            params,
            money,
            tree,
            prior,
            cond,
            terms,
            triples: triple_table(params.multiway_samples),
            larger: FastMap::default(),
        }
    }

    fn amarg(&self, sigma: &[f64]) -> Vec<f64> {
        (0..C)
            .map(|c| {
                let row = &self.cond[c * C..][..C];
                row.iter().zip(sigma).map(|(p, s)| p * s).sum()
            })
            .collect()
    }

    fn view(&self, observer: usize, prof: &Profile) -> View {
        let rows: Vec<(Vec<f64>, Vec<f64>, Vec<f64>)> = self
            .tree
            .nodes
            .par_iter()
            .enumerate()
            .map(|(v, node)| {
                if node.seat == observer {
                    return (Vec::new(), Vec::new(), Vec::new());
                }
                let mut b = vec![1.0; C];
                for &(u, acted) in &node.history {
                    if self.tree.nodes[u].seat == observer {
                        continue;
                    }
                    for (b, &am) in b.iter_mut().zip(&prof.amarg[u]) {
                        *b *= if acted { am } else { 1.0 - am };
                    }
                }
                let sigma = &prof.sigma[v];
                let (mut a, mut z) = (vec![0.0; C], vec![0.0; C]);
                for c in 0..C {
                    let row = &self.cond[c * C..][..C];
                    let (mut num, mut den) = (0.0, 0.0);
                    for x in 0..C {
                        let w = row[x] * b[x];
                        den += w;
                        num += w * sigma[x];
                    }
                    z[c] = den;
                    a[c] = if den > 0.0 { num / den } else { 0.0 };
                }
                (a, b, z)
            })
            .collect();
        let mut view = View { a: Vec::new(), b: Vec::new(), z: Vec::new() };
        for (a, b, z) in rows {
            view.a.push(a);
            view.b.push(b);
            view.z.push(z);
        }
        view
    }

    fn profile(&self, sigma: Vec<Vec<f64>>) -> Profile {
        let amarg = sigma.par_iter().map(|s| self.amarg(s)).collect();
        Profile { sigma, amarg }
    }

    // --- table building ---

    /// Supports a box over terminal `t` needs for `prof` (`role`: the observer's
    /// position, left out; heads-up showdowns use full tables built once).
    fn needed(&self, t: usize, role: Option<usize>, prof: &Profile) -> Vec<Vec<u8>> {
        let term = &self.tree.terms[t];
        let k = term.involved.len();
        (0..k)
            .filter(|&o| Some(o) != role)
            .map(|o| {
                if k == 2 {
                    (0..C as u8).collect()
                } else {
                    played(&prof.sigma[term.act_nodes[o]])
                }
            })
            .collect()
    }

    /// Builds the showdown tables that `observer`'s evaluation of `prof` needs.
    fn refresh(&mut self, observer: usize, prof: &Profile) {
        let icm = self.money.icm();
        let mut jobs: Vec<Job> = Vec::new();
        for (t, term) in self.tree.terms.iter().enumerate() {
            if term.involved.len() < 2 {
                continue;
            }
            let data = &self.terms[t];
            let (role, have) = match term.involved.iter().position(|&s| s == observer) {
                Some(m) => (Some(m), data.roles[m].as_ref().map(|b| &b.supports)),
                None if icm => (None, data.joint.as_ref().map(|b| &b.supports)),
                None => continue,
            };
            let need = self.needed(t, role, prof);
            let size = |s: &[Vec<u8>]| s.iter().map(|x| x.len().max(1) as f64).product::<f64>();
            if have.is_some_and(|h| covers(h, &need) && size(h) <= 2.0 * size(&need)) {
                continue;
            }
            jobs.push((t, role, grow(have.map(|h| h.as_slice()), need)));
        }
        if jobs.is_empty() {
            return;
        }
        self.simulate_missing(&jobs);

        let mut by_term: FastMap<usize, Vec<(Option<usize>, Supports)>> = FastMap::default();
        for (t, role, supports) in jobs {
            by_term.entry(t).or_default().push((role, supports));
        }
        let mut by_term: Vec<_> = by_term.into_iter().collect();
        by_term.sort_by_key(|e| e.0);
        let mut owned: Vec<(usize, Utilities)> = by_term
            .iter()
            .map(|(t, _)| (*t, std::mem::replace(&mut self.terms[*t].utilities, Utilities::new(0))))
            .collect();
        let this = &*self;
        let triples = this.triples.read().expect("triple table lock");
        let sd = Showdowns {
            triples: &triples,
            samples: this.params.multiway_samples as f64,
            larger: &this.larger,
        };
        let built: Vec<Vec<(Option<usize>, Built)>> = by_term
            .into_par_iter()
            .zip(owned.par_iter_mut())
            .map(|((t, boxes), (_, utilities))| {
                boxes
                    .into_iter()
                    .map(|(role, supports)| (role, this.build_box(t, role, supports, &sd, utilities)))
                    .collect()
            })
            .collect();
        drop(triples);
        for ((t, utilities), boxes) in owned.into_iter().zip(built) {
            let data = &mut self.terms[t];
            data.utilities = utilities;
            for (role, b) in boxes {
                match (role, b) {
                    (Some(m), Built::Role(b)) => data.roles[m] = Some(b),
                    (None, Built::Joint(b)) => data.joint = Some(b),
                    _ => unreachable!("box kind follows the role"),
                }
            }
        }
    }

    /// Simulates the class tuples of three or more players the jobs' tables need.
    fn simulate_missing(&mut self, jobs: &[Job]) {
        let samples = self.params.multiway_samples;
        let tree = &self.tree;
        let triples = self.triples.read().expect("triple table lock");
        let (three, more): (HashSet<usize>, HashSet<u128>) = jobs
            .par_iter()
            .filter(|(t, _, _)| tree.terms[*t].involved.len() >= 3)
            .map(|(t, role, supports)| {
                let (mut three, mut more) = (HashSet::new(), HashSet::new());
                let k = tree.terms[*t].involved.len();
                self.for_each_box_tuple(k, *role, supports, |classes| {
                    let mut s = classes.to_vec();
                    s.sort_unstable();
                    if k == 3 {
                        let i = triple_index(&s);
                        if triples[i] == [0; WEAK3] {
                            three.insert(i);
                        }
                    } else {
                        let key = tuple_key(&s);
                        if !self.larger.contains_key(&key) {
                            more.insert(key);
                        }
                    }
                });
                (three, more)
            })
            .reduce(
                || (HashSet::new(), HashSet::new()),
                |mut a, b| {
                    a.0.extend(b.0);
                    a.1.extend(b.1);
                    a
                },
            );
        drop(triples);
        let three: Vec<usize> = three.into_iter().collect();
        let rows: Vec<(usize, TripleRow)> = three
            .par_iter()
            .map(|&i| (i, simulate_triple(&triple_classes(i), samples)))
            .collect();
        let mut triples = self.triples.write().expect("triple table lock");
        for (i, row) in rows {
            triples[i] = row;
        }
        drop(triples);
        let more: Vec<u128> = more.into_iter().collect();
        let fresh: Vec<(u128, Outcome)> = more
            .par_iter()
            .map(|&key| {
                let sorted: Vec<u8> = (0..16)
                    .map(|i| (key >> (8 * i)) as u8)
                    .take_while(|&b| b != 0)
                    .map(|b| b - 1)
                    .collect();
                (key, simulate_outcome(&sorted, samples))
            })
            .collect();
        self.larger.extend(fresh);
    }

    /// Every class tuple (seat order) a box would look up.
    fn for_each_box_tuple(&self, k: usize, role: Option<usize>, supports: &[Vec<u8>], mut f: impl FnMut(&[u8])) {
        match role {
            Some(m) => {
                let mut classes = vec![0u8; k];
                for c in 0..C as u8 {
                    for_each_tuple(supports, |_, xs| {
                        let mut it = xs.iter();
                        for (o, cl) in classes.iter_mut().enumerate() {
                            *cl = if o == m { c } else { *it.next().expect("one class per other seat") };
                        }
                        f(&classes);
                    });
                }
            }
            None => for_each_tuple(supports, |_, xs| f(xs)),
        }
    }

    /// Three-player role table: utilities per (seat permutation, weak order) are
    /// looked up once, then the rows are filled in parallel from the triple counts.
    fn fill_three(
        &self,
        g: &mut [f32],
        m: usize,
        supports: &[Vec<u8>],
        sd: &Showdowns,
        mut utility: impl FnMut(u64) -> f64,
    ) {
        const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        let mut by_perm = [[0.0; WEAK3]; 6];
        for (row, order) in by_perm.iter_mut().zip(&PERMS) {
            for (u, l) in row.iter_mut().zip(&WEAK3_ORDERS.0) {
                *u = utility(reorder(levels_code(l), order)) / sd.samples;
            }
        }
        let split = utility(0);
        let flat: usize = supports.iter().map(|s| s.len()).product();
        if flat == 0 {
            return;
        }
        g.par_chunks_mut(flat).enumerate().for_each(|(c, gc)| {
            let row = &self.cond[c * C..][..C];
            let mut classes = [0u8; 3];
            for_each_tuple(supports, |f, xs| {
                let w = row[xs[0] as usize] * row[xs[1] as usize];
                if w == 0.0 {
                    return;
                }
                let mut it = xs.iter();
                for (o, cl) in classes.iter_mut().enumerate() {
                    *cl = if o == m { c as u8 } else { *it.next().expect("two other seats") };
                }
                // Stable sort by class, like `Showdowns::for_each`.
                let mut order = [0usize, 1, 2];
                order.sort_by_key(|&i| classes[i]);
                let p = PERMS.iter().position(|q| *q == order).expect("a permutation of three");
                let sorted = [classes[order[0]], classes[order[1]], classes[order[2]]];
                let counts = &sd.triples[triple_index(&sorted)];
                let u = if *counts == INFEASIBLE {
                    split
                } else {
                    counts.iter().zip(&by_perm[p]).map(|(&n, u)| n as f64 * u).sum()
                };
                gc[f] = (w * u) as f32;
            });
        });
    }

    fn build_box(
        &self,
        t: usize,
        role: Option<usize>,
        supports: Vec<Vec<u8>>,
        sd: &Showdowns,
        utilities: &mut Utilities,
    ) -> Built {
        let term = &self.tree.terms[t];
        let involved = &term.involved;
        let k = involved.len();
        let flat: usize = supports.iter().map(|s| s.len()).product();
        match role {
            Some(m) => {
                let seat = involved[m];
                let mut g = vec![0f32; C * flat];
                if k == 3 {
                    self.fill_three(&mut g, m, &supports, sd, |code| {
                        utilities.get(&self.money, involved, code)[seat]
                    });
                    let others = (0..k).filter(|&o| o != m).collect();
                    return Built::Role(RoleBox { others, supports, g, flat });
                }
                let mut classes = vec![0u8; k];
                for c in 0..C {
                    let row = &self.cond[c * C..][..C];
                    for_each_tuple(&supports, |f, xs| {
                        let w: f64 = xs.iter().map(|&x| row[x as usize]).product();
                        if w == 0.0 {
                            return;
                        }
                        let mut it = xs.iter();
                        for (o, cl) in classes.iter_mut().enumerate() {
                            *cl = if o == m { c as u8 } else { *it.next().expect("one class per other seat") };
                        }
                        let mut u = 0.0;
                        sd.for_each(&classes, |code, p| u += p * utilities.get(&self.money, involved, code)[seat]);
                        g[c * flat + f] = (w * u) as f32;
                    });
                }
                let others = (0..k).filter(|&o| o != m).collect();
                Built::Role(RoleBox { others, supports, g, flat })
            }
            None => {
                let n = self.money.n;
                let mut jw = vec![0.0; flat];
                let mut eu = vec![0.0; flat * n];
                for_each_tuple(&supports, |f, xs| {
                    let w: f64 = xs.iter().map(|&x| self.prior[x as usize]).product();
                    if w == 0.0 {
                        return;
                    }
                    let eu = &mut eu[f * n..][..n];
                    sd.for_each(xs, |code, p| {
                        let u = utilities.get(&self.money, involved, code);
                        for (e, u) in eu.iter_mut().zip(u) {
                            *e += p * u;
                        }
                    });
                    jw[f] = w;
                });
                Built::Joint(JointBox { supports, jw, eu })
            }
        }
    }

    // --- evaluation ---

    /// Value of terminal `t` for `observer` per class, without the folders' factors.
    fn term_value(&self, t: usize, observer: usize, prof: &Profile, view: &View) -> Vec<f64> {
        let term = &self.tree.terms[t];
        let data = &self.terms[t];
        let inside = term.involved.iter().position(|&s| s == observer);
        if let Some(fixed) = &data.fixed {
            let u = fixed[observer];
            return match (inside, term.act_nodes.first()) {
                (None, Some(&v)) => view.a[v].iter().map(|a| a * u).collect(),
                _ => vec![u; C],
            };
        }
        if let Some(m) = inside {
            let b = data.roles[m].as_ref().expect("role table built before use");
            // Each other player's class: its removal weight against the observer's
            // class (in `g`) × the likelihood of the history before its all-in × its
            // all-in frequency, normalised per observer class by `z`.
            let nodes: Vec<usize> = b.others.iter().map(|&o| term.act_nodes[o]).collect();
            let weights: Vec<Vec<f64>> = nodes
                .iter()
                .map(|&v| prof.sigma[v].iter().zip(&view.b[v]).map(|(s, b)| s * b).collect())
                .collect();
            let weights: Vec<&[f64]> = weights.iter().map(|w| w.as_slice()).collect();
            let w = product_weights(&b.supports, &weights);
            let w: Vec<f32> = w.iter().map(|&x| x as f32).collect();
            let dot = |c: usize| -> f64 {
                let g = &b.g[c * b.flat..][..b.flat];
                // Eight partial sums keep the f32 dot product vectorized and accurate.
                let mut acc = [0f32; 8];
                let (gc, gr) = g.as_chunks::<8>();
                let (wc, wr) = w.as_chunks::<8>();
                for (g8, w8) in gc.iter().zip(wc) {
                    for i in 0..8 {
                        acc[i] += g8[i] * w8[i];
                    }
                }
                let tail: f64 = gr.iter().zip(wr).map(|(g, w)| (*g as f64) * (*w as f64)).sum();
                acc.iter().map(|&a| a as f64).sum::<f64>() + tail
            };
            let value = |c: usize| -> f64 {
                let z: f64 = nodes.iter().map(|&v| view.z[v][c]).product();
                if z > 0.0 { dot(c) / z } else { 0.0 }
            };
            return if b.flat >= 2048 {
                (0..C).into_par_iter().map(value).collect()
            } else {
                (0..C).map(value).collect()
            };
        }
        let reach: Vec<f64> = (0..C)
            .map(|c| term.act_nodes.iter().map(|&v| view.a[v][c]).product())
            .collect();
        let ubar = if self.money.icm() {
            let b = data.joint.as_ref().expect("joint table built before use");
            let weights: Vec<Vec<f64>> = term
                .act_nodes
                .iter()
                .map(|&v| prof.sigma[v].iter().zip(&view.b[v]).map(|(s, b)| s * b).collect())
                .collect();
            let weights: Vec<&[f64]> = weights.iter().map(|w| w.as_slice()).collect();
            let w = product_weights(&b.supports, &weights);
            let n = self.money.n;
            let (mut num, mut den) = (0.0, 0.0);
            for (f, &wf) in w.iter().enumerate() {
                let x = wf * b.jw[f];
                den += x;
                num += x * b.eu[f * n + observer];
            }
            if den > 0.0 { num / den } else { 0.0 }
        } else {
            // A folder's chips don't depend on the showdown.
            -(self.money.stack[observer] - self.money.live[observer] + self.money.blind[observer])
        };
        reach.iter().map(|r| r * ubar).collect()
    }

    /// Returns (on-policy value, best-response value) per class below `child`.
    fn walk(&self, ctx: &WalkCtx, child: Child, prefix: &[f64], out: &mut PassOut) -> (Vec<f64>, Vec<f64>) {
        let v = match child {
            Child::Term(t) => return (ctx.tv[t].clone(), ctx.tv[t].clone()),
            Child::Node(v) => v,
        };
        let node = &self.tree.nodes[v];
        let view = ctx.view;
        if node.seat == ctx.observer {
            let (act, _) = self.walk(ctx, node.act, prefix, out);
            let (fold, _) = self.walk(ctx, node.fold, prefix, out);
            let s = &ctx.prof.sigma[v];
            let val = (0..C).map(|c| s[c] * act[c] + (1.0 - s[c]) * fold[c]).collect();
            let br = (0..C).map(|c| act[c].max(fold[c])).collect();
            out.act.insert(v, act);
            out.fold.insert(v, fold);
            out.prefix.insert(v, prefix.to_vec());
            let mass = (0..C)
                .map(|c| node.involved_above.iter().map(|&u| view.a[u][c]).product())
                .collect();
            out.mass.insert(v, mass);
            return (val, br);
        }
        let a = &view.a[v];
        let fold_prefix: Vec<f64> = prefix.iter().zip(a).map(|(p, a)| p * (1.0 - a)).collect();
        let (fv, fb) = self.walk(ctx, node.fold, &fold_prefix, out);
        let (av, ab) = self.walk(ctx, node.act, prefix, out);
        let mix = |f: &[f64], x: &[f64]| (0..C).map(|c| (1.0 - a[c]) * f[c] + x[c]).collect();
        (mix(&fv, &av), mix(&fb, &ab))
    }

    /// One observer's values: (EV, best-response EV, node records).
    fn pass(&mut self, observer: usize, prof: &Profile) -> (f64, f64, PassOut) {
        self.refresh(observer, prof);
        let this = &*self;
        let view = this.view(observer, prof);
        let tv: Vec<Vec<f64>> = (0..this.tree.terms.len())
            .into_par_iter()
            .map(|t| this.term_value(t, observer, prof, &view))
            .collect();
        let mut out = PassOut::default();
        let ctx = WalkCtx { observer, prof, view: &view, tv: &tv };
        let (val, br) = this.walk(&ctx, this.tree.root, &vec![1.0; C], &mut out);
        let ev = val.iter().zip(&this.prior).map(|(v, p)| v * p).sum();
        let best = br.iter().zip(&this.prior).map(|(v, p)| v * p).sum();
        (ev, best, out)
    }

    /// EV and best-response gain per seat of a profile.
    fn evaluate(&mut self, prof: Profile) -> Checked {
        let (mut ev, mut gain, mut outs) = (Vec::new(), Vec::new(), Vec::new());
        for p in 0..self.money.n {
            let (e, b, o) = self.pass(p, &prof);
            ev.push(e);
            gain.push((b - e).max(0.0));
            outs.push(o);
        }
        Checked { prof, ev, gain, outs }
    }

    /// Starting strategies. With multiway pots allowed, the same game capped at
    /// heads-up pots (cheap: no simulated showdowns) gives the shoves and first calls;
    /// overcalls start with strong hands only. Starting close to the equilibrium keeps
    /// the played ranges, and with them the showdown tables, small.
    fn warm_start(&self) -> Vec<Vec<f64>> {
        let m = PreflopMatrix::embedded();
        let strength: Vec<f64> = HandClass::all().map(|c| m.equity_vs_random(c)).collect();
        let pure = |cut: f64| -> Vec<f64> { strength.iter().map(|&s| if s >= cut { 1.0 } else { 0.0 }).collect() };
        let heads_up = (self.tree.cap > 2).then(|| {
            let mut p = self.params.clone();
            p.max_all_in = 2;
            solve(&p).expect("same validated parameters")
        });
        self.tree
            .nodes
            .iter()
            .map(|node| match &heads_up {
                Some(sol) if node.callers.is_empty() => {
                    let i = sol.node(node.seat, node.shover, &[]).expect("heads-up tree has every first decision");
                    sol.nodes[i].act.clone()
                }
                _ if node.shover.is_none() => pure(START_SHOVE_EQUITY),
                _ => pure(START_CALL_EQUITY),
            })
            .collect()
    }

    /// Linear average with frequencies near 0 or 1 snapped to pure.
    fn average(&self, learners: &[Learner], weight: f64) -> Vec<Vec<f64>> {
        learners
            .iter()
            .map(|l| {
                l.sum
                    .iter()
                    .map(|s| {
                        let x = s / weight;
                        if x < SNAP {
                            0.0
                        } else if x > 1.0 - SNAP {
                            1.0
                        } else {
                            x
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn run(&mut self) -> Solution {
        let n = self.money.n;
        let target = self.params.target_exploitability;
        let mut prof = self.profile(self.warm_start());
        let mut learners: Vec<Learner> = (0..self.tree.nodes.len())
            .map(|_| Learner { regret: vec![(0.0, 0.0); C], sum: vec![0.0; C] })
            .collect();
        let (mut weight, mut iterations, mut converged) = (0.0, 0, false);
        let mut best: Option<Checked> = None;
        while iterations < self.params.max_iterations && !converged {
            iterations += 1;
            let t = iterations as f64;
            for p in 0..n {
                let (_, _, out) = self.pass(p, &prof);
                for &v in &self.tree.nodes_of[p] {
                    let (act, fold, pre) = (&out.act[&v], &out.fold[&v], &out.prefix[&v]);
                    let l = &mut learners[v];
                    let s = &mut prof.sigma[v];
                    for c in 0..C {
                        let (a, f) = (pre[c] * act[c], pre[c] * fold[c]);
                        let val = s[c] * a + (1.0 - s[c]) * f;
                        let r = &mut l.regret[c];
                        r.0 = (r.0 + a - val).max(0.0);
                        r.1 = (r.1 + f - val).max(0.0);
                        // With no positive regret any strategy keeps the RM+ bound;
                        // keeping the last one keeps ranges and showdown tables small.
                        if r.0 + r.1 > 0.0 {
                            s[c] = r.0 / (r.0 + r.1);
                        }
                        l.sum[c] += t * s[c];
                    }
                    prof.amarg[v] = self.amarg(&prof.sigma[v]);
                }
            }
            weight += t;
            if iterations % CHECK_EVERY == 0 || iterations == self.params.max_iterations {
                let avg = self.profile(self.average(&learners, weight));
                let checked = self.evaluate(avg);
                let total: f64 = checked.gain.iter().sum();
                converged = total < target;
                if best.as_ref().is_none_or(|b| total < b.gain.iter().sum::<f64>()) {
                    best = Some(checked);
                }
            }
        }
        let checked = best.expect("at least one iteration, and the last one is checked");
        self.solution(checked, iterations, converged)
    }

    fn solution(&self, checked: Checked, iterations: u32, converged: bool) -> Solution {
        let Checked { prof, ev, gain, outs } = checked;
        let n = self.money.n;
        let exploitability = gain.iter().sum();
        let nodes = self.report_nodes(&prof, &outs);
        let approximation = self.approximation(&prof, &ev);
        Solution {
            players: n,
            icm: self.money.icm(),
            nodes,
            ev,
            gain,
            exploitability,
            iterations,
            converged,
            approximation,
        }
    }

    fn report_nodes(&self, prof: &Profile, outs: &[PassOut]) -> Vec<Node> {
        self.tree
            .nodes
            .iter()
            .enumerate()
            .map(|(v, node)| {
                let out = &outs[node.seat];
                let (act, fold, pre) = (&out.act[&v], &out.fold[&v], &out.prefix[&v]);
                let inv = &out.mass[&v];
                let reach = (0..C).map(|c| self.prior[c] * pre[c] * inv[c]).sum();
                let given = |x: &[f64]| -> Vec<f64> {
                    (0..C).map(|c| if inv[c] > 0.0 { x[c] / inv[c] } else { 0.0 }).collect()
                };
                Node {
                    seat: node.seat,
                    shover: node.shover,
                    callers: node.callers.clone(),
                    reach,
                    act: prof.sigma[v].clone(),
                    ev_act: given(act),
                    ev_fold: given(fold),
                }
            })
            .collect()
    }

    fn approximation(&self, prof: &Profile, ev: &[f64]) -> Approximation {
        let marginal = |v: usize| -> f64 { (0..C).map(|c| self.prior[c] * prof.sigma[v][c]).sum() };
        // Terminal probabilities from the product of marginal action frequencies.
        let (mut multiway, mut capped) = (0.0, 0.0);
        for term in &self.tree.terms {
            if term.involved.len() < 3 {
                continue;
            }
            let first = term.act_nodes[0];
            let mut p: f64 = (0..term.involved[0]).map(|s| 1.0 - marginal(s)).product();
            p *= marginal(first);
            let mut next = self.tree.nodes[first].act;
            while let Child::Node(u) = next {
                let acts = term.act_nodes.contains(&u);
                let a = marginal(u);
                p *= if acts { a } else { 1.0 - a };
                next = if acts { self.tree.nodes[u].act } else { self.tree.nodes[u].fold };
            }
            multiway += p;
            if term.involved.len() == self.tree.cap && term.involved.last() != Some(&(self.money.n - 1)) {
                capped += p;
            }
        }
        Approximation {
            multiway_monte_carlo: multiway > 0.0,
            multiway_samples: self.params.multiway_samples,
            multiway_probability: multiway,
            capped_probability: capped,
            pairwise_card_removal: self.params.card_removal,
            ev_sum_error: ev.iter().sum::<f64>() - self.money.total(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn money(stacks: &[f64], payouts: &[f64]) -> Money {
        let mut p = Params::new(stacks.to_vec());
        p.ante_bb = 0.1;
        p.payouts = payouts.to_vec();
        Money::new(&p).unwrap()
    }

    #[test]
    fn side_pots_settle_exactly() {
        let m = money(&[5.0, 10.0, 20.0, 20.0], &[]);
        // Dead: 4 antes. Seat 0 (4.9 live) wins the main pot, seat 3 beats seat 1 for
        // the side pot; the SB's folded 0.5 belongs to the main pot.
        let fin = m.settle(&[0, 1, 3], &[2, 0, 1]);
        assert!((fin[0] - (3.0 * 4.9 + 0.5 + 0.4)).abs() < 1e-12, "{fin:?}");
        assert!((fin[3] - (2.0 * 5.0 + 10.0)).abs() < 1e-12, "{fin:?}");
        assert!((fin[1]).abs() < 1e-12 && (fin[2] - 19.4).abs() < 1e-12, "{fin:?}");
        let total: f64 = fin.iter().sum();
        assert!((total - 55.0).abs() < 1e-9, "{total}");
        // Ties split each layer among the best eligible hands.
        let fin = m.settle(&[2, 3], &[1, 1]);
        assert!((fin[2] - fin[3]).abs() < 1e-12);
        // Nobody shoves: the big blind takes blinds and antes.
        let fin = m.settle(&[], &[]);
        assert!((fin[3] - (19.9 - 1.0 + 1.5 + 0.4)).abs() < 1e-12, "{fin:?}");
    }

    #[test]
    fn tree_sizes() {
        for n in 2..=9 {
            let t = Tree::build(n, n);
            assert_eq!(t.terms.len(), (1 << n) - 1, "{n} players: terminals");
            assert_eq!(t.nodes.len(), (1 << n) - 2, "{n} players: nodes");
            for (s, nodes) in t.nodes_of.iter().enumerate() {
                assert!(nodes.iter().all(|&v| t.nodes[v].seat == s));
            }
        }
    }

    #[test]
    fn capped_tree() {
        // Nine players, at most three all in: shover + up to two callers.
        let t = Tree::build(9, 3);
        let expected: usize = 1 + (0..8).map(|i| 1 + (8 - i) + (8 - i) * (7 - i) / 2).sum::<usize>();
        assert_eq!(t.terms.len(), expected);
        assert!(t.terms.iter().all(|term| term.involved.len() <= 3));
        assert!(t.nodes.iter().all(|node| node.callers.len() <= 1));
    }

    #[test]
    fn seat_names() {
        assert_eq!(seat_name(2, 0), "SB");
        assert_eq!(seat_name(3, 0), "BTN");
        let nine: Vec<_> = (0..9).map(|s| seat_name(9, s)).collect();
        assert_eq!(nine, ["UTG", "UTG+1", "UTG+2", "LJ", "HJ", "CO", "BTN", "SB", "BB"]);
    }

    #[test]
    fn triple_index_round_trips() {
        for i in [0, 1, 2, 500, 12_345, TRIPLES - 1] {
            let t = triple_classes(i);
            assert!(t[0] <= t[1] && t[1] <= t[2]);
            assert_eq!(triple_index(&t), i);
        }
        assert_eq!(triple_index(&[168, 168, 168]), TRIPLES - 1);
    }

    #[test]
    fn infeasible_tuples_have_no_outcome() {
        let aa = "AA".parse::<HandClass>().unwrap().index() as u8;
        let ako = "AKo".parse::<HandClass>().unwrap().index() as u8;
        let kk = "KK".parse::<HandClass>().unwrap().index() as u8;
        let mut t = vec![aa, aa, ako];
        t.sort_unstable();
        assert_eq!(simulate_triple(&t, 64), INFEASIBLE);
        let qq = "QQ".parse::<HandClass>().unwrap().index() as u8;
        let mut t = vec![aa, kk, ako, qq];
        t.sort_unstable();
        let o = simulate_outcome(&t, 256);
        let total: f32 = o.iter().map(|e| e.1).sum();
        assert!((total - 1.0).abs() < 1e-5);
    }
}

/// Measurements against full deals; run with
/// `cargo test --release -p ringside-engine --lib multiway::deals -- --ignored --nocapture`.
#[cfg(test)]
mod deals {
    use super::*;
    use crate::range::Combo;

    /// Expected utility per seat of a profile over `deals` random full deals (all
    /// players' combos disjoint, uniform board): the exact deal distribution, without
    /// the pairwise card-removal model.
    fn deal_ev(params: &Params, sigma: &[Vec<f64>], deals: u64) -> Vec<f64> {
        let money = Money::new(params).unwrap();
        let tree = Tree::build(money.n, params.max_all_in);
        let n = money.n;
        let chunks = 256u64;
        let per = deals / chunks;
        let sums: Vec<Vec<f64>> = (0..chunks)
            .into_par_iter()
            .map(|chunk| {
                let mut rng = Wy(0x5eed ^ chunk.wrapping_mul(0x9e37_79b9_7f4a_7c15));
                let mut sum = vec![0.0; n];
                let mut cache: FastMap<(usize, u64), Vec<f64>> = FastMap::default();
                for _ in 0..per {
                    let mut used = 0u64;
                    let mut class = vec![0usize; n];
                    let mut hand = vec![Hand::EMPTY; n];
                    for s in 0..n {
                        loop {
                            let combo = Combo::from_index(rng.below(1326));
                            let m = combo.mask().0;
                            if m & used == 0 {
                                used |= m;
                                class[s] = combo.class().index();
                                let (a, b) = combo.cards();
                                hand[s] = Hand::from_cards([a, b]);
                                break;
                            }
                        }
                    }
                    let mut board = Hand::EMPTY;
                    let mut got = 0;
                    while got < 5 {
                        let card = rng.below(52) as u8;
                        if used >> card & 1 == 0 {
                            used |= 1 << card;
                            board = board.with(Card::from_index(card));
                            got += 1;
                        }
                    }
                    let rank: Vec<u16> = hand.iter().map(|h| h.combine(board).rank().0).collect();
                    let mut stack = vec![(tree.root, 1.0)];
                    while let Some((child, p)) = stack.pop() {
                        match child {
                            Child::Term(t) => {
                                let inv = &tree.terms[t].involved;
                                let levels: Vec<u8> = inv
                                    .iter()
                                    .map(|&s| {
                                        let mut below: Vec<u16> =
                                            inv.iter().map(|&o| rank[o]).filter(|&r| r < rank[s]).collect();
                                        below.sort_unstable();
                                        below.dedup();
                                        below.len() as u8
                                    })
                                    .collect();
                                let u = cache
                                    .entry((t, levels_code(&levels)))
                                    .or_insert_with(|| money.utilities(inv, &levels));
                                for (x, u) in sum.iter_mut().zip(u.iter()) {
                                    *x += p * u;
                                }
                            }
                            Child::Node(v) => {
                                let a = sigma[v][class[tree.nodes[v].seat]];
                                if a > 0.0 {
                                    stack.push((tree.nodes[v].act, p * a));
                                }
                                if a < 1.0 {
                                    stack.push((tree.nodes[v].fold, p * (1.0 - a)));
                                }
                            }
                        }
                    }
                }
                sum
            })
            .collect();
        (0..n)
            .map(|s| sums.iter().map(|x| x[s]).sum::<f64>() / (per * chunks) as f64)
            .collect()
    }

    fn compare(params: &Params) {
        let sol = solve(params).unwrap();
        let sigma: Vec<Vec<f64>> = sol.nodes.iter().map(|n| n.act.clone()).collect();
        let deals = 8_000_000;
        let exact = deal_ev(params, &sigma, deals);
        let worst = sol.ev.iter().zip(&exact).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        println!(
            "{} players {:?} removal {}: model EV {:?}\n  deals EV {:?}\n  max |diff| {worst:.4}, model ΣEV error {:.4}",
            sol.players,
            params.stacks_bb,
            params.card_removal,
            sol.ev.iter().map(|x| format!("{x:+.4}")).collect::<Vec<_>>(),
            exact.iter().map(|x| format!("{x:+.4}")).collect::<Vec<_>>(),
            sol.approximation.ev_sum_error,
        );
    }

    /// Pot share of each player of a sorted triple from its simulated counts.
    fn table_equity(sorted: &[u8; 3], samples: u32) -> [f64; 3] {
        let row = simulate_triple(sorted, samples);
        let mut eq = [0.0; 3];
        for (&n, l) in row.iter().zip(&WEAK3_ORDERS.0) {
            let top = *l.iter().max().unwrap();
            let winners = l.iter().filter(|&&x| x == top).count() as f64;
            for i in 0..3 {
                if l[i] == top {
                    eq[i] += n as f64 / samples as f64 / winners;
                }
            }
        }
        eq
    }

    #[test]
    #[ignore = "measurement, ~1 min in release"]
    fn triple_table_against_exact_enumeration() {
        use crate::cards::CardSet;
        use crate::equity::{Method, equity};
        use crate::range::Range;
        let mut rng = Wy(77);
        let mut triples: Vec<[u8; 3]> = (0..16)
            .map(|_| {
                let mut t = [rng.below(C) as u8, rng.below(C) as u8, rng.below(C) as u8];
                t.sort_unstable();
                t
            })
            .collect();
        for named in [["AA", "KK", "QQ"], ["AKs", "QQ", "JTs"], ["A5s", "KQo", "22"], ["AKo", "AQo", "AJo"]] {
            let mut t = named.map(|c| c.parse::<HandClass>().unwrap().index() as u8);
            t.sort_unstable();
            triples.push(t);
        }
        for samples in [1024u32, 8192] {
            let (mut worst, mut sum, mut count) = (0.0f64, 0.0, 0);
            for t in &triples {
                if !feasible(t) {
                    continue;
                }
                let players: Vec<Range> = t
                    .iter()
                    .map(|&c| Range::parse(&HandClass::from_index(c as usize).to_string()).unwrap())
                    .collect();
                let exact = equity(&players, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).unwrap();
                let table = table_equity(t, samples);
                for i in 0..3 {
                    let e = (exact.players[i].equity_pct / 100.0 - table[i]).abs();
                    worst = worst.max(e);
                    sum += e;
                    count += 1;
                }
            }
            let sigma = (0.25f64 / samples as f64).sqrt();
            println!(
                "{samples} samples: equity error mean {:.4}, max {worst:.4} over {count} shares (σ ≤ {sigma:.4})",
                sum / count as f64
            );
            assert!(worst < 5.0 * sigma, "{samples}: {worst}");
        }
        let start = std::time::Instant::now();
        let rows: Vec<TripleRow> = (0..TRIPLES).into_par_iter().map(|i| simulate_triple(&triple_classes(i), 1024)).collect();
        println!(
            "all {TRIPLES} triples × 1024 samples: {:.2} s on {} threads, {:.1} MB",
            start.elapsed().as_secs_f64(),
            rayon::current_num_threads(),
            (rows.len() * std::mem::size_of::<TripleRow>()) as f64 / 1e6
        );
    }

    /// How much the sample count moves a decision that rests on 3-way pots alone.
    #[test]
    #[ignore = "measurement, ~10 s in release"]
    fn overcall_sensitivity_to_samples() {
        let mut solutions = Vec::new();
        for samples in [1024, 16384] {
            let mut p = Params::equal(3, 7.5);
            p.target_exploitability = 1e-4;
            p.multiway_samples = samples;
            solutions.push(solve(&p).unwrap());
        }
        let (a, b) = (&solutions[0], &solutions[1]);
        let v = a.node(2, Some(0), &[1]).unwrap();
        let (na, nb) = (&a.nodes[v], &b.nodes[v]);
        let worst = (0..C)
            .map(|c| ((na.ev_act[c] - na.ev_fold[c]) - (nb.ev_act[c] - nb.ev_fold[c])).abs())
            .fold(0.0, f64::max);
        let differ = (0..C).filter(|&c| (na.act[c] - nb.act[c]).abs() > 0.5).count();
        println!(
            "BB overcall 7.5 BB: {:.2}% (1024) vs {:.2}% (16384), max |Δ(call − fold)| {worst:.4} BB, {differ} classes flip",
            a.act_pct(v),
            b.act_pct(v)
        );
    }

    #[test]
    #[ignore = "measurement, ~1 min in release"]
    fn card_removal_model_against_full_deals() {
        for n in [2, 3, 6, 9] {
            for removal in [true, false] {
                let mut p = Params::equal(n, 10.0);
                p.card_removal = removal;
                compare(&p);
            }
        }
    }
}
