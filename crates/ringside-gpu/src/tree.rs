//! The game tree of a `PostFlopGame` laid out flat for the GPU, and the way back: the GPU's
//! cumulative strategy written into the game, so postflop-solver measures its exploitability.

use std::collections::HashMap;

use postflop_solver::{Game, GameNode, NOT_DEALT, PostFlopGame, PostFlopNode};
use ringside_engine::{Card, eval::evaluate_cards};

pub(crate) const NODE_WORDS: usize = 10;
pub(crate) const MAX_ACTIONS: usize = 8;
/// Words per board and player in the showdown header, see `showdown_block`.
pub(crate) const HEADER_WORDS: usize = 8;
/// Words per hand record in the showdown data.
const RECORD_WORDS: usize = 9;
const DECISION: u32 = 0;
const FOLD: u32 = 1;
const SHOWDOWN: u32 = 2;
const CHANCE: u32 = 3;
const NONE: u32 = u32::MAX;

/// A decision node: where its regrets live and whose hands they belong to.
#[derive(Clone, Copy, Debug)]
pub struct Decision {
    pub offset: usize,
    pub actions: usize,
    pub player: usize,
    /// Index in the node records, for the parent chain.
    pub node: usize,
    pub river: u8,
}

pub struct FlatTree {
    /// Node records (`NODE_WORDS` each: kind, player or folder, parent, action index, children
    /// offset, child count, storage offset, payoff or chance weight, showdown block, unused),
    /// then the child lists, then the node lists of `levels`, `inner` and `terminals`.
    pub topo: Vec<u32>,
    pub num_nodes: usize,
    /// Per depth: offset and length in `topo` of all nodes, and of the decision and chance nodes.
    pub levels: Vec<(u32, u32)>,
    pub inner: Vec<(u32, u32)>,
    pub terminals: (u32, u32),
    pub hands: [usize; 2],
    pub stride: usize,
    /// Regrets and strategy sums, `actions × hands of the acting player` per decision node.
    pub storage_len: usize,
    /// In depth-first order, the order of `write_strategy`.
    pub decisions: Vec<Decision>,
    /// Per board a header for each player, then sorted hand lists and per-hand records.
    pub showdown: Vec<u32>,
    /// Longest shared array the terminal kernel scans, in floats.
    pub scan_len: usize,
    pub initial_weights: [Vec<f32>; 2],
    private_cards: [Vec<(u8, u8)>; 2],
}

struct Rec {
    kind: u32,
    player: u32,
    parent: u32,
    action: u32,
    children: Vec<u32>,
    store: u32,
    value: f32,
    block: u32,
    depth: usize,
}

struct Export<'a> {
    game: &'a PostFlopGame,
    hands: [usize; 2],
    pot: f64,
    num_combinations: f64,
    board: u64,
    blocks: HashMap<u64, u32>,
    nodes: Vec<Rec>,
    storage_len: usize,
    decisions: Vec<Decision>,
}

fn mask(cards: &[u8]) -> u64 {
    cards.iter().fold(0, |m, &c| m | 1 << c)
}

