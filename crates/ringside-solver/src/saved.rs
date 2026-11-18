//! A solved spot of any ranges and sizes saved to disk, so it opens again without solving.
//!
//! The library format (`library::format`) cannot hold these: it leans on the catalog for
//! the ranges and sizes and keeps the turn only per hand class. A full tree is gigabytes,
//! so a snapshot keeps what the solver page shows node by node, per combo:
//! - every node of the street the spot starts on,
//! - after each line that reaches the next card, the first decision on every card.
//!
//! A river spot is complete that way. Deeper nodes of a flop or turn spot are not stored;
//! [`Snapshot::node`] says so, and the page offers to solve the spot again. Storing the
//! whole turn and river of a flop spot would multiply the size by about the number of
//! river cards times the turn lines, for nodes a player rarely opens.
//!
//! # Encoding
//! Little-endian. Frequencies are bytes summing to 255 per hand (`quantize`), equities
//! u16 (1/65535), reach weights u16 (at least 1 while positive), EVs i16 relative to a
//! per-node scale f32, amounts i32 hundredths of a BB:
//!
//! ```text
//! header  magic "RSMS", format u8, meta length u32 + meta JSON (spot, summary, target)
//! body    deflate of: per player combo count u16 + card pairs u8 u8 (solver order),
//!         node count u32, nodes
//! node    path (u8 length, per step: action index, or 0x80 | card index), kind u8,
//!         board (u8 length + cards), bets 2×i32, street bets 2×i32, per player combos,
//!         equity, EV f32; chance nodes: card mask u64; action nodes: player u8, action
//!         count u8, per action kind u8 + amount i32, EV scale f32, hand count u16, then
//!         columns: hand indices u16, weights u16, frequencies, equities u16, EVs i16,
//!         EVs per action i16; class count u8, per class index u8, equity u16, EV i16
//! ```

use std::collections::HashMap;
use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use ringside_engine::{Card, CardSet, Combo, HandClass, Range};
use serde::{Deserialize, Serialize};

use crate::library::format::{quantize, suit_map};
use crate::node::{ActionInfo, ActionKind, ClassInfo, HandInfo, Node, NodeKind, Player, PlayerSummary, Step, Street};
use crate::solve::{Settings, Solution, Summary};
use crate::spot::{Spot, bb, chips, parse_board};
use crate::Error;

pub const MAGIC: &[u8; 4] = b"RSMS";
pub const FORMAT: u8 = 1;
/// Largest body a reader inflates; a three-size flop spot stays far below.
const MAX_INFLATED: u64 = 512 << 20;
const CARD_STEP: u8 = 0x80;

pub const NOT_STORED: &str = "Gespeichert ist die erste Straße und der Einstieg in die nächste. Für tiefere Knoten rechnet Ringside den Spot neu.";

/// What the header says about a snapshot, readable without inflating the body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub spot: Spot,
    pub summary: Summary,
    /// The exploitability the solve aimed for, in percent of the pot.
    pub target_pct: f64,
}

#[derive(Clone)]
struct SavedHands {
    k: usize,
    scale: f32,
    index: Vec<u16>,
    weight: Vec<u16>,
    strategy: Vec<u8>,
    equity: Vec<u16>,
    ev: Vec<i16>,
    evs: Vec<i16>,
    classes: Vec<(u8, u16, i16)>,
}

#[derive(Clone)]
struct SavedNode {
    path: Vec<Step>,
    kind: NodeKind,
    board: Vec<u8>,
    bets: [i32; 2],
    street_bets: [i32; 2],
    players: [[f32; 3]; 2],
    cards: u64,
    player: u8,
    actions: Vec<(ActionKind, i32)>,
    hands: Option<SavedHands>,
}

/// A saved solution: the nodes the solver page shows, without the game tree.
pub struct Snapshot {
    pub meta: Meta,
    combos: [Vec<(u8, u8)>; 2],
    nodes: Vec<SavedNode>,
    by_path: HashMap<Vec<Step>, usize>,
}

