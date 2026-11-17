//! The benchmark behind the app's "GPU-Test": fixed spots solved by postflop-solver on all CPU
//! cores and by the GPU prototype, both to the same exploitability, measured by the same code.

use std::fmt::Write;
use std::time::{Duration, Instant};

use postflop_solver::{
    ActionTree, BetSizeOptions, BoardState, CardConfig, NOT_DEALT, PostFlopGame, TreeConfig, card_from_str,
    compute_current_ev, compute_exploitability, flop_from_str, solve_step,
};
use serde::Serialize;

use crate::gpu::{Gpu, GpuInfo, GpuSolver};
use crate::tree::FlatTree;

/// Exploitability is measured every this many iterations, as in the app's solver.
const MEASURE_EVERY: u32 = 10;
const MAX_ITERATIONS: u32 = 1000;
pub const TARGET_PCT: f64 = 0.3;

/// A heads-up spot with one bet structure for both players; amounts in chips.
#[derive(Clone, Debug)]
pub struct BenchSpot {
    pub name: &'static str,
    pub oop_range: &'static str,
    pub ip_range: &'static str,
    /// Four or five cards; a turn board needs four different suits (no isomorphic rivers).
    pub board: &'static str,
    pub pot: i32,
    pub stack: i32,
    /// Bets and raises in percent of the pot, as postflop-solver parses them.
    pub turn: (&'static str, &'static str),
    pub river: (&'static str, &'static str),
}

/// Wide single-raised-pot ranges, so the spots are about as heavy as the ones users solve.
const OOP: &str = "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,74s+,63s+,53s+,43s,A2o+,K7o+,Q8o+,J8o+,T8o+,98o";
const IP: &str = "22+,A2s+,K2s+,Q2s+,J4s+,T6s+,96s+,85s+,74s+,63s+,53s+,43s,A2o+,K5o+,Q7o+,J7o+,T7o+,97o+,87o";
const SIZES: (&str, &str) = ("33%, 75%, 150%", "60%");

pub fn river_spot() -> BenchSpot {
    BenchSpot {
        name: "River",
        oop_range: OOP,
        ip_range: IP,
        board: "QsJh7d2c5h",
        pot: 1000,
        stack: 4000,
        turn: ("", ""),
        river: SIZES,
    }
}

pub fn turn_spot() -> BenchSpot {
    BenchSpot {
        name: "Turn",
        oop_range: OOP,
        ip_range: IP,
        board: "QsJh7d2c",
        pot: 1000,
        stack: 4000,
        turn: SIZES,
        river: SIZES,
    }
}

