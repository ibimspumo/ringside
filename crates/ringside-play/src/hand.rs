//! One hand from the deal to the end: opponents act by their frequencies, the hero's
//! decisions are graded, a flop continues in a prepared heads-up spot.
//!
//! Everything random comes from the deal's seed, so the same [`Deal`] plays the same hand
//! again ("Hand wiederholen"): same cards and board, and the same opponent choices as long
//! as the hero plays the same way.

use std::sync::{Arc, Mutex};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::preflop::Kind;
use ringside_engine::range::{HandClass, NUM_CLASSES};
use ringside_engine::{Card, CardSet, Combo, HandCategory, evaluate};
use ringside_solver::{ActionKind, Node, NodeKind, Player, Step as PostStep};
use serde::Serialize;

use crate::grade::{HINT_FACTOR, Judgement, judge};
use crate::model::{End, Walk};
use crate::pool::{Pool, Taken};
use crate::table::{Config, Deep, Mode, PUSH_FOLD_MAX_BB, Short, TableKey, deep, short};
use crate::tree::{Tree, TreeSource};

/// Random deals tried until the hero gets a decision (a walk in the big blind has none).
const MAX_REDEALS: u64 = 64;

pub fn rng(seed: u64, salt: u64) -> ChaCha8Rng {
    // splitmix64, so neighbouring seeds and salts don't share streams.
    let mut z = seed ^ salt.wrapping_mul(0xd1b5_4a32_d192_ed03);
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ChaCha8Rng::seed_from_u64(z ^ (z >> 31))
}

/// Index drawn with probability proportional to `weights`; `None` if all are 0.
pub fn pick(weights: &[f64], rng: &mut impl Rng) -> Option<usize> {
    let total: f64 = weights.iter().map(|w| w.max(0.0)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut r = rng.random::<f64>() * total;
    for (i, w) in weights.iter().enumerate() {
        r -= w.max(0.0);
        if r < 0.0 {
            return Some(i);
        }
    }
    weights.iter().rposition(|&w| w > 0.0)
}

/// A combo avoiding `used`, weighted by `weight(class)`; uniform if all weights are 0.
fn draw_combo(rng: &mut impl Rng, used: &mut CardSet, weight: impl Fn(HandClass) -> f64) -> [Card; 2] {
    let combos: Vec<Combo> = Combo::all().filter(|c| !c.mask().intersects(*used)).collect();
    let w: Vec<f64> = combos.iter().map(|c| weight(c.class())).collect();
    let i = pick(&w, rng).unwrap_or_else(|| rng.random_range(0..combos.len()));
    let (a, b) = combos[i].cards();
    used.insert(a);
    used.insert(b);
    [a, b]
}

fn draw_card(rng: &mut impl Rng, used: &mut CardSet) -> Card {
    let free = used.complement().to_vec();
    let c = free[rng.random_range(0..free.len())];
    used.insert(c);
    c
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

/// Where a decision's strategy comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Ringside's preflop model.
    Model,
    /// The push/fold equilibrium.
    Equilibrium,
    Solver,
    Library,
    Saved,
}

impl From<TreeSource> for Source {
    fn from(s: TreeSource) -> Source {
        match s {
            TreeSource::Solved => Source::Solver,
            TreeSource::Library => Source::Library,
            TreeSource::Saved => Source::Saved,
        }
    }
}

/// Everything that makes a hand; playing it again starts from here.
#[derive(Clone)]
pub struct Deal {
    pub seed: u64,
    pub config: Config,
    pub table: TableKey,
    pub hero: usize,
    /// Hole cards per seat in action order.
    pub holes: Vec<[Card; 2]>,
    pub board: [Card; 5],
    /// The prepared spot the hand is steered into.
    pub spot: Option<Taken>,
    /// No spot was ready although the mode wants one.
    pub waiting: bool,
}

/// A deal for `config`: into a ready spot of the pool where the mode plays postflop,
/// else a random preflop hand.
pub fn deal(config: &Config, seed: u64, pool: Option<&Pool>) -> Result<Deal, String> {
    config.validate()?;
    let mut rng = rng(seed, 0);
    if config.mode != Mode::PushFold
        && let Some(taken) = pool.and_then(|p| p.take(&config.deep_keys(), config.hero_seat, &mut rng))
    {
        return steered(config, seed, taken, &mut rng);
    }
    let stacks = config.stacks();
    let stack_bb = stacks[rng.random_range(0..stacks.len())];
    let table = TableKey { players: config.players, ante: config.ante, stack_bb };
    let hero = config.hero_seat.unwrap_or_else(|| rng.random_range(0..config.players));
    let mut used = CardSet::EMPTY;
    let holes = (0..config.players).map(|_| draw_combo(&mut rng, &mut used, |_| 1.0)).collect();
    let board = [(); 5].map(|_| draw_card(&mut rng, &mut used));
    let waiting = config.mode != Mode::PushFold && stack_bb > PUSH_FOLD_MAX_BB;
    Ok(Deal { seed, config: config.clone(), table, hero, holes, board, spot: None, waiting })
}

fn steered(config: &Config, seed: u64, taken: Taken, rng: &mut ChaCha8Rng) -> Result<Deal, String> {
    let table = deep(taken.table).ok_or("Preflop-Lösung fehlt")?;
    let sc = &taken.scenario;
    let seats = sc.seats();
    let side = match config.hero_seat {
        Some(h) => seats.iter().position(|&s| s == h).ok_or("Platz passt nicht zum Spot")?,
        None => rng.random_range(0..2),
    };
    let mut used = CardSet::from_cards(&taken.flop);
    let n = config.players;
    let mut holes = vec![None; n];
    for k in [side, 1 - side] {
        holes[seats[k]] = Some(draw_combo(rng, &mut used, |c| sc.weights[k][c.index()]));
    }
    // Everyone else folds on the way: their hands come from what folds there.
    for &(v, a) in &sc.path {
        let node = &table.solution.nodes[v];
        if holes[node.seat].is_none() {
            holes[node.seat] = Some(draw_combo(rng, &mut used, |c| node.freq[a][c.index()]));
        }
    }
    let holes: Vec<[Card; 2]> = holes.into_iter().map(|h| h.unwrap_or_else(|| draw_combo(rng, &mut used, |_| 1.0))).collect();
    let turn = draw_card(rng, &mut used);
    let river = draw_card(rng, &mut used);
    let [a, b, c] = taken.flop;
    Ok(Deal {
        seed,
        config: config.clone(),
        table: taken.table,
        hero: seats[side],
        holes,
        board: [a, b, c, turn, river],
        spot: Some(taken),
        waiting: false,
    })
}

// ---------------------------------------------------------------------------------
// What the page shows.

#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    /// "fold", "check", "call", "limp", "bet", "raise" or "all_in".
    pub kind: &'static str,
    /// Chips in front after the action on this street, BB.
    pub to_bb: f64,
    /// Chips the action puts in, BB.
    pub add_bb: f64,
    /// Bet or raise size in percent of the pot (raises: of the pot after calling).
    pub pct_pot: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub street: Street,
    pub board: Vec<Card>,
    /// Chips in front of each seat on this street.
    pub bets: Vec<f64>,
    pub stacks: Vec<f64>,
    /// Everything in the middle including the bets in front.
    pub pot: f64,
    pub folded: Vec<bool>,
    /// Who acts next.
    pub to_act: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Action {
        street: Street,
        seat: usize,
        choice: Choice,
        /// Index into `verdicts` for a hero decision.
        verdict: Option<usize>,
        snapshot: usize,
    },
    Board {
        street: Street,
        cards: Vec<Card>,
        snapshot: usize,
    },
}

