//! Fits the estimate's free numbers and measures how well its grades agree with the
//! solver (after the flop) and the preflop model (before it). With `--write` the result
//! goes to `data/estimate_calibration.json`, which the analysis shows as the estimate's
//! reliability:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin analysis-calibrate -- \
//!     [--hands DIR] [--real N] [--minutes M] [--threads T] [--parallel P] [--cases FILE] [--csv FILE] [--postflop-tuning JSON] [--fixture FILE EVERY] [--write]
//! ```
//!
//! - Postflop: heads-up flops of the preflop model's tables (every family, ante, table
//!   size and stack, random lines and boards) solved to 1 % of the pot for `--minutes`
//!   (default 30). Along a random line through flop, turn and river the decisions of
//!   both players are checked (first to act, facing a bet, facing a raise); every option
//!   of a sample of the acting player's hands is graded by the solver and by the estimate,
//!   the opponent's range narrowed by the estimate's own rules along the line.
//! - Preflop: every option of every model-rated decision of `--hands`.
//! - Real: the decisions actually played in those hands at solved nodes (the `--real`
//!   biggest pots, 0 for all, default 0). Only measured, never fitted to.
//!
//! The solves are kept in `--cases` (default `/tmp/ringside-calibration-cases.json`):
//! when the file exists nothing is solved again. Hands of a spot are all fitted or all
//! tested, every other spot (real hands by id), so the agreement is not measured on what
//! the numbers were fitted to. Grades are compared on five levels (best and mix count as
//! one).

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use ringside_analysis::calibration::{Agreement, Calibration, SolvedHand, SolvedNode};
use ringside_analysis::estimate::{Opt, Prepared, Signal, Spot, Tuning, grades, prepare, settle, strength};
use ringside_analysis::{Decision, PostflopJob, Rating, Source, analyze_hand, replay, solve_job};
use ringside_core::model::Hand;
use ringside_core::{parse_hand, split_hands};
use ringside_engine::preflop::Kind as PreKind;
use ringside_engine::preflop_table::{self, Ante, Family, STACKS};
use ringside_engine::{Card, Combo};
use ringside_play::grade::{Grade, MIX_MIN_FREQUENCY, judge};
use ringside_play::scenario::PotType;
use ringside_solver::{ActionKind as Tree, NodeKind, Player, Settings, Solution, Step, solve};
use serde::{Deserialize, Serialize};

/// What the solves leave to measure against, kept between runs.
#[derive(Default, Serialize, Deserialize)]
struct Store {
    nodes: Vec<SolvedNode>,
    /// The solver's decisions of real hands, by hand id.
    real: Vec<(u64, Vec<Decision>)>,
    solve_seconds: f64,
    spots: usize,
}

/// One graded hand at one node: the reference grade of every option.
struct Case {
    spot: Spot,
    options: Vec<Opt>,
    reference: Vec<Grade>,
    /// How often and with what EV the reference plays each option with this hand.
    freq: Vec<f64>,
    evs: Vec<f64>,
    pot: f64,
    /// Which half it belongs to (fit or test), by spot.
    test: bool,
    seed: u64,
    /// Index into the shared board strengths.
    board: usize,
    node: usize,
    /// Position among the sampled hands of its node.
    hand: usize,
    true_equity: f64,
}

/// Hands per node the numbers are fitted on; the rest of the fitting half adds little
/// but time.
const FIT_HANDS: usize = 5;

fn level(g: Grade) -> usize {
    match g {
        Grade::Best | Grade::Mix => 0,
        Grade::Good => 1,
        Grade::Inaccurate => 2,
        Grade::Mistake => 3,
        Grade::Blunder => 4,
    }
}

#[derive(Default, Clone)]
struct Tally {
    samples: usize,
    same: usize,
    within: usize,
    matrix: [[usize; 5]; 5],
    by_kind: BTreeMap<String, (usize, usize)>,
    /// Options the reference plays at least as often as a mix, and how many of them the
    /// estimate calls a mistake or worse.
    mixed: usize,
    mixed_wrong: usize,
}

impl Tally {
    fn add(&mut self, kind: &str, a: Grade, b: Grade) {
        let d = level(a).abs_diff(level(b));
        self.matrix[level(a)][level(b)] += 1;
        self.samples += 1;
        self.same += usize::from(d == 0);
        self.within += usize::from(d <= 1);
        let e = self.by_kind.entry(kind.to_string()).or_default();
        e.0 += 1;
        e.1 += usize::from(d <= 1);
    }

    fn mix(&mut self, freq: f64, mine: Grade) {
        if freq >= MIX_MIN_FREQUENCY {
            self.mixed += 1;
            self.mixed_wrong += usize::from(mine >= Grade::Mistake);
        }
    }

    fn merge(mut self, o: Tally) -> Tally {
        self.mixed += o.mixed;
        self.mixed_wrong += o.mixed_wrong;
        self.samples += o.samples;
        self.same += o.same;
        self.within += o.within;
        for i in 0..5 {
            for j in 0..5 {
                self.matrix[i][j] += o.matrix[i][j];
            }
        }
        for (k, (n, w)) in o.by_kind {
            let e = self.by_kind.entry(k).or_default();
            e.0 += n;
            e.1 += w;
        }
        self
    }

