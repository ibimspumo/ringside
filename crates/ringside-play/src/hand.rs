//! One hand from the deal to the end: opponents act by their frequencies, the hero's
//! decisions are graded, a flop continues in a prepared heads-up spot.
//!
//! Every hand ends in a fold or a showdown. A heads-up flop nobody prepared (another
//! line, an empty stock) and a stored spot that ends wait in [`Phase::Solve`] until a live
//! solve arrives ([`Hand::resume`]). Three players after the flop, a line the preflop
//! model does not play and a stopped solve go on in [`crate::free`]: the opponents follow
//! a simple rule and the hero's moves stay ungraded unless an [`Assess`] grades them.
//!
//! Everything random comes from the deal's seed, so the same [`Deal`] plays the same hand
//! again ("Hand wiederholen"): same cards and board, and the same opponent choices as long
//! as the hero plays the same way.

use std::sync::{Arc, Mutex};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::preflop::{Kind, postflop_rank};
use ringside_engine::range::{HandClass, NUM_CLASSES};
use ringside_engine::{Card, CardSet, Combo, HandCategory, evaluate};
use ringside_solver::{ActionKind, Node, NodeKind, Player, Spot, Step as PostStep};
use serde::Serialize;

use crate::assess::{Assess, Move, Situation};
use crate::free::{self, Round};
use crate::grade::{HINT_FACTOR, Judgement, judge};
use crate::model::{End, Walk};
use crate::pool::{LIVE_PCT, Pool, Ready, Taken};
use crate::scenario::heads_up;
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
    /// Solved while the hand waited, to [`LIVE_PCT`].
    Live,
    Library,
    Saved,
    /// The calibrated estimate where no solver plays ([`crate::assess`]).
    Estimate,
}

impl From<TreeSource> for Source {
    fn from(s: TreeSource) -> Source {
        match s {
            TreeSource::Solved => Source::Solver,
            TreeSource::Live => Source::Live,
            TreeSource::Library => Source::Library,
            TreeSource::Saved => Source::Saved,
        }
    }
}

/// Why a decision has no grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unrated {
    /// The solution never plays the hero's hand this way to here.
    NotReached,
    /// Three players after the flop: no solver plays multiway.
    Multiway,
    /// A line the preflop model does not play (it lets only two players go on here).
    OffModel,
    /// The live solve was stopped or failed.
    NoSolve,
}

/// Why a hand waits for a live solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// The hand left the line to the prepared spot.
    NewLine,
    /// No spot was prepared for this flop.
    NoSpot,
    /// The stored solution ends here; the spot is solved again as a whole.
    Deeper,
}

/// A heads-up spot a hand waits for.
#[derive(Clone, Debug, PartialEq)]
pub struct Need {
    pub spot: Spot,
    pub table: TableKey,
    /// OOP and IP.
    pub seats: [usize; 2],
    /// It must hold every node to the river: a stored solution will not do.
    pub complete: bool,
    pub why: Why,
}