/// One player's 13×13 strategy preflop: per class the share of its combos that reaches the
/// node and the frequency of each choice.
#[derive(Clone, Debug, Serialize)]
pub struct Grid {
    pub reach: Vec<f64>,
    pub strategy: Vec<Vec<f64>>,
}

/// What the solution does with the hero's hand at a decision.
#[derive(Clone, Debug, Serialize)]
pub struct Lesson {
    pub street: Street,
    pub choices: Vec<Choice>,
    pub source: Source,
    /// `None`: the hero's hand does not reach this spot in the solution, so nothing grades it.
    pub strategy: Option<Vec<f64>>,
    pub evs: Option<Vec<f64>>,
    pub pot: f64,
    pub to_call: f64,
    pub equity: Option<f64>,
    pub class: String,
    /// The postflop node, for the range and the explanations.
    pub node: Option<Node>,
    pub grid: Option<Grid>,
    /// Path of the postflop node, for opening it in the solver.
    pub path: Option<Vec<PostStep>>,
    pub exploitability_pct: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    #[serde(flatten)]
    pub lesson: Lesson,
    pub chosen: usize,
    pub judgement: Option<Judgement>,
    pub hinted: bool,
    /// Out of 100 after the hint discount; `None` when nothing grades the decision.
    pub points: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    /// Everyone but one folded.
    Folded,
    Showdown,
    /// The hero left the line to the prepared spot; there is no solution for the new line.
    LeftLine,
    /// A flop without a prepared spot (the stock was empty).
    FlopUnsolved,
    /// A stored spot knows no more decisions from here.
    NotStored,
}

