//! Fits the estimate's free numbers and measures how well its grades agree with the
//! solver (after the flop) and the preflop model (before it). With `--write` the result
//! goes to `data/estimate_calibration.json`, which the analysis shows as the estimate's
//! reliability:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin analysis-calibrate -- \
//!     [--hands DIR] [--real N] [--minutes M] [--threads T] [--parallel P] [--cases FILE] [--csv FILE] [--postflop-tuning JSON] [--fit STREETS] [--fixture FILE EVERY] [--write]
//! ```
//!
//! `--fit preflop,flop,turn,river` (the default) names the streets whose numbers are
//! fitted; the others keep the stored ones, so one street is refitted in a fraction of
//! the time.
//!
//! - Postflop: heads-up flops of the preflop model's tables (every family, ante, table
//!   size and stack, random lines and boards) solved to 1 % of the pot for `--minutes`
//!   (default 30). Along a random line through flop, turn and river the decisions of
//!   both players are checked (first to act, facing a bet, facing a raise); every option
//!   of a sample of the acting player's hands is graded by the solver and by the estimate,
//!   the opponent's range narrowed along the line by the curves of the moves after the
//!   flop. The curves come from the fitting half: first how often the solver takes each
//!   move with each strength, then moved bin by bin towards the solver's equity of the
//!   hands it holds.
//! - Mixes: a move the solver plays at least 10 % of the time counts as its mix only
//!   within the accuracy of the solve; the share of those the estimate calls a mistake
//!   is reported and weighs in the fit.
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
use ringside_analysis::calibration::{Agreement, Calibration, SolvedHand, SolvedNode, narrowed};
use ringside_analysis::estimate::{BIN_EDGES, BINS, COMBOS, Curve, MoveKind, Opt, Prepared, Range, SIZES, Signal, Spot, Strength, Tuning, grades, prepare, settle, size_pct, strength, top_share};
use ringside_analysis::{Decision, PostflopJob, Rating, Source, analyze_hand, replay, solve_job};
use ringside_core::model::Hand;
use ringside_core::{parse_hand, split_hands};
use ringside_engine::preflop::Kind as PreKind;
use ringside_engine::preflop_table::{self, Ante, Family, STACKS};
use ringside_engine::{Card, CardSet, Combo};
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
    true_equity: f64,
}

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
    /// Options the reference plays at least as often as a mix and within the solve's
    /// accuracy of its best one, and how many of them the estimate calls a mistake or worse.
    mixed: usize,
    mixed_wrong: usize,
    /// The same for every option played as often as a mix, whatever it costs.
    played: usize,
    played_wrong: usize,
    /// Sum of |estimated equity − the solver's| and the hands it is summed over.
    equity_error: f64,
    equity_hands: usize,
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

    /// A move played `freq` of the time, `loss_pct` of the pot below the best one by the
    /// reference's EVs. A solve to [`SOLVE_PCT`] can't tell moves closer than that apart,
    /// so only those are a mix it is indifferent between; a move it plays although its
    /// own EVs say it costs more is what the solve left unfinished (hands it rarely
    /// reaches converge last), not a mix.
    fn mix(&mut self, freq: f64, loss_pct: f64, mine: Grade) {
        if freq >= MIX_MIN_FREQUENCY {
            let wrong = usize::from(mine >= Grade::Mistake);
            self.played += 1;
            self.played_wrong += wrong;
            if loss_pct <= SOLVE_PCT {
                self.mixed += 1;
                self.mixed_wrong += wrong;
            }
        }
    }

    fn merge(mut self, o: Tally) -> Tally {
        self.mixed += o.mixed;
        self.mixed_wrong += o.mixed_wrong;
        self.played += o.played;
        self.played_wrong += o.played_wrong;
        self.equity_error += o.equity_error;
        self.equity_hands += o.equity_hands;
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
        let pct = |a: usize, b: usize| 100.0 * a as f64 / b.max(1) as f64;
        eprintln!(
            "  Mischung des Solvers (≥ {:.0} %, EV-Abstand ≤ {SOLVE_PCT} % Pot) als Fehler oder grob: {} von {} ({:.2} %)",
            MIX_MIN_FREQUENCY * 100.0,
            self.mixed_wrong,
            self.mixed,
            pct(self.mixed_wrong, self.mixed)
        );
        eprintln!("  jeder Zug ≥ {:.0} % gespielt als Fehler oder grob: {} von {} ({:.2} %)", MIX_MIN_FREQUENCY * 100.0, self.played_wrong, self.played, pct(self.played_wrong, self.played));
        if self.equity_hands > 0 {
            eprintln!("  Equity-Abweichung zum Solver im Mittel: {:.3} ({} Hände)", self.equity_error / self.equity_hands as f64, self.equity_hands);
        }
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

/// What every estimate of a run shares: the strength of each board and the curves.
#[derive(Clone, Copy)]
struct Shared<'a> {
    strengths: &'a [Strength],
    curves: &'a [Curve],
}