fn invalid(m: impl Into<String>) -> Error {
    Error::Invalid(m.into())
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

fn node_kind_code(k: NodeKind) -> u8 {
    match k {
        NodeKind::Action => 0,
        NodeKind::Chance => 1,
        NodeKind::Terminal => 2,
    }
}

fn node_kind_of(code: u8) -> Result<NodeKind, Error> {
    Ok(match code {
        0 => NodeKind::Action,
        1 => NodeKind::Chance,
        2 => NodeKind::Terminal,
        _ => return Err(invalid("unknown node kind")),
    })
}

fn card_index(text: &str) -> Result<u8, Error> {
    let card: Card = text.parse().map_err(|_| invalid(format!("„{text}“ ist keine Karte.")))?;
    Ok(card.index())
}

fn unit16(x: f64) -> u16 {
    (x.clamp(0.0, 1.0) * 65535.0).round() as u16
}

fn scaled(x: f64, scale: f32) -> i16 {
    if scale <= 0.0 {
        return 0;
    }
    (x / scale as f64 * 32767.0).round().clamp(-32767.0, 32767.0) as i16
}

fn unscaled(x: i16, scale: f32) -> f64 {
    x as f64 / 32767.0 * scale as f64
}

// -----------------------------------------------------------------------------------
// Which spots count as the same.

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

/// A stable name for what a solve depends on: the board (flop cards in any order), the
/// live combos and weights of both ranges, pot, stack, the sizes of the streets still to
/// come and the all-in threshold. Two spots with the same key solve to the same tree.
pub fn spot_key(spot: &Spot) -> Result<String, Error> {
    let mut board = parse_board(&spot.board)?;
    board[..3].sort_by_key(|c| c.index());
    let dead = CardSet::from_cards(&board);
    let mut bytes: Vec<u8> = board.iter().map(|c| c.index()).collect();
    bytes.push(0xFF);
    for text in [&spot.oop_range, &spot.ip_range] {
        let range = Range::parse(text).map_err(|e| invalid(format!("Range: {e}")))?;
        for (combo, weight) in range.live_combos(dead) {
            bytes.extend_from_slice(&(combo.index() as u16).to_le_bytes());
            bytes.extend_from_slice(&((weight as f64 * 10_000.0).round() as u32).to_le_bytes());
        }
        bytes.push(0xFF);
    }
    for x in [spot.pot, spot.stack, spot.allin_threshold] {
        bytes.extend_from_slice(&chips(x).to_le_bytes());
    }
    for street in &spot.sizes[board.len() - 3..] {
        for list in [&street.bet, &street.raise] {
            let mut pct: Vec<i32> = list.iter().map(|&p| chips(p)).collect();
            pct.sort_unstable();
            pct.dedup();
            bytes.push(pct.len() as u8);
            for p in pct {
                bytes.extend_from_slice(&p.to_le_bytes());
            }
        }
    }
    Ok(format!("{:016x}", fnv(&bytes)))
}

// -----------------------------------------------------------------------------------
// Taking a snapshot.

fn save_node(node: &Node, lookup: &[HashMap<(u8, u8), u16>; 2]) -> Result<SavedNode, Error> {
    let board = node.board.iter().map(|c| card_index(c)).collect::<Result<Vec<_>, _>>()?;
    let mut saved = SavedNode {
        path: node.path.clone(),
        kind: node.kind,
        board,
        bets: node.bets.map(chips),
        street_bets: node.street_bets.map(chips),
        players: node.players.clone().map(|p| [p.combos as f32, p.equity as f32, p.ev as f32]),
        cards: 0,
        player: 0,
        actions: Vec::new(),
        hands: None,
    };
    match node.kind {
        NodeKind::Terminal => {}
        NodeKind::Chance => {
            for c in &node.cards {
                saved.cards |= 1u64 << card_index(c)?;
            }
        }
        NodeKind::Action => {
            let p = match node.player {
                Some(Player::Oop) => 0,
                Some(Player::Ip) => 1,
                None => return Err(invalid("action node without a player")),
            };
            saved.player = p as u8;
            saved.actions = node.actions.iter().map(|a| (a.kind, chips(a.amount))).collect();
            let k = node.actions.len();
            let scale = node
                .hands
                .iter()
                .flat_map(|h| std::iter::once(h.ev).chain(h.evs.iter().copied()))
                .chain(node.classes.iter().map(|c| c.ev))
                .fold(0.0f64, |m, x| m.max(x.abs())) as f32;
            let mut hands = SavedHands {
                k,
                scale,
                index: Vec::with_capacity(node.hands.len()),
                weight: Vec::with_capacity(node.hands.len()),
                strategy: Vec::with_capacity(node.hands.len() * k),
                equity: Vec::with_capacity(node.hands.len()),
                ev: Vec::with_capacity(node.hands.len()),
                evs: Vec::with_capacity(node.hands.len() * k),
                classes: Vec::with_capacity(node.classes.len()),
            };
            for h in &node.hands {
                let combo: Combo = parse_combo(&h.combo)?;
                let (a, b) = combo.cards();
                let key = (a.index().min(b.index()), a.index().max(b.index()));
                let i = *lookup[p].get(&key).ok_or_else(|| invalid("hand outside the range"))?;
                hands.index.push(i);
                hands.weight.push(unit16(h.weight).max(1));
                hands.strategy.extend(quantize(&h.strategy));
                hands.equity.push(unit16(h.equity));
                hands.ev.push(scaled(h.ev, scale));
                hands.evs.extend(h.evs.iter().map(|&x| scaled(x, scale)));
            }
            hands.classes = node.classes.iter().map(|c| (c.class as u8, unit16(c.equity), scaled(c.ev, scale))).collect();
            saved.hands = Some(hands);
        }
    }
    Ok(saved)
}

fn parse_combo(text: &str) -> Result<Combo, Error> {
    if text.len() != 4 {
        return Err(invalid("combo"));
    }
    let a: Card = text[..2].parse().map_err(|_| invalid("combo"))?;
    let b: Card = text[2..].parse().map_err(|_| invalid("combo"))?;
    Ok(Combo::new(a, b))
}

impl Snapshot {
    /// Walks `solution` and keeps the nodes described in the module docs.
    pub fn take(solution: &mut Solution, settings: &Settings) -> Result<Snapshot, Error> {
        let combos: [Vec<(u8, u8)>; 2] =
            [0, 1].map(|p| solution.game.private_cards(p).iter().map(|&(a, b)| (a.min(b), a.max(b))).collect());
        let lookup: [HashMap<(u8, u8), u16>; 2] =
            [0, 1].map(|p| combos[p].iter().enumerate().map(|(i, &c)| (c, i as u16)).collect());
        let mut nodes = Vec::new();
        let mut stack: Vec<Vec<Step>> = vec![Vec::new()];
        while let Some(path) = stack.pop() {
            let node = solution.node(&path)?;
            let dealt = path.iter().any(|s| matches!(s, Step::Card(_)));
            match node.kind {
                NodeKind::Action if !dealt => {
                    for a in (0..node.actions.len()).rev() {
                        let mut p = path.clone();
                        p.push(Step::Action(a));
                        stack.push(p);
                    }
                }
                NodeKind::Chance if !dealt => {
                    for c in node.cards.iter().rev() {
                        let mut p = path.clone();
                        p.push(Step::Card(c.clone()));
                        stack.push(p);
                    }
                }
                _ => {}
            }
            nodes.push(save_node(&node, &lookup)?);
        }
        let meta = Meta { spot: solution.spot.clone(), summary: solution.summary.clone(), target_pct: settings.target_pct };
        Ok(Snapshot::from_parts(meta, combos, nodes))
    }

    fn from_parts(meta: Meta, combos: [Vec<(u8, u8)>; 2], nodes: Vec<SavedNode>) -> Snapshot {
        let by_path = nodes.iter().enumerate().map(|(i, n)| (n.path.clone(), i)).collect();
        Snapshot { meta, combos, nodes, by_path }
    }

    /// The same spot on `board`, a suit renaming of this one's board: hands, board and
    /// dealt cards move, nothing is recomputed. Both ranges must weigh every live hand
    /// like its renamed hand (class ranges do); otherwise the strategies would belong to
    /// other hands.
    pub fn renamed(&self, board: &str) -> Result<Snapshot, Error> {
        let from = parse_board(&self.meta.spot.board)?;
        let to = parse_board(board)?;
        let perm = suit_map(&from, &to).ok_or_else(|| invalid("the boards are no suit renaming of each other"))?;
        let rename = |c: Card| Card::new(c.rank(), perm[c.suit() as usize]);
        let rename_index = |i: u8| rename(Card::from_index(i)).index();
        let dead = CardSet::from_cards(&from);
        for text in [&self.meta.spot.oop_range, &self.meta.spot.ip_range] {
            let range = Range::parse(text).map_err(|e| invalid(format!("Range: {e}")))?;
            for (combo, weight) in range.live_combos(dead) {
                let (a, b) = combo.cards();
                if range.weight(Combo::new(rename(a), rename(b))) != weight {
                    return Err(invalid("the ranges treat suits differently"));
                }
            }
        }
        let combos = self.combos.clone().map(|list| {
            list.into_iter()
                .map(|(a, b)| {
                    let (a, b) = (rename_index(a), rename_index(b));
                    (a.min(b), a.max(b))
                })
                .collect()
        });
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for n in &self.nodes {
            let mut n = n.clone();
            for s in &mut n.path {
                if let Step::Card(c) = s {
                    *c = rename(c.parse::<Card>().map_err(|_| invalid("Karte"))?).to_string();
                }
            }
            n.board.iter_mut().for_each(|c| *c = rename_index(*c));
            n.cards = (0..52u8).filter(|c| n.cards & (1u64 << c) != 0).fold(0, |m, c| m | 1u64 << rename_index(c));
            nodes.push(n);
        }
        let meta = Meta { spot: Spot { board: board.to_string(), ..self.meta.spot.clone() }, ..self.meta.clone() };
        Ok(Snapshot::from_parts(meta, combos, nodes))
    }

    /// Whether every node of the tree is stored (spots that start on the river).
    pub fn complete(&self) -> bool {
        parse_board(&self.meta.spot.board).is_ok_and(|b| b.len() == 5)
    }

    /// The node reached by `path`, as `Solution::node` shows it, as far as it is stored.
    pub fn node(&self, path: &[Step]) -> Result<Node, Error> {
        let Some(&i) = self.by_path.get(path) else {
            // One step past a stored node that has children means: not stored this deep.
            let parent = path.len().checked_sub(1).and_then(|n| self.by_path.get(&path[..n]));
            return Err(match parent.map(|&i| &self.nodes[i]) {
                Some(n) if n.kind == NodeKind::Action && matches!(path.last(), Some(Step::Action(a)) if *a >= n.actions.len()) => {
                    invalid("Diese Aktion gibt es hier nicht.")
                }
                Some(n) if n.kind == NodeKind::Terminal => invalid("Die Hand ist an dieser Stelle schon zu Ende."),
                _ => invalid(NOT_STORED),
            });
        };
        Ok(self.view(&self.nodes[i], path))
    }

    fn view(&self, n: &SavedNode, path: &[Step]) -> Node {
        let street = match n.board.len() {
            3 => Street::Flop,
            4 => Street::Turn,
            _ => Street::River,
        };
        let bets = n.bets.map(|b| bb(b as f64));
        let spot = &self.meta.spot;
        let mut node = Node {
            path: path.to_vec(),
            kind: n.kind,
            street,
            board: n.board.iter().map(|&c| Card::from_index(c).to_string()).collect(),
            pot: spot.pot + bets[0] + bets[1],
            bets,
            stacks: bets.map(|b| spot.stack - b),
            street_bets: n.street_bets.map(|b| bb(b as f64)),
            player: None,
            actions: Vec::new(),
            cards: (0..52u8).filter(|c| n.cards & (1u64 << c) != 0).map(|c| Card::from_index(c).to_string()).collect(),
            hands: Vec::new(),
            classes: Vec::new(),
            players: n.players.map(|p| PlayerSummary { combos: p[0] as f64, equity: p[1] as f64, ev: p[2] as f64 }),
        };
        let Some(h) = &n.hands else { return node };
        node.player = Some(if n.player == 0 { Player::Oop } else { Player::Ip });
        let k = h.k;
        let combos = &self.combos[n.player as usize];
        let mut classes: Vec<Option<ClassInfo>> = vec![None; 169];
        let mut per_action = vec![0.0f64; k];
        for (j, &i) in h.index.iter().enumerate() {
            let (a, b) = combos[i as usize];
            let combo = Combo::new(Card::from_index(a), Card::from_index(b));
            let weight = h.weight[j] as f64 / 65535.0;
            let strategy: Vec<f64> = h.strategy[j * k..(j + 1) * k].iter().map(|&x| x as f64 / 255.0).collect();
            let class = combo.class().index();
            let entry = classes[class].get_or_insert_with(|| ClassInfo {
                class,
                label: HandClass::from_index(class).to_string(),
                combos: 0.0,
                strategy: vec![0.0; k],
                equity: 0.0,
                ev: 0.0,
            });
            entry.combos += weight;
            for (s, &x) in entry.strategy.iter_mut().zip(&strategy) {
                *s += weight * x;
            }
            for (t, &x) in per_action.iter_mut().zip(&strategy) {
                *t += weight * x;
            }
            node.hands.push(HandInfo {
                combo: combo.to_string(),
                class,
                weight,
                strategy,
                equity: h.equity[j] as f64 / 65535.0,
                ev: unscaled(h.ev[j], h.scale),
                evs: h.evs[j * k..(j + 1) * k].iter().map(|&x| unscaled(x, h.scale)).collect(),
            });
        }
        for &(class, equity, ev) in &h.classes {
            if let Some(c) = classes[class as usize].as_mut() {
                c.equity = equity as f64 / 65535.0;
                c.ev = unscaled(ev, h.scale);
            }
        }
        node.classes = classes
            .into_iter()
            .flatten()
            .map(|mut c| {
                c.strategy.iter_mut().for_each(|s| *s /= c.combos);
                c
            })
            .collect();
        let total: f64 = per_action.iter().sum();
        node.actions = n
            .actions
            .iter()
            .zip(&per_action)
            .map(|(&(kind, amount), &combos)| ActionInfo {
                kind,
                amount: bb(amount as f64),
                frequency: if total > 0.0 { combos / total } else { 0.0 },
                combos,
            })
            .collect();
        node
    }

    // -------------------------------------------------------------------------------
    // Bytes.

    pub fn to_bytes(&self) -> Vec<u8> {
        let meta = serde_json::to_vec(&self.meta).expect("meta serializes");
        let mut head = Vec::with_capacity(meta.len() + 9);
        head.extend_from_slice(MAGIC);
        head.push(FORMAT);
        head.extend_from_slice(&(meta.len() as u32).to_le_bytes());
        head.extend_from_slice(&meta);

        let mut w = W(Vec::new());
        for list in &self.combos {
            w.u16(list.len() as u16);
            for &(a, b) in list {
                w.u8(a);
                w.u8(b);
            }
        }
        w.u32(self.nodes.len() as u32);
        for n in &self.nodes {
            w.u8(n.path.len() as u8);
            for s in &n.path {
                match s {
                    Step::Action(a) => w.u8(*a as u8),
                    Step::Card(c) => w.u8(CARD_STEP | card_index(c).expect("stored paths hold cards")),
                }
            }
            w.u8(node_kind_code(n.kind));
            w.u8(n.board.len() as u8);
            w.0.extend_from_slice(&n.board);
            for x in n.bets.iter().chain(&n.street_bets) {
                w.i32(*x);
            }
            for p in &n.players {
                for &x in p {
                    w.f32(x);
                }
            }
            match n.kind {
                NodeKind::Terminal => {}
                NodeKind::Chance => w.0.extend_from_slice(&n.cards.to_le_bytes()),
                NodeKind::Action => {
                    let h = n.hands.as_ref().expect("action nodes hold hands");
                    w.u8(n.player);
                    w.u8(n.actions.len() as u8);
                    for &(kind, amount) in &n.actions {
                        w.u8(kind_code(kind));
                        w.i32(amount);
                    }
                    w.f32(h.scale);
                    w.u16(h.index.len() as u16);
                    h.index.iter().for_each(|&x| w.u16(x));
                    h.weight.iter().for_each(|&x| w.u16(x));
                    w.0.extend_from_slice(&h.strategy);
                    h.equity.iter().for_each(|&x| w.u16(x));
                    h.ev.iter().for_each(|&x| w.i16(x));
                    h.evs.iter().for_each(|&x| w.i16(x));
                    w.u8(h.classes.len() as u8);
                    for &(c, e, v) in &h.classes {
                        w.u8(c);
                        w.u16(e);
                        w.i16(v);
                    }
                }
            }
        }
        let mut z = DeflateEncoder::new(head, Compression::default());
        z.write_all(&w.0).expect("writing to memory");
        z.finish().expect("writing to memory")
    }

    /// The header alone, without inflating the nodes.
    pub fn meta(bytes: &[u8]) -> Result<Meta, Error> {
        Ok(Self::split(bytes)?.0)
    }

    fn split(bytes: &[u8]) -> Result<(Meta, &[u8]), Error> {
        let mut r = R(bytes);
        if r.take(4)? != MAGIC {
            return Err(invalid("keine gespeicherte Lösung"));
        }
        if r.u8()? != FORMAT {
            return Err(invalid("unbekannte Formatversion"));
        }
        let len = r.u32()? as usize;
        let meta: Meta = serde_json::from_slice(r.take(len)?).map_err(|e| invalid(format!("Kopf: {e}")))?;
        Ok((meta, r.0))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Snapshot, Error> {
        let (meta, rest) = Self::split(bytes)?;
        let mut body = Vec::new();
        DeflateDecoder::new(rest).take(MAX_INFLATED + 1).read_to_end(&mut body).map_err(|_| invalid("beschädigt"))?;
        if body.len() as u64 > MAX_INFLATED {
            return Err(invalid("zu groß"));
        }
        let mut r = R(&body);
        let mut combos: [Vec<(u8, u8)>; 2] = [Vec::new(), Vec::new()];
        for list in &mut combos {
            let n = r.u16()? as usize;
            for _ in 0..n {
                let (a, b) = (r.u8()?, r.u8()?);
                if a >= 52 || b >= 52 || a == b {
                    return Err(invalid("Karte"));
                }
                list.push((a, b));
            }
        }
        let count = r.u32()? as usize;
        let mut nodes = Vec::with_capacity(count.min(1 << 20));
        for _ in 0..count {
            let len = r.u8()? as usize;
            let mut path = Vec::with_capacity(len);
            for _ in 0..len {
                let s = r.u8()?;
                path.push(if s & CARD_STEP != 0 {
                    let c = s & !CARD_STEP;
                    if c >= 52 {
                        return Err(invalid("Karte"));
                    }
                    Step::Card(Card::from_index(c).to_string())
                } else {
                    Step::Action(s as usize)
                });
            }
            let kind = node_kind_of(r.u8()?)?;
            let board_len = r.u8()? as usize;
            if !(3..=5).contains(&board_len) {
                return Err(invalid("Board"));
            }
            let board = r.take(board_len)?.to_vec();
            if board.iter().any(|&c| c >= 52) {
                return Err(invalid("Karte"));
            }
            let bets = [r.i32()?, r.i32()?];
            let street_bets = [r.i32()?, r.i32()?];
            let players = [[r.f32()?, r.f32()?, r.f32()?], [r.f32()?, r.f32()?, r.f32()?]];
            let mut node = SavedNode {
                path,
                kind,
                board,
                bets,
                street_bets,
                players,
                cards: 0,
                player: 0,
                actions: Vec::new(),
                hands: None,
            };
            match kind {
                NodeKind::Terminal => {}
                NodeKind::Chance => node.cards = u64::from_le_bytes(r.take(8)?.try_into().unwrap()),
                NodeKind::Action => {
                    node.player = r.u8()?;
                    if node.player > 1 {
                        return Err(invalid("Spieler"));
                    }
                    let k = r.u8()? as usize;
                    if !(1..=32).contains(&k) {
                        return Err(invalid("Aktionen"));
                    }
                    for _ in 0..k {
                        node.actions.push((kind_of(r.u8()?)?, r.i32()?));
                    }
                    let scale = r.f32()?;
                    let n = r.u16()? as usize;
                    let index: Vec<u16> = (0..n).map(|_| r.u16()).collect::<Result<_, _>>()?;
                    if index.iter().any(|&i| i as usize >= combos[node.player as usize].len()) {
                        return Err(invalid("Hand"));
                    }
                    let weight = (0..n).map(|_| r.u16()).collect::<Result<_, _>>()?;
                    let strategy = r.take(n * k)?.to_vec();
                    let equity = (0..n).map(|_| r.u16()).collect::<Result<_, _>>()?;
                    let ev = (0..n).map(|_| r.i16()).collect::<Result<_, _>>()?;
                    let evs = (0..n * k).map(|_| r.i16()).collect::<Result<_, _>>()?;
                    let nc = r.u8()? as usize;
                    let mut classes = Vec::with_capacity(nc);
                    for _ in 0..nc {
                        let c = r.u8()?;
                        if c as usize >= 169 {
                            return Err(invalid("Klasse"));
                        }
                        classes.push((c, r.u16()?, r.i16()?));
                    }
                    node.hands = Some(SavedHands { k, scale, index, weight, strategy, equity, ev, evs, classes });
                }
            }
            nodes.push(node);
        }
        if !r.0.is_empty() {
            return Err(invalid("überzählige Bytes"));
        }
        Ok(Snapshot::from_parts(meta, combos, nodes))
    }
}

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
    fn i16(&mut self, x: i16) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn i32(&mut self, x: i32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn f32(&mut self, x: f32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
}

struct R<'a>(&'a [u8]);

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        if self.0.len() < n {
            return Err(invalid("abgeschnitten"));
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
    fn i16(&mut self) -> Result<i16, Error> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, Error> {
        let x = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        if x.is_finite() { Ok(x) } else { Err(invalid("keine endliche Zahl")) }
    }
}

/// Whether a saved solve is at least as exact as one asked for with `target_pct`.
pub fn good_enough(meta: &Meta, target_pct: f64) -> bool {
    meta.summary.exploitability_pct <= target_pct + 1e-9
}