#[derive(Clone, Debug, Serialize)]
pub struct Outcome {
    pub reason: EndReason,
    pub winners: Vec<usize>,
    /// What every seat won or lost in the hand, BB.
    pub net: Vec<f64>,
    /// Hands shown: the players still in, and the heads-up opponent.
    pub shown: Vec<Option<[Card; 2]>>,
    /// German name of each shown hand on a full board.
    pub made: Vec<Option<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Decision {
    pub street: Street,
    pub choices: Vec<Choice>,
    pub pot: f64,
    pub to_call: f64,
    pub hinted: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct HandView {
    pub seed: u64,
    pub config: Config,
    pub players: usize,
    pub positions: Vec<&'static str>,
    pub stack_bb: f64,
    pub ante: Vec<f64>,
    pub hero: usize,
    pub hero_cards: [Card; 2],
    pub board: Vec<Card>,
    pub events: Vec<Event>,
    pub snapshots: Vec<Snapshot>,
    pub verdicts: Vec<Verdict>,
    pub decision: Option<Decision>,
    pub outcome: Option<Outcome>,
    pub preflop_source: Source,
    /// Postflop spot: where it came from and how exact it is.
    pub spot: Option<SpotInfo>,
    pub waiting: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpotInfo {
    pub id: u64,
    pub source: Source,
    pub complete: bool,
    pub exploitability_pct: f64,
    pub oop: usize,
    pub ip: usize,
    pub pot: f64,
    pub stack: f64,
}

/// What a finished hand adds to the learning area.
#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    /// Mean points of the graded decisions, 0–1.
    pub score: f64,
    pub ev_loss_bb: f64,
    pub graded: usize,
}

// ---------------------------------------------------------------------------------
// The hand.

/// Frequencies, EVs per action (where stored per combo) and equity of one hand.
type HandStrategy = (Vec<f64>, Option<Vec<f64>>, Option<f64>);

enum Phase {
    Deep(Walk),
    Short(ShortWalk),
    Post { path: Vec<PostStep>, node: Box<Node> },
    Over,
}

pub struct Hand {
    pub deal: Deal,
    deep: Option<Arc<Deep>>,
    short: Option<Arc<Short>>,
    tree: Option<Arc<Mutex<Box<dyn Tree>>>>,
    phase: Phase,
    n: usize,
    ante: Vec<f64>,
    /// Chips put in during the hand (blinds included, antes not), and at the flop.
    put: Vec<f64>,
    pre_put: Vec<f64>,
    street_put: Vec<f64>,
    folded: Vec<bool>,
    street: Street,
    board_len: usize,
    /// Path through the prepared spot's preflop line while the hand follows it.
    path_pos: Option<usize>,
    hero_steps: Vec<(usize, usize)>,
    events: Vec<Event>,
    snapshots: Vec<Snapshot>,
    verdicts: Vec<Verdict>,
    draws: u64,
    hinted: bool,
    outcome: Option<Outcome>,
}

impl Hand {
    /// A new hand where the hero has something to decide.
    pub fn start(config: &Config, seed: u64, pool: Option<&Pool>) -> Result<Hand, String> {
        let mut last = None;
        for k in 0..MAX_REDEALS {
            let s = if k == 0 { seed } else { rng(seed, 1000 + k).random::<u64>() >> 11 };
            let d = deal(config, s, pool)?;
            let steered = d.spot.is_some();
            let hand = Hand::play(d)?;
            if steered || hand.decision().is_some() {
                return Ok(hand);
            }
            last = Some(hand);
        }
        last.ok_or_else(|| "Keine Hand gefunden".into())
    }

    /// Plays `deal` from the start.
    pub fn play(deal: Deal) -> Result<Hand, String> {
        let n = deal.config.players;
        let stack = deal.table.stack_bb;
        let (deep_t, short_t, ante, phase) = if stack <= PUSH_FOLD_MAX_BB {
            let t = short(deal.table).ok_or("Für diesen Stack gibt es kein Push/Fold-Gleichgewicht.")?;
            let ante: Vec<f64> = (0..n).map(|s| t.params.ante_bb + if s == n - 1 { t.params.bb_ante_bb } else { 0.0 }).collect();
            let walk = ShortWalk::start(&t, &ante);
            (None, Some(t), ante, Phase::Short(walk))
        } else {
            let t = deep(deal.table).ok_or("Für diesen Stack gibt es keine Preflop-Lösung.")?;
            let walk = Walk::start(&t.params);
            let ante = walk.ante.clone();
            (Some(t), None, ante, Phase::Deep(walk))
        };
        let mut put = vec![0.0; n];
        put[n - 2] = 0.5;
        put[n - 1] = 1.0;
        let tree = deal.spot.as_ref().map(|s| s.tree.clone());
        let path_pos = deal.spot.as_ref().map(|_| 0);
        let mut hand = Hand {
            deep: deep_t,
            short: short_t,
            tree,
            phase,
            n,
            ante,
            street_put: put.clone(),
            pre_put: put.clone(),
            put,
            folded: vec![false; n],
            street: Street::Preflop,
            board_len: 0,
            path_pos,
            hero_steps: Vec::new(),
            events: Vec::new(),
            snapshots: Vec::new(),
            verdicts: Vec::new(),
            draws: 0,
            hinted: false,
            outcome: None,
            deal,
        };
        hand.snapshot();
        hand.advance()?;
        Ok(hand)
    }

    pub fn is_over(&self) -> bool {
        self.outcome.is_some()
    }

    fn next_rng(&mut self) -> ChaCha8Rng {
        self.draws += 1;
        rng(self.deal.seed, self.draws)
    }

    fn class_of(&self, seat: usize) -> HandClass {
        let [a, b] = self.deal.holes[seat];
        Combo::new(a, b).class()
    }

    fn stacks(&self) -> Vec<f64> {
        (0..self.n).map(|s| self.deal.table.stack_bb - self.ante[s] - self.put[s]).collect()
    }

    fn pot(&self) -> f64 {
        self.ante.iter().sum::<f64>() + self.put.iter().sum::<f64>()
    }

    fn to_act(&self) -> Option<usize> {
        match &self.phase {
            Phase::Deep(w) => w.seat(&self.deep.as_ref()?.solution),
            Phase::Short(w) => w.node.map(|(s, _)| s),
            Phase::Post { node, .. } => node.player.map(|p| self.post_seat(p)),
            Phase::Over => None,
        }
    }

    fn snapshot(&mut self) -> usize {
        let snap = Snapshot {
            street: self.street,
            board: self.deal.board[..self.board_len].to_vec(),
            bets: self.street_put.clone(),
            stacks: self.stacks(),
            pot: self.pot(),
            folded: self.folded.clone(),
            to_act: self.to_act(),
        };
        self.snapshots.push(snap);
        self.snapshots.len() - 1
    }

    fn post_seat(&self, p: Player) -> usize {
        let sc = &self.deal.spot.as_ref().expect("postflop needs a spot").scenario;
        if p == Player::Oop { sc.oop } else { sc.ip }
    }

    fn hero_acts(&self) -> bool {
        let Some(seat) = self.to_act() else { return false };
        // In the postflop mode the hero's preflop moves follow the line to the spot.
        let auto = self.deal.config.mode == Mode::Postflop && self.street == Street::Preflop && self.path_pos.is_some();
        seat == self.deal.hero && !auto
    }

    /// Opponents (and the hero's automatic moves) act until the hero decides or the hand ends.
    fn advance(&mut self) -> Result<(), String> {
        loop {
            if self.outcome.is_some() || self.hero_acts() {
                return Ok(());
            }
            match &self.phase {
                Phase::Deep(walk) => match walk.node {
                    None => self.deep_end()?,
                    Some(v) => {
                        let a = self.deep_choice(v);
                        self.deep_play(v, a, None);
                    }
                },
                Phase::Short(walk) => match walk.node {
                    None => self.short_end(),
                    Some((seat, v)) => {
                        let t = self.short.clone().expect("short table");
                        let act = t.solution.nodes[v].act[self.class_of(seat).index()];
                        let a = pick(&[1.0 - act, act], &mut self.next_rng()).unwrap_or(0);
                        self.short_play(a, None);
                    }
                },
                Phase::Post { node, .. } => match node.kind {
                    NodeKind::Terminal => self.post_end(),
                    NodeKind::Chance => {
                        let card = self.next_board_card(node);
                        self.post_step(PostStep::Card(card.to_string()), None)?;
                    }
                    NodeKind::Action => {
                        let seat = self.post_seat(node.player.expect("action node"));
                        let strategy = self.post_strategy(node, seat).map(|(s, _, _)| s);
                        let fallback = most_frequent(node);
                        let a = strategy.and_then(|s| pick(&s, &mut self.next_rng())).unwrap_or(fallback);
                        self.post_step(PostStep::Action(a), None)?;
                    }
                },
                Phase::Over => return Ok(()),
            }
        }
    }

    /// An opponent's choice at a model node: the line to the spot while the hand follows it,
    /// else by frequency for its class.
    fn deep_choice(&mut self, v: usize) -> usize {
        let t = self.deep.clone().expect("deep table");
        if let (Some(pos), Some(spot)) = (self.path_pos, &self.deal.spot)
            && let Some(&(pv, pa)) = spot.scenario.path.get(pos)
            && pv == v
        {
            return pa;
        }
        let node = &t.solution.nodes[v];
        let c = self.class_of(node.seat).index();
        let freq: Vec<f64> = node.freq.iter().map(|f| f[c]).collect();
        pick(&freq, &mut self.next_rng()).unwrap_or(0)
    }

    fn deep_play(&mut self, v: usize, a: usize, verdict: Option<usize>) {
        let t = self.deep.clone().expect("deep table");
        let node = &t.solution.nodes[v];
        if let Some(pos) = self.path_pos {
            let spot = self.deal.spot.as_ref().expect("spot");
            self.path_pos = (spot.scenario.path.get(pos) == Some(&(v, a))).then_some(pos + 1);
        }
        if node.seat == self.deal.hero {
            self.hero_steps.push((v, a));
        }
        let Phase::Deep(walk) = &mut self.phase else { unreachable!() };
        let before = walk.contrib[node.seat];
        let passed = walk.play(&t.solution, a);
        self.put = walk.contrib.clone();
        self.street_put = walk.contrib.clone();
        self.folded = walk.folded.clone();
        let action = node.actions[a];
        let pot = self.ante.iter().sum::<f64>() + self.put.iter().sum::<f64>() - (action.to_bb - before);
        let choice = model_choice(action.kind, action.to_bb, before, pot, &self.street_put);
        self.push_action(node.seat, choice, verdict, &passed);
    }

    /// The event of `seat`'s action, then a fold of each seat in `passed` (already marked
    /// folded), one snapshot each, so the page can show them one after another.
    fn push_action(&mut self, seat: usize, choice: Choice, verdict: Option<usize>, passed: &[usize]) {
        for &s in passed {
            self.folded[s] = false;
        }
        let snapshot = self.snapshot();
        if let Some(&first) = passed.first() {
            self.snapshots[snapshot].to_act = Some(first);
        }
        self.events.push(Event::Action { street: self.street, seat, choice, verdict, snapshot });
        for (i, &s) in passed.iter().enumerate() {
            self.folded[s] = true;
            let snapshot = self.snapshot();
            if let Some(&next) = passed.get(i + 1) {
                self.snapshots[snapshot].to_act = Some(next);
            }
            let choice = Choice { kind: "fold", to_bb: self.street_put[s], add_bb: 0.0, pct_pot: None };
            self.events.push(Event::Action { street: self.street, seat: s, choice, verdict: None, snapshot });
        }
    }

    fn deep_end(&mut self) -> Result<(), String> {
        let Phase::Deep(walk) = &self.phase else { unreachable!() };
        match walk.end() {
            End::Won(w) => self.finish(EndReason::Folded, vec![w]),
            End::Showdown(left) => self.showdown(left),
            End::Flop(left) => {
                let spot = self.deal.spot.clone();
                match (spot, self.path_pos) {
                    (Some(spot), Some(_)) => {
                        let tree = self.tree.clone().expect("tree");
                        let node = tree.lock().unwrap().node(&[]).map_err(|e| e.to_string())?;
                        self.pre_put = self.put.clone();
                        self.street_put = vec![0.0; self.n];
                        self.street = Street::Flop;
                        self.board_len = 3;
                        self.phase = Phase::Post { path: Vec::new(), node: Box::new(node) };
                        let snapshot = self.snapshot();
                        self.events.push(Event::Board { street: Street::Flop, cards: spot.flop.to_vec(), snapshot });
                    }
                    (Some(_), None) => self.stop(EndReason::LeftLine, left),
                    (None, _) => self.stop(EndReason::FlopUnsolved, left),
                }
            }
        }
        Ok(())
    }

    fn short_play(&mut self, a: usize, verdict: Option<usize>) {
        let t = self.short.clone().expect("short table");
        let Phase::Short(walk) = &mut self.phase else { unreachable!() };
        let (seat, _) = walk.node.expect("short node");
        let before = walk.contrib[seat];
        let pot_before = self.ante.iter().sum::<f64>() + walk.contrib.iter().sum::<f64>();
        let was_folded = walk.folded.clone();
        let kind = walk.play(&t, a);
        self.put = walk.contrib.clone();
        self.street_put = walk.contrib.clone();
        self.folded = walk.folded.clone();
        let choice = model_choice(kind, self.put[seat], before, pot_before, &self.street_put);
        if seat == self.deal.hero {
            self.hero_steps.push((seat, a));
        }
        // Once the pot is full the equilibrium folds everyone left without a node.
        let passed: Vec<usize> =
            (1..self.n).map(|k| (seat + k) % self.n).filter(|&s| self.folded[s] && !was_folded[s]).collect();
        self.push_action(seat, choice, verdict, &passed);
    }

    fn short_end(&mut self) {
        let Phase::Short(walk) = &self.phase else { unreachable!() };
        match walk.end() {
            End::Won(w) => self.finish(EndReason::Folded, vec![w]),
            End::Showdown(left) | End::Flop(left) => self.showdown(left),
        }
    }

    fn next_board_card(&self, node: &Node) -> Card {
        let planned = self.deal.board[self.board_len];
        if node.cards.iter().any(|c| *c == planned.to_string()) {
            return planned;
        }
        // The planned card cannot come here (it never happens with a full deck); take the
        // first one that can and that nobody holds.
        let held = CardSet::from_cards(&self.deal.holes.concat());
        node.cards.iter().filter_map(|c| c.parse::<Card>().ok()).find(|c| !held.contains(*c)).unwrap_or(planned)
    }

    /// The acting seat's strategy, EVs (if stored per combo) and equity at a postflop node.
    fn post_strategy(&self, node: &Node, seat: usize) -> Option<HandStrategy> {
        let [a, b] = self.deal.holes[seat];
        let combo = Combo::new(a, b).to_string();
        let swapped = format!("{b}{a}");
        if let Some(h) = node.hands.iter().find(|h| h.combo == combo || h.combo == swapped) {
            let evs = (h.evs.len() == h.strategy.len()).then(|| h.evs.clone());
            return Some((h.strategy.clone(), evs, Some(h.equity)));
        }
        let class = Combo::new(a, b).class().index();
        node.classes.iter().find(|c| c.class == class && c.combos > 0.0).map(|c| (c.strategy.clone(), None, Some(c.equity)))
    }

    fn post_step(&mut self, step: PostStep, verdict: Option<usize>) -> Result<(), String> {
        let Phase::Post { path, node } = &self.phase else { unreachable!() };
        let mut path = path.clone();
        let old = node.clone();
        path.push(step.clone());
        let tree = self.tree.clone().expect("tree");
        let next = tree.lock().unwrap().node(&path);
        let side = |p: Player| if p == Player::Oop { 0 } else { 1 };
        if let PostStep::Action(a) = step {
            let seat = self.post_seat(old.player.expect("action node"));
            let action = &old.actions[a];
            let me = side(old.player.expect("action node"));
            let facing = old.street_bets[1 - me];
            let before = old.street_bets[me];
            let to = match action.kind {
                ActionKind::Fold | ActionKind::Check => before,
                ActionKind::Call => facing.min(before + old.stacks[me]),
                _ => action.amount,
            };
            if action.kind == ActionKind::Fold {
                self.folded[seat] = true;
            }
            let pot = old.pot;
            let pct = match action.kind {
                ActionKind::Bet => Some(action.amount / pot * 100.0),
                ActionKind::Raise => Some((action.amount - facing) / (pot + facing - before) * 100.0),
                ActionKind::AllIn if facing == 0.0 => Some(action.amount / pot * 100.0),
                _ => None,
            };
            let choice = Choice { kind: post_kind(action.kind), to_bb: to, add_bb: to - before, pct_pot: pct };
            self.put[seat] = self.pre_put[seat] + old.bets[me] - before + to;
            self.street_put[seat] = to;
            let snapshot = self.snapshot_after(&next);
            self.events.push(Event::Action { street: self.street, seat, choice, verdict, snapshot });
        } else if let PostStep::Card(_) = step {
            self.board_len += 1;
            self.street = if self.board_len == 4 { Street::Turn } else { Street::River };
            self.street_put = vec![0.0; self.n];
            let snapshot = self.snapshot_after(&next);
            let card = self.deal.board[self.board_len - 1];
            let street = self.street;
            self.events.push(Event::Board { street, cards: vec![card], snapshot });
        }
        match next {
            Ok(node) => {
                self.phase = Phase::Post { path, node: Box::new(node) };
                Ok(())
            }
            Err(_) => {
                let seats = self.deal.spot.as_ref().expect("spot").scenario.seats().to_vec();
                self.stop(EndReason::NotStored, seats);
                Ok(())
            }
        }
    }

    fn snapshot_after(&mut self, next: &Result<Node, ringside_solver::Error>) -> usize {
        // `to_act` reads the phase, which still holds the old node; patch it afterwards.
        let i = self.snapshot();
        let seat = next.as_ref().ok().and_then(|n| n.player).map(|p| self.post_seat(p));
        self.snapshots[i].to_act = seat;
        i
    }

    fn post_end(&mut self) {
        let Phase::Post { node, .. } = &self.phase else { unreachable!() };
        let seats = self.deal.spot.as_ref().expect("spot").scenario.seats();
        let left: Vec<usize> = seats.into_iter().filter(|&s| !self.folded[s]).collect();
        let _ = node;
        if left.len() == 1 {
            self.finish(EndReason::Folded, left);
        } else {
            self.showdown(left);
        }
    }

    fn showdown(&mut self, left: Vec<usize>) {
        while self.board_len < 5 {
            self.board_len += 1;
            let card = self.deal.board[self.board_len - 1];
            let (street, cards) = match self.board_len {
                3 => (Street::Flop, self.deal.board[..3].to_vec()),
                4 => (Street::Turn, vec![card]),
                5 => (Street::River, vec![card]),
                _ => continue,
            };
            self.street = street;
            let snapshot = self.snapshot();
            self.events.push(Event::Board { street, cards, snapshot });
        }
        let ranks: Vec<u16> = (0..self.n).map(|s| self.rank(s).0).collect();
        let best = left.iter().map(|&s| ranks[s]).max().unwrap_or(0);
        let winners: Vec<usize> = left.iter().copied().filter(|&s| ranks[s] == best).collect();
        let net = settle(&self.put, &self.ante, &left, &ranks);
        self.outcome = Some(self.outcome_of(EndReason::Showdown, winners, net, &left));
        self.phase = Phase::Over;
    }

    fn rank(&self, seat: usize) -> ringside_engine::HandRank {
        let mut cards = self.deal.board.to_vec();
        cards.extend(self.deal.holes[seat]);
        evaluate(CardSet::from_cards(&cards))
    }

    /// Everyone else folded.
    fn finish(&mut self, reason: EndReason, winners: Vec<usize>) {
        let pot = self.pot();
        let net: Vec<f64> = (0..self.n)
            .map(|s| if winners.contains(&s) { pot } else { 0.0 } - self.put[s] - self.ante[s])
            .collect();
        let shown = self.deal.spot.as_ref().map(|s| s.scenario.seats().to_vec()).unwrap_or_default();
        self.outcome = Some(self.outcome_of(reason, winners, net, &shown));
        self.phase = Phase::Over;
    }

    /// The hand stops before its end: nobody wins, the money stays where it is.
    fn stop(&mut self, reason: EndReason, left: Vec<usize>) {
        let net = (0..self.n).map(|s| -self.put[s] - self.ante[s]).collect();
        self.outcome = Some(self.outcome_of(reason, Vec::new(), net, &left));
        self.phase = Phase::Over;
    }

    fn outcome_of(&self, reason: EndReason, winners: Vec<usize>, net: Vec<f64>, show: &[usize]) -> Outcome {
        let full = self.board_len == 5;
        let shown: Vec<Option<[Card; 2]>> =
            (0..self.n).map(|s| (s != self.deal.hero && show.contains(&s)).then_some(self.deal.holes[s])).collect();
        let made = (0..self.n)
            .map(|s| (full && (s == self.deal.hero || shown[s].is_some()) && !self.folded[s]).then(|| category_de(self.rank(s).category()).to_string()))
            .collect();
        Outcome { reason, winners, net, shown, made }
    }

    // -----------------------------------------------------------------------------
    // The hero.

    pub fn decision(&self) -> Option<Decision> {
        if !self.hero_acts() {
            return None;
        }
        let (choices, pot, to_call) = self.choices()?;
        Some(Decision { street: self.street, choices, pot, to_call, hinted: self.hinted })
    }

    fn choices(&self) -> Option<(Vec<Choice>, f64, f64)> {
        let pot = self.pot();
        let me = self.deal.hero;
        let top = self.street_put.iter().copied().fold(0.0, f64::max);
        let to_call = (top - self.street_put[me]).min(self.stacks()[me]).max(0.0);
        let choices = match &self.phase {
            Phase::Deep(walk) => {
                let node = &self.deep.as_ref()?.solution.nodes[walk.node?];
                node.actions.iter().map(|a| model_choice(a.kind, a.to_bb, walk.contrib[me], pot, &walk.contrib)).collect()
            }
            Phase::Short(walk) => {
                let (seat, _) = walk.node?;
                let t = self.short.as_ref()?;
                (0..2).map(|a| model_choice(walk.kind(t, seat, a), walk.amount(seat, a), walk.contrib[seat], pot, &walk.contrib)).collect()
            }
            Phase::Post { node, .. } => {
                let p = node.player?;
                let side = if p == Player::Oop { 0 } else { 1 };
                let facing = node.street_bets[1 - side];
                let before = node.street_bets[side];
                node.actions
                    .iter()
                    .map(|a| {
                        let to = match a.kind {
                            ActionKind::Fold | ActionKind::Check => before,
                            ActionKind::Call => facing.min(before + node.stacks[side]),
                            _ => a.amount,
                        };
                        let pct = match a.kind {
                            ActionKind::Bet => Some(a.amount / node.pot * 100.0),
                            ActionKind::Raise => Some((a.amount - facing) / (node.pot + facing - before) * 100.0),
                            ActionKind::AllIn if facing == 0.0 => Some(a.amount / node.pot * 100.0),
                            _ => None,
                        };
                        Choice { kind: post_kind(a.kind), to_bb: to, add_bb: to - before, pct_pot: pct }
                    })
                    .collect()
            }
            Phase::Over => return None,
        };
        Some((choices, pot, to_call))
    }

    /// What the solution does with the hero's hand right now.
    pub fn lesson(&self) -> Option<Lesson> {
        if !self.hero_acts() {
            return None;
        }
        let (choices, pot, to_call) = self.choices()?;
        let me = self.deal.hero;
        let class = self.class_of(me);
        let label = class.to_string();
        let base = |source, strategy, evs, equity| Lesson {
            street: self.street,
            choices: choices.clone(),
            source,
            strategy,
            evs,
            pot,
            to_call,
            equity,
            class: label.clone(),
            node: None,
            grid: None,
            path: None,
            exploitability_pct: None,
        };
        match &self.phase {
            Phase::Deep(walk) => {
                let sol = &self.deep.as_ref()?.solution;
                let v = walk.node?;
                let node = &sol.nodes[v];
                let c = class.index();
                let freq: Vec<f64> = node.freq.iter().map(|f| f[c]).collect();
                let reached = freq.iter().sum::<f64>() > 0.0;
                let evs: Vec<f64> = node.ev.iter().map(|e| e[c]).collect();
                let mut l = base(Source::Model, reached.then_some(freq), reached.then_some(evs), None);
                // The hero's own earlier decisions shape which hands reach this node.
                let reach: Vec<f64> = (0..NUM_CLASSES)
                    .map(|k| self.hero_steps.iter().map(|&(u, a)| sol.nodes[u].freq[a][k]).product())
                    .collect();
                let strategy = (0..NUM_CLASSES).map(|k| node.freq.iter().map(|f| f[k]).collect()).collect();
                l.grid = Some(Grid { reach, strategy });
                Some(l)
            }
            Phase::Short(walk) => {
                let (_, v) = walk.node?;
                let n = &self.short.as_ref()?.solution.nodes[v];
                let c = class.index();
                let mut l = base(Source::Equilibrium, Some(vec![1.0 - n.act[c], n.act[c]]), Some(vec![n.ev_fold[c], n.ev_act[c]]), None);
                l.grid = Some(Grid { reach: vec![1.0; NUM_CLASSES], strategy: n.act.iter().map(|&x| vec![1.0 - x, x]).collect() });
                Some(l)
            }
            Phase::Post { path, node } => {
                let found = self.post_strategy(node, me);
                let (strategy, evs, equity) = match found {
                    Some((s, e, q)) => (Some(s), e, q),
                    None => (None, None, None),
                };
                let source = self.deal.spot.as_ref().map(|s| s.source.into()).unwrap_or(Source::Solver);
                let mut l = base(source, strategy, evs, equity);
                l.node = Some((**node).clone());
                l.path = Some(path.clone());
                l.exploitability_pct = self.deal.spot.as_ref().map(|s| s.exploitability_pct);
                Some(l)
            }
            Phase::Over => None,
        }
    }

    /// Shows the strategy before the move; the decision then counts only partly.
    pub fn hint(&mut self) -> Option<Lesson> {
        let l = self.lesson()?;
        self.hinted = true;
        Some(l)
    }

    /// The hero takes choice `a`; returns its verdict.
    pub fn act(&mut self, a: usize) -> Result<Verdict, String> {
        let lesson = self.lesson().ok_or("Du bist gerade nicht am Zug.")?;
        if a >= lesson.choices.len() {
            return Err("Diese Aktion gibt es hier nicht.".into());
        }
        let judgement = lesson.strategy.as_ref().map(|s| judge(s, lesson.evs.as_deref(), a, lesson.pot));
        let hinted = std::mem::take(&mut self.hinted);
        let points = judgement.as_ref().map(|j| j.grade.points() * if hinted { HINT_FACTOR } else { 1.0 });
        let verdict = Verdict { lesson, chosen: a, judgement, hinted, points };
        self.verdicts.push(verdict.clone());
        let index = Some(self.verdicts.len() - 1);
        match &self.phase {
            Phase::Deep(walk) => {
                let v = walk.node.expect("node");
                self.deep_play(v, a, index);
            }
            Phase::Short(_) => self.short_play(a, index),
            Phase::Post { .. } => self.post_step(PostStep::Action(a), index)?,
            Phase::Over => {}
        }
        self.advance()?;
        Ok(verdict)
    }

    pub fn summary(&self) -> Summary {
        let graded: Vec<&Verdict> = self.verdicts.iter().filter(|v| v.points.is_some()).collect();
        let score = if graded.is_empty() { 0.0 } else { graded.iter().filter_map(|v| v.points).sum::<f64>() / graded.len() as f64 / 100.0 };
        let ev_loss_bb = graded.iter().filter_map(|v| v.judgement.as_ref()?.loss_bb).sum();
        Summary { score, ev_loss_bb, graded: graded.len() }
    }

    pub fn view(&self) -> HandView {
        let spot = self.deal.spot.as_ref().map(|s| SpotInfo {
            id: s.id,
            source: s.source.into(),
            complete: s.complete,
            exploitability_pct: s.exploitability_pct,
            oop: s.scenario.oop,
            ip: s.scenario.ip,
            pot: s.scenario.pot,
            stack: s.scenario.stack,
        });
        HandView {
            seed: self.deal.seed,
            config: self.deal.config.clone(),
            players: self.n,
            positions: ringside_engine::multiway::position_names(self.n),
            stack_bb: self.deal.table.stack_bb,
            ante: self.ante.clone(),
            hero: self.deal.hero,
            hero_cards: self.deal.holes[self.deal.hero],
            board: self.deal.board[..self.board_len].to_vec(),
            events: self.events.clone(),
            snapshots: self.snapshots.clone(),
            verdicts: self.verdicts.clone(),
            decision: self.decision(),
            outcome: self.outcome.clone(),
            preflop_source: if self.short.is_some() { Source::Equilibrium } else { Source::Model },
            spot,
            waiting: self.deal.waiting,
        }
    }

    /// The postflop tree and the path of the hero's costliest postflop decision, for the
    /// solver page.
    pub fn worst_postflop(&self) -> Option<(u64, Vec<PostStep>)> {
        let worst = self
            .verdicts
            .iter()
            .filter(|v| v.lesson.path.is_some())
            .max_by(|a, b| loss(a).total_cmp(&loss(b)))?;
        Some((self.deal.spot.as_ref()?.id, worst.lesson.path.clone()?))
    }

    pub fn tree(&self) -> Option<Arc<Mutex<Box<dyn Tree>>>> {
        self.tree.clone()
    }
}

fn loss(v: &Verdict) -> f64 {
    v.judgement.as_ref().map_or(0.0, |j| j.loss_pct.unwrap_or(100.0 - j.frequency * 100.0))
}

fn most_frequent(node: &Node) -> usize {
    (0..node.actions.len()).max_by(|&a, &b| node.actions[a].frequency.total_cmp(&node.actions[b].frequency)).unwrap_or(0)
}

fn post_kind(k: ActionKind) -> &'static str {
    match k {
        ActionKind::Fold => "fold",
        ActionKind::Check => "check",
        ActionKind::Call => "call",
        ActionKind::Bet => "bet",
        ActionKind::Raise => "raise",
        ActionKind::AllIn => "all_in",
    }
}

fn model_choice(kind: Kind, to: f64, before: f64, pot: f64, contrib: &[f64]) -> Choice {
    let top = contrib.iter().copied().fold(0.0, f64::max);
    let (k, to) = match kind {
        Kind::Fold => ("fold", before),
        Kind::Check => ("check", to),
        Kind::Call => ("call", to),
        Kind::Limp => ("limp", to),
        Kind::Raise => ("raise", to),
        Kind::AllIn => ("all_in", to),
    };
    let pct = matches!(kind, Kind::Raise | Kind::AllIn).then(|| {
        let call = (top - before).max(0.0);
        (to - top) / (pot + call) * 100.0
    });
    Choice { kind: k, to_bb: to, add_bb: (to - before).max(0.0), pct_pot: pct }
}

fn category_de(c: HandCategory) -> &'static str {
    match c {
        HandCategory::HighCard => "Höchste Karte",
        HandCategory::Pair => "Paar",
        HandCategory::TwoPair => "Zwei Paare",
        HandCategory::Trips => "Drilling",
        HandCategory::Straight => "Straße",
        HandCategory::Flush => "Flush",
        HandCategory::FullHouse => "Full House",
        HandCategory::Quads => "Vierling",
        HandCategory::StraightFlush => "Straight Flush",
    }
}