fn evaluate(cases: &[Case], t: TuningOf, shared: Shared, test: Option<bool>) -> Tally {
    evaluate_where(cases, t, shared, |c| test.is_none_or(|x| x == c.test))
}

fn evaluate_where(cases: &[Case], t: TuningOf, shared: Shared, keep: impl Fn(&Case) -> bool + Sync) -> Tally {
    cases
        .par_iter()
        .filter(|c| keep(c))
        .map(|c| {
            let t = t(c);
            let est = settle(&prepared(c, &t, shared), &t);
            let mut tally = compare(c, &grades(&est, &c.options, c.pot, &t));
            if c.true_equity.is_finite() {
                tally.equity_error += (est.equity - c.true_equity).abs();
                tally.equity_hands += 1;
            }
            tally
        })
        .reduce(Tally::default, Tally::merge)
}

/// The estimate's grades `mine` of a case against the reference.
fn compare(c: &Case, mine: &[Grade]) -> Tally {
    let top = c.evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut tally = Tally::default();
    for k in 0..c.options.len() {
        tally.add(c.options[k].key(), c.reference[k], mine[k]);
        tally.mix(c.freq[k], (top - c.evs[k]) / c.pot.max(1e-9) * 100.0, mine[k]);
    }
    tally
}

fn prepared(c: &Case, t: &Tuning, shared: Shared) -> Prepared {
    match shared.strengths.get(c.board) {
        Some(s) => prepare(&c.spot, &c.options, c.seed, t, s, shared.curves),
        None => {
            let s = strength(&c.spot.board, &mut ChaCha8Rng::seed_from_u64(c.seed ^ 0x5eed));
            prepare(&c.spot, &c.options, c.seed, t, &s, shared.curves)
        }
    }
}

fn tally_of(cases: &[&Case], preps: &[Prepared], t: &Tuning) -> Tally {
    cases
        .par_iter()
        .zip(preps)
        .map(|(c, p)| compare(c, &grades(&settle(p, t), &c.options, c.pot, t)))
        .reduce(Tally::default, Tally::merge)
}

const SHRINKS: [f64; 11] = [1.0, 0.8, 0.6, 0.45, 0.35, 0.25, 0.18, 0.12, 0.08, 0.05, 0.03];
const TIES: [f64; 8] = [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0];
const LINE_MARGIN: [f64; 11] = [-1.0, -0.1, -0.05, 0.0, 0.03, 0.06, 0.1, 0.15, 0.2, 0.25, 0.3];
const LINE_PCT: [f64; 7] = [0.0, 0.25, 1.5, 4.0, 10.0, 25.0, 1000.0];
const LINE_TOP: [f64; 8] = [0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 0.98, 1.01];
const STRENGTHS: [f64; 5] = [0.0, 0.5, 1.0, 1.5, 2.0];
const DEFENDS: [f64; 12] = [0.4, 0.6, 0.8, 1.0, 1.3, 1.6, 2.2, 3.0, 4.0, 5.5, 7.5, 10.0];
const BEHINDS: [f64; 8] = [1.0, 0.5, 0.3, 0.2, 0.12, 0.07, 0.03, 0.0];

/// Every free number of `t` with its search grid and which of the grid's ends (low, high)
/// are limits of the number itself rather than of the search: no shrink, no tie, the line
/// cap off, all or none of a range, realization not falling with equity. A fit may end
/// there; anywhere else at an end the grid is too narrow. The line numbers count only
/// while the cap acts.
fn grids(t: &Tuning) -> Vec<(&'static str, f64, &'static [f64], [bool; 2])> {
    let mut g: Vec<(&'static str, f64, &'static [f64], [bool; 2])> = vec![
        ("shrink", t.shrink, &SHRINKS, [false, true]),
        ("strength", t.strength, &STRENGTHS, [true, false]),
        ("defend", t.defend, &DEFENDS, [false, false]),
        ("tie_pct", t.tie_pct, &TIES, [true, false]),
        ("behind", t.behind, &BEHINDS, [true, true]),
    ];
    if t.line_margin > -1.0 && t.line_pct < 1000.0 {
        g.push(("line_margin", t.line_margin, &LINE_MARGIN, [true, false]));
        g.push(("line_pct", t.line_pct, &LINE_PCT, [true, true]));
        g.push(("line_top", t.line_top, &LINE_TOP, [false, true]));
    }
    g
}

/// The numbers of `t` at an end of their grid that is no limit of the number.
fn at_edge(t: &Tuning) -> Vec<String> {
    grids(t)
        .into_iter()
        .filter_map(|(name, v, grid, limit)| {
            let low = grid.iter().copied().fold(f64::INFINITY, f64::min);
            let high = grid.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            ((v <= low && !limit[0]) || (v >= high && !limit[1])).then(|| format!("{name} = {v}"))
        })
        .collect()
}

/// How much more a mix of the reference called a mistake weighs than agreement: telling
/// a player that a move the solver plays as well is a mistake costs more trust than a
/// grade one step off.
const MIX_WEIGHT: f64 = 3.0;

