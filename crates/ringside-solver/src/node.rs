use postflop_solver::{Action, PostFlopGame};
use ringside_engine::{Card, Combo};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::solve::Solution;
use crate::spot::bb;

/// One move from the root: the index of an action, or the card dealt at a chance node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Action(usize),
    Card(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Player {
    Oop,
    Ip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// A player decides.
    Action,
    /// The next card is dealt.
    Chance,
    /// Someone folded or the hand reached showdown.
    Terminal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Street {
    Flop,
    Turn,
    River,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    Fold,
    Check,
    Call,
    Bet,
    Raise,
    AllIn,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActionInfo {
    pub kind: ActionKind,
    /// Bet, raise or all-in to this amount on the current street, in BB; 0 otherwise.
    pub amount: f64,
    /// Share of the acting player's range here that takes this action.
    pub frequency: f64,
    /// Combos taking this action, weights summed.
    pub combos: f64,
}

/// One combo of the acting player.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HandInfo {
    /// E.g. "AhKd".
    pub combo: String,
    /// `HandClass::index`: row × 13 + column in the usual 13×13 grid.
    pub class: usize,
    /// How much of the combo reaches this node, 0–1.
    pub weight: f64,
    /// Probability of each action, in the order of `Node::actions`.
    pub strategy: Vec<f64>,
    /// Share of the pot won at showdown against the opponent's range here.
    pub equity: f64,
    /// Expected share of the current pot minus further bets, in BB.
    pub ev: f64,
}

/// The combos of one hand class, averaged by how often each reaches the node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassInfo {
    pub class: usize,
    pub label: String,
    pub combos: f64,
    pub strategy: Vec<f64>,
    pub equity: f64,
    pub ev: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayerSummary {
    pub combos: f64,
    pub equity: f64,
    pub ev: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Node {
    pub path: Vec<Step>,
    pub kind: NodeKind,
    pub street: Street,
    pub board: Vec<String>,
    /// Starting pot and every bet since, in BB.
    pub pot: f64,
    /// What OOP and IP put in since the root, and what they have left behind.
    pub bets: [f64; 2],
    pub stacks: [f64; 2],
    /// Who acts at an action node.
    pub player: Option<Player>,
    pub actions: Vec<ActionInfo>,
    /// Cards that can come at a chance node.
    pub cards: Vec<String>,
    /// The acting player's combos that reach this node.
    pub hands: Vec<HandInfo>,
    /// `hands` per hand class, for the 13×13 grid.
    pub classes: Vec<ClassInfo>,
    /// OOP and IP as a whole.
    pub players: [PlayerSummary; 2],
}

fn card_name(card: u8) -> String {
    Card::from_index(card).to_string()
}

impl Solution {
    /// The node reached from the root by `path`.
    pub fn node(&mut self, path: &[Step]) -> Result<Node, Error> {
        let game = &mut self.game;
        game.back_to_root();
        for step in path {
            walk(game, step)?;
        }
        game.cache_normalized_weights();
        Ok(view(game, path, self.spot.stack))
    }
}

fn walk(game: &mut PostFlopGame, step: &Step) -> Result<(), Error> {
    if game.is_terminal_node() {
        return Err(Error::Invalid("Die Hand ist an dieser Stelle schon zu Ende.".into()));
    }
    match step {
        Step::Card(text) => {
            if !game.is_chance_node() {
                return Err(Error::Invalid("Hier kommt keine Karte, zuerst ist ein Spieler dran.".into()));
            }
            let card: Card = text.parse().map_err(|_| Error::Invalid(format!("„{text}“ ist keine Karte.")))?;
            if game.possible_cards() & card.mask() == 0 {
                return Err(Error::Invalid(format!("{card} kann hier nicht kommen.")));
            }
            game.play(card.index() as usize);
        }
        Step::Action(index) => {
            if game.is_chance_node() {
                return Err(Error::Invalid("Hier kommt zuerst die nächste Karte.".into()));
            }
            if *index >= game.available_actions().len() {
                return Err(Error::Invalid("Diese Aktion gibt es hier nicht.".into()));
            }
            game.play(*index);
        }
    }
    Ok(())
}

fn weighted_average(values: &[f32], weights: &[f32]) -> f64 {
    let total: f64 = weights.iter().map(|&w| w as f64).sum();
    if total <= 0.0 {
        return 0.0;
    }
    values.iter().zip(weights).map(|(&v, &w)| v as f64 * w as f64).sum::<f64>() / total
}

fn view(game: &PostFlopGame, path: &[Step], stack: f64) -> Node {
    let board = game.current_board();
    let street = match board.len() {
        3 => Street::Flop,
        4 => Street::Turn,
        _ => Street::River,
    };
    let bets = game.total_bet_amount().map(|b| bb(b as f64));
    let pot = bb(game.tree_config().starting_pot as f64) + bets[0] + bets[1];
    let kind = if game.is_terminal_node() {
        NodeKind::Terminal
    } else if game.is_chance_node() {
        NodeKind::Chance
    } else {
        NodeKind::Action
    };

    // Combos count each player's own reach; averages also weigh in the opponent's blockers.
    let players = [0, 1].map(|p| {
        let normalized = game.normalized_weights(p);
        PlayerSummary {
            combos: game.weights(p).iter().map(|&w| w as f64).sum(),
            equity: weighted_average(&game.equity(p), normalized),
            ev: bb(weighted_average(&game.expected_values(p), normalized)),
        }
    });

    let mut node = Node {
        path: path.to_vec(),
        kind,
        street,
        board: board.iter().map(|&c| card_name(c)).collect(),
        pot,
        bets,
        stacks: bets.map(|b| stack - b),
        player: None,
        actions: Vec::new(),
        cards: Vec::new(),
        hands: Vec::new(),
        classes: Vec::new(),
        players,
    };
    match kind {
        NodeKind::Terminal => {}
        NodeKind::Chance => {
            let possible = game.possible_cards();
            node.cards = (0..52u8).filter(|c| possible & (1 << c) != 0).map(card_name).collect();
        }
        NodeKind::Action => fill_action(game, &mut node),
    }
    node
}

fn fill_action(game: &PostFlopGame, node: &mut Node) {
    let player = game.current_player();
    node.player = Some(if player == 0 { Player::Oop } else { Player::Ip });
    let actions = game.available_actions();
    let strategy = game.strategy();
    let equity = game.equity(player);
    let ev = game.expected_values(player);
    let weights = game.weights(player);
    let normalized = game.normalized_weights(player);
    let cards = game.private_cards(player);
    let n = cards.len();

    let mut per_action = vec![0.0f64; actions.len()];
    let mut classes: Vec<Option<ClassInfo>> = vec![None; 169];
    let mut class_norm = [0.0f64; 169];
    for (i, &(a, b)) in cards.iter().enumerate() {
        let weight = weights[i] as f64;
        if weight <= 0.0 {
            continue;
        }
        let norm = normalized[i] as f64;
        let combo = Combo::new(Card::from_index(a), Card::from_index(b));
        let class = combo.class();
        let hand = HandInfo {
            combo: combo.to_string(),
            class: class.index(),
            weight,
            strategy: (0..actions.len()).map(|k| strategy[k * n + i] as f64).collect(),
            equity: equity[i] as f64,
            ev: bb(ev[i] as f64),
        };
        let entry = classes[class.index()].get_or_insert_with(|| ClassInfo {
            class: class.index(),
            label: class.to_string(),
            combos: 0.0,
            strategy: vec![0.0; actions.len()],
            equity: 0.0,
            ev: 0.0,
        });
        entry.combos += weight;
        entry.equity += norm * hand.equity;
        entry.ev += norm * hand.ev;
        class_norm[class.index()] += norm;
        for (k, &s) in hand.strategy.iter().enumerate() {
            entry.strategy[k] += weight * s;
            per_action[k] += weight * s;
        }
        node.hands.push(hand);
    }
    node.classes = classes
        .into_iter()
        .flatten()
        .map(|mut c| {
            let norm = class_norm[c.class];
            (c.equity, c.ev) = if norm > 0.0 { (c.equity / norm, c.ev / norm) } else { (0.0, 0.0) };
            c.strategy.iter_mut().for_each(|s| *s /= c.combos);
            c
        })
        .collect();

    let total: f64 = per_action.iter().sum();
    node.actions = actions
        .iter()
        .zip(&per_action)
        .map(|(action, &combos)| {
            let (kind, amount) = match *action {
                Action::Fold => (ActionKind::Fold, 0),
                Action::Check => (ActionKind::Check, 0),
                Action::Call => (ActionKind::Call, 0),
                Action::Bet(a) => (ActionKind::Bet, a),
                Action::Raise(a) => (ActionKind::Raise, a),
                Action::AllIn(a) => (ActionKind::AllIn, a),
                Action::None | Action::Chance(_) => unreachable!("action nodes offer player actions only"),
            };
            ActionInfo {
                kind,
                amount: bb(amount as f64),
                frequency: if total > 0.0 { combos / total } else { 0.0 },
                combos,
            }
        })
        .collect();
}