impl Need {
    /// A flop spot from its root.
    pub fn fresh(spot: Spot, table: TableKey, seats: [usize; 2]) -> Need {
        Need { spot, table, seats, complete: false, why: Why::NoSpot }
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
    /// Where the strategy comes from; `None` where nothing covers the decision.
    pub source: Option<Source>,
    /// `None`: nothing grades the decision, `unrated` says why.
    pub strategy: Option<Vec<f64>>,
    pub unrated: Option<Unrated>,
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
    /// For [`Source::Estimate`]: how often its grade was within one step of the solver's.
    pub reliability: Option<f64>,
    /// Why the best move is best, where the source says it.
    pub why: Option<String>,
    /// The postflop spot `path` belongs to.
    #[serde(skip)]
    pub spot_id: Option<u64>,
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
    /// The players left show down; once the hero folded, the others check it down.
    Showdown,
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
    /// A source grades this decision; else it stays "nicht bewertet".
    pub graded: bool,
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
    /// The spot the hand waits for.
    pub solving: Option<Solving>,
    /// Played on without a solution from here.
    pub free: Option<FreeInfo>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Solving {
    pub why: Why,
    pub street: Street,
    pub oop: usize,
    pub ip: usize,
    pub pot: f64,
    pub stack: f64,
    /// The precision of the live solve, percent of the pot.
    pub target_pct: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FreeInfo {
    pub why: Unrated,
    pub street: Street,
    /// How the opponents play from here, in German.
    pub rule: &'static str,
    /// Why the live solve stopped, if it did.
    pub note: Option<String>,
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

#[derive(Clone)]
enum Phase {
    Deep(Walk),
    Short(ShortWalk),
    Post { path: Vec<PostStep>, node: Box<Node> },
    /// Waits at `path` (in the spot to solve) for [`Hand::resume`].
    Solve { need: Box<Need>, path: Vec<PostStep> },
    Free(Round),
    Over,
}

/// The heads-up spot the hand plays in after the flop.
#[derive(Clone)]
struct PostSpot {
    seats: [usize; 2],
    table: TableKey,
    /// What a solve of it again would solve.
    spot: Spot,
    ready: Ready,
}

#[derive(Clone)]
pub struct Hand {
    pub deal: Deal,
    deep: Option<Arc<Deep>>,
    short: Option<Arc<Short>>,
    post: Option<PostSpot>,
    phase: Phase,
    /// Every preflop decision of the model's tree on the way (node, action).
    pre_path: Vec<(usize, usize)>,
    free_why: Option<Unrated>,
    note: Option<String>,
    assess: Option<Arc<dyn Assess>>,
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
            if steered || hand.decision().is_some() || hand.need().is_some() {
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
        let path_pos = deal.spot.as_ref().map(|_| 0);
        let mut hand = Hand {
            deep: deep_t,
            short: short_t,
            post: None,
            phase,
            pre_path: Vec::new(),
            free_why: None,
            note: None,
            assess: None,
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

    /// Grades decisions no solution covers from now on.
    pub fn set_assess(&mut self, assess: Arc<dyn Assess>) {
        self.assess = Some(assess);
    }

    /// The spot the hand waits for.
    pub fn need(&self) -> Option<&Need> {
        match &self.phase {
            Phase::Solve { need, .. } => Some(need),
            _ => None,
        }
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
            Phase::Free(r) => r.to_act,
            Phase::Solve { .. } | Phase::Over => None,
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
        let seats = self.post.as_ref().expect("postflop needs a spot").seats;
        if p == Player::Oop { seats[0] } else { seats[1] }
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
                Phase::Free(r) => match r.to_act {
                    None => self.free_end(),
                    Some(seat) => {
                        let a = self.free_choice(seat);
                        self.free_play(seat, a, None);
                    }
                },
                Phase::Solve { .. } | Phase::Over => return Ok(()),
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
        self.pre_path.push((v, a));
        let Phase::Deep(walk) = &mut self.phase else { unreachable!() };
        let before = walk.contrib[node.seat];
        let passed = walk.play(&t.solution, a);
        self.put = walk.contrib.clone();
        self.street_put = walk.contrib.clone();
        self.folded = walk.folded.clone();
        let action = node.actions[a];
        let pot = self.ante.iter().sum::<f64>() + self.put.iter().sum::<f64>() - (action.to_bb - before);
        let choice = model_choice(action.kind, action.to_bb, before, pot, &self.street_put);
        self.after_model(node.seat, choice, verdict, passed);
    }

    /// The event of a model action. Where the model folds the hero without a decision of
    /// its own (it lets only two players go on after a 3-bet or an all-in), the hero
    /// decides instead and the hand goes on without the model.
    fn after_model(&mut self, seat: usize, choice: Choice, verdict: Option<usize>, passed: Vec<usize>) {
        let Some(i) = passed.iter().position(|&s| s == self.deal.hero) else {
            return self.push_action(seat, choice, verdict, &passed);
        };
        for &s in &passed[i..] {
            self.folded[s] = false;
        }
        self.push_action(seat, choice, verdict, &passed[..i]);
        self.enter_free(Unrated::OffModel);
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
        let walk = walk.clone();
        match walk.end() {
            End::Won(w) => self.finish(EndReason::Folded, vec![w]),
            End::Showdown(left) => self.showdown(left),
            End::Flop(left) if !left.contains(&self.deal.hero) => self.showdown(left),
            End::Flop(left) => self.flop(&walk, left)?,
        }
        Ok(())
    }

    /// The hero sees the flop with `left`: in the prepared spot, a spot to solve now, or
    /// without a solution.
    fn flop(&mut self, walk: &Walk, left: Vec<usize>) -> Result<(), String> {
        self.pre_put = self.put.clone();
        self.street_put = vec![0.0; self.n];
        self.street = Street::Flop;
        self.board_len = 3;
        let t = self.deep.clone().expect("deep table");
        let steered = self.deal.spot.clone().filter(|_| self.path_pos.is_some());
        let scenario = heads_up(&t.solution, &t.params, &self.pre_path, walk, None, 0.0);
        match (steered, scenario) {
            (Some(taken), _) => {
                let spot = taken.scenario.spot(&cards_text(&taken.flop));
                let node = taken.ready.tree.lock().unwrap().node(&[]).map_err(|e| e.to_string())?;
                self.post = Some(PostSpot { seats: taken.scenario.seats(), table: taken.table, spot, ready: taken.ready });
                self.phase = Phase::Post { path: Vec::new(), node: Box::new(node) };
            }
            (None, Some(sc)) => {
                let side = sc.seats().iter().position(|&s| s == self.deal.hero).expect("the hero sees the flop");
                let [a, b] = self.deal.holes[self.deal.hero];
                let spot = sc.spot_with(&cards_text(&self.deal.board[..3]), Some((side, Combo::new(a, b))));
                let why = if self.deal.spot.is_some() { Why::NewLine } else { Why::NoSpot };
                let need = Need { spot, table: self.deal.table, seats: sc.seats(), complete: false, why };
                self.phase = Phase::Solve { need: Box::new(need), path: Vec::new() };
            }
            (None, None) => self.enter_free(if left.len() > 2 { Unrated::Multiway } else { Unrated::NoSolve }),
        }
        let snapshot = self.snapshot();
        self.events.push(Event::Board { street: Street::Flop, cards: self.deal.board[..3].to_vec(), snapshot });
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
        self.after_model(seat, choice, verdict, passed);
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
        let tree = self.post.as_ref().expect("a postflop spot").ready.tree.clone();
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
            Ok(node) => self.phase = Phase::Post { path, node: Box::new(node) },
            Err(_) => {
                // The stored solution ends here: a called all-in only runs out the board,
                // else the spot is solved again as a whole and the hand goes on at `path`.
                let post = self.post.clone().expect("a postflop spot");
                let left: Vec<usize> = post.seats.into_iter().filter(|&s| !self.folded[s]).collect();
                let stacks = self.stacks();
                let top = left.iter().map(|&s| self.street_put[s]).fold(0.0, f64::max);
                let closed = left.iter().all(|&s| self.street_put[s] >= top - 1e-9 || stacks[s] <= 1e-9);
                if left.len() < 2 || (closed && left.iter().any(|&s| stacks[s] <= 1e-9)) {
                    self.phase = Phase::Over;
                    if left.len() < 2 {
                        self.finish(EndReason::Folded, left);
                    } else {
                        self.showdown(left);
                    }
                } else {
                    let need = Need { spot: post.spot, table: post.table, seats: post.seats, complete: true, why: Why::Deeper };
                    self.phase = Phase::Solve { need: Box::new(need), path };
                }
            }
        }
        Ok(())
    }

    fn snapshot_after(&mut self, next: &Result<Node, ringside_solver::Error>) -> usize {
        // `to_act` reads the phase, which still holds the old node; patch it afterwards.
        let i = self.snapshot();
        let seat = next.as_ref().ok().and_then(|n| n.player).map(|p| self.post_seat(p));
        self.snapshots[i].to_act = seat;
        i
    }

    fn post_end(&mut self) {
        let seats = self.post.as_ref().expect("spot").seats;
        let left: Vec<usize> = seats.into_iter().filter(|&s| !self.folded[s]).collect();
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
        let shown = self.post.as_ref().map(|p| p.seats.to_vec()).unwrap_or_default();
        self.outcome = Some(self.outcome_of(reason, winners, net, &shown));
        self.phase = Phase::Over;
    }

    // -----------------------------------------------------------------------------
    // Waiting for a live solve, and playing on without one.

    /// Goes on with the spot the hand waited for, or without it (`Err`: why there is none).
    pub fn resume(&mut self, result: Result<Ready, String>) -> Result<(), String> {
        let Phase::Solve { need, path } = &self.phase else { return Err("Die Hand wartet auf keinen Spot.".into()) };
        let (need, path) = ((**need).clone(), path.clone());
        let node = result.and_then(|ready| {
            let node = ready.tree.lock().unwrap().node(&path).map_err(|e| e.to_string())?;
            Ok((ready, node))
        });
        match node {
            Ok((ready, node)) => {
                self.post = Some(PostSpot { seats: need.seats, table: need.table, spot: need.spot, ready });
                let seat = node.player.map(|p| self.post_seat(p));
                self.phase = Phase::Post { path, node: Box::new(node) };
                if let Some(last) = self.snapshots.last_mut() {
                    last.to_act = seat;
                }
            }
            Err(e) => {
                self.note = Some(e);
                self.enter_free(Unrated::NoSolve);
            }
        }
        self.advance()
    }

    /// Plays on from here without a solution.
    fn enter_free(&mut self, why: Unrated) {
        let preflop = self.street == Street::Preflop;
        let mut order: Vec<usize> = (0..self.n).filter(|&s| preflop || !self.folded[s]).collect();
        if !preflop {
            order.sort_by_key(|&s| postflop_rank(self.n, s));
        }
        // Who had a turn on this street since the last bet or raise, and who acted last.
        let mut acted = vec![false; self.n];
        let mut last = None;
        let mut top = if preflop { 1.0 } else { 0.0 };
        let street_start = self.events.iter().rposition(|e| matches!(e, Event::Board { .. })).map_or(0, |i| i + 1);
        for e in &self.events[street_start..] {
            if let Event::Action { seat, choice, .. } = e {
                if choice.to_bb > top + 1e-9 {
                    top = choice.to_bb;
                    acted.iter_mut().for_each(|a| *a = false);
                }
                acted[*seat] = true;
                last = Some(*seat);
            }
        }
        let round = Round::resume(order, self.street_put.clone(), self.stacks(), self.folded.clone(), acted, last);
        let seat = round.to_act;
        self.phase = Phase::Free(round);
        self.free_why.get_or_insert(why);
        if let Some(snap) = self.snapshots.last_mut() {
            snap.to_act = seat;
        }
    }

    /// An opponent's move by [`free::RULE`].
    fn free_choice(&mut self, seat: usize) -> usize {
        let Phase::Free(r) = &self.phase else { unreachable!() };
        let pot = self.pot();
        let choices = r.choices(seat, pot);
        let players = self.folded.iter().filter(|f| !**f).count();
        let seed = self.next_rng().random::<u64>();
        let strength = free::strength(self.deal.holes[seat], &self.deal.board[..self.board_len], players - 1, seed);
        free::rule(strength, players, &choices, pot, &mut self.next_rng())
    }

    fn free_play(&mut self, seat: usize, a: usize, verdict: Option<usize>) {
        let pot = self.pot();
        let Phase::Free(r) = &mut self.phase else { unreachable!() };
        let choice = r.choices(seat, pot)[a].clone();
        let add = r.play(seat, &choice);
        self.put[seat] += add;
        self.street_put = r.bets.clone();
        self.folded = r.folded.clone();
        self.push_action(seat, choice, verdict, &[]);
    }

    /// The street is over: the next one, or the end of the hand.
    fn free_end(&mut self) {
        let left: Vec<usize> = (0..self.n).filter(|&s| !self.folded[s]).collect();
        let stacks = self.stacks();
        if left.len() == 1 {
            return self.finish(EndReason::Folded, left);
        }
        let can_bet = left.iter().filter(|&&s| stacks[s] > 1e-9).count();
        if !left.contains(&self.deal.hero) || self.street == Street::River || can_bet <= 1 {
            return self.showdown(left);
        }
        let (street, cards) = match self.street {
            Street::Preflop => (Street::Flop, 3),
            Street::Flop => (Street::Turn, 1),
            _ => (Street::River, 1),
        };
        self.street = street;
        self.board_len += cards;
        self.pre_put = self.put.clone();
        self.street_put = vec![0.0; self.n];
        let mut order = left;
        order.sort_by_key(|&s| postflop_rank(self.n, s));
        self.phase = Phase::Free(Round::start(order, stacks, self.folded.clone()));
        let snapshot = self.snapshot();
        let cards = self.deal.board[self.board_len - cards..self.board_len].to_vec();
        self.events.push(Event::Board { street, cards, snapshot });
    }

    /// The spot the hand most likely needs after the hero's next move, to solve ahead.
    pub fn forecast(&self) -> Option<Need> {
        // A stored spot from the turn on ends after its first decision there.
        if let (Phase::Post { .. }, Some(post)) = (&self.phase, &self.post)
            && !post.ready.complete
            && self.street != Street::Flop
        {
            return Some(Need { spot: post.spot.clone(), table: post.table, seats: post.seats, complete: true, why: Why::Deeper });
        }
        let lesson = self.lesson()?;
        let strategy = lesson.strategy?;
        let mut order: Vec<usize> = (0..strategy.len()).filter(|&a| lesson.choices[a].kind != "fold" && strategy[a] > 0.0).collect();
        order.sort_by(|&a, &b| strategy[b].total_cmp(&strategy[a]));
        order.into_iter().find_map(|a| {
            let mut next = self.clone();
            next.act(a).ok()?;
            next.need().cloned()
        })
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
        let graded = self.lesson().is_some_and(|l| l.strategy.is_some());
        Some(Decision { street: self.street, choices, pot, to_call, hinted: self.hinted, graded })
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
            Phase::Free(r) => r.choices(me, pot),
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
            Phase::Solve { .. } | Phase::Over => return None,
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
        let base = |source: Source, strategy: Option<Vec<f64>>, evs, equity| Lesson {
            street: self.street,
            choices: choices.clone(),
            source: Some(source),
            unrated: strategy.is_none().then_some(Unrated::NotReached),
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
            reliability: None,
            why: None,
            spot_id: None,
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
                let post = self.post.as_ref()?;
                let mut l = base(post.ready.source.into(), strategy, evs, equity);
                l.node = Some((**node).clone());
                l.path = Some(path.clone());
                l.exploitability_pct = Some(post.ready.exploitability_pct);
                l.spot_id = Some(post.ready.id);
                Some(l)
            }
            Phase::Free(r) => {
                let why = self.free_why.unwrap_or(Unrated::NoSolve);
                let opponents: Vec<usize> = (0..self.n).filter(|&s| s != me && !self.folded[s]).collect();
                let stacks = self.stacks();
                let ranges = opponents.iter().map(|&s| self.model_range(s)).collect();
                let situation = Situation {
                    street: self.street,
                    board: &self.deal.board[..self.board_len],
                    hero: self.deal.holes[me],
                    hero_seat: me,
                    opponents,
                    players: self.n,
                    pot,
                    to_call: r.to_call(me),
                    stacks: &stacks,
                    bets: &self.street_put,
                    ranges,
                    moves: self.postflop_moves(),
                    aggressor: self.aggressor(),
                    choices: &choices,
                    why,
                };
                let found = self.assess.as_ref().and_then(|a| a.assess(&situation)).filter(|x| x.strategy.len() == choices.len());
                let mut l = match found {
                    Some(x) => {
                        let mut l = base(Source::Estimate, Some(x.strategy), x.evs, x.equity);
                        l.reliability = x.reliability;
                        l.why = x.why;
                        l
                    }
                    None => base(Source::Estimate, None, None, None),
                };
                if l.strategy.is_none() {
                    l.source = None;
                    l.unrated = Some(why);
                }
                Some(l)
            }
            Phase::Solve { .. } | Phase::Over => None,
        }
    }

    /// The preflop model's range of `seat` along the hand's line, weight per class.
    fn model_range(&self, seat: usize) -> Option<Vec<f64>> {
        let sol = &self.deep.as_ref()?.solution;
        let steps: Vec<(usize, usize)> = self.pre_path.iter().copied().filter(|&(v, _)| sol.nodes[v].seat == seat).collect();
        Some(
            (0..NUM_CLASSES)
                .map(|c| steps.iter().map(|&(v, a)| sol.nodes[v].freq[a][c]).map(|f| if ringside_engine::preflop::taken(f) { f } else { 0.0 }).product())
                .collect(),
        )
    }

    fn postflop_moves(&self) -> Vec<Move> {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::Action { street, seat, choice, .. } if *street != Street::Preflop => Some(Move { street: *street, seat: *seat, kind: choice.kind }),
                _ => None,
            })
            .collect()
    }

    fn aggressor(&self) -> Option<usize> {
        self.events.iter().rev().find_map(|e| match e {
            Event::Action { seat, choice, .. } if matches!(choice.kind, "bet" | "raise" | "all_in") => Some(*seat),
            _ => None,
        })
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
            Phase::Free(_) => self.free_play(self.deal.hero, a, index),
            Phase::Solve { .. } | Phase::Over => {}
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
        let spot = self.post.as_ref().map(|p| SpotInfo {
            id: p.ready.id,
            source: p.ready.source.into(),
            complete: p.ready.complete,
            exploitability_pct: p.ready.exploitability_pct,
            oop: p.seats[0],
            ip: p.seats[1],
            pot: p.spot.pot,
            stack: p.spot.stack,
        });
        let solving = self.need().map(|n| Solving {
            why: n.why,
            street: self.street,
            oop: n.seats[0],
            ip: n.seats[1],
            pot: self.pot(),
            stack: n.seats.iter().map(|&s| self.stacks()[s]).fold(f64::INFINITY, f64::min),
            target_pct: LIVE_PCT,
        });
        let free = self.free_why.map(|why| FreeInfo {
            why,
            street: self.street_of_free(),
            rule: free::RULE,
            note: self.note.clone(),
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
            solving,
            free,
        }
    }

    /// The street on which the hand left the solutions.
    fn street_of_free(&self) -> Street {
        let after = self.verdicts.iter().find(|v| v.lesson.unrated.is_some_and(|u| u != Unrated::NotReached));
        after.map_or(self.street, |v| v.lesson.street)
    }

    /// The postflop tree and the path of the hero's costliest postflop decision, for the
    /// solver page.
    pub fn worst_postflop(&self) -> Option<(u64, Vec<PostStep>)> {
        let id = self.post.as_ref()?.ready.id;
        let worst = self
            .verdicts
            .iter()
            .filter(|v| v.lesson.path.is_some() && v.lesson.spot_id == Some(id))
            .max_by(|a, b| loss(a).total_cmp(&loss(b)))?;
        Some((id, worst.lesson.path.clone()?))
    }

    /// The postflop spot the hand plays in now.
    pub fn tree(&self) -> Option<Arc<Mutex<Box<dyn Tree>>>> {
        self.post.as_ref().map(|p| p.ready.tree.clone())
    }
}

fn cards_text(cards: &[Card]) -> String {
    cards.iter().map(|c| c.to_string()).collect()
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
