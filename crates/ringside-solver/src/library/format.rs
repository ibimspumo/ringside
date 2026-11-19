//! Compact storage of a solved catalog spot.
//!
//! A full solution is the whole game tree, gigabytes. What a player studies is the flop
//! and the first decision on each turn, so that is what is kept:
//! - every flop decision of both players, per combo: the strategy, the equity and the EV;
//! - for every flop line that reaches the turn with chips behind, and every turn card,
//!   the first turn decision per hand class: combos, strategy, equity and EV.
//!
//! Everything else follows from the catalog: the ranges, the board and the sizes. Hands
//! are listed in `Combo::index` order of the range's live combos, so the file names no
//! cards; a hand is stored at a node only while its own quantized frequencies on the way
//! there are all positive, which reader and writer decide the same way. Turn cards that a
//! suit swap maps onto an earlier one (the flop stays the same) refer to it, because their
//! class data is the same.
//!
//! # Encoding
//! Frequencies are bytes summing to 255 per hand (largest remainder), equity a byte,
//! EVs i16 hundredths of a BB, amounts i32 hundredths of a BB, all little-endian:
//!
//! ```text
//! header  magic "RSLS", format u8, catalog version u16, scenario (u8 length + ASCII),
//!         flop (6 ASCII), iterations u32, exploitability f32 (percent of the pot)
//! body    deflate of: flop node count u16, nodes in depth-first order, turn line count
//!         u16, turn lines
//! node    player u8, bets 2×i32, action count u8, per action kind u8 + amount i32 +
//!         child u8 (0 end, 1 next node, 2 turn line, 3 all-in run-out), per player
//!         equity f32 + EV f32, then per stored hand the action bytes, equity u8, EV i16
//! line    per turn card (49, by card index): 0xFF + data, or the index of the earlier
//!         card it equals; data: bets 2×i32, action count u8, per action kind u8 + amount
//!         i32, per player equity f32 + EV f32, class count u8, per class index u8,
//!         combos u8 (in 1/20), action bytes, equity u8, EV i16
//! ```

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use ringside_engine::{Card, CardSet, Combo, HandClass, Range};

use super::catalog::{Catalog, Scenario};
use crate::Error;
use crate::node::{ActionInfo, ActionKind, ClassInfo, HandInfo, Node, NodeKind, Player, PlayerSummary, Step, Street};
use crate::solve::Solution;
use crate::spot::{CHIPS_PER_BB, bb, parse_board};

pub const MAGIC: &[u8; 4] = b"RSLS";
pub const FORMAT: u8 = 1;
/// Largest body a reader inflates; real spots are far below.
const MAX_INFLATED: u64 = 4 << 20;
const SAME_AS_NONE: u8 = 0xFF;
const COMBO_STEP: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    End,
    Node,
    Turn,
    Runout,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredAction {
    pub kind: ActionKind,
    /// Hundredths of a BB.
    pub amount: i32,
}

/// Equity (0–1) and EV (BB) of each player at a node.
pub type Summary = [[f32; 2]; 2];