impl Export<'_> {
    fn visit(&mut self, node: &PostFlopNode, parent: u32, action: u32, depth: usize) -> Result<u32, String> {
        let id = self.nodes.len() as u32;
        let mut rec =
            Rec { kind: DECISION, player: 0, parent, action, children: Vec::new(), store: 0, value: 0.0, block: 0, depth };
        if node.is_terminal() {
            let river = node.river();
            let board = if river == NOT_DEALT { self.board } else { self.board | 1 << river };
            let next = self.blocks.len() as u32;
            rec.block = *self.blocks.entry(board).or_insert(next);
            rec.value = ((self.pot / 2.0 + node.amount() as f64) / self.num_combinations) as f32;
            match node.folded_player() {
                Some(folder) => (rec.kind, rec.player) = (FOLD, folder as u32),
                None if board.count_ones() == 5 => rec.kind = SHOWDOWN,
                None => return Err("Showdown vor dem River".into()),
            }
            self.nodes.push(rec);
            return Ok(id);
        }
        let num_actions = node.num_actions();
        if node.is_chance() {
            if !self.game.isomorphic_chances(node).is_empty() {
                return Err("Boards mit gleichwertigen Farben werden noch nicht unterstützt.".into());
            }
            rec.kind = CHANCE;
            rec.value = 1.0 / self.game.chance_factor(node) as f32;
        } else {
            if num_actions > MAX_ACTIONS {
                return Err(format!("Höchstens {MAX_ACTIONS} Aktionen je Knoten."));
            }
            let player = node.player();
            rec.player = player as u32;
            rec.store = self.storage_len as u32;
            let river = node.river();
            self.decisions.push(Decision { offset: self.storage_len, actions: num_actions, player, node: id as usize, river });
            self.storage_len += num_actions * self.hands[player];
        }
        self.nodes.push(rec);
        for a in 0..num_actions {
            let child = self.visit(&node.play(a), id, a as u32, depth + 1)?;
            self.nodes[id as usize].children.push(child);
        }
        Ok(id)
    }
}

impl FlatTree {
    /// `game` must have its memory allocated without compression.
    pub fn new(game: &PostFlopGame) -> Result<FlatTree, String> {
        let config = game.card_config();
        let mut board = config.flop.to_vec();
        board.extend([config.turn, config.river].into_iter().filter(|&c| c != NOT_DEALT));
        let private_cards = [game.private_cards(0).to_vec(), game.private_cards(1).to_vec()];
        let hands = [private_cards[0].len(), private_cards[1].len()];
        let initial_weights = [game.initial_weights(0).to_vec(), game.initial_weights(1).to_vec()];

        let mut num_combinations = 0.0;
        for (&(a, b), &w0) in private_cards[0].iter().zip(&initial_weights[0]) {
            for (&(c, d), &w1) in private_cards[1].iter().zip(&initial_weights[1]) {
                if mask(&[a, b]) & mask(&[c, d]) == 0 {
                    num_combinations += w0 as f64 * w1 as f64;
                }
            }
        }

        let mut export = Export {
            game,
            hands,
            pot: game.tree_config().starting_pot as f64,
            num_combinations,
            board: mask(&board),
            blocks: HashMap::new(),
            nodes: Vec::new(),
            storage_len: 0,
            decisions: Vec::new(),
        };
        export.visit(&game.root(), NONE, 0, 0)?;
        let Export { nodes, blocks, storage_len, decisions, .. } = export;

        let num_nodes = nodes.len();
        let mut topo = vec![0u32; num_nodes * NODE_WORDS];
        for (i, rec) in nodes.iter().enumerate() {
            let offset = topo.len() as u32;
            topo.extend(&rec.children);
            let value = rec.value.to_bits();
            let words = [rec.kind, rec.player, rec.parent, rec.action, offset, rec.children.len() as u32, rec.store, value, rec.block, 0];
            topo[i * NODE_WORDS..(i + 1) * NODE_WORDS].copy_from_slice(&words);
        }
        let depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0) + 1;
        let mut list = |keep: &dyn Fn(&Rec) -> bool, d: Option<usize>| {
            let offset = topo.len() as u32;
            topo.extend(nodes.iter().enumerate().filter(|(_, n)| keep(n) && d.is_none_or(|d| n.depth == d)).map(|(i, _)| i as u32));
            (offset, topo.len() as u32 - offset)
        };
        let levels = (0..depth).map(|d| list(&|_| true, Some(d))).collect();
        let inner = (0..depth).map(|d| list(&|n| n.kind == DECISION || n.kind == CHANCE, Some(d))).collect();
        let terminals = list(&|n| n.kind == FOLD || n.kind == SHOWDOWN, None);

        let mut boards: Vec<(u64, u32)> = blocks.into_iter().collect();
        boards.sort_by_key(|&(_, b)| b);
        let mut showdown = vec![0u32; boards.len() * 2 * HEADER_WORDS];
        let mut scan_len = 0;
        for (board, block) in boards {
            for player in 0..2 {
                scan_len = scan_len.max(showdown_block(&private_cards, board, player, block as usize, &mut showdown)?);
            }
        }