/// What every seat wins or loses at a showdown of `left`, with side pots. Antes and the
/// chips of folded players go to the pots they reach.
pub fn settle(put: &[f64], ante: &[f64], left: &[usize], ranks: &[u16]) -> Vec<f64> {
    let n = put.len();
    let mut net: Vec<f64> = (0..n).map(|s| -put[s] - ante[s]).collect();
    let mut levels: Vec<f64> = left.iter().map(|&s| put[s]).collect();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let mut below = 0.0;
    let mut dead = ante.iter().sum::<f64>();
    for (i, &level) in levels.iter().enumerate() {
        // The last level also takes whatever folded players put in above it.
        let top = if i + 1 == levels.len() { f64::INFINITY } else { level };
        let mut pot: f64 = (0..n).map(|s| put[s].min(top) - put[s].min(below)).map(|x| x.max(0.0)).sum();
        pot += std::mem::take(&mut dead);
        let eligible: Vec<usize> = left.iter().copied().filter(|&s| put[s] >= level - 1e-9).collect();
        let best = eligible.iter().map(|&s| ranks[s]).max().unwrap_or(0);
        let winners: Vec<usize> = eligible.into_iter().filter(|&s| ranks[s] == best).collect();
        for &w in &winners {
            net[w] += pot / winners.len() as f64;
        }
        below = level;
    }
    net
}

