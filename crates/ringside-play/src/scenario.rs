//! Heads-up flops of the preflop model as postflop spots: the line that leads there, the
//! ranges both players arrive with, pot and stacks.
//!
//! The model reports its most frequent heads-up flops ([`Solution::realized`]). A player's
//! range at the flop is the product of the frequencies of its own actions on the way, the
//! same chain the spot library uses; weights are rounded to [`WEIGHT_STEP`] like there.

use ringside_engine::Combo;
use ringside_engine::preflop::{Kind, Params, Solution, Step, postflop_rank};
use ringside_engine::range::{HandClass, NUM_CLASSES, Range};
use ringside_solver::{Spot, StreetSizes};
use serde::Serialize;

use crate::model::{End, Walk, action_index};

pub const WEIGHT_STEP: f64 = 0.05;
/// All-in is offered once it is at most this share of the pot, in percent, as in the library.
const ALLIN_THRESHOLD: f64 = 150.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PotType {
    Limped,
    /// Open and call.
    Srp,
    /// 3-bet or more, then a call.
    ThreeBet,
}

#[derive(Clone, Debug, Serialize)]
pub struct Scenario {
    /// Index into the solution's `realized` list, stable for one table; `None` for a line
    /// a hand took that the list does not have.
    pub index: Option<usize>,
    pub stack_bb: f64,
    pub line: Vec<Step>,
    /// Out of position and in position after the flop.
    pub oop: usize,
    pub ip: usize,
    pub pot_type: PotType,
    /// How often the model plays to this flop, per hand.
    pub reach: f64,
    pub pot: f64,
    /// The smaller stack behind.
    pub stack: f64,
    /// Every decision on the way: node and the action taken there (folds included).
    #[serde(skip)]
    pub path: Vec<(usize, usize)>,
    /// Class weights of OOP and IP at the flop, rounded.
    #[serde(skip)]
    pub weights: [Vec<f64>; 2],
}

impl Scenario {
    pub fn seats(&self) -> [usize; 2] {
        [self.oop, self.ip]
    }

    pub fn sizes(&self) -> [StreetSizes; 3] {
        let s = |bet: &[f64], raise: &[f64]| StreetSizes { bet: bet.to_vec(), raise: raise.to_vec() };
        match self.pot_type {
            PotType::Limped | PotType::Srp => [s(&[33.0], &[60.0]), s(&[75.0], &[]), s(&[75.0], &[])],
            PotType::ThreeBet => [s(&[33.0], &[]), s(&[66.0], &[]), s(&[75.0], &[])],
        }
    }

    pub fn range(&self, side: usize) -> String {
        self.range_with(side, None)
    }

    /// The range of `side`, with `add` in it at the smallest weight if the model never
    /// plays it this way, as the hand analysis does with the hero's hand.
    fn range_with(&self, side: usize, add: Option<Combo>) -> String {
        let mut r = Range::empty();
        for c in HandClass::all() {
            let w = self.weights[side][c.index()];
            if w > 0.0 {
                r.set_class(c, w as f32);
            }
        }
        if let Some(combo) = add
            && r.weight(combo) <= 0.0
        {
            r.set_combo(combo, WEIGHT_STEP as f32);
        }
        r.to_string()
    }

    pub fn spot(&self, flop: &str) -> Spot {
        self.spot_with(flop, None)
    }

    /// [`Scenario::spot`] with the hero's hand (`side`, combo) in the hero's range.
    pub fn spot_with(&self, flop: &str, hero: Option<(usize, Combo)>) -> Spot {
        let add = |side: usize| hero.filter(|h| h.0 == side).map(|h| h.1);
        Spot {
            oop_range: self.range_with(0, add(0)),
            ip_range: self.range_with(1, add(1)),
            board: flop.to_string(),
            pot: self.pot,
            stack: self.stack,
            sizes: self.sizes(),
            allin_threshold: ALLIN_THRESHOLD,
        }
    }
}

fn round(w: f64) -> f64 {
    ((w / WEIGHT_STEP).round() * WEIGHT_STEP).min(1.0)
}

/// Plays `line` through the tree, every seat not in it folding. `None` if the tree does
/// not lead there.
pub fn steer(sol: &Solution, params: &Params, line: &[Step]) -> Option<(Vec<(usize, usize)>, Walk)> {
    let mut walk = Walk::start(params);
    let mut path = Vec::new();
    let mut k = 0;
    while let Some(v) = walk.node {
        let seat = sol.nodes[v].seat;
        let a = match line.get(k) {
            Some(step) if step.seat == seat => {
                k += 1;
                action_index(sol, v, step)?
            }
            _ if sol.nodes[v].actions[0].kind == Kind::Fold => 0,
            _ => return None,
        };
        path.push((v, a));
        walk.play(sol, a);
    }
    (k == line.len()).then_some((path, walk))
}

/// The model's heads-up flops at this table, most frequent first.
pub fn scenarios(sol: &Solution, params: &Params) -> Vec<Scenario> {
    sol.realized
        .iter()
        .enumerate()
        .filter_map(|(index, r)| {
            let (path, walk) = steer(sol, params, &r.line)?;
            let sc = heads_up(sol, params, &path, &walk, Some(index), r.reach)?;
            (sc.seats()[..] == r.seats[..]).then_some(sc)
        })
        .collect()
}

/// The heads-up flop a hand reaches by `path` (every decision on the way, folds included),
/// `None` unless exactly two players see it and both arrive with a range.
pub fn heads_up(sol: &Solution, params: &Params, path: &[(usize, usize)], walk: &Walk, index: Option<usize>, reach: f64) -> Option<Scenario> {
    let End::Flop(left) = walk.end() else { return None };
    let [a, b] = left[..] else { return None };
    let n = params.players;
    let (oop, ip) = if postflop_rank(n, a) < postflop_rank(n, b) { (a, b) } else { (b, a) };
    let weights = [oop, ip].map(|seat| {
        let mut w = vec![1.0; NUM_CLASSES];
        for &(v, a) in path.iter().filter(|&&(v, _)| sol.nodes[v].seat == seat) {
            for (c, x) in w.iter_mut().enumerate() {
                *x *= sol.nodes[v].freq[a][c];
            }
        }
        w.into_iter().map(round).collect::<Vec<f64>>()
    });
    if weights.iter().any(|w| w.iter().all(|&x| x == 0.0)) {
        return None;
    }
    let raises = walk.line.iter().filter(|s| matches!(s.action.kind, Kind::Raise | Kind::AllIn)).count();
    let pot_type = match raises {
        0 => PotType::Limped,
        1 => PotType::Srp,
        _ => PotType::ThreeBet,
    };
    let behind = walk.behind();
    Some(Scenario {
        index,
        stack_bb: params.stack_bb,
        line: walk.line.clone(),
        oop,
        ip,
        pot_type,
        reach,
        pot: (walk.pot() * 100.0).round() / 100.0,
        stack: (behind[oop].min(behind[ip]) * 100.0).round() / 100.0,
        path: path.to_vec(),
        weights,
    })
}