        Ok(FlatTree {
            topo,
            num_nodes,
            levels,
            inner,
            terminals,
            hands,
            stride: hands[0].max(hands[1]),
            storage_len,
            decisions,
            showdown,
            scan_len,
            initial_weights,
            private_cards,
        })
    }

    /// Copies cumulative strategies (laid out like the GPU storage) into the game's nodes.
    pub fn write_strategy(&self, game: &PostFlopGame, strategy_sum: &[f32]) {
        let mut next = 0;
        self.visit_decisions(&mut game.root(), &mut next, &mut |node, range| {
            node.strategy_mut().copy_from_slice(&strategy_sum[range]);
        });
    }

    /// The game's cumulative strategies in the GPU's layout.
    pub fn read_strategy(&self, game: &PostFlopGame) -> Vec<f32> {
        let mut out = vec![0.0; self.storage_len];
        let mut next = 0;
        self.visit_decisions(&mut game.root(), &mut next, &mut |node, range| {
            out[range].copy_from_slice(node.strategy());
        });
        out
    }

    /// Calls `f` on every decision node in depth-first order, with its range in the GPU storage.
    fn visit_decisions(
        &self,
        node: &mut PostFlopNode,
        next: &mut usize,
        f: &mut dyn FnMut(&mut PostFlopNode, std::ops::Range<usize>),
    ) {
        if node.is_terminal() {
            return;
        }
        if !node.is_chance() {
            let d = self.decisions[*next];
            *next += 1;
            f(node, d.offset..d.offset + d.actions * self.hands[d.player]);
        }
        for a in 0..node.num_actions() {
            self.visit_decisions(&mut node.play(a), next, f);
        }
    }

    /// Mean and largest difference in action probabilities between two cumulative strategies.
    /// Each hand counts with the chance that it reaches the node under `weighting`, so hands
    /// that never get there (and whose strategy is arbitrary) do not count.
    pub fn strategy_difference(&self, weighting: &[f32], other: &[f32]) -> (f64, f64) {
        let probability = |s: &[f32], d: &Decision, a: usize, h: usize| -> f64 {
            let hands = self.hands[d.player];
            let sum: f64 = (0..d.actions).map(|k| s[d.offset + k * hands + h] as f64).sum();
            if sum > 0.0 { s[d.offset + a * hands + h] as f64 / sum } else { 1.0 / d.actions as f64 }
        };
        let by_node: HashMap<usize, &Decision> = self.decisions.iter().map(|d| (d.node, d)).collect();
        let field = |n: usize, f: usize| self.topo[n * NODE_WORDS + f];
        let (mut total, mut weight, mut max) = (0.0, 0.0, 0.0f64);
        for d in &self.decisions {
            let hands = self.hands[d.player];
            for h in 0..hands {
                let (c1, c2) = self.private_cards[d.player][h];
                if d.river != NOT_DEALT && (c1 == d.river || c2 == d.river) {
                    continue;
                }
                let mut reach = self.initial_weights[d.player][h] as f64;
                let mut n = d.node;
                while field(n, 2) != NONE {
                    let parent = field(n, 2) as usize;
                    if let Some(p) = by_node.get(&parent).filter(|p| p.player == d.player) {
                        reach *= probability(weighting, p, field(n, 3) as usize, h);
                    }
                    n = parent;
                }
                for a in 0..d.actions {
                    let diff = (probability(weighting, d, a, h) - probability(other, d, a, h)).abs();
                    total += reach * diff;
                    if reach > 1e-3 {
                        max = max.max(diff);
                    }
                }
                weight += reach;
            }
        }
        (if weight > 0.0 { total / weight / 2.0 } else { 0.0 }, max)
    }
}