    /// Within one step per reference grade, averaged over the grades that occur.
    fn balanced(&self) -> f64 {
        let rows: Vec<f64> = (0..5)
            .filter_map(|r| {
                let n: usize = self.matrix[r].iter().sum();
                let near: usize = (r.saturating_sub(1)..=(r + 1).min(4)).map(|c| self.matrix[r][c]).sum();
                (n > 0).then(|| near as f64 / n as f64)
            })
            .collect();
        rows.iter().sum::<f64>() / rows.len().max(1) as f64
    }

    fn within_share(&self) -> f64 {
        self.within as f64 / self.samples.max(1) as f64
    }

    fn agreement(&self, spots: usize) -> Agreement {
        let share = |x: usize| if self.samples == 0 { 0.0 } else { (x as f64 / self.samples as f64 * 1000.0).round() / 1000.0 };
        Agreement {
            samples: self.samples,
            same: share(self.same),
            within_one: share(self.within),
            balanced: (self.balanced() * 1000.0).round() / 1000.0,
            spots,
            off_best: self.matrix[1..].iter().flatten().sum(),
        }
    }

    fn report(&self, name: &str) {
        eprintln!(
            "{name}: {} Vergleiche, gleich {:.1} %, ±1 {:.1} %, ±1 ausgewogen {:.1} %",
            self.samples,
            100.0 * self.same as f64 / self.samples.max(1) as f64,
            100.0 * self.within_share(),
            100.0 * self.balanced()
        );
        eprintln!("  Mischung des Solvers (≥ {:.0} %) als Fehler oder grob: {} von {}", MIX_MIN_FREQUENCY * 100.0, self.mixed_wrong, self.mixed);
        eprintln!("  Referenz (Zeile) gegen Einschätzung (Spalte): beste/gut/ungenau/Fehler/grob");
        for row in &self.matrix {
            eprintln!("  {}", row.iter().map(|x| format!("{x:6}")).collect::<String>());
        }
        for (k, (n, w)) in &self.by_kind {
            eprintln!("  {k:12} {n:6} Vergleiche, ±1: {:.1} %", 100.0 * *w as f64 / (*n).max(1) as f64);
        }
    }
}

/// Which numbers a case is estimated with.
type TuningOf<'a> = &'a (dyn Fn(&Case) -> Tuning + Sync);

fn street_index(s: ringside_core::model::Street) -> usize {
    match s {
        ringside_core::model::Street::Turn => 1,
        ringside_core::model::Street::River => 2,
        _ => 0,
    }
}

fn street_of_cards(n: usize) -> ringside_core::model::Street {
    use ringside_core::model::Street;
    match n {
        0 => Street::Preflop,
        3 => Street::Flop,
        4 => Street::Turn,
        _ => Street::River,
    }
}

fn evaluate(cases: &[Case], t: TuningOf, strengths: &[Vec<f32>], test: Option<bool>) -> Tally {
    evaluate_where(cases, t, strengths, |c| test.is_none_or(|x| x == c.test))
}

fn evaluate_where(cases: &[Case], t: TuningOf, strengths: &[Vec<f32>], keep: impl Fn(&Case) -> bool + Sync) -> Tally {
    cases
        .par_iter()
        .filter(|c| keep(c))
        .map(|c| {
            let mine = graded(c, &t(c), strengths);
            let mut tally = Tally::default();
            for k in 0..c.options.len() {
                tally.add(c.options[k].key(), c.reference[k], mine[k]);
                tally.mix(c.freq[k], mine[k]);
            }
            tally
        })
        .reduce(Tally::default, Tally::merge)
}

fn prepared(c: &Case, t: &Tuning, strengths: &[Vec<f32>]) -> Prepared {
    match strengths.get(c.board) {
        Some(s) => prepare(&c.spot, &c.options, c.seed, t, s),
        None => {
            let s = strength(&c.spot.board, &mut ChaCha8Rng::seed_from_u64(c.seed ^ 0x5eed));
            prepare(&c.spot, &c.options, c.seed, t, &s)
        }
    }
}

fn tally_of(cases: &[&Case], preps: &[Prepared], t: &Tuning) -> Tally {
    cases
        .par_iter()
        .zip(preps)
        .map(|(c, p)| {
            let mine = grades(&settle(p, t), &c.options, c.pot, t);
            let mut tally = Tally::default();
            for a in 0..c.options.len() {
                tally.add(c.options[a].key(), c.reference[a], mine[a]);
                tally.mix(c.freq[a], mine[a]);
            }
            tally
        })
        .reduce(Tally::default, Tally::merge)
}