/// Agreement on the fitting half: within one step per reference grade averaged over the
/// grades, plus plain agreement within one step, less [`MIX_WEIGHT`] times the share of
/// the reference's mixes called a mistake. Plain agreement alone would favour calling
/// everything fine, as most options are fine; the balanced share alone would trade many
/// good moves for a few mistakes found.
fn score(t: &Tally) -> f64 {
    t.balanced() + t.within_share() - MIX_WEIGHT * t.mixed_wrong as f64 / t.mixed.max(1) as f64
}

/// The tuning with the best [`score`] on the fitting half. What [`prepare`] does is
/// estimated once per strength and defence, the rest is searched coordinate by
/// coordinate on top.
fn fit(cases: &[&Case], shared: Shared, base: &Tuning) -> Tuning {
    let fitting: Vec<&Case> = cases.iter().copied().filter(|c| !c.test).collect();
    eprintln!("  Anpassung an {} Fällen", fitting.len());
    let mut best = (*base, -1.0);
    for strength in STRENGTHS {
        for defend in DEFENDS {
            let mut t = Tuning { strength, defend, ..best.0 };
            let preps: Vec<Prepared> = fitting.par_iter().map(|c| prepared(c, &t, shared)).collect();
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
    for e in at_edge(&t) {
        eprintln!("  Achtung: {e} liegt am Rand des Suchgitters");
    }
    t
}

/// [`Tuning::behind`] with the best [`score`] on the fitting half, the rest as `base`. Only
/// before the flop: the solved spots after it are heads-up, nobody is behind there.
fn fit_behind(cases: &[&Case], shared: Shared, base: &Tuning) -> Tuning {
    let fitting: Vec<&Case> = cases.iter().copied().filter(|c| !c.test).collect();
    let mut best = (*base, f64::NEG_INFINITY);
    for behind in BEHINDS {
        let t = Tuning { behind, ..*base };
        let preps: Vec<Prepared> = fitting.par_iter().map(|c| prepared(c, &t, shared)).collect();
        let sc = score(&tally_of(&fitting, &preps, &t));
        eprintln!("  Anpassung: wer noch antworten muss, geht mit {behind} seiner Range weiter: {:.1} %", sc * 50.0);
        if sc > best.1 {
            best = (t, sc);
        }
    }
    best.0
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

/// What a move of `me` (0 out of position) says, `kind` as [`tree_key`] names it, at a
/// node with `pot` (bets in front included) and `bets` in front; `to` is its amount,
/// `aggressor` whether `me` made the last bet or raise before the street.
fn signal_of(kind: &str, to: f64, pot: f64, bets: [f64; 2], me: usize, aggressor: bool) -> Option<Signal> {
    let (facing, own) = (bets[1 - me], bets[me]);
    let size = size_pct(pot, facing, own, to);
    Some(match kind {
        "check" => Signal::PostCheck { behind: me == 1 },
        "call" => Signal::Call(size_pct(pot, facing, own, facing)),
        "all_in" if to <= facing + 1e-9 => Signal::Call(size),
        "raise" | "all_in" if facing > own => Signal::Raise(size),
        "bet" | "all_in" => Signal::Bet { size, aggressor },
        _ => return None,
    })
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
    let (mut before_street, mut street) = (aggressor, 3);
    let mut walk: Vec<Step> = Vec::new();
    for step in path {
        if let Step::Action(a) = step
            && let Ok(at) = sol.node(&walk)
        {
            if at.board.len() != street {
                (before_street, street) = (aggressor, at.board.len());
            }
            let actor = if at.player == Some(Player::Oop) { 0 } else { 1 };
            let action = &at.actions[*a];
            if actor == other
                && let Some(sig) = signal_of(tree_key(action.kind), action.amount, at.pot, at.street_bets, actor, before_street == Some(actor))
            {
                line.push((at.board.len(), sig));
            }
            if matches!(action.kind, Tree::Bet | Tree::Raise | Tree::AllIn) {
                aggressor = Some(actor);
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

/// What the solves are asked for: exploitability in percent of the pot.
const SOLVE_PCT: f64 = 1.0;

fn settings(threads: usize) -> Settings {
    Settings { target_pct: SOLVE_PCT, max_iterations: 300, threads }
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

/// Every board of the stored nodes (flop, turn and river of each) with its index into
/// the strengths, and the strength of each.
fn board_strengths(store: &Store) -> (HashMap<Vec<String>, usize>, Vec<Strength>) {
    let mut boards: HashMap<Vec<String>, usize> = HashMap::new();
    for n in &store.nodes {
        for len in [3, 4, 5].into_iter().filter(|&l| l <= n.board.len()) {
            let k = boards.len();
            boards.entry(n.board[..len].to_vec()).or_insert(k);
        }
    }
    let mut keys: Vec<(&Vec<String>, &usize)> = boards.iter().collect();
    keys.sort_by_key(|x| *x.1);
    let strengths: Vec<Strength> = keys
        .par_iter()
        .map(|(b, i)| {
            let cards: Vec<Card> = b.iter().filter_map(|c| c.parse().ok()).collect();
            strength(&cards, &mut ChaCha8Rng::seed_from_u64(**i as u64))
        })
        .collect();
    (boards, strengths)
}

/// The postflop cases of the stored nodes, the opponents' ranges narrowed by `curves`.
fn postflop_cases(store: &Store, boards: &HashMap<Vec<String>, usize>, strengths: &[Strength], curves: &[Curve], rng: &mut ChaCha8Rng) -> Vec<Case> {
    let mut cases = Vec::new();
    for (ni, n) in store.nodes.iter().enumerate() {
        let opp = n.opponent_range(|b| strengths[boards[&b.iter().map(|c| c.to_string()).collect::<Vec<_>>()]].clone(), curves);
        let options = n.options();
        for h in &n.hands {
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
                true_equity: h.equity,
            });
        }
    }
    cases
}

/// Below this many solver hands a curve is borrowed from the nearest size class.
const CURVE_MIN: usize = 150;
/// Hands of a bin's own the bin weighs as much as the curve's mean.
const BIN_PRIOR: f64 = 5.0;
const CURVE_PASSES: usize = 3;

type Key = (usize, MoveKind, usize);
/// Frequency sums and hands per strength bin; drawing hands in the last slot.
type Bins = (Vec<f64>, Vec<f64>);

/// The curves of the moves after the flop, from the solver's strategies on the fitting
/// half: at every node, each sampled hand of the acting player is placed by its strength
/// within that player's range (the flop range narrowed by what they did before, by the
/// curves of the pass before), and each of its moves counts with the solver's frequency.
fn fit_moves(store: &Store, boards: &HashMap<Vec<String>, usize>, strengths: &[Strength]) -> Vec<Curve> {
    let mut by_spot: BTreeMap<usize, Vec<&SolvedNode>> = BTreeMap::new();
    for n in store.nodes.iter().filter(|n| n.spot % 2 == 0) {
        by_spot.entry(n.spot).or_default().push(n);
    }
    let strength_of = |b: &[Card]| strengths[boards[&b.iter().map(|c| c.to_string()).collect::<Vec<_>>()]].clone();
    let mut curves: Vec<Curve> = Vec::new();
    for pass in 0..CURVE_PASSES {
        let acc: HashMap<Key, Bins> = by_spot
            .par_iter()
            .map(|(_, nodes)| {
                let mut acc: HashMap<Key, Bins> = HashMap::new();
                for n in nodes {
                    let (me, cards) = (n.me, n.cards());
                    let len = cards.len();
                    // A node of the other player as late tells what this one did before.
                    let Some(q) = nodes.iter().find(|q| q.me != me && q.board.len() >= len) else { continue };
                    let mut line: Vec<(usize, Signal)> = q.line.iter().copied().filter(|&(l, _)| l < len).collect();
                    let start = n.pot - n.street_bets[0] - n.street_bets[1];
                    if n.street_bets[me] > 0.0 {
                        // Where it bet, nobody had bet on the street yet.
                        let aggressor = nodes.iter().any(|m| m.me == me && m.board.len() == len && m.street_bets == [0.0, 0.0] && m.aggressor);
                        line.push((len, Signal::Bet { size: size_pct(start, 0.0, 0.0, n.street_bets[me]), aggressor }));
                    } else if me == 0 && n.street_bets[1] > 0.0 {
                        line.push((len, Signal::PostCheck { behind: false }));
                    }
                    let range = narrowed(&q.opp_range, &line, &cards, strength_of, &curves);
                    let here = strength_of(&cards);
                    let top = top_share(&range, &here.equity, CardSet::from_cards(&cards));
                    for h in &n.hands {
                        let Ok(combo) = h.combo.parse::<Combo>() else { continue };
                        let t = top[combo.index()];
                        let bin = if here.draw[combo.index()] { BINS } else { (0..BINS).find(|&b| t <= BIN_EDGES[b + 1]).unwrap_or(BINS - 1) };
                        for (k, (kind, to)) in n.options.iter().enumerate() {
                            let Some((mk, size)) = signal_of(kind, *to, n.pot, n.street_bets, me, n.aggressor).and_then(Signal::postflop) else { continue };
                            let e = acc.entry((len, mk, size)).or_insert_with(|| (vec![0.0; BINS + 1], vec![0.0; BINS + 1]));
                            e.0[bin] += h.strategy[k];
                            e.1[bin] += 1.0;
                        }
                    }
                }
                acc
            })
            .reduce(HashMap::new, |mut a, b| {
                for (k, (s, n)) in b {
                    let e = a.entry(k).or_insert_with(|| (vec![0.0; BINS + 1], vec![0.0; BINS + 1]));
                    (0..=BINS).for_each(|i| {
                        e.0[i] += s[i];
                        e.1[i] += n[i];
                    });
                }
                a
            });
        let before = curves.clone();
        curves = curves_of(&acc);
        let change = curves
            .iter()
            .filter_map(|c| before.iter().find(|b| (b.cards, b.kind, b.size) == (c.cards, c.kind, c.size)).map(|b| (c, b)))
            .flat_map(|(c, b)| c.keep.iter().chain([&c.draw]).zip(b.keep.iter().chain([&b.draw])).map(|(x, y)| (x - y).abs()))
            .fold(0.0f32, f32::max);
        eprintln!("Kurven Durchgang {}: größte Änderung {change:.3}", pass + 1);
    }
    for c in &curves {
        eprintln!(
            "  {} Karten {:?} Größe {}: {} Hände [{}] Draws {:.2}",
            c.cards,
            c.kind,
            c.size,
            c.samples,
            c.keep.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" "),
            c.draw
        );
    }
    curves
}

/// Runouts a flop's equities are averaged over in [`refine_moves`].
const FLOP_RUNOUTS: usize = 60;
const REFINE_SWEEPS: usize = 2;

/// A solved node for [`refine_moves`]: per sampled hand, against every opponent combo,
/// the showdown points summed over the runouts and the runouts it takes part in.
struct EquityNode<'a> {
    node: &'a SolvedNode,
    cards: Vec<Card>,
    hands: Vec<(Vec<f32>, Vec<f32>, f32)>,
    test: bool,
}

impl EquityNode<'_> {
    fn new(node: &SolvedNode) -> EquityNode<'_> {
        let cards = node.cards();
        let dead = CardSet::from_cards(&cards);
        let open: Vec<Card> = ringside_engine::cards::deck().into_iter().filter(|c| dead.0 & c.mask() == 0).collect();
        let runouts: Vec<Vec<Card>> = match cards.len() {
            5 => vec![vec![]],
            4 => open.iter().map(|&c| vec![c]).collect(),
            _ => {
                let mut rng = ChaCha8Rng::seed_from_u64(node.spot as u64 ^ 0x0f10);
                (0..FLOP_RUNOUTS)
                    .map(|_| {
                        let a = open[rng.random_range(0..open.len())];
                        let b = loop {
                            let b = open[rng.random_range(0..open.len())];
                            if b != a {
                                break b;
                            }
                        };
                        vec![a, b]
                    })
                    .collect()
            }
        };
        let ranks: Vec<(u64, Vec<Option<ringside_engine::eval::HandRank>>)> = runouts
            .iter()
            .map(|r| {
                let used = CardSet::from_cards(r).0 | dead.0;
                let board = ringside_engine::eval::Hand::from_cards(cards.iter().chain(r).copied());
                let ranks = Combo::all()
                    .map(|c| {
                        let (a, b) = c.cards();
                        (c.mask().0 & used == 0).then(|| board.with(a).with(b).rank())
                    })
                    .collect();
                (CardSet::from_cards(r).0, ranks)
            })
            .collect();
        let hands = node
            .hands
            .iter()
            .filter_map(|h| {
                let hero = h.combo.parse::<Combo>().ok()?;
                let mut points = vec![0.0f32; COMBOS];
                let mut seen = vec![0.0f32; COMBOS];
                for (cards, r) in &ranks {
                    let Some(mine) = r[hero.index()] else { continue };
                    for (o, theirs) in r.iter().enumerate() {
                        let Some(theirs) = theirs else { continue };
                        if Combo::from_index(o).mask().0 & (hero.mask().0 | cards) != 0 {
                            continue;
                        }
                        seen[o] += 1.0;
                        points[o] += if mine > *theirs { 1.0 } else if mine == *theirs { 0.5 } else { 0.0 };
                    }
                }
                Some((points, seen, h.equity as f32))
            })
            .collect();
        EquityNode { node, cards, hands, test: node.spot % 2 == 1 }
    }

    /// Squared error of the hands' equities against `range` to the solver's.
    fn error(&self, range: &Range) -> f64 {
        self.hands
            .iter()
            .map(|(points, seen, truth)| {
                let (mut won, mut all) = (0.0f64, 0.0f64);
                for o in 0..COMBOS {
                    let w = range.0[o] as f64;
                    won += w * points[o] as f64;
                    all += w * seen[o] as f64;
                }
                let eq = if all > 0.0 { won / all } else { 0.5 };
                (eq - *truth as f64).powi(2)
            })
            .sum()
    }

    fn uses(&self, keys: &[Key]) -> bool {
        self.node.line.iter().any(|&(len, sig)| sig.postflop().is_some_and(|(k, s)| keys.contains(&(len, k, s))))
    }
}

/// The curves moved bin by bin, as long as that brings the estimate's equity of the
/// solver's hands closer to the solver's own on the fitting half: the solver's
/// frequencies say how a hand it holds plays, not how much of the range it still holds
/// (the stored hands are those it reaches at all, without their weight).
fn refine_moves(store: &Store, boards: &HashMap<Vec<String>, usize>, strengths: &[Strength], mut curves: Vec<Curve>) -> Vec<Curve> {
    let counted = curves.clone();
    let strength_of = |b: &[Card]| strengths[boards[&b.iter().map(|c| c.to_string()).collect::<Vec<_>>()]].clone();
    let nodes: Vec<EquityNode> = store.nodes.par_iter().filter(|n| !n.line.is_empty()).map(EquityNode::new).collect();
    let errors = |curves: &[Curve], test: bool, keys: Option<&[Key]>| -> Vec<(usize, f64, usize)> {
        nodes
            .par_iter()
            .enumerate()
            .filter(|(_, n)| n.test == test && keys.is_none_or(|k| n.uses(k)))
            .map(|(i, n)| (i, n.error(&narrowed(&n.node.opp_range, &n.node.line, &n.cards, strength_of, curves)), n.hands.len()))
            .collect()
    };
    let mse = |e: &[(usize, f64, usize)]| e.iter().map(|x| x.1).sum::<f64>() / e.iter().map(|x| x.2).sum::<usize>().max(1) as f64;
    eprintln!("Equity-Fehler (mittleres Quadrat) vor der Verfeinerung: Anpassung {:.5}, Test {:.5}", mse(&errors(&curves, false, None)), mse(&errors(&curves, true, None)));
    // Curves of a street that share their hands or borrow from each other move together.
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in 0..curves.len() {
        let same = |g: &Vec<usize>| {
            let c = &curves[g[0]];
            (c.cards, &c.keep, c.draw) == (curves[i].cards, &curves[i].keep, curves[i].draw)
        };
        match groups.iter_mut().find(|g| same(g)) {
            Some(g) => g.push(i),
            None => groups.push(vec![i]),
        }
    }
    let mut current: Vec<f64> = vec![0.0; nodes.len()];
    for (i, e, _) in errors(&curves, false, None) {
        current[i] = e;
    }
    for sweep in 0..REFINE_SWEEPS {
        let mut moved = 0;
        for group in &groups {
            let keys: Vec<Key> = group.iter().map(|&j| (curves[j].cards, curves[j].kind, curves[j].size)).collect();
            let draws = if curves[group[0]].cards < 5 { 1 } else { 0 };
            for bin in 0..BINS + draws {
                let get = |c: &Curve| if bin == BINS { c.draw } else { c.keep[bin] };
                let v = get(&curves[group[0]]);
                for to in [v + 0.1, v - 0.1, v * 1.4, v / 1.4] {
                    let to = to.clamp(0.002, 1.0);
                    if (to - v).abs() < 1e-4 {
                        continue;
                    }
                    let mut trial = curves.clone();
                    for &j in group {
                        if bin == BINS {
                            trial[j].draw = to;
                        } else {
                            trial[j].keep[bin] = to;
                        }
                    }
                    let e = errors(&trial, false, Some(&keys));
                    let before: f64 = e.iter().map(|x| current[x.0]).sum();
                    let after: f64 = e.iter().map(|x| x.1).sum();
                    if after < before - 1e-6 {
                        curves = trial;
                        for (i, x, _) in e {
                            current[i] = x;
                        }
                        moved += 1;
                        break;
                    }
                }
            }
        }
        eprintln!(
            "Verfeinerung {}: {moved} Schritte, Anpassung {:.5}, Test {:.5}",
            sweep + 1,
            mse(&errors(&curves, false, None)),
            mse(&errors(&curves, true, None))
        );
    }
    // What no fitting node faces keeps its frequencies, moved bin by bin as the nearest
    // size of its street and kind was: the stored hands lack the weight for every size
    // alike.
    let used = |j: usize| groups.iter().find(|g| g.contains(&j)).is_some_and(|g| nodes.iter().any(|n| !n.test && n.uses(&g.iter().map(|&k| (curves[k].cards, curves[k].kind, curves[k].size)).collect::<Vec<_>>())));
    let refined: Vec<bool> = (0..curves.len()).map(used).collect();
    let before = curves.clone();
    for (j, c) in curves.iter_mut().enumerate().filter(|&(j, _)| !refined[j]) {
        let Some(r) = (0..before.len()).filter(|&k| refined[k] && before[k].cards == c.cards && before[k].kind == c.kind).min_by_key(|&k| before[k].size.abs_diff(c.size)) else { continue };
        let ratio = |now: f32, was: f32| if was > 1e-6 { now / was } else { 1.0 };
        for b in 0..BINS {
            c.keep[b] = (counted[j].keep[b] * ratio(before[r].keep[b], counted[r].keep[b])).clamp(0.002, 1.0);
        }
        c.draw = (counted[j].draw * ratio(before[r].draw, counted[r].draw)).clamp(0.002, 1.0);
    }
    for c in &curves {
        eprintln!("  {} Karten {:?} Größe {}: [{}] Draws {:.2}", c.cards, c.kind, c.size, c.keep.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(" "), c.draw);
    }
    curves
}

/// A curve for every street, kind and size class: the bins' frequencies, each drawn
/// towards the curve's mean as if it held [`BIN_PRIOR`] more hands. With too few hands a
/// bet takes those of both kinds of bets of its size, then the nearest size class of
/// the street lends its curve, failing that the nearest street.
fn curves_of(acc: &HashMap<Key, Bins>) -> Vec<Curve> {
    let kinds = [MoveKind::Bet, MoveKind::Lead, MoveKind::Raise, MoveKind::Call, MoveKind::Check, MoveKind::CheckBehind];
    let own = |keys: &Vec<Key>| -> Option<Curve> {
        let (mut sum, mut n) = (vec![0.0; BINS + 1], vec![0.0; BINS + 1]);
        for (s, m) in keys.iter().filter_map(|k| acc.get(k)) {
            (0..=BINS).for_each(|b| {
                sum[b] += s[b];
                n[b] += m[b];
            });
        }
        let total: f64 = n.iter().sum();
        if (total as usize) < CURVE_MIN {
            return None;
        }
        let mean = sum.iter().sum::<f64>() / total;
        let at = |b: usize| ((sum[b] + BIN_PRIOR * mean) / (n[b] + BIN_PRIOR)) as f32;
        let k = keys[0];
        Some(Curve { cards: k.0, kind: k.1, size: k.2, keep: (0..BINS).map(at).collect(), draw: at(BINS), samples: total as usize })
    };
    let mut out = Vec::new();
    for cards in [3, 4, 5] {
        for kind in kinds {
            let sizes = if matches!(kind, MoveKind::Check | MoveKind::CheckBehind) { 1 } else { SIZES };
            for size in 0..sizes {
                let mut near: Vec<Vec<Key>> = Vec::new();
                for d in 0..SIZES {
                    for s in [size.checked_sub(d), (d > 0).then_some(size + d)].into_iter().flatten().filter(|&s| s < sizes) {
                        near.push(vec![(cards, kind, s)]);
                        if matches!(kind, MoveKind::Bet | MoveKind::Lead) {
                            near.push(vec![(cards, kind, s), (cards, if kind == MoveKind::Bet { MoveKind::Lead } else { MoveKind::Bet }, s)]);
                        }
                    }
                }
                let mut streets: Vec<usize> = [3, 4, 5].into_iter().filter(|&c| c != cards).collect();
                streets.sort_by_key(|c| c.abs_diff(cards));
                near.extend(streets.into_iter().map(|c| vec![(c, kind, size)]));
                if let Some(mut c) = near.iter().find_map(own) {
                    if (c.cards, c.size) != (cards, size) {
                        c.samples = 0;
                    }
                    (c.cards, c.kind, c.size) = (cards, kind, size);
                    out.push(c);
                }
            }
        }
    }
    out
}

fn preflop_cases(hands: &[Hand], rng: &mut ChaCha8Rng) -> (Vec<Case>, usize) {
    let mut pre = Vec::new();
    let mut spots = 0;
    for hand in hands {
        let Some(hero) = hand.hero.clone() else { continue };
        let analysis = analyze_hand(hand);
        for d in analysis.decisions.iter().filter(|d| d.source == Some(Source::Model)) {
            let Ok(spot) = replay::spot_at(hand, &hero, d.action_index, None, &[]) else { continue };
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
fn write_csv(path: &str, cases: &[Case], tuning_of: TuningOf, shared: Shared, store: &Store) {
    let rows: Vec<String> = cases
        .par_iter()
        .flat_map_iter(|c| {
            let t = &tuning_of(c);
            let prep = prepared(c, t, shared);
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
    let mut fitted = vec!["preflop".to_string(), "flop".into(), "turn".into(), "river".into()];
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
            "--fit" => fitted = args.next().expect("--fit STREETS").split(',').map(str::to_string).collect(),
            "--postflop-tuning" => given = args.next().map(|j| serde_json::from_str(&j).expect("tuning JSON")),
            _ => panic!("unknown argument {a}"),
        }
    }
    ringside_engine::eval::init();
    let mut rng = ChaCha8Rng::seed_from_u64(2026);
    let started = Instant::now();
    let hands = hands_dir.as_deref().map(hands_in).unwrap_or_default();

    let store: Store = match std::fs::read(&cases_path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{cases_path}: {e}; Fälle ohne Einsatzgrößen der Linie lassen sich nicht lesen, Datei löschen, dann wird neu gelöst")),
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
    let (boards, strengths) = board_strengths(&store);
    let stored = ringside_analysis::calibration::stored();
    let post_stored = postflop_cases(&store, &boards, &strengths, &stored.moves, &mut rng.clone());
    let moves = refine_moves(&store, &boards, &strengths, fit_moves(&store, &boards, &strengths));
    let post = postflop_cases(&store, &boards, &strengths, &moves, &mut rng);
    let shared = Shared { strengths: &strengths, curves: &moves };
    let (pre, pre_spots) = preflop_cases(&hands, &mut rng);
    eprintln!("Postflop: {} Fälle aus {} Spots; Preflop: {} Fälle", post.len(), store.spots, pre.len());
    // Without cases of a kind the stored numbers stay; after the flop each street is
    // fitted on its own.
    let street_tunings: [Tuning; 3] = [3, 4, 5].map(|n| {
        let k = n - 3;
        let mine: Vec<&Case> = post.iter().filter(|c| c.spot.board.len() == n).collect();
        match given {
            Some(t) => t,
            None if mine.is_empty() || !fitted.iter().any(|f| *f == ["flop", "turn", "river"][k]) => stored.street_tunings[k],
            None => {
                eprintln!("Anpassung mit {n} Karten:");
                fit(&mine, shared, &stored.street_tunings[k])
            }
        }
    });
    let pre_refs: Vec<&Case> = pre.iter().collect();
    let preflop_tuning = if pre.is_empty() || !fitted.iter().any(|f| f == "preflop") {
        stored.preflop_tuning
    } else {
        eprintln!("Anpassung vor dem Flop:");
        // Searched coordinate by coordinate, where the fit ends depends on where it starts:
        // it starts again from there until nothing moves.
        let mut t = fit_behind(&pre_refs, shared, &stored.preflop_tuning);
        for _ in 0..5 {
            let next = fit_behind(&pre_refs, shared, &fit(&pre_refs, shared, &t));
            if next == t {
                break;
            }
            t = next;
        }
        t
    };
    let tuning_of = |c: &Case| if c.spot.board.is_empty() { preflop_tuning } else { street_tunings[c.spot.board.len() - 3] };
    if let Some(path) = &csv {
        write_csv(path, &post, &tuning_of, shared, &store);
    }
    evaluate(&post_stored, &|c: &Case| stored.tuning(street_of_cards(c.spot.board.len())), Shared { strengths: &strengths, curves: &stored.moves }, Some(true)).report("Postflop mit gespeicherten Zahlen (Testhälfte)");
    let post_test = evaluate(&post, &tuning_of, shared, Some(true));
    post_test.report("Postflop angepasst (Testhälfte)");
    let pre_test = evaluate(&pre, &tuning_of, shared, Some(true));
    pre_test.report("Preflop angepasst (Testhälfte)");

    // The decisions really played, rated by the analysis both ways.
    let by_id: HashMap<u64, &Hand> = hands.iter().map(|h| (h.id, h)).collect();
    let mut real = Tally::default();
    let mut by_street: [Tally; 3] = Default::default();
    let mut real_hands = 0;
    let numbers = Calibration { street_tunings, moves: moves.clone(), ..stored.clone() };
    for (id, solved) in &store.real {
        let Some(hand) = by_id.get(id) else { continue };
        real_hands += 1;
        let hero = hand.hero.as_deref().unwrap_or_default();
        let ranges = replay::flop_ranges(hand, hero);
        for d in solved.iter().filter(|d| d.rating.is_some()) {
            if let Ok(e) = replay::rate_with(hand, hero, d.action_index, d.spot, ranges.as_ref(), "", &numbers) {
                let (a, b) = (grade_of(d.rating.unwrap()), grade_of(e.rating.unwrap_or(Rating::Blunder)));
                let kind = e.strategy.iter().find(|c| c.played).map_or("?", |c| c.kind.as_str());
                real.add(kind, a, b);
                let ev = |c: &ringside_core::db::Choice| c.ev_bb.unwrap_or(0.0);
                let top = d.strategy.iter().map(ev).fold(f64::NEG_INFINITY, f64::max);
                let loss = d.strategy.iter().find(|c| c.played).map_or(0.0, |c| (top - ev(c)) / d.pot_bb.unwrap_or(1.0).max(1e-9) * 100.0);
                real.mix(d.played_freq.unwrap_or(0.0), loss, b);
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
        let t = evaluate_where(&post, &tuning_of, shared, |c| c.test && c.spot.board.len() == n);
        t.report(&format!("Postflop mit {n} Karten (Testhälfte)"));
        let spots = store.nodes.iter().filter(|x| x.spot % 2 == 1 && x.board.len() == n).map(|x| x.spot).collect::<std::collections::BTreeSet<_>>().len();
        t.agreement(spots)
    });
    let mut c = Calibration {
        postflop_streets: post_streets,
        real_streets: by_street.each_ref().map(|t| t.agreement(real_hands)),
        preflop_tuning,
        street_tunings,
        preflop: pre_test.agreement(pre_spots / 2),
        postflop: post_test.agreement(store.spots / 2),
        real: real.agreement(real_hands),
        moves,
    };
    if hands.is_empty() {
        eprintln!("Ohne --hands: Preflop und echte Entscheidungen nicht neu gemessen, die gespeicherten Prüfungen bleiben.");
        (c.preflop, c.real, c.real_streets) = (stored.preflop.clone(), stored.real.clone(), stored.real_streets.clone());
    }
    println!("{}", serde_json::to_string_pretty(&c).unwrap());
    println!("in {:.0} s", started.elapsed().as_secs_f64());
    if write {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/estimate_calibration.json");
        std::fs::write(path, serde_json::to_string_pretty(&c).unwrap() + "\n").expect("write calibration");
        println!("geschrieben: {path}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fitted number at an end of its grid may have its best value beyond it.
    #[test]
    fn no_fitted_number_ends_at_the_edge_of_its_grid() {
        let c = ringside_analysis::calibration::stored();
        for (street, t) in ["Preflop", "Flop", "Turn", "River"].iter().zip(std::iter::once(&c.preflop_tuning).chain(&c.street_tunings)) {
            assert!(at_edge(t).is_empty(), "{street}: {:?} am Rand des Suchgitters", at_edge(t));
        }
    }
}