// ---------------------------------------------------------------------------------
// Push/fold.

/// Walking the push/fold equilibrium: shoves while everyone before folded, then calls.
#[derive(Clone, Debug)]
pub struct ShortWalk {
    /// Seat and node index of the next decision.
    pub node: Option<(usize, usize)>,
    pub shover: Option<usize>,
    pub callers: Vec<usize>,
    pub contrib: Vec<f64>,
    pub folded: Vec<bool>,
    pub live: Vec<f64>,
    seat: usize,
}

impl ShortWalk {
    fn start(t: &Short, ante: &[f64]) -> ShortWalk {
        let n = t.params.stacks_bb.len();
        let mut contrib = vec![0.0; n];
        contrib[n - 2] = t.params.small_blind_bb;
        contrib[n - 1] = 1.0;
        let live = (0..n).map(|s| t.params.stacks_bb[s] - ante[s]).collect();
        let mut w = ShortWalk { node: None, shover: None, callers: Vec::new(), contrib, folded: vec![false; n], live, seat: 0 };
        w.find(t);
        w
    }

    fn find(&mut self, t: &Short) {
        let n = self.contrib.len();
        self.node = None;
        while self.seat < n {
            let s = self.seat;
            let skip = match self.shover {
                None => s == n - 1,
                Some(sh) => s <= sh,
            };
            if !skip && let Some(v) = t.solution.node(s, self.shover, &self.callers) {
                self.node = Some((s, v));
                return;
            }
            if !skip && self.shover.is_some() {
                // The pot is full: everyone left folds.
                self.folded[s] = true;
            }
            self.seat += 1;
        }
    }