/// Showdown and fold data of `board` from `player`'s side, the O(n) scheme of postflop-solver:
/// the opponent's hands sorted by strength (one prefix sum gives what a hand beats) and, per
/// card, the opponent's hands holding it (their prefix sums remove the hands that collide).
///
/// All lists share one array that the terminal kernel scans in one go, a segment per list,
/// each segment starting with a 0 so that position `start + k` holds the sum of the first `k`.
/// Header: opponent hands `m`, array length, offset and count of the (hand, position) pairs
/// that fill it, offset of the bitmask of segment starts, offset of the hand records. A record:
/// positions of "weaker" and "weaker or equal" in the sorted list, the same and the list total
/// for each of the hand's two cards, the opponent's index of the identical combo (or NONE).
/// A first word of NONE: the hand collides with the board.
fn showdown_block(cards: &[Vec<(u8, u8)>; 2], board: u64, player: usize, block: usize, out: &mut Vec<u32>) -> Result<usize, String> {
    let opp = &cards[player ^ 1];
    let own = &cards[player];
    let board_cards: Vec<Card> = (0..52u8).filter(|c| board >> c & 1 == 1).map(Card::from_index).collect();
    let strength = |&(a, b): &(u8, u8)| -> Option<u16> {
        if board & mask(&[a, b]) != 0 {
            return None;
        }
        if board.count_ones() < 5 {
            return Some(0);
        }
        let mut hand = board_cards.clone();
        hand.extend([Card::from_index(a), Card::from_index(b)]);
        Some(evaluate_cards(&hand).0)
    };

    let mut sorted: Vec<(u16, u32)> =
        opp.iter().enumerate().filter_map(|(j, c)| strength(c).map(|s| (s, j as u32))).collect();
    sorted.sort();
    let mut lists: Vec<Vec<(u16, u32)>> = vec![Vec::new(); 52];
    for &(s, j) in &sorted {
        let (a, b) = opp[j as usize];
        lists[a as usize].push((s, j));
        lists[b as usize].push((s, j));
    }
    // Segment starts: the sorted list at 0, then one per card.
    let mut starts = vec![0usize];
    let mut len = sorted.len() + 1;
    for list in &lists {
        starts.push(len);
        len += list.len() + 1;
    }

    let header = (block * 2 + player) * HEADER_WORDS;
    let pairs_at = out.len();
    for (k, &(_, j)) in sorted.iter().enumerate() {
        out.extend([j, 1 + k as u32]);
    }
    for (c, list) in lists.iter().enumerate() {
        for (k, &(_, j)) in list.iter().enumerate() {
            out.extend([j, (starts[c + 1] + 1 + k) as u32]);
        }
    }
    let pairs_len = (out.len() - pairs_at) / 2;
    let heads_at = out.len();
    let mut heads = vec![0u32; len.div_ceil(32)];
    for &x in &starts {
        heads[x / 32] |= 1 << (x % 32);
    }
    out.extend(heads);
    let records_at = out.len();
    let below = |list: &[(u16, u32)], s: u16, equal: bool| list.partition_point(|&(t, _)| t < s || (equal && t == s));
    for hand in own {
        let Some(s) = strength(hand) else {
            out.extend([NONE; RECORD_WORDS]);
            continue;
        };
        let (a, b) = (starts[hand.0 as usize + 1], starts[hand.1 as usize + 1]);
        let (la, lb) = (&lists[hand.0 as usize], &lists[hand.1 as usize]);
        let same = opp.iter().position(|&(c, d)| mask(&[c, d]) == mask(&[hand.0, hand.1])).map_or(NONE, |j| j as u32);
        let record = [
            below(&sorted, s, false),
            below(&sorted, s, true),
            a + below(la, s, false),
            a + below(la, s, true),
            a + la.len(),
            b + below(lb, s, false),
            b + below(lb, s, true),
            b + lb.len(),
        ];
        out.extend(record.map(|x| x as u32));
        out.push(same);
    }
    let words = [sorted.len(), len, pairs_at, pairs_len, heads_at, records_at, 0, 0];
    for (i, w) in words.into_iter().enumerate() {
        out[header + i] = w as u32;
    }
    Ok(len)
}