impl BenchSpot {
    /// The game with its memory allocated, 32-bit storage.
    pub fn game(&self) -> Result<PostFlopGame, String> {
        let range = |s: &str| s.parse().map_err(|e| format!("Range: {e}"));
        let board: Vec<u8> =
            self.board.as_bytes().chunks(2).map(|c| card_from_str(std::str::from_utf8(c).unwrap_or(""))).collect::<Result<_, _>>()?;
        if !(4..=5).contains(&board.len()) {
            return Err("Das Board braucht vier oder fünf Karten.".into());
        }
        let flop: String = board[..3].iter().map(|&c| postflop_solver::card_to_string(c).unwrap_or_default()).collect();
        let card_config = CardConfig {
            range: [range(self.oop_range)?, range(self.ip_range)?],
            flop: flop_from_str(&flop)?,
            turn: board[3],
            river: board.get(4).copied().unwrap_or(NOT_DEALT),
        };
        let sizes = |(bet, raise): (&str, &str)| -> Result<[BetSizeOptions; 2], String> {
            let o = BetSizeOptions::try_from((bet, raise))?;
            Ok([o.clone(), o])
        };
        let tree_config = TreeConfig {
            initial_state: if board.len() == 5 { BoardState::River } else { BoardState::Turn },
            starting_pot: self.pot,
            effective_stack: self.stack,
            turn_bet_sizes: sizes(self.turn)?,
            river_bet_sizes: sizes(self.river)?,
            add_allin_threshold: 1.5,
            force_allin_threshold: 0.15,
            merging_threshold: 0.1,
            ..Default::default()
        };
        let mut game = PostFlopGame::with_config(card_config, ActionTree::new(tree_config)?)?;
        game.allocate_memory(false);
        Ok(game)
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Run {
    pub iterations: u32,
    /// Time spent iterating until the target was reached (or the limit), measurements excluded.
    pub ms: f64,
    pub iterations_per_second: f64,
    pub exploitability_pct: f64,
    pub reached: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SpotResult {
    pub name: String,
    pub board: String,
    pub hands: [usize; 2],
    pub nodes: usize,
    /// GPU storage for this spot, in MB.
    pub gpu_mb: f64,
    pub cpu: Run,
    pub gpu: Option<Run>,
    /// Why the GPU run did not happen or failed.
    pub gpu_error: Option<String>,
    /// CPU time over GPU time to the target, and CPU over GPU time per iteration.
    pub speedup: Option<f64>,
    pub speedup_per_iteration: Option<f64>,
    /// OOP's expected value under the GPU's strategies minus under the CPU's, in percent of the pot.
    pub value_diff_pct: Option<f64>,
    /// Share of play that differs, weighted by how often a hand reaches a decision, and the
    /// largest difference of one action's probability. Hands that are indifferent between
    /// actions may settle on another mix after rounding; that costs no exploitability.
    pub strategy_diff: Option<f64>,
    pub strategy_diff_max: Option<f64>,
    /// Both reached the target and the game values differ by at most the target.
    pub matches: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub gpu: Option<GpuInfo>,
    pub cpu_threads: usize,
    pub target_pct: f64,
    pub spots: Vec<SpotResult>,
}

fn cancelled() -> String {
    "Abgebrochen".into()
}

fn finish(iterations: u32, time: Duration, expl: f32, pot: f32) -> Run {
    let pct = (expl / pot * 100.0) as f64;
    let secs = time.as_secs_f64();
    Run {
        iterations,
        ms: secs * 1000.0,
        iterations_per_second: if secs > 0.0 { iterations as f64 / secs } else { 0.0 },
        exploitability_pct: pct,
        reached: pct <= TARGET_PCT,
    }
}

fn cpu_run(spot: &BenchSpot, pool: &rayon::ThreadPool, stop: &dyn Fn() -> bool) -> Result<(Run, PostFlopGame), String> {
    let game = pool.install(|| spot.game())?;
    let pot = spot.pot as f32;
    let target = TARGET_PCT as f32 / 100.0 * pot;
    let mut expl = pool.install(|| compute_exploitability(&game));
    let (mut t, mut time) = (0, Duration::ZERO);
    while t < MAX_ITERATIONS && expl > target {
        if stop() {
            return Err(cancelled());
        }
        let start = Instant::now();
        pool.install(|| solve_step(&game, t));
        time += start.elapsed();
        t += 1;
        if t.is_multiple_of(MEASURE_EVERY) {
            expl = pool.install(|| compute_exploitability(&game));
        }
    }
    Ok((finish(t, time, expl, pot), game))
}

fn gpu_run(
    gpu: &Gpu,
    spot: &BenchSpot,
    pool: &rayon::ThreadPool,
    stop: &dyn Fn() -> bool,
) -> Result<(Run, Vec<f32>, FlatTree, PostFlopGame), String> {
    let check = pool.install(|| spot.game())?;
    let mut solver = GpuSolver::new(gpu, FlatTree::new(&check)?)?;
    let pot = spot.pot as f32;
    let target = TARGET_PCT as f32 / 100.0 * pot;
    let mut expl = pool.install(|| compute_exploitability(&check));
    let mut time = Duration::ZERO;
    let mut sum = vec![0.0; solver.tree.storage_len];
    while solver.iteration < MAX_ITERATIONS && expl > target {
        if stop() {
            return Err(cancelled());
        }
        time += solver.run(MEASURE_EVERY)?;
        sum = solver.strategy_sum()?;
        solver.tree.write_strategy(&check, &sum);
        expl = pool.install(|| compute_exploitability(&check));
    }
    let run = finish(solver.iteration, time, expl, pot);
    Ok((run, sum, solver.tree, check))
}

/// Solves `spot` on the CPU and on `gpu` and compares the two.
pub fn compare(
    gpu: &Gpu,
    spot: &BenchSpot,
    pool: &rayon::ThreadPool,
    progress: &mut dyn FnMut(&str),
    stop: &dyn Fn() -> bool,
) -> Result<SpotResult, String> {
    progress(&format!("{}: CPU rechnet …", spot.name));
    let (cpu, cpu_game) = cpu_run(spot, pool, stop)?;
    progress(&format!("{}: GPU rechnet …", spot.name));
    let mut result = SpotResult {
        name: spot.name.into(),
        board: spot.board.into(),
        hands: [cpu_game.private_cards(0).len(), cpu_game.private_cards(1).len()],
        nodes: 0,
        gpu_mb: 0.0,
        cpu,
        gpu: None,
        gpu_error: None,
        speedup: None,
        speedup_per_iteration: None,
        value_diff_pct: None,
        strategy_diff: None,
        strategy_diff_max: None,
        matches: false,
    };
    match gpu_run(gpu, spot, pool, stop) {
        Ok((run, sum, tree, check)) => {
            let (mean, max) = tree.strategy_difference(&tree.read_strategy(&cpu_game), &sum);
            let value = |game: &PostFlopGame| pool.install(|| compute_current_ev(game))[0] as f64;
            let value_diff = (value(&check) - value(&cpu_game)) / spot.pot as f64 * 100.0;
            result.nodes = tree.num_nodes;
            result.gpu_mb = ((tree.num_nodes + 2) * tree.stride * 8 + tree.storage_len * 8 + tree.showdown.len() * 4) as f64
                / (1 << 20) as f64;
            result.speedup = (run.ms > 0.0).then(|| result.cpu.ms / run.ms);
            result.speedup_per_iteration =
                (result.cpu.iterations_per_second > 0.0).then(|| run.iterations_per_second / result.cpu.iterations_per_second);
            result.value_diff_pct = Some(value_diff);
            result.strategy_diff = Some(mean);
            result.strategy_diff_max = Some(max);
            result.matches = result.cpu.reached && run.reached && value_diff.abs() <= TARGET_PCT;
            result.gpu = Some(run);
        }
        Err(e) if e == cancelled() => return Err(e),
        Err(e) => result.gpu_error = Some(e),
    }
    Ok(result)
}

/// Solves the river and the turn spot on the CPU (`threads` workers) and on the GPU.
/// Without a hardware GPU the report has no spots.
pub fn benchmark(threads: usize, progress: &mut dyn FnMut(&str), stop: &dyn Fn() -> bool) -> Result<Report, String> {
    progress("Suche GPU …");
    let gpu = Gpu::open();
    let mut report = Report { gpu: gpu.as_ref().map(|g| g.info.clone()), cpu_threads: threads, target_pct: TARGET_PCT, spots: Vec::new() };
    let Some(gpu) = gpu else { return Ok(report) };
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().map_err(|e| e.to_string())?;
    for spot in [river_spot(), turn_spot()] {
        report.spots.push(compare(&gpu, &spot, &pool, progress, stop)?);
    }
    Ok(report)
}

/// A number with a decimal comma.
fn de(x: f64, digits: usize) -> String {
    format!("{x:.digits$}").replace('.', ",")
}

impl Report {
    /// A short plain-text summary to paste into a message.
    pub fn text(&self) -> String {
        let mut s = String::from("Ringside GPU-Test\n");
        let Some(gpu) = &self.gpu else {
            s.push_str("Keine GPU gefunden.\n");
            return s;
        };
        let driver = if gpu.driver.is_empty() { String::new() } else { format!(", Treiber {}", gpu.driver) };
        let _ = writeln!(s, "GPU: {} ({}, {}{driver})", gpu.name, gpu.backend, gpu.kind);
        let _ = writeln!(s, "CPU: {} Threads; Ziel {} % des Pots", self.cpu_threads, de(self.target_pct, 1));
        for spot in &self.spots {
            let _ = writeln!(s, "{} {}: {}/{} Hände, {} Knoten", spot.name, spot.board, spot.hands[0], spot.hands[1], spot.nodes);
            let cpu = &spot.cpu;
            let _ = writeln!(
                s,
                "  CPU {} s, {} It., {} It./s, {} %",
                de(cpu.ms / 1000.0, 2),
                cpu.iterations,
                de(cpu.iterations_per_second, 0),
                de(cpu.exploitability_pct, 3)
            );
            match (&spot.gpu, &spot.gpu_error) {
                (Some(gpu), _) => {
                    let _ = writeln!(
                        s,
                        "  GPU {} s, {} It., {} It./s, {} %",
                        de(gpu.ms / 1000.0, 2),
                        gpu.iterations,
                        de(gpu.iterations_per_second, 0),
                        de(gpu.exploitability_pct, 3)
                    );
                    let _ = writeln!(
                        s,
                        "  Faktor je It. {} (bis Ziel {}), Spielwert-Abw. {} %, Strategie-Abw. {} %: {}",
                        de(spot.speedup_per_iteration.unwrap_or(0.0), 1),
                        de(spot.speedup.unwrap_or(0.0), 1),
                        de(spot.value_diff_pct.unwrap_or(0.0), 3),
                        de(spot.strategy_diff.unwrap_or(0.0) * 100.0, 1),
                        if spot.matches { "gleich" } else { "ABWEICHUNG" }
                    );
                }
                (None, error) => {
                    let _ = writeln!(s, "  GPU-Fehler: {}", error.as_deref().unwrap_or("unbekannt"));
                }
            }
        }
        s
    }
}
