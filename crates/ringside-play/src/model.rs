//! Walking the preflop model's tree (see `ringside_engine::preflop`) action by action.
//!
//! The model stores its nodes depth first with the fold child first, and a node is named by
//! its seat and the non-fold actions before it. So the node after a fold is the next one in
//! the list if it has the same line, and the node after any other action is the first later
//! node whose line is the old line plus that action. Without such a node the hand left the
//! tree: someone won the blinds, an all-in is called or a flop is seen.

use ringside_engine::preflop::{Kind, Params, Solution, Step};

/// How a preflop hand ended.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    Won(usize),
    /// An all-in was called; the seats play for the pot without further decisions.
    Showdown(Vec<usize>),
    /// The seats see a flop, in seat order.
    Flop(Vec<usize>),
}

#[derive(Clone, Debug)]
pub struct Walk {
    /// The node to act at; `None` once the hand left the tree.
    pub node: Option<usize>,
    pub line: Vec<Step>,
    /// Chips in front of each seat (blinds included, antes not), BB.
    pub contrib: Vec<f64>,
    pub folded: Vec<bool>,
    /// Stack of each seat after its ante, BB.
    pub live: Vec<f64>,
    pub ante: Vec<f64>,
}

impl Walk {
    pub fn start(params: &Params) -> Walk {
        let n = params.players;
        let ante: Vec<f64> = (0..n).map(|s| params.ante_bb + if s == n - 1 { params.bb_ante_bb } else { 0.0 }).collect();
        let live = ante.iter().map(|a| params.stack_bb - a).collect();
        let mut contrib = vec![0.0; n];
        contrib[n - 2] = params.small_blind_bb;
        contrib[n - 1] = 1.0;
        Walk { node: Some(0), line: Vec::new(), contrib, folded: vec![false; n], live, ante }
    }

    pub fn seat(&self, sol: &Solution) -> Option<usize> {
        self.node.map(|v| sol.nodes[v].seat)
    }

    pub fn pot(&self) -> f64 {
        self.ante.iter().sum::<f64>() + self.contrib.iter().sum::<f64>()
    }

    pub fn play(&mut self, sol: &Solution, a: usize) {
        let v = self.node.expect("a node to act at");
        let node = &sol.nodes[v];
        let action = node.actions[a];
        if action.kind == Kind::Fold {
            self.folded[node.seat] = true;
            self.node = sol.nodes.get(v + 1).filter(|m| m.line == node.line).map(|_| v + 1);
            return;
        }
        self.contrib[node.seat] = action.to_bb;
        self.line.push(Step { seat: node.seat, action });
        let line = &self.line;
        self.node = sol.nodes[v + 1..].iter().position(|m| &m.line == line).map(|i| v + 1 + i);
    }

    /// Who is still in once the hand left the tree. A seat that did not match the largest
    /// bet (and is not all in for less) dropped out, even without a fold node of its own:
    /// the model lets everyone but the first to continue against a 3-bet fold silently.
    pub fn end(&self) -> End {
        let top = self.contrib.iter().copied().fold(0.0, f64::max);
        let all_in = |s: usize| self.contrib[s] >= self.live[s] - 1e-9;
        let left: Vec<usize> = (0..self.contrib.len())
            .filter(|&s| !self.folded[s] && (self.contrib[s] >= top - 1e-9 || all_in(s)))
            .collect();
        if left.len() == 1 {
            End::Won(left[0])
        } else if left.iter().any(|&s| all_in(s)) {
            End::Showdown(left)
        } else {
            End::Flop(left)
        }
    }

    /// Chips behind each seat, BB.
    pub fn behind(&self) -> Vec<f64> {
        self.live.iter().zip(&self.contrib).map(|(l, c)| l - c).collect()
    }
}

/// Index of `step`'s action at node `v`, if the node offers it. Amounts are compared
/// loosely: the model's flop lines are stored with single precision.
pub fn action_index(sol: &Solution, v: usize, step: &Step) -> Option<usize> {
    sol.nodes[v].actions.iter().position(|a| a.kind == step.action.kind && (a.to_bb - step.action.to_bb).abs() < 1e-3)
}