const SHRINKS: [f64; 8] = [0.6, 0.45, 0.35, 0.25, 0.18, 0.12, 0.08, 0.05];
const TIES: [f64; 5] = [0.5, 1.0, 2.0, 3.0, 5.0];
const LINE_MARGIN: [f64; 7] = [-1.0, -0.05, 0.0, 0.03, 0.06, 0.1, 0.15];
const LINE_PCT: [f64; 5] = [0.25, 1.5, 4.0, 10.0, 1000.0];
const LINE_TOP: [f64; 4] = [0.9, 0.95, 0.98, 1.01];
const STRENGTHS: [f64; 3] = [0.0, 0.5, 1.0];
const DEFENDS: [f64; 10] = [0.6, 0.8, 1.0, 1.3, 1.6, 2.2, 3.0, 4.0, 5.5, 7.5];

/// Agreement on the fitting half: within one step per reference grade averaged over
/// the grades, plus plain agreement within one step, less the share of the reference's
/// mixed moves called a mistake. Plain agreement alone would favour calling everything
/// fine, as most options are fine; the balanced share alone would trade many good moves
/// for a few mistakes found; a move the reference mixes is never a mistake.
fn score(t: &Tally) -> f64 {
    t.balanced() + t.within_share() - t.mixed_wrong as f64 / t.mixed.max(1) as f64
}

/// The tuning with the best [`score`] on the fitting half. What [`prepare`] does is
/// estimated once per strength and defence, the rest is searched coordinate by
/// coordinate on top.
fn fit(cases: &[&Case], strengths: &[Vec<f32>], base: &Tuning) -> Tuning {
    let fitting: Vec<&Case> = cases.iter().copied().filter(|c| !c.test && c.hand < FIT_HANDS).collect();
    eprintln!("  Anpassung an {} Fällen", fitting.len());
    let mut best = (*base, -1.0);
    for strength in STRENGTHS {
        for defend in DEFENDS {
            let mut t = Tuning { strength, defend, ..best.0 };
            let preps: Vec<Prepared> = fitting.par_iter().map(|c| prepared(c, &t, strengths)).collect();
            let mut here = score(&tally_of(&fitting, &preps, &t));
            for _ in 0..3 {
                let before = here;
                // The line cap acts only with all three numbers together.
                for &line_margin in &LINE_MARGIN {
                    for &line_pct in &LINE_PCT {
                        for &line_top in &LINE_TOP {
                            let x = Tuning { line_margin, line_pct, line_top, ..t };
                            let sc = score(&tally_of(&fitting, &preps, &x));
                            if sc > here {
                                (t, here) = (x, sc);
                            }
                        }
                    }
                }
                for shrink in SHRINKS {
                    let x = Tuning { shrink, ..t };
                    let sc = score(&tally_of(&fitting, &preps, &x));
                    if sc > here {
                        (t, here) = (x, sc);
                    }
                }
                for tie_pct in TIES {
                    let x = Tuning { tie_pct, ..t };
                    let sc = score(&tally_of(&fitting, &preps, &x));
                    if sc > here {
                        (t, here) = (x, sc);
                    }
                }
                if here <= before {
                    break;
                }
            }
            if here > best.1 {
                best = (t, here);
            }
        }
        eprintln!("  Anpassung: Stärke {strength} fertig, bisher {:?} mit {:.1} %", best.0, best.1 * 50.0);
    }
    let t = best.0;
    let edge = |name: &str, v: f64, grid: &[f64]| {
        if v == grid[0] || v == grid[grid.len() - 1] {
            eprintln!("  Achtung: {name} = {v} liegt am Rand des Suchgitters");
        }
    };
    edge("shrink", t.shrink, &SHRINKS);
    edge("defend", t.defend, &DEFENDS);
    edge("line_margin", t.line_margin, &LINE_MARGIN);
    t
}

fn graded(c: &Case, t: &Tuning, strengths: &[Vec<f32>]) -> Vec<Grade> {
    grades(&settle(&prepared(c, t, strengths), t), &c.options, c.pot, t)
}

fn hands_in(dir: &str) -> Vec<Hand> {
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            stack.extend(std::fs::read_dir(&p).unwrap().flatten().map(|e| e.path()));
        } else if p.extension().is_some_and(|x| x == "txt") {
            let text = String::from_utf8_lossy(&std::fs::read(&p).unwrap()).into_owned();
            out.extend(split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok()));
        }
    }
    out
}

fn tree_key(kind: Tree) -> &'static str {
    match kind {
        Tree::Fold => "fold",
        Tree::Check => "check",
        Tree::Call => "call",
        Tree::Bet => "bet",
        Tree::Raise => "raise",
        Tree::AllIn => "all_in",
    }
}

fn signal_of(kind: Tree) -> Option<Signal> {
    match kind {
        Tree::Bet | Tree::AllIn => Some(Signal::Bet),
        Tree::Raise => Some(Signal::Raise),
        Tree::Call => Some(Signal::Call),
        Tree::Check => Some(Signal::PostCheck),
        Tree::Fold => None,
    }
}

