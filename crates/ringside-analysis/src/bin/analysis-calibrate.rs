//! Fits the estimate's free numbers and measures how well its grades agree with the
//! solver (after the flop) and the preflop model (before it). With `--write` the result
//! goes to `data/estimate_calibration.json`, which the analysis shows as the estimate's
//! reliability:
//!
//! ```text
//! cargo run --release -p ringside-analysis --bin analysis-calibrate -- [--hands DIR] [--real N] [--flops N] [--threads T] [--write]
//! ```
//!
//! - Postflop: heads-up spots of the preflop model's most frequent flops (`--flops`
//!   random boards each, tables from `CALIBRATE_TABLES`, e.g. `6x30,9x25`) solved to
//!   1 % of the pot. At the first decisions of flop, turn and river every option of a
//!   sample of the acting player's hands is graded by the solver and by the estimate,
//!   the opponent's range narrowed by the estimate's own rules along the line.
//! - Preflop: every option of every model-rated decision of `--hands`.
//! - Real: the decisions actually played in those hands at solved nodes (the `--real`
//!   biggest pots, default 20).
//!
//! The numbers are fitted on every other spot and the agreement is reported on the rest,
//! so it is not measured on what it was fitted to. Grades are compared on five levels
//! (best and mix count as one).

use std::time::Instant;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use ringside_analysis::calibration::{Agreement, Calibration};
use ringside_analysis::estimate::{Opponent, Opt, Range, Signal, Spot, Tuning, estimate, narrow, strength};
use ringside_analysis::{PostflopJob, Rating, Source, analyze_hand, replay, solve_job};
use ringside_core::model::Hand;
use ringside_core::{parse_hand, split_hands};
use ringside_engine::preflop_table::{self, Ante};
use ringside_engine::{Card, CardSet, Combo};
use ringside_play::grade::{BEST_PCT, Grade, judge};
use ringside_solver::{ActionKind as Tree, Node, NodeKind, Player, Settings, Solution, Step, solve};

/// One graded hand at one node: the reference grade of every option.
struct Case {
    spot: Spot,
    options: Vec<Opt>,
    reference: Vec<Grade>,
    /// How often the reference plays each option with this hand.
    freq: Vec<f64>,
    pot: f64,
    /// Which half it belongs to (fit or test), by spot.
    test: bool,
    seed: u64,
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
    by_kind: std::collections::BTreeMap<&'static str, (usize, usize)>,
}

impl Tally {
    fn add(&mut self, kind: &'static str, a: Grade, b: Grade) {
        let d = level(a).abs_diff(level(b));
        self.matrix[level(a)][level(b)] += 1;
        self.samples += 1;
        self.same += usize::from(d == 0);
        self.within += usize::from(d <= 1);
        let e = self.by_kind.entry(kind).or_default();
        e.0 += 1;
        e.1 += usize::from(d <= 1);
    }

    fn merge(mut self, o: Tally) -> Tally {
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
        eprintln!("  Referenz (Zeile) gegen Einschätzung (Spalte): beste/gut/ungenau/Fehler/grob");
        for row in &self.matrix {
            eprintln!("  {}", row.iter().map(|x| format!("{x:6}")).collect::<String>());
        }
        for (k, (n, w)) in &self.by_kind {
            eprintln!("  {k:7} {n:6} Vergleiche, ±1: {:.1} %", 100.0 * *w as f64 / (*n).max(1) as f64);
        }
    }
}

/// Grades of every option for the estimate's EVs: options within `tie_pct` of the pot
/// of the best share the frequency, as in the analysis.
fn estimate_grades(evs: &[f64], pot: f64, tie_pct: f64) -> Vec<Grade> {
    let top = evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let near: Vec<f64> = evs.iter().map(|&e| if (top - e) / pot * 100.0 <= tie_pct { 1.0 } else { 0.0 }).collect();
    let total: f64 = near.iter().sum();
    let freq: Vec<f64> = near.iter().map(|x| x / total).collect();
    (0..evs.len()).map(|k| judge(&freq, Some(evs), k, pot).grade).collect()
}