#[derive(Clone, Debug, PartialEq)]
pub struct FlopNode {
    pub player: u8,
    pub bets: [i32; 2],
    pub actions: Vec<StoredAction>,
    pub children: Vec<Child>,
    pub summary: Summary,
    /// Per stored hand: one byte per action, summing to 255.
    pub strategy: Vec<u8>,
    pub equity: Vec<u8>,
    pub ev: Vec<i16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnClass {
    pub class: u8,
    /// In 1/20 combo.
    pub combos: u8,
    pub strategy: Vec<u8>,
    pub equity: u8,
    pub ev: i16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnRoot {
    pub bets: [i32; 2],
    pub actions: Vec<StoredAction>,
    pub summary: Summary,
    pub classes: Vec<TurnClass>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TurnEntry {
    /// Index (into the line) of an earlier card with the same data.
    Same(u8),
    Root(TurnRoot),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stored {
    pub catalog: u32,
    pub scenario: String,
    pub flop: String,
    pub iterations: u32,
    pub exploitability_pct: f32,
    pub nodes: Vec<FlopNode>,
    /// One entry per turn card, see [`turn_cards`].
    pub turns: Vec<Vec<TurnEntry>>,
}

fn invalid(m: impl Into<String>) -> Error {
    Error::Invalid(m.into())
}

fn chips(bb: f64) -> i32 {
    (bb * CHIPS_PER_BB).round() as i32
}

fn kind_code(k: ActionKind) -> u8 {
    match k {
        ActionKind::Fold => 0,
        ActionKind::Check => 1,
        ActionKind::Call => 2,
        ActionKind::Bet => 3,
        ActionKind::Raise => 4,
        ActionKind::AllIn => 5,
    }
}

fn kind_of(code: u8) -> Result<ActionKind, Error> {
    Ok(match code {
        0 => ActionKind::Fold,
        1 => ActionKind::Check,
        2 => ActionKind::Call,
        3 => ActionKind::Bet,
        4 => ActionKind::Raise,
        5 => ActionKind::AllIn,
        _ => return Err(invalid("unknown action")),
    })
}

/// Bytes summing to 255 with the smallest rounding error (largest remainder); an action
/// with frequency 0 stays 0.
pub fn quantize(freqs: &[f64]) -> Vec<u8> {
    let total: f64 = freqs.iter().map(|f| f.max(0.0)).sum();
    if total.is_nan() || total <= 0.0 {
        let mut out = vec![0u8; freqs.len()];
        if let Some(first) = out.first_mut() {
            *first = 255;
        }
        return out;
    }
    let scaled: Vec<f64> = freqs.iter().map(|f| f.max(0.0) / total * 255.0).collect();
    let mut out: Vec<u8> = scaled.iter().map(|x| x.floor() as u8).collect();
    let mut left = 255 - out.iter().map(|&x| x as i32).sum::<i32>();
    let mut order: Vec<usize> = (0..freqs.len()).filter(|&i| scaled[i] > 0.0).collect();
    order.sort_by(|&a, &b| (scaled[b] - scaled[b].floor()).total_cmp(&(scaled[a] - scaled[a].floor())).then(a.cmp(&b)));
    for &i in order.iter().cycle() {
        if left <= 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    out
}

/// The 49 cards that can come on the turn, by card index.
pub fn turn_cards(flop: &[Card]) -> Vec<Card> {
    let dead = CardSet::from_cards(flop);
    (0..52u8).map(Card::from_index).filter(|c| !dead.contains(*c)).collect()
}

/// For each turn card, the earliest turn card a suit swap that keeps the flop maps it to.
fn turn_aliases(flop: &[Card], cards: &[Card]) -> Vec<usize> {
    let flop_set = CardSet::from_cards(flop);
    let mut perms = Vec::new();
    for a in 0..4u8 {
        for b in (0..4).filter(|&b| b != a) {
            for c in (0..4).filter(|&c| c != a && c != b) {
                let p = [a, b, c, 6 - a - b - c];
                let mapped: Vec<Card> = flop.iter().map(|x| Card::new(x.rank(), p[x.suit() as usize])).collect();
                if CardSet::from_cards(&mapped) == flop_set {
                    perms.push(p);
                }
            }
        }
    }
    cards
        .iter()
        .enumerate()
        .map(|(i, t)| {
            perms
                .iter()
                .filter_map(|p| cards.iter().position(|x| *x == Card::new(t.rank(), p[t.suit() as usize])))
                .min()
                .unwrap_or(i)
        })
        .collect()
}

/// Live combos of a range on the flop with their weights, in `Combo::index` order.
pub fn hands(range: &str, flop: &[Card]) -> Result<Vec<(Combo, f32)>, Error> {
    let r = Range::parse(range).map_err(|e| invalid(format!("range: {e}")))?;
    Ok(r.live_combos(CardSet::from_cards(flop)))
}

// Encoding from a solution.

fn actions_of(node: &Node) -> Vec<StoredAction> {
    node.actions.iter().map(|a| StoredAction { kind: a.kind, amount: chips(a.amount) }).collect()
}

fn summary_of(node: &Node) -> Summary {
    node.players.clone().map(|p| [p.equity as f32, p.ev as f32])
}

fn bets_of(node: &Node) -> [i32; 2] {
    node.bets.map(chips)
}

fn equity_byte(e: f64) -> u8 {
    (e.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn ev_steps(ev: f64) -> i16 {
    (ev * 100.0).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

struct Encoder<'a> {
    solution: &'a mut Solution,
    /// Per player: live combos in storage order.
    combos: [Vec<Combo>; 2],
    turn_cards: Vec<Card>,
    aliases: Vec<usize>,
    nodes: Vec<FlopNode>,
    turns: Vec<Vec<TurnEntry>>,
}

impl Encoder<'_> {
    /// `alive[p][h]`: hand `h` of player `p` still reaches this node by the stored bytes.
    fn flop_node(&mut self, path: Vec<Step>, alive: [Vec<bool>; 2]) -> Result<(), Error> {
        let node = self.solution.node(&path)?;
        let player = match node.player {
            Some(Player::Oop) => 0,
            Some(Player::Ip) => 1,
            None => return Err(invalid("flop node without a player")),
        };
        let by_combo: std::collections::HashMap<&str, &HandInfo> =
            node.hands.iter().map(|h| (h.combo.as_str(), h)).collect();
        let n_actions = node.actions.len();
        let mut stored = FlopNode {
            player: player as u8,
            bets: bets_of(&node),
            actions: actions_of(&node),
            children: Vec::with_capacity(n_actions),
            summary: summary_of(&node),
            strategy: Vec::new(),
            equity: Vec::new(),
            ev: Vec::new(),
        };
        // Quantized strategy of every hand, for the hands still alive after each action.
        let mut quantized: Vec<Option<Vec<u8>>> = vec![None; self.combos[player].len()];
        for (h, combo) in self.combos[player].iter().enumerate() {
            if !alive[player][h] {
                continue;
            }
            let info = by_combo.get(combo.to_string().as_str()).ok_or_else(|| invalid("stored hand missing at node"))?;
            let q = quantize(&info.strategy);
            stored.strategy.extend_from_slice(&q);
            stored.equity.push(equity_byte(info.equity));
            stored.ev.push(ev_steps(info.ev));
            quantized[h] = Some(q);
        }
        let index = self.nodes.len();
        self.nodes.push(stored);

        for a in 0..n_actions {
            let mut next = alive.clone();
            for (h, q) in quantized.iter().enumerate() {
                if let Some(q) = q {
                    next[player][h] = q[a] > 0;
                }
            }
            let mut child_path = path.clone();
            child_path.push(Step::Action(a));
            let child = self.solution.node(&child_path)?;
            let kind = match child.kind {
                NodeKind::Terminal => Child::End,
                NodeKind::Action => Child::Node,
                NodeKind::Chance if child.stacks.iter().any(|&s| s <= 1e-9) => Child::Runout,
                NodeKind::Chance => Child::Turn,
            };
            self.nodes[index].children.push(kind);
            match kind {
                Child::Node => self.flop_node(child_path, next)?,
                Child::Turn => self.turn_line(child_path)?,
                Child::End | Child::Runout => {}
            }
        }
        Ok(())
    }

    fn turn_line(&mut self, path: Vec<Step>) -> Result<(), Error> {
        let mut line = Vec::with_capacity(self.turn_cards.len());
        for (i, card) in self.turn_cards.clone().into_iter().enumerate() {
            if self.aliases[i] != i {
                line.push(TurnEntry::Same(self.aliases[i] as u8));
                continue;
            }
            let mut p = path.clone();
            p.push(Step::Card(card.to_string()));
            let node = self.solution.node(&p)?;
            if node.kind != NodeKind::Action {
                return Err(invalid("turn root is not a decision"));
            }
            let classes = node
                .classes
                .iter()
                .filter(|c| c.combos > 0.0)
                .map(|c| TurnClass {
                    class: c.class as u8,
                    combos: (c.combos / COMBO_STEP).round().clamp(0.0, 255.0) as u8,
                    strategy: quantize(&c.strategy),
                    equity: equity_byte(c.equity),
                    ev: ev_steps(c.ev),
                })
                .collect();
            line.push(TurnEntry::Root(TurnRoot {
                bets: bets_of(&node),
                actions: actions_of(&node),
                summary: summary_of(&node),
                classes,
            }));
        }
        self.turns.push(line);
        Ok(())
    }
}

/// Stores a solved catalog spot of `scenario` on `flop`.
pub fn encode(solution: &mut Solution, catalog: &Catalog, scenario: &Scenario, flop: &str) -> Result<Stored, Error> {
    let flop_cards = parse_board(flop)?;
    if solution.spot != scenario.spot(flop) {
        return Err(invalid("the solution is for another spot"));
    }
    let combos = [&scenario.oop_range, &scenario.ip_range]
        .map(|r| hands(r, &flop_cards).map(|h| h.into_iter().map(|(c, _)| c).collect::<Vec<_>>()));
    let [oop, ip] = combos;
    let combos = [oop?, ip?];
    let turn_cards = turn_cards(&flop_cards);
    let aliases = turn_aliases(&flop_cards, &turn_cards);
    let alive = [vec![true; combos[0].len()], vec![true; combos[1].len()]];
    let summary = solution.summary.clone();
    let mut enc = Encoder { solution, combos, turn_cards, aliases, nodes: Vec::new(), turns: Vec::new() };
    enc.flop_node(Vec::new(), alive)?;
    Ok(Stored {
        catalog: catalog.version,
        scenario: scenario.id.clone(),
        flop: flop.to_string(),
        iterations: summary.iterations,
        exploitability_pct: summary.exploitability_pct as f32,
        nodes: enc.nodes,
        turns: enc.turns,
    })
}

// Suit renaming.

impl Stored {
    /// The same spot on `flop`, a suit renaming of this one's flop: hands and turn cards
    /// move, nothing is recomputed. `scenario` gives the ranges, which must weigh every
    /// suit alike (class ranges do).
    pub fn renamed(&self, scenario: &Scenario, flop: &str) -> Result<Stored, Error> {
        let from = parse_board(&self.flop)?;
        let to = parse_board(flop)?;
        let perm = crate::suits::between(&from, &to).ok_or_else(|| invalid("the flops are no suit renaming of each other"))?;
        let rename = |c: Card| Card::new(c.rank(), perm[c.suit() as usize]);
        // Per player: for each hand in this flop's storage order, its place on the new flop.
        let mut places: [Vec<usize>; 2] = [Vec::new(), Vec::new()];
        for (p, range) in [&scenario.oop_range, &scenario.ip_range].into_iter().enumerate() {
            let old = hands(range, &from)?;
            let new = hands(range, &to)?;
            let mut at = vec![usize::MAX; 1326];
            for (i, (combo, _)) in new.iter().enumerate() {
                at[combo.index()] = i;
            }
            for (combo, weight) in &old {
                let (a, b) = combo.cards();
                let i = at[Combo::new(rename(a), rename(b)).index()];
                if i == usize::MAX || new[i].1 != *weight || old.len() != new.len() {
                    return Err(invalid("the ranges treat suits differently"));
                }
                places[p].push(i);
            }
        }
        let alive = [vec![true; places[0].len()], vec![true; places[1].len()]];
        let mut nodes = Vec::with_capacity(self.nodes.len());
        let mut next = 0;
        self.rename_node(&mut next, alive, &places, &mut nodes)?;
        if next != self.nodes.len() {
            return Err(invalid("extra flop nodes"));
        }

        let old_cards = turn_cards(&from);
        let new_cards = turn_cards(&to);
        let aliases = turn_aliases(&to, &new_cards);
        let mut back = [0u8; 4];
        for (old, &new) in perm.iter().enumerate() {
            back[new as usize] = old as u8;
        }
        let mut turns = Vec::with_capacity(self.turns.len());
        for line in &self.turns {
            let root = |i: usize| match line.get(i) {
                Some(TurnEntry::Root(r)) => Ok(r),
                Some(TurnEntry::Same(j)) => match line.get(*j as usize) {
                    Some(TurnEntry::Root(r)) => Ok(r),
                    _ => Err(invalid("turn reference")),
                },
                None => Err(invalid("turn line too short")),
            };
            let mut out = Vec::with_capacity(new_cards.len());
            for (j, card) in new_cards.iter().enumerate() {
                if aliases[j] != j {
                    out.push(TurnEntry::Same(aliases[j] as u8));
                    continue;
                }
                let old = Card::new(card.rank(), back[card.suit() as usize]);
                let i = old_cards.iter().position(|c| *c == old).ok_or_else(|| invalid("turn card"))?;
                out.push(TurnEntry::Root(root(i)?.clone()));
            }
            turns.push(out);
        }
        Ok(Stored { flop: flop.to_string(), nodes, turns, ..self.clone() })
    }

    /// Copies node `*next` and its subtree with every hand row moved to its new place.
    fn rename_node(&self, next: &mut usize, alive: [Vec<bool>; 2], places: &[Vec<usize>; 2], out: &mut Vec<FlopNode>) -> Result<(), Error> {
        let node = self.nodes.get(*next).ok_or_else(|| invalid("missing flop nodes"))?;
        *next += 1;
        let p = node.player as usize;
        let k = node.actions.len();
        let hands: Vec<usize> = (0..alive[p].len()).filter(|&h| alive[p][h]).collect();
        if node.equity.len() != hands.len() || node.strategy.len() != hands.len() * k {
            return Err(invalid("stored hands do not fit the ranges"));
        }
        let mut order: Vec<usize> = (0..hands.len()).collect();
        order.sort_by_key(|&row| places[p][hands[row]]);
        let mut renamed = FlopNode { strategy: Vec::new(), equity: Vec::new(), ev: Vec::new(), ..node.clone() };
        for &row in &order {
            renamed.strategy.extend_from_slice(&node.strategy[row * k..(row + 1) * k]);
            renamed.equity.push(node.equity[row]);
            renamed.ev.push(node.ev[row]);
        }
        out.push(renamed);
        for a in 0..k {
            if node.children[a] != Child::Node {
                continue;
            }
            let mut child = alive.clone();
            for (row, &h) in hands.iter().enumerate() {
                child[p][h] = node.strategy[row * k + a] > 0;
            }
            self.rename_node(next, child, places, out)?;
        }
        Ok(())
    }
}

// Bytes.

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, x: u8) {
        self.0.push(x);
    }
    fn u16(&mut self, x: u16) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn u32(&mut self, x: u32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn i32(&mut self, x: i32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn i16(&mut self, x: i16) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn f32(&mut self, x: f32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn actions(&mut self, a: &[StoredAction]) {
        self.u8(a.len() as u8);
        for x in a {
            self.u8(kind_code(x.kind));
            self.i32(x.amount);
        }
    }
    fn summary(&mut self, s: &Summary) {
        for p in s {
            self.f32(p[0]);
            self.f32(p[1]);
        }
    }
}

struct R<'a>(&'a [u8]);

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.0.len() < n {
            return Err(invalid("truncated"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i16(&mut self) -> Result<i16, Error> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, Error> {
        let x = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        if x.is_finite() { Ok(x) } else { Err(invalid("not a finite number")) }
    }
    fn actions(&mut self) -> Result<Vec<StoredAction>, Error> {
        let n = self.u8()? as usize;
        if !(1..=16).contains(&n) {
            return Err(invalid("action count"));
        }
        (0..n).map(|_| Ok(StoredAction { kind: kind_of(self.u8()?)?, amount: self.i32()? })).collect()
    }
    fn summary(&mut self) -> Result<Summary, Error> {
        Ok([[self.f32()?, self.f32()?], [self.f32()?, self.f32()?]])
    }
    fn bets(&mut self) -> Result<[i32; 2], Error> {
        Ok([self.i32()?, self.i32()?])
    }
}

/// The header of a stored spot: catalog version, scenario and flop.
pub fn header(bytes: &[u8]) -> Result<(u32, String, String), Error> {
    let mut r = R(bytes);
    if r.take(4)? != MAGIC {
        return Err(invalid("not a stored spot"));
    }
    if r.u8()? != FORMAT {
        return Err(invalid("unknown format version"));
    }
    let version = r.u16()? as u32;
    let len = r.u8()? as usize;
    let scenario = String::from_utf8(r.take(len)?.to_vec()).map_err(|_| invalid("scenario"))?;
    let flop = String::from_utf8(r.take(6)?.to_vec()).map_err(|_| invalid("flop"))?;
    Ok((version, scenario, flop))
}

/// The exploitability a stored spot reports about itself, read from its header alone.
pub fn reported_exploitability(bytes: &[u8]) -> Result<f32, Error> {
    let (_, scenario, _) = header(bytes)?;
    let mut r = R(bytes.get(4 + 1 + 2 + 1 + scenario.len() + 6..).ok_or_else(|| invalid("header"))?);
    r.u32()?;
    r.f32()
}

impl Stored {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut head = W(Vec::new());
        head.0.extend_from_slice(MAGIC);
        head.u8(FORMAT);
        head.u16(self.catalog as u16);
        head.u8(self.scenario.len() as u8);
        head.0.extend_from_slice(self.scenario.as_bytes());
        head.0.extend_from_slice(self.flop.as_bytes());
        head.u32(self.iterations);
        head.f32(self.exploitability_pct);

        let mut w = W(Vec::new());
        w.u16(self.nodes.len() as u16);
        for n in &self.nodes {
            w.u8(n.player);
            w.i32(n.bets[0]);
            w.i32(n.bets[1]);
            w.u8(n.actions.len() as u8);
            for (a, c) in n.actions.iter().zip(&n.children) {
                w.u8(kind_code(a.kind));
                w.i32(a.amount);
                w.u8(*c as u8);
            }
            w.summary(&n.summary);
            let k = n.actions.len();
            for (h, (&e, &v)) in n.equity.iter().zip(&n.ev).enumerate() {
                w.0.extend_from_slice(&n.strategy[h * k..(h + 1) * k]);
                w.u8(e);
                w.i16(v);
            }
        }
        w.u16(self.turns.len() as u16);
        for line in &self.turns {
            for e in line {
                match e {
                    TurnEntry::Same(i) => w.u8(*i),
                    TurnEntry::Root(r) => {
                        w.u8(SAME_AS_NONE);
                        w.i32(r.bets[0]);
                        w.i32(r.bets[1]);
                        w.actions(&r.actions);
                        w.summary(&r.summary);
                        w.u8(r.classes.len() as u8);
                        for c in &r.classes {
                            w.u8(c.class);
                            w.u8(c.combos);
                            w.0.extend_from_slice(&c.strategy);
                            w.u8(c.equity);
                            w.i16(c.ev);
                        }
                    }
                }
            }
        }
        let mut z = DeflateEncoder::new(head.0, Compression::best());
        z.write_all(&w.0).expect("writing to memory");
        z.finish().expect("writing to memory")
    }

    /// Reads a stored spot and checks it against the catalog: the scenario and flop must
    /// be in it, and every node must fit its ranges.
    pub fn from_bytes(bytes: &[u8], catalog: &Catalog) -> Result<Stored, Error> {
        let mut r = R(bytes);
        if r.take(4)? != MAGIC {
            return Err(invalid("not a stored spot"));
        }
        if r.u8()? != FORMAT {
            return Err(invalid("unknown format version"));
        }
        let version = r.u16()? as u32;
        if version != catalog.version {
            return Err(invalid("other catalog version"));
        }
        let len = r.u8()? as usize;
        let scenario = String::from_utf8(r.take(len)?.to_vec()).map_err(|_| invalid("scenario"))?;
        let flop = String::from_utf8(r.take(6)?.to_vec()).map_err(|_| invalid("flop"))?;
        let iterations = r.u32()?;
        let exploitability_pct = r.f32()?;
        let s = catalog.scenario(&scenario).ok_or_else(|| invalid("scenario not in the catalog"))?;
        catalog.flop(&flop).ok_or_else(|| invalid("flop not in the catalog"))?;
        let scenario_ref = &catalog.scenarios[s];

        let mut body = Vec::new();
        DeflateDecoder::new(r.0).take(MAX_INFLATED + 1).read_to_end(&mut body).map_err(|_| invalid("body does not inflate"))?;
        if body.len() as u64 > MAX_INFLATED {
            return Err(invalid("body too large"));
        }
        let flop_cards = parse_board(&flop)?;
        let counts = [hands(&scenario_ref.oop_range, &flop_cards)?.len(), hands(&scenario_ref.ip_range, &flop_cards)?.len()];
        let mut r = R(&body);
        let n_nodes = r.u16()? as usize;
        let mut nodes = Vec::with_capacity(n_nodes);
        // Hands alive per player, replayed in the same depth-first order the writer used.
        let mut stack: Vec<[Vec<bool>; 2]> = vec![[vec![true; counts[0]], vec![true; counts[1]]]];
        let mut turn_lines = 0usize;
        for _ in 0..n_nodes {
            let alive = stack.pop().ok_or_else(|| invalid("node order"))?;
            let player = r.u8()?;
            if player > 1 {
                return Err(invalid("player"));
            }
            let bets = r.bets()?;
            let k = r.u8()? as usize;
            if !(1..=16).contains(&k) {
                return Err(invalid("action count"));
            }
            let mut actions = Vec::with_capacity(k);
            let mut children = Vec::with_capacity(k);
            for _ in 0..k {
                actions.push(StoredAction { kind: kind_of(r.u8()?)?, amount: r.i32()? });
                children.push(match r.u8()? {
                    0 => Child::End,
                    1 => Child::Node,
                    2 => Child::Turn,
                    3 => Child::Runout,
                    _ => return Err(invalid("child")),
                });
            }
            let summary = r.summary()?;
            let p = player as usize;
            let mut strategy = Vec::new();
            let mut equity = Vec::new();
            let mut ev = Vec::new();
            let mut per_hand: Vec<Option<Vec<u8>>> = vec![None; counts[p]];
            for (h, slot) in per_hand.iter_mut().enumerate() {
                if !alive[p][h] {
                    continue;
                }
                let q = r.take(k)?.to_vec();
                if q.iter().map(|&x| x as u32).sum::<u32>() != 255 {
                    return Err(invalid("frequencies do not sum to 255"));
                }
                strategy.extend_from_slice(&q);
                equity.push(r.u8()?);
                ev.push(r.i16()?);
                *slot = Some(q);
            }
            // Children are visited in action order, so they are pushed in reverse.
            for a in (0..k).rev() {
                match children[a] {
                    Child::Node => {
                        let mut next = alive.clone();
                        for (h, q) in per_hand.iter().enumerate() {
                            if let Some(q) = q {
                                next[p][h] = q[a] > 0;
                            }
                        }
                        stack.push(next);
                    }
                    Child::Turn => turn_lines += 1,
                    Child::End | Child::Runout => {}
                }
            }
            nodes.push(FlopNode { player, bets, actions, children, summary, strategy, equity, ev });
        }
        if !stack.is_empty() {
            return Err(invalid("missing flop nodes"));
        }
        let n_lines = r.u16()? as usize;
        if n_lines != turn_lines {
            return Err(invalid("turn line count"));
        }
        let n_cards = turn_cards(&flop_cards).len();
        let mut turns = Vec::with_capacity(n_lines);
        for _ in 0..n_lines {
            let mut line = Vec::with_capacity(n_cards);
            for i in 0..n_cards {
                let tag = r.u8()?;
                if tag != SAME_AS_NONE {
                    if tag as usize >= i || !matches!(line.get(tag as usize), Some(TurnEntry::Root(_))) {
                        return Err(invalid("turn reference"));
                    }
                    line.push(TurnEntry::Same(tag));
                    continue;
                }
                let bets = r.bets()?;
                let actions = r.actions()?;
                let summary = r.summary()?;
                let n = r.u8()? as usize;
                let mut classes = Vec::with_capacity(n);
                let mut last: Option<u8> = None;
                for _ in 0..n {
                    let class = r.u8()?;
                    if class as usize >= 169 || last.is_some_and(|l| class <= l) {
                        return Err(invalid("class order"));
                    }
                    last = Some(class);
                    let combos = r.u8()?;
                    let strategy = r.take(actions.len())?.to_vec();
                    if strategy.iter().map(|&x| x as u32).sum::<u32>() != 255 {
                        return Err(invalid("frequencies do not sum to 255"));
                    }
                    classes.push(TurnClass { class, combos, strategy, equity: r.u8()?, ev: r.i16()? });
                }
                line.push(TurnEntry::Root(TurnRoot { bets, actions, summary, classes }));
            }
            turns.push(line);
        }
        if !r.0.is_empty() {
            return Err(invalid("trailing bytes"));
        }
        Ok(Stored { catalog: version, scenario, flop, iterations, exploitability_pct, nodes, turns })
    }
}

// Viewing.

const NOT_STORED: &str =
    "Ab hier speichert die Bibliothek nichts mehr. Öffne den Spot im Solver und rechne ihn neu, um tiefer zu gehen.";

/// A stored spot with its catalog entry, ready to show node by node.
pub struct View<'a> {
    pub stored: &'a Stored,
    pub scenario: &'a Scenario,
    flop: Vec<Card>,
    hands: [Vec<(Combo, f32)>; 2],
}

impl<'a> View<'a> {
    pub fn new(stored: &'a Stored, catalog: &'a Catalog) -> Result<View<'a>, Error> {
        let s = catalog.scenario(&stored.scenario).ok_or_else(|| invalid("scenario not in the catalog"))?;
        let scenario = &catalog.scenarios[s];
        let flop = parse_board(&stored.flop)?;
        let hands = [hands(&scenario.oop_range, &flop)?, hands(&scenario.ip_range, &flop)?];
        Ok(View { stored, scenario, flop, hands })
    }

    /// The node reached by `path`, as the solver would show it, as far as it is stored.
    pub fn node(&self, path: &[Step]) -> Result<Node, Error> {
        let mut reach: [Vec<f64>; 2] = self.hands.clone().map(|h| h.iter().map(|&(_, w)| w as f64).collect());
        let mut index = 0usize;
        let mut turn_line = 0usize;
        let mut i = 0;
        // Flop nodes are stored depth first, so a child's index is found by skipping the
        // subtrees of its earlier siblings.
        loop {
            let node = &self.stored.nodes[index];
            if i == path.len() {
                return Ok(self.flop_view(path, node, &reach));
            }
            let Step::Action(a) = path[i] else {
                return Err(invalid("Hier kommt keine Karte, zuerst ist ein Spieler dran."));
            };
            if a >= node.actions.len() {
                return Err(invalid("Diese Aktion gibt es hier nicht."));
            }
            let p = node.player as usize;
            let k = node.actions.len();
            let mut h_stored = 0;
            for w in reach[p].iter_mut() {
                if *w > 0.0 {
                    *w *= node.strategy[h_stored * k + a] as f64 / 255.0;
                    h_stored += 1;
                }
            }
            let (mut next, mut lines) = (index + 1, turn_line);
            for b in 0..a {
                let (n, l) = self.skip(index, b, next, lines);
                next = n;
                lines = l;
            }
            match node.children[a] {
                Child::Node => {
                    index = next;
                    turn_line = lines;
                    i += 1;
                }
                Child::End | Child::Runout => {
                    if i + 1 != path.len() {
                        return Err(invalid("Die Hand ist an dieser Stelle schon zu Ende."));
                    }
                    return Ok(self.end_view(path, node, a, &reach));
                }
                Child::Turn => return self.turn_view(path, i + 1, node, a, lines, &reach),
            }
        }
    }

    /// (next flop node index, next turn line) after the subtree behind action `b` of `index`.
    fn skip(&self, index: usize, b: usize, next: usize, lines: usize) -> (usize, usize) {
        match self.stored.nodes[index].children[b] {
            Child::Node => {
                let child = next;
                let (mut n, mut l) = (child + 1, lines);
                for c in 0..self.stored.nodes[child].actions.len() {
                    (n, l) = self.skip(child, c, n, l);
                }
                (n, l)
            }
            Child::Turn => (next, lines + 1),
            Child::End | Child::Runout => (next, lines),
        }
    }

    fn base(&self, path: &[Step], kind: NodeKind, street: Street, board: Vec<Card>, bets: [i32; 2]) -> Node {
        let bets = bets.map(|b| bb(b as f64));
        Node {
            path: path.to_vec(),
            kind,
            street,
            board: board.iter().map(|c| c.to_string()).collect(),
            pot: self.scenario.pot + bets[0] + bets[1],
            bets,
            stacks: bets.map(|b| self.scenario.stack - b),
            // Stored lines have no chips in front once a later street is dealt.
            street_bets: if street == Street::Flop { bets } else { [0.0, 0.0] },
            player: None,
            actions: Vec::new(),
            cards: Vec::new(),
            hands: Vec::new(),
            classes: Vec::new(),
            players: [0, 1].map(|_| PlayerSummary { combos: 0.0, equity: 0.0, ev: 0.0 }),
        }
    }

    fn summaries(summary: &Summary, reach: &[Vec<f64>; 2]) -> [PlayerSummary; 2] {
        [0, 1].map(|p| PlayerSummary {
            combos: reach[p].iter().sum(),
            equity: summary[p][0] as f64,
            ev: summary[p][1] as f64,
        })
    }

    fn flop_view(&self, path: &[Step], node: &FlopNode, reach: &[Vec<f64>; 2]) -> Node {
        let mut out = self.base(path, NodeKind::Action, Street::Flop, self.flop.clone(), node.bets);
        let p = node.player as usize;
        out.player = Some(if p == 0 { Player::Oop } else { Player::Ip });
        out.players = Self::summaries(&node.summary, reach);
        let k = node.actions.len();
        let mut stored = 0;
        for (h, &(combo, _)) in self.hands[p].iter().enumerate() {
            let w = reach[p][h];
            if w <= 0.0 {
                continue;
            }
            out.hands.push(HandInfo {
                combo: combo.to_string(),
                class: combo.class().index(),
                weight: w,
                strategy: node.strategy[stored * k..(stored + 1) * k].iter().map(|&x| x as f64 / 255.0).collect(),
                equity: node.equity[stored] as f64 / 255.0,
                ev: node.ev[stored] as f64 / 100.0,
                evs: Vec::new(),
            });
            stored += 1;
        }
        out.classes = classes_of(&out.hands, k);
        out.actions = action_infos(&node.actions, &out.classes, k);
        out
    }

    fn end_view(&self, path: &[Step], node: &FlopNode, a: usize, reach: &[Vec<f64>; 2]) -> Node {
        let mut bets = node.bets;
        let p = node.player as usize;
        let action = &node.actions[a];
        match action.kind {
            ActionKind::Call => bets[p] = bets[1 - p],
            ActionKind::Bet | ActionKind::Raise | ActionKind::AllIn => bets[p] = action.amount,
            _ => {}
        }
        let mut out = self.base(path, NodeKind::Terminal, Street::Flop, self.flop.clone(), bets);
        out.players = Self::summaries(&node.summary, reach);
        out
    }

    fn turn_view(
        &self,
        path: &[Step],
        at: usize,
        node: &FlopNode,
        a: usize,
        line: usize,
        reach: &[Vec<f64>; 2],
    ) -> Result<Node, Error> {
        let entries = self.stored.turns.get(line).ok_or_else(|| invalid("turn line missing"))?;
        let cards = turn_cards(&self.flop);
        let mut bets = node.bets;
        let p = node.player as usize;
        if node.actions[a].kind == ActionKind::Call {
            bets[p] = bets[1 - p];
        }
        if at == path.len() {
            let mut out = self.base(path, NodeKind::Chance, Street::Flop, self.flop.clone(), bets);
            out.cards = cards.iter().map(|c| c.to_string()).collect();
            out.players = Self::summaries(&node.summary, reach);
            return Ok(out);
        }
        let Step::Card(text) = &path[at] else {
            return Err(invalid("Hier kommt zuerst die nächste Karte."));
        };
        let card: Card = text.parse().map_err(|_| invalid(format!("„{text}“ ist keine Karte.")))?;
        let i = cards.iter().position(|c| *c == card).ok_or_else(|| invalid(format!("{card} kann hier nicht kommen.")))?;
        let root = match &entries[i] {
            TurnEntry::Root(r) => r,
            TurnEntry::Same(j) => match &entries[*j as usize] {
                TurnEntry::Root(r) => r,
                TurnEntry::Same(_) => return Err(invalid("turn reference")),
            },
        };
        if at + 1 != path.len() {
            return Err(invalid(NOT_STORED));
        }
        let mut board = self.flop.clone();
        board.push(card);
        let mut out = self.base(path, NodeKind::Action, Street::Turn, board, root.bets);
        out.player = Some(Player::Oop);
        let k = root.actions.len();
        out.classes = root
            .classes
            .iter()
            .map(|c| {
                let class = HandClass::from_index(c.class as usize);
                ClassInfo {
                    class: c.class as usize,
                    label: class.to_string(),
                    combos: c.combos as f64 * COMBO_STEP,
                    strategy: c.strategy.iter().map(|&x| x as f64 / 255.0).collect(),
                    equity: c.equity as f64 / 255.0,
                    ev: c.ev as f64 / 100.0,
                }
            })
            .collect();
        out.actions = action_infos(&root.actions, &out.classes, k);
        let combos: f64 = out.classes.iter().map(|c| c.combos).sum();
        out.players = [0, 1].map(|q| PlayerSummary {
            combos: if q == 0 { combos } else { reach[1].iter().sum() },
            equity: root.summary[q][0] as f64,
            ev: root.summary[q][1] as f64,
        });
        Ok(out)
    }
}

fn classes_of(hands: &[HandInfo], k: usize) -> Vec<ClassInfo> {
    let mut classes: Vec<Option<ClassInfo>> = vec![None; 169];
    for h in hands {
        let c = classes[h.class].get_or_insert_with(|| ClassInfo {
            class: h.class,
            label: HandClass::from_index(h.class).to_string(),
            combos: 0.0,
            strategy: vec![0.0; k],
            equity: 0.0,
            ev: 0.0,
        });
        c.combos += h.weight;
        c.equity += h.weight * h.equity;
        c.ev += h.weight * h.ev;
        for (s, x) in c.strategy.iter_mut().zip(&h.strategy) {
            *s += h.weight * x;
        }
    }
    classes
        .into_iter()
        .flatten()
        .map(|mut c| {
            c.equity /= c.combos;
            c.ev /= c.combos;
            c.strategy.iter_mut().for_each(|s| *s /= c.combos);
            c
        })
        .collect()
}

fn action_infos(actions: &[StoredAction], classes: &[ClassInfo], k: usize) -> Vec<ActionInfo> {
    let mut per = vec![0.0; k];
    for c in classes {
        for (a, s) in c.strategy.iter().enumerate() {
            per[a] += c.combos * s;
        }
    }
    let total: f64 = per.iter().sum();
    actions
        .iter()
        .zip(per)
        .map(|(a, combos)| ActionInfo {
            kind: a.kind,
            amount: bb(a.amount as f64),
            frequency: if total > 0.0 { combos / total } else { 0.0 },
            combos,
        })
        .collect()
}

// Packs.

pub const PACK_MAGIC: &[u8; 4] = b"RSLB";
const PACK_FORMAT: u8 = 1;

/// Several stored spots in one file: magic "RSLB", format u8, count u32, then each spot
/// as length u32 + bytes.
pub fn pack(spots: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(9 + spots.iter().map(|s| s.len() + 4).sum::<usize>());
    out.extend_from_slice(PACK_MAGIC);
    out.push(PACK_FORMAT);
    out.extend_from_slice(&(spots.len() as u32).to_le_bytes());
    for s in spots {
        out.extend_from_slice(&(s.len() as u32).to_le_bytes());
        out.extend_from_slice(s);
    }
    out
}

/// The spots of a pack, still encoded.
pub fn unpack(bytes: &[u8]) -> Result<Vec<&[u8]>, Error> {
    let mut r = R(bytes);
    if r.take(4)? != PACK_MAGIC || r.u8()? != PACK_FORMAT {
        return Err(invalid("not a library pack"));
    }
    let n = r.u32()? as usize;
    let mut out = Vec::with_capacity(n.min(10_000));
    for _ in 0..n {
        let len = r.u32()? as usize;
        out.push(r.take(len)?);
    }
    if !r.0.is_empty() {
        return Err(invalid("trailing bytes"));
    }
    Ok(out)
}