fn find(sol: &mut Solution, path: &[Step], kinds: &[Tree]) -> Option<usize> {
    let n = sol.node(path).ok()?;
    (n.kind == NodeKind::Action).then_some(())?;
    n.actions.iter().position(|a| kinds.contains(&a.kind))
}

fn is_action(sol: &mut Solution, path: &[Step]) -> bool {
    sol.node(path).is_ok_and(|n| n.kind == NodeKind::Action && n.actions.len() >= 2)
}

fn with(path: &[Step], a: usize) -> Vec<Step> {
    let mut p = path.to_vec();
    p.push(Step::Action(a));
    p
}

/// The decision nodes worth checking along one random line: on every street the first
/// decision, the answer to a bet and to a raise, the in-position decision after a check
/// and the answer to its bet; to the next street by a random one of check–check, bet–call
/// and check–bet–call.
fn paths(sol: &mut Solution, rng: &mut ChaCha8Rng) -> Vec<Vec<Step>> {
    let mut out = Vec::new();
    let mut start: Vec<Step> = Vec::new();
    while is_action(sol, &start) {
        out.push(start.clone());
        let mut next: Vec<Vec<Step>> = Vec::new();
        if let Some(b) = find(sol, &start, &[Tree::Bet]) {
            let bet = with(&start, b);
            if is_action(sol, &bet) {
                out.push(bet.clone());
                if let Some(r) = find(sol, &bet, &[Tree::Raise, Tree::AllIn]) {
                    let raised = with(&bet, r);
                    if is_action(sol, &raised) {
                        out.push(raised);
                    }
                }
                if let Some(c) = find(sol, &bet, &[Tree::Call]) {
                    next.push(with(&bet, c));
                }
            }
        }
        if let Some(c) = find(sol, &start, &[Tree::Check]) {
            let checked = with(&start, c);
            if is_action(sol, &checked) {
                out.push(checked.clone());
                if let Some(b) = find(sol, &checked, &[Tree::Bet]) {
                    let bet = with(&checked, b);
                    if is_action(sol, &bet) {
                        out.push(bet.clone());
                        if let Some(c) = find(sol, &bet, &[Tree::Call]) {
                            next.push(with(&bet, c));
                        }
                    }
                }
                if let Some(x) = find(sol, &checked, &[Tree::Check]) {
                    next.push(with(&checked, x));
                }
            }
        }
        if next.is_empty() {
            break;
        }
        let mut p = next.swap_remove(rng.random_range(0..next.len()));
        let Ok(chance) = sol.node(&p) else { break };
        if chance.kind != NodeKind::Chance || chance.cards.is_empty() {
            break;
        }
        p.push(Step::Card(chance.cards[rng.random_range(0..chance.cards.len())].clone()));
        start = p;
    }
    out
}

/// The node of `path` with a sample of the acting player's hands.
fn node_case(sol: &mut Solution, path: &[Step], ranges: [&str; 2], preflop_aggressor: Option<usize>, rng: &mut ChaCha8Rng) -> Option<SolvedNode> {
    let node = sol.node(path).ok()?;
    let me = if node.player == Some(Player::Oop) { 0 } else { 1 };
    let other = 1 - me;
    let mut line = Vec::new();
    let mut aggressor = preflop_aggressor;
    let mut walk: Vec<Step> = Vec::new();
    for step in path {
        if let Step::Action(a) = step
            && let Ok(at) = sol.node(&walk)
        {
            let actor = if at.player == Some(Player::Oop) { 0 } else { 1 };
            let kind = at.actions[*a].kind;
            if matches!(kind, Tree::Bet | Tree::Raise | Tree::AllIn) {
                aggressor = Some(actor);
            }
            if actor == other
                && let Some(sig) = signal_of(kind)
            {
                line.push((at.board.len(), sig));
            }
        }
        walk.push(step.clone());
    }
    let mut hands: Vec<_> = node.hands.iter().filter(|h| h.weight > 0.05 && h.evs.len() == node.actions.len()).collect();
    hands.shuffle(rng);
    let mut picked: Vec<SolvedHand> = hands
        .into_iter()
        .take(16)
        .map(|h| SolvedHand { combo: h.combo.clone(), strategy: h.strategy.clone(), evs: h.evs.clone(), equity: h.equity })
        .collect();
    picked.sort_by(|a, b| a.combo.cmp(&b.combo));
    Some(SolvedNode {
        spot: 0,
        label: String::new(),
        stack: 0.0,
        pot_type: String::new(),
        board: node.board.clone(),
        me,
        pot: node.pot,
        street_bets: node.street_bets,
        stacks: node.stacks,
        aggressor: aggressor == Some(me),
        opp_range: ranges[other].to_string(),
        line,
        options: node.actions.iter().map(|a| (tree_key(a.kind).to_string(), a.amount)).collect(),
        hands: picked,
    })
}

fn settings(threads: usize) -> Settings {
    Settings { target_pct: 1.0, max_iterations: 300, threads }
}