    fn kind(&self, _t: &Short, _seat: usize, a: usize) -> Kind {
        match (a, self.shover) {
            (0, _) => Kind::Fold,
            (_, None) => Kind::AllIn,
            _ => Kind::Call,
        }
    }

    fn amount(&self, seat: usize, a: usize) -> f64 {
        match (a, self.shover) {
            (0, _) => self.contrib[seat],
            (_, None) => self.live[seat],
            (_, Some(sh)) => self.contrib[sh].min(self.live[seat]),
        }
    }

    /// Takes choice `a` (0 fold, 1 shove or call) for the seat to act.
    fn play(&mut self, t: &Short, a: usize) -> Kind {
        let (seat, _) = self.node.expect("a decision");
        let kind = self.kind(t, seat, a);
        match kind {
            Kind::Fold => self.folded[seat] = true,
            Kind::AllIn => {
                self.contrib[seat] = self.live[seat];
                self.shover = Some(seat);
            }
            _ => {
                self.contrib[seat] = self.amount(seat, a);
                self.callers.push(seat);
            }
        }
        self.seat += 1;
        self.find(t);
        kind
    }

    fn end(&self) -> End {
        let n = self.contrib.len();
        match self.shover {
            None => End::Won(n - 1),
            Some(sh) if self.callers.is_empty() => End::Won(sh),
            Some(sh) => {
                let mut left = vec![sh];
                left.extend(&self.callers);
                left.sort_unstable();
                End::Showdown(left)
            }
        }
    }
}