/// Agreement within one step on the test half, each option weighted by how often the
/// reference plays it: how the estimate does on the moves the reference would make.
fn played_within(cases: &[Case], t: &Tuning) -> f64 {
    let (w, n) = cases
        .par_iter()
        .filter(|c| c.test)
        .map(|c| {
            let est = estimate(&c.spot, &c.options, c.seed, t);
            let mine = estimate_grades(&est.evs, c.pot, t.tie_pct);
            (0..c.options.len()).fold((0.0, 0.0), |(w, n), a| {
                let f = c.freq.get(a).copied().unwrap_or(0.0);
                (w + f * f64::from(u8::from(level(c.reference[a]).abs_diff(level(mine[a])) <= 1)), n + f)
            })
        })
        .reduce(|| (0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
    w / n.max(1e-9)
}

fn evaluate(cases: &[Case], t: &Tuning, test: Option<bool>) -> Tally {
    cases
        .par_iter()
        .filter(|c| test.is_none_or(|x| x == c.test))
        .map(|c| {
            let est = estimate(&c.spot, &c.options, c.seed, t);
            let mine = estimate_grades(&est.evs, c.pot, t.tie_pct);
            let mut tally = Tally::default();
            for k in 0..c.options.len() {
                tally.add(c.options[k].key(), c.reference[k], mine[k]);
            }
            tally
        })
        .reduce(Tally::default, Tally::merge)
}

/// The tuning with the most balanced agreement within one step on the fitting half: the
/// share within one step per reference grade, averaged over the grades. Plain agreement
/// would favour calling everything fine, as most options are fine. The shrink and the
/// tie only act on the EVs, so each other combination is estimated once.
fn fit(cases: &[Case]) -> Tuning {
    const SHRINKS: [f64; 9] = [1.0, 0.7, 0.5, 0.35, 0.3, 0.25, 0.2, 0.15, 0.1];
    let fitting: Vec<&Case> = cases.iter().filter(|c| !c.test).collect();
    const TIES: [f64; 5] = [BEST_PCT, 1.0, 2.0, 3.0, 4.0];
    let mut best = (Tuning::default(), -1.0);
    for strength in [0.0, 0.5, 1.0, 1.5] {
        for defend in [0.9, 1.2, 1.6, 2.2, 3.0, 4.0] {
            let raw: Vec<Vec<f64>> = fitting
                .par_iter()
                .map(|c| estimate(&c.spot, &c.options, c.seed, &Tuning { shrink: 1.0, strength, defend, tie_pct: BEST_PCT }).evs)
                .collect();
            for shrink in SHRINKS {
                for tie_pct in TIES {
                    let mut tally = Tally::default();
                    for (c, evs) in fitting.iter().zip(&raw) {
                        let top = evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                        let shrunk: Vec<f64> = evs.iter().map(|&e| top - shrink * (top - e)).collect();
                        let mine = estimate_grades(&shrunk, c.pot, tie_pct);
                        for a in 0..c.options.len() {
                            tally.add(c.options[a].key(), c.reference[a], mine[a]);
                        }
                    }
                    let score = tally.balanced();
                    if score > best.1 {
                        best = (Tuning { shrink, strength, defend, tie_pct }, score);
                    }
                }
            }
        }
        eprintln!("  Anpassung: Stärke {strength} fertig, bisher bestes {:?} mit {:.1} %", best.0, best.1 * 100.0);
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

fn opt_of(kind: Tree, amount: f64) -> Opt {
    match kind {
        Tree::Fold => Opt::Fold,
        Tree::Check => Opt::Check,
        Tree::Call => Opt::Call,
        Tree::Bet => Opt::Bet(amount),
        Tree::Raise => Opt::Raise(amount),
        Tree::AllIn => Opt::AllIn,
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

/// The decision nodes worth checking: flop root and the answers to check and to the
/// first bet; the same on one turn and one river card after checks.
fn paths(sol: &mut Solution, rng: &mut ChaCha8Rng) -> Vec<Vec<Step>> {
    let mut out = Vec::new();
    let mut street_start: Vec<Step> = Vec::new();
    for _ in 0..3 {
        let Ok(root) = sol.node(&street_start) else { break };
        if root.kind != NodeKind::Action {
            break;
        }
        out.push(street_start.clone());
        if let Some(b) = root.actions.iter().position(|a| matches!(a.kind, Tree::Bet)) {
            let mut p = street_start.clone();
            p.push(Step::Action(b));
            out.push(p);
        }
        let Some(c) = root.actions.iter().position(|a| a.kind == Tree::Check) else { break };
        let mut after_check = street_start.clone();
        after_check.push(Step::Action(c));
        let Ok(ip) = sol.node(&after_check) else { break };
        if ip.kind != NodeKind::Action {
            break;
        }
        out.push(after_check.clone());
        if let Some(b) = ip.actions.iter().position(|a| matches!(a.kind, Tree::Bet)) {
            let mut p = after_check.clone();
            p.push(Step::Action(b));
            out.push(p);
        }
        let Some(c2) = ip.actions.iter().position(|a| a.kind == Tree::Check) else { break };
        let mut next = after_check;
        next.push(Step::Action(c2));
        let Ok(chance) = sol.node(&next) else { break };
        if chance.kind != NodeKind::Chance || chance.cards.is_empty() {
            break;
        }
        next.push(Step::Card(chance.cards[rng.random_range(0..chance.cards.len())].clone()));
        street_start = next;
    }
    out
}

fn spot_of(node: &Node, me: usize, combo: Combo, board: &[Card], opp: &Range) -> Spot {
    let other = 1 - me;
    Spot {
        preflop: false,
        river: board.len() == 5,
        board: board.to_vec(),
        hero: combo,
        opponents: vec![Opponent { range: opp.clone(), behind: node.stacks[other], street_in: node.street_bets[other] }],
        pot: node.pot,
        hero_in: node.street_bets[me],
        hero_behind: node.stacks[me],
        in_position: me == 1,
        aggressor: false,
    }
}

/// Cases of a sample of the acting player's hands at the node of `path`.
fn cases_at(sol: &mut Solution, path: &[Step], ranges: [&Range; 2], test: bool, rng: &mut ChaCha8Rng, out: &mut Vec<Case>) {
    let Ok(node) = sol.node(path) else { return };
    if node.kind != NodeKind::Action || node.actions.len() < 2 {
        return;
    }
    let me = if node.player == Some(Player::Oop) { 0 } else { 1 };
    let other = 1 - me;
    let mut opp = ranges[other].clone();
    let mut walk: Vec<Step> = Vec::new();
    for step in path {
        if let Step::Action(a) = step
            && let Ok(at) = sol.node(&walk)
            && at.player == Some(if other == 0 { Player::Oop } else { Player::Ip })
            && let Some(sig) = signal_of(at.actions[*a].kind)
        {
            let board: Vec<Card> = at.board.iter().filter_map(|c| c.parse().ok()).collect();
            let s = strength(&board, rng);
            narrow(&mut opp, sig, &s, CardSet::from_cards(&board));
        }
        walk.push(step.clone());
    }
    let board: Vec<Card> = node.board.iter().filter_map(|c| c.parse().ok()).collect();
    let options: Vec<Opt> = node.actions.iter().map(|a| opt_of(a.kind, a.amount)).collect();
    let hands: Vec<_> = node.hands.iter().filter(|h| h.weight > 0.05 && h.evs.len() == node.actions.len()).collect();
    for _ in 0..12.min(hands.len()) {
        let h = hands[rng.random_range(0..hands.len())];
        let Ok(combo) = h.combo.parse::<Combo>() else { continue };
        let reference = (0..options.len()).map(|k| judge(&h.strategy, Some(&h.evs), k, node.pot).grade).collect();
        out.push(Case {
            spot: spot_of(&node, me, combo, &board, &opp),
            options: options.clone(),
            reference,
            freq: h.strategy.clone(),
            pot: node.pot,
            test,
            seed: rng.random(),
        });
    }
}

fn settings(threads: usize) -> Settings {
    Settings { target_pct: 1.0, max_iterations: 300, threads }
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

fn main() {
    let mut hands_dir: Option<String> = None;
    let mut flops = 3usize;
    let mut threads = 8usize;
    let mut write = false;
    let mut real_limit = 20usize;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--hands" => hands_dir = args.next(),
            "--flops" => flops = args.next().and_then(|v| v.parse().ok()).expect("--flops N"),
            "--threads" => threads = args.next().and_then(|v| v.parse().ok()).expect("--threads N"),
            "--write" => write = true,
            "--real" => real_limit = args.next().and_then(|v| v.parse().ok()).expect("--real N"),
            _ => panic!("unknown argument {a}"),
        }
    }
    ringside_engine::eval::init();
    let mut rng = ChaCha8Rng::seed_from_u64(2026);
    let started = Instant::now();
    let tables: Vec<(usize, f64)> = std::env::var("CALIBRATE_TABLES")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.split_once('x')).map(|(n, b)| (n.parse().unwrap(), b.parse().unwrap())).collect())
        .unwrap_or_else(|| vec![(6, 30.0), (6, 100.0), (8, 50.0), (9, 25.0)]);

    let mut post: Vec<Case> = Vec::new();
    let mut post_spots = 0;
    for (players, stack) in tables {
        let params = preflop_table::limp_params(Ante::BigBlind, players, stack);
        let Some(sol) = preflop_table::lookup(&params) else { continue };
        for sc in ringside_play::scenario::scenarios(&sol, &params).into_iter().take(3) {
            for _ in 0..flops {
                let flop = ringside_play::pool::random_flop(&mut rng);
                let text: String = flop.iter().map(|c| c.to_string()).collect();
                let spot = sc.spot(&text);
                let Ok(mut solved) = solve(&spot, &settings(threads), 8 << 30, |_| {}, || false) else { continue };
                let ranges = [Range::parse(&spot.oop_range).expect("range"), Range::parse(&spot.ip_range).expect("range")];
                let test = post_spots % 2 == 1;
                for p in paths(&mut solved, &mut rng) {
                    cases_at(&mut solved, &p, [&ranges[0], &ranges[1]], test, &mut rng, &mut post);
                }
                post_spots += 1;
                eprintln!("{stack} BB, {players} Spieler, {text}: {} Fälle, {:.0} s", post.len(), started.elapsed().as_secs_f64());
            }
        }
    }

    let mut pre: Vec<Case> = Vec::new();
    let mut pre_spots = 0;
    let mut real_hands: Vec<(Hand, Vec<ringside_analysis::Decision>)> = Vec::new();
    let mut jobs: Vec<(f64, Hand, PostflopJob)> = Vec::new();
    if let Some(dir) = &hands_dir {
        for hand in hands_in(dir) {
            let Some(hero) = hand.hero.clone() else { continue };
            let analysis = analyze_hand(&hand);
            for d in analysis.decisions.iter().filter(|d| d.source == Some(Source::Model)) {
                let Ok(spot) = replay::spot_at(&hand, &hero, d.action_index, None) else { continue };
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
                pre.push(Case { spot, options, reference, freq: freqs, pot, test: pre_spots % 2 == 1, seed: rng.random() });
                pre_spots += 1;
            }
            if let Some((pot, json)) = &analysis.job {
                jobs.push((*pot, hand, serde_json::from_str::<PostflopJob>(json).expect("job")));
            }
        }
    }
    jobs.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, hand, job) in jobs.into_iter().take(real_limit) {
        if let Ok(solved) = solve_job(&job, threads, 8 << 30, |_| {}, || false) {
            real_hands.push((hand, solved));
            eprintln!("echte Hand gelöst: {} bisher, {:.0} s", real_hands.len(), started.elapsed().as_secs_f64());
        }
    }

    eprintln!("Postflop: {} Fälle aus {post_spots} Spots; Preflop: {} Fälle", post.len(), pre.len());
    // Without cases of a kind the stored numbers stay.
    let stored = ringside_analysis::calibration::stored();
    let postflop_tuning = if post.is_empty() { stored.postflop_tuning } else { fit(&post) };
    let preflop_tuning = if pre.is_empty() { stored.preflop_tuning } else { fit(&pre) };
    let post_test = evaluate(&post, &postflop_tuning, Some(true));
    let pre_test = evaluate(&pre, &preflop_tuning, Some(true));
    evaluate(&post, &Tuning::default(), Some(true)).report("Postflop ohne Anpassung (Testhälfte)");
    post_test.report("Postflop angepasst (Testhälfte)");
    evaluate(&pre, &Tuning::default(), Some(true)).report("Preflop ohne Anpassung (Testhälfte)");
    pre_test.report("Preflop angepasst (Testhälfte)");
    eprintln!(
        "Gewichtet nach der Strategie der Referenz (Testhälfte), ±1: postflop {:.1} %, preflop {:.1} %",
        100.0 * played_within(&post, &postflop_tuning),
        100.0 * played_within(&pre, &preflop_tuning)
    );

    // The decisions really played, rated by the analysis both ways.
    let mut real = Tally::default();
    for (hand, solved) in &real_hands {
        let hero = hand.hero.as_deref().unwrap_or_default();
        let ranges = replay::flop_ranges(hand, hero);
        for d in solved.iter().filter(|d| d.rating.is_some()) {
            if let Ok(e) = replay::rate_with(hand, hero, d.action_index, d.spot, ranges.as_ref(), "", &postflop_tuning) {
                real.add("echt", grade_of(d.rating.unwrap()), grade_of(e.rating.unwrap_or(Rating::Blunder)));
                if std::env::var_os("CALIBRATE_SHOW").is_some() {
                    let fmt = |x: &ringside_analysis::Decision| {
                        x.strategy.iter().map(|c| format!("{} {:.0}% {:+.2}", c.label, c.freq * 100.0, c.ev_bb.unwrap_or(0.0))).collect::<Vec<_>>().join(" | ")
                    };
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

    let c = Calibration {
        preflop_tuning,
        postflop_tuning,
        preflop: pre_test.agreement(pre_spots / 2),
        postflop: post_test.agreement(post_spots / 2),
        real: real.agreement(real_hands.len()),
    };
    println!("{}", serde_json::to_string_pretty(&c).unwrap());
    println!("in {:.0} s", started.elapsed().as_secs_f64());
    if write {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/estimate_calibration.json");
        std::fs::write(path, serde_json::to_string_pretty(&c).unwrap() + "\n").expect("write calibration");
        println!("geschrieben: {path}");
    }
}