/// `f` of every item on `parallel` threads of their own, in order. Not on rayon: a
/// worker waiting for the solver's pool would start more solves meanwhile.
fn in_parallel<T: Sync, R: Send>(items: &[T], parallel: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let mut out: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..parallel.max(1))
            .map(|_| {
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn_scoped(scope, || {
                        let mut mine = Vec::new();
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            let Some(item) = items.get(i) else { break };
                            mine.push((i, f(item)));
                        }
                        mine
                    })
                    .expect("thread")
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().expect("worker")).collect()
    });
    out.sort_by_key(|x| x.0);
    out.into_iter().map(|x| x.1).collect()
}

/// A heads-up flop of a model table to solve.
struct Job {
    spot: ringside_solver::Spot,
    aggressor: Option<usize>,
    label: String,
    stack: f64,
    pot_type: String,
    seed: u64,
}

/// Random heads-up flops of the model's tables: every family, ante, table size and
/// stack in random order, two random lines of each.
fn jobs(count: usize, rng: &mut ChaCha8Rng) -> Vec<Job> {
    let mut tables = Vec::new();
    for family in Family::ALL {
        for ante in Ante::ALL {
            for &players in family.players() {
                for stack in STACKS {
                    tables.push(family.params(ante, players, stack));
                }
            }
        }
    }
    let mut out = Vec::new();
    while out.len() < count {
        tables.shuffle(rng);
        for params in &tables {
            let Some(sol) = preflop_table::lookup(params) else { continue };
            let mut scenarios = ringside_play::scenario::scenarios(&sol, params);
            scenarios.shuffle(rng);
            for sc in scenarios.into_iter().take(2) {
                let flop = ringside_play::pool::random_flop(rng);
                let text: String = flop.iter().map(|c| c.to_string()).collect();
                let raiser = sc
                    .path
                    .iter()
                    .rev()
                    .find(|&&(v, a)| matches!(sol.nodes[v].actions[a].kind, PreKind::Raise | PreKind::AllIn))
                    .map(|&(v, _)| sol.nodes[v].seat);
                let pot_type = format!("{:?}", sc.pot_type);
                let family = if params.open_limps > 0.0 { "Limps" } else { "Plain" };
                out.push(Job {
                    spot: sc.spot(&text),
                    aggressor: raiser.and_then(|s| if s == sc.oop { Some(0) } else if s == sc.ip { Some(1) } else { None }),
                    label: format!("{} BB, {} Spieler, {pot_type}, {family} {:?}, SPR {:.1}", params.stack_bb, params.players, ante_of(params), sc.stack / sc.pot),
                    stack: sc.stack,
                    pot_type,
                    seed: rng.random(),
                });
                if sc.pot_type == PotType::Limped && rng.random_bool(0.5) {
                    break;
                }
            }
        }
    }
    out.truncate(count);
    out
}

/// Solves random heads-up flops of the model's tables for `minutes`, `parallel` at once.
fn solve_model_spots(minutes: f64, threads: usize, parallel: usize, rng: &mut ChaCha8Rng, store: &mut Store) {
    let started = Instant::now();
    let jobs = jobs(4000, rng);
    eprintln!("{} Spots vorbereitet in {:.0} s", jobs.len(), started.elapsed().as_secs_f64());
    let done = AtomicUsize::new(0);
    let solved = in_parallel(&jobs, parallel, |job| {
        if started.elapsed().as_secs_f64() > minutes * 60.0 {
            return None;
        }
        let mut rng = ChaCha8Rng::seed_from_u64(job.seed);
        let mut sol = solve(&job.spot, &settings(threads), 8 << 30, |_| {}, || false).ok()?;
        let mut nodes = Vec::new();
        for p in paths(&mut sol, &mut rng) {
            if let Some(mut n) = node_case(&mut sol, &p, [&job.spot.oop_range, &job.spot.ip_range], job.aggressor, &mut rng) {
                n.label = job.label.clone();
                n.stack = job.stack;
                n.pot_type = job.pot_type.clone();
                nodes.push(n);
            }
        }
        let k = done.fetch_add(1, Ordering::Relaxed) + 1;
        eprintln!("Spot {k}: {}, {}: {} Knoten, {:.0} s", job.label, job.spot.board, nodes.len(), started.elapsed().as_secs_f64());
        Some(nodes)
    });
    for nodes in solved.into_iter().flatten() {
        for mut n in nodes {
            n.spot = store.spots;
            store.nodes.push(n);
        }
        store.spots += 1;
    }
}

fn ante_of(p: &ringside_engine::preflop::Params) -> Ante {
    Ante::ALL.into_iter().find(|a| a.amounts() == (p.ante_bb, p.bb_ante_bb)).unwrap_or(Ante::None)
}

/// The postflop cases of the stored nodes and the strength of each board.
fn postflop_cases(store: &Store, rng: &mut ChaCha8Rng) -> (Vec<Case>, Vec<Vec<f32>>) {
    let mut boards: HashMap<Vec<String>, usize> = HashMap::new();
    for n in &store.nodes {
        for len in [3, 4, 5].into_iter().filter(|&l| l <= n.board.len()) {
            let k = boards.len();
            boards.entry(n.board[..len].to_vec()).or_insert(k);
        }
    }
    let mut keys: Vec<(&Vec<String>, &usize)> = boards.iter().collect();
    keys.sort_by_key(|x| *x.1);
    let strengths: Vec<Vec<f32>> = keys
        .par_iter()
        .map(|(b, i)| {
            let cards: Vec<Card> = b.iter().filter_map(|c| c.parse().ok()).collect();
            strength(&cards, &mut ChaCha8Rng::seed_from_u64(**i as u64))
        })
        .collect();
    let mut cases = Vec::new();
    for (ni, n) in store.nodes.iter().enumerate() {
        let opp = n.opponent_range(|b| strengths[boards[&b.iter().map(|c| c.to_string()).collect::<Vec<_>>()]].clone());
        let options = n.options();
        for (hi, h) in n.hands.iter().enumerate() {
            let Ok(combo) = h.combo.parse::<Combo>() else { continue };
            let reference = (0..options.len()).map(|k| judge(&h.strategy, Some(&h.evs), k, n.pot).grade).collect();
            cases.push(Case {
                spot: n.spot(combo, &opp),
                options: options.clone(),
                reference,
                freq: h.strategy.clone(),
                evs: h.evs.clone(),
                pot: n.pot,
                test: n.spot % 2 == 1,
                seed: rng.random(),
                board: boards[&n.board],
                node: ni,
                hand: hi,
                true_equity: h.equity,
            });
        }
    }
    (cases, strengths)
}

fn preflop_cases(hands: &[Hand], rng: &mut ChaCha8Rng) -> (Vec<Case>, usize) {
    let mut pre = Vec::new();
    let mut spots = 0;
    for hand in hands {
        let Some(hero) = hand.hero.clone() else { continue };
        let analysis = analyze_hand(hand);
        for d in analysis.decisions.iter().filter(|d| d.source == Some(Source::Model)) {
            let Ok(spot) = replay::spot_at(hand, &hero, d.action_index, None) else { continue };
            let options: Vec<Opt> = d
                .strategy
                .iter()
                .map(|c| match c.kind.as_str() {
                    "fold" => Opt::Fold,
                    "check" => Opt::Check,
                    "call" | "limp" => Opt::Call,
                    "all_in" => Opt::AllIn,
                    _ => Opt::Raise(c.label.split_whitespace().nth(2).and_then(|x| x.replace(',', ".").parse().ok()).unwrap_or(2.5)),
                })
                .collect();
            let pot = d.pot_bb.unwrap_or(spot.pot);
            let freqs: Vec<f64> = d.strategy.iter().map(|c| c.freq).collect();
            let evs: Vec<f64> = d.strategy.iter().map(|c| c.ev_bb.unwrap_or(0.0)).collect();
            let reference = (0..options.len()).map(|k| judge(&freqs, Some(&evs), k, pot).grade).collect();
            pre.push(Case {
                spot,
                options,
                reference,
                freq: freqs,
                evs,
                pot,
                test: hand.id % 2 == 1,
                seed: rng.random(),
                board: usize::MAX,
                node: spots,
                hand: 0,
                true_equity: f64::NAN,
            });
            spots += 1;
        }
    }
    (pre, spots)
}

fn solve_real(hands: &[Hand], limit: usize, threads: usize, parallel: usize, store: &mut Store) {
    let started = Instant::now();
    let mut jobs: Vec<(f64, u64, PostflopJob)> = hands
        .iter()
        .filter_map(|hand| {
            let (pot, json) = analyze_hand(hand).job?;
            Some((pot, hand.id, serde_json::from_str::<PostflopJob>(&json).expect("job")))
        })
        .collect();
    jobs.sort_by(|a, b| b.0.total_cmp(&a.0));
    let limit = if limit == 0 { jobs.len() } else { limit };
    jobs.truncate(limit);
    store.real = in_parallel(&jobs, parallel, |(_, id, job)| match solve_job(job, threads, 8 << 30, |_| {}, || false) {
        Ok(solved) => {
            eprintln!("echte Hand gelöst: #{id}, {:.0} s", started.elapsed().as_secs_f64());
            Some((*id, solved))
        }
        Err(e) => {
            eprintln!("echte Hand #{id} nicht gelöst: {e:?}");
            None
        }
    })
    .into_iter()
    .flatten()
    .collect();
}

fn grade_of(r: Rating) -> Grade {
    match r {
        Rating::Best => Grade::Best,
        Rating::Mix => Grade::Mix,
        Rating::Good => Grade::Good,
        Rating::Inaccuracy => Grade::Inaccurate,
        Rating::Mistake => Grade::Mistake,
        Rating::Blunder => Grade::Blunder,
    }
}

/// One row per option for a look at the data outside: reference, estimate and features.
fn write_csv(path: &str, cases: &[Case], tuning_of: TuningOf, strengths: &[Vec<f32>], store: &Store) {
    let rows: Vec<String> = cases
        .par_iter()
        .flat_map_iter(|c| {
            let t = &tuning_of(c);
            let prep = prepared(c, t, strengths);
            let est = settle(&prep, &Tuning { shrink: 1.0, line_margin: -1.0, ..*t });
            let mine = grades(&settle(&prep, t), &c.options, c.pot, t);
            let n = &store.nodes[c.node];
            let call = c.spot.to_call();
            let spr = (c.spot.hero_behind - call).min(c.spot.opponents[0].behind) / (c.pot + call).max(0.01);
            let top_ref = c.evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let top_est = est.evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (0..c.options.len())
                .map(|k| {
                    format!(
                        "{},{},{},{},{},{:.2},{},{},{},{:.2},{:.3},{},{:.4},{:.3},{},{},{:.4},{:.3},{:.3},{},{}",
                        n.spot,
                        c.node,
                        u8::from(c.test),
                        c.spot.board.len(),
                        n.pot_type,
                        spr,
                        n.me,
                        u8::from(n.aggressor),
                        c.options[k].key(),
                        c.pot,
                        call,
                        c.options.len(),
                        c.freq[k],
                        (top_ref - c.evs[k]) / c.pot * 100.0,
                        level(c.reference[k]),
                        level(mine[k]),
                        (top_est - est.evs[k]) / c.pot * 100.0,
                        est.equity,
                        c.true_equity,
                        est.pot_odds.unwrap_or(0.0),
                        n.stack,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let head = "spot,node,test,cards,pot_type,spr,me,aggr,opt,pot,call,nopts,freq,ref_loss,ref_lvl,est_lvl,est_loss,est_eq,true_eq,odds,stack";
    std::fs::write(path, format!("{head}\n{}\n", rows.join("\n"))).expect("csv");
    eprintln!("CSV: {path} ({} Zeilen)", rows.len());
}

fn main() {
    let mut hands_dir: Option<String> = None;
    let mut minutes = 30.0;
    let mut threads = 3usize;
    let mut parallel = 6usize;
    let mut write = false;
    let mut real_limit = 0usize;
    let mut cases_path = "/tmp/ringside-calibration-cases.json".to_string();
    let mut csv: Option<String> = None;
    let mut given: Option<Tuning> = None;
    let mut fixture: Option<(String, usize)> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--hands" => hands_dir = args.next(),
            "--minutes" => minutes = args.next().and_then(|v| v.parse().ok()).expect("--minutes M"),
            "--threads" => threads = args.next().and_then(|v| v.parse().ok()).expect("--threads N"),
            "--parallel" => parallel = args.next().and_then(|v| v.parse().ok()).expect("--parallel N"),
            "--write" => write = true,
            "--real" => real_limit = args.next().and_then(|v| v.parse().ok()).expect("--real N"),
            "--cases" => cases_path = args.next().expect("--cases FILE"),
            "--csv" => csv = args.next(),
            "--fixture" => fixture = args.next().zip(args.next().and_then(|n| n.parse().ok())),
            "--postflop-tuning" => given = args.next().map(|j| serde_json::from_str(&j).expect("tuning JSON")),
            _ => panic!("unknown argument {a}"),
        }
    }
    ringside_engine::eval::init();
    let mut rng = ChaCha8Rng::seed_from_u64(2026);
    let started = Instant::now();
    let hands = hands_dir.as_deref().map(hands_in).unwrap_or_default();

    let store: Store = match std::fs::read(&cases_path) {
        Ok(bytes) => serde_json::from_slice(&bytes).expect("cases file"),
        Err(_) => {
            let mut store = Store::default();
            solve_real(&hands, real_limit, threads, parallel, &mut store);
            solve_model_spots(minutes, threads, parallel, &mut rng, &mut store);
            store.solve_seconds = started.elapsed().as_secs_f64();
            std::fs::write(&cases_path, serde_json::to_vec(&store).unwrap()).expect("write cases");
            store
        }
    };
    eprintln!(
        "Gelöst: {} Spots, {} Knoten, {} echte Hände in {:.0} s Rechenzeit",
        store.spots,
        store.nodes.len(),
        store.real.len(),
        store.solve_seconds
    );

    if let Some((path, every)) = &fixture {
        // Every n-th node of the test half, for the estimate's tests.
        let nodes: Vec<&SolvedNode> = store.nodes.iter().filter(|n| n.spot % 2 == 1).step_by(*every).collect();
        std::fs::write(path, serde_json::to_string(&nodes).unwrap() + "\n").expect("fixture");
        eprintln!("Fixture: {path} ({} Knoten)", nodes.len());
    }
    let (post, strengths) = postflop_cases(&store, &mut rng);
    let (pre, pre_spots) = preflop_cases(&hands, &mut rng);
    eprintln!("Postflop: {} Fälle aus {} Spots; Preflop: {} Fälle", post.len(), store.spots, pre.len());
    // Without cases of a kind the stored numbers stay; after the flop each street is
    // fitted on its own.
    let stored = ringside_analysis::calibration::stored();
    let street_tunings: [Tuning; 3] = [3, 4, 5].map(|n| {
        let k = n - 3;
        let mine: Vec<&Case> = post.iter().filter(|c| c.spot.board.len() == n).collect();
        match given {
            Some(t) => t,
            None if mine.is_empty() => stored.street_tunings[k],
            None => {
                eprintln!("Anpassung mit {n} Karten:");
                fit(&mine, &strengths, &stored.street_tunings[k])
            }
        }
    });
    let pre_refs: Vec<&Case> = pre.iter().collect();
    let preflop_tuning = if pre.is_empty() { stored.preflop_tuning } else { fit(&pre_refs, &strengths, &stored.preflop_tuning) };
    let tuning_of = |c: &Case| if c.spot.board.is_empty() { preflop_tuning } else { street_tunings[c.spot.board.len() - 3] };
    if let Some(path) = &csv {
        write_csv(path, &post, &tuning_of, &strengths, &store);
    }
    evaluate(&post, &|c: &Case| stored.tuning(street_of_cards(c.spot.board.len())), &strengths, Some(true)).report("Postflop mit gespeicherten Zahlen (Testhälfte)");
    let post_test = evaluate(&post, &tuning_of, &strengths, Some(true));
    post_test.report("Postflop angepasst (Testhälfte)");
    let pre_test = evaluate(&pre, &tuning_of, &strengths, Some(true));
    pre_test.report("Preflop angepasst (Testhälfte)");

    // The decisions really played, rated by the analysis both ways.
    let by_id: HashMap<u64, &Hand> = hands.iter().map(|h| (h.id, h)).collect();
    let mut real = Tally::default();
    let mut by_street: [Tally; 3] = Default::default();
    let mut real_hands = 0;
    for (id, solved) in &store.real {
        let Some(hand) = by_id.get(id) else { continue };
        real_hands += 1;
        let hero = hand.hero.as_deref().unwrap_or_default();
        let ranges = replay::flop_ranges(hand, hero);
        for d in solved.iter().filter(|d| d.rating.is_some()) {
            let t = street_tunings[street_index(d.street)];
            if let Ok(e) = replay::rate_with(hand, hero, d.action_index, d.spot, ranges.as_ref(), "", &t) {
                let (a, b) = (grade_of(d.rating.unwrap()), grade_of(e.rating.unwrap_or(Rating::Blunder)));
                let kind = e.strategy.iter().find(|c| c.played).map_or("?", |c| c.kind.as_str());
                real.add(kind, a, b);
                real.mix(d.played_freq.unwrap_or(0.0), b);
                by_street[street_index(d.street)].add(kind, a, b);
                if std::env::var_os("CALIBRATE_SHOW").is_some() {
                    let fmt = |x: &Decision| x.strategy.iter().map(|c| format!("{} {:.0}% {:+.2}", c.label, c.freq * 100.0, c.ev_bb.unwrap_or(0.0))).collect::<Vec<_>>().join(" | ");
                    eprintln!(
                        "#{} {:?} {} {} gespielt {}: Solver {:?} [{}] / Einschätzung {:?} [{}] Pot {:.1}",
                        hand.id,
                        d.street,
                        hand.hero_cards.join(""),
                        hand.board.join(""),
                        d.played,
                        d.rating,
                        fmt(d),
                        e.rating,
                        fmt(&e),
                        e.pot_bb.unwrap_or(0.0)
                    );
                }
            }
        }
    }
    real.report("Echte Entscheidungen");
    for (street, t) in ["Flop", "Turn", "River"].iter().zip(&by_street) {
        eprintln!("  echt {street}: {} Vergleiche, ±1 {:.1} %, ausgewogen {:.1} %", t.samples, 100.0 * t.within_share(), 100.0 * t.balanced());
    }

    let post_streets = [3, 4, 5].map(|n| {
        let t = evaluate_where(&post, &tuning_of, &strengths, |c| c.test && c.spot.board.len() == n);
        t.report(&format!("Postflop mit {n} Karten (Testhälfte)"));
        let spots = store.nodes.iter().filter(|x| x.spot % 2 == 1 && x.board.len() == n).map(|x| x.spot).collect::<std::collections::BTreeSet<_>>().len();
        t.agreement(spots)
    });
    let real_streets = by_street.each_ref().map(|t| t.agreement(real_hands));
    let c = Calibration {
        postflop_streets: post_streets,
        real_streets,
        preflop_tuning,
        street_tunings,
        preflop: pre_test.agreement(pre_spots / 2),
        postflop: post_test.agreement(store.spots / 2),
        real: real.agreement(real_hands),
    };
    println!("{}", serde_json::to_string_pretty(&c).unwrap());
    println!("in {:.0} s", started.elapsed().as_secs_f64());
    if write {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/estimate_calibration.json");
        std::fs::write(path, serde_json::to_string_pretty(&c).unwrap() + "\n").expect("write calibration");
        println!("geschrieben: {path}");
    }
}
