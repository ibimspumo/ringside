use ringside_gpu::{BenchSpot, FlatTree, Gpu, GpuSolver, benchmark, compare};

fn threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get())
}

fn pool() -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(threads()).build().unwrap()
}

/// CI runners have no GPU; the test passes there without running.
fn gpu() -> Option<Gpu> {
    let gpu = Gpu::open();
    if gpu.is_none() {
        eprintln!("Keine GPU gefunden, Test übersprungen.");
    }
    gpu
}

#[test]
fn gpu_solves_a_river_like_the_cpu() {
    let Some(gpu) = gpu() else { return };
    let result = compare(&gpu, &small_river(), &pool(), &mut |_| {}, &|| false).unwrap();
    let run = result.gpu.as_ref().unwrap_or_else(|| panic!("{:?}", result.gpu_error));
    assert!(result.cpu.reached && run.reached, "{result:?}");
    assert!(result.matches, "{result:?}");
}

fn small_river() -> BenchSpot {
    BenchSpot {
        name: "Klein",
        oop_range: "AA,KK,QQ,JJ,TT,AK,AQs,KQs,QJs,JTs,T9s,98s,76s,65s",
        ip_range: "AA-77,AQ+,KQ,QJs,JTs,T9s,A5s-A2s",
        board: "Qh9d4s2c7h",
        pot: 1000,
        stack: 3000,
        turn: ("", ""),
        river: ("50%, 100%", "60%"),
    }
}

/// The app's GPU test with timings: `cargo test -p ringside-gpu --release -- --ignored --nocapture`.
#[test]
#[ignore]
fn full_benchmark() {
    let report = benchmark(threads(), &mut |s| eprintln!("{s}"), &|| false).unwrap();
    println!("{}", report.text());
}

/// Same algorithm, same numbers: the first two iterations agree up to rounding. Later on,
/// regrets that are exactly 0 on the CPU and ±1e-7 on the GPU flip indifferent hands to another
/// mix, so only exploitability and game value are compared after that.
#[test]
fn gpu_iterations_match_postflop_solver() {
    let Some(gpu) = gpu() else { return };
    for spot in [small_river(), small_turn()] {
        let cpu = spot.game().unwrap();
        let mut solver = GpuSolver::new(&gpu, FlatTree::new(&spot.game().unwrap()).unwrap()).unwrap();
        for t in 0..2 {
            postflop_solver::solve_step(&cpu, t);
            solver.run(1).unwrap();
            let gpu_sum = solver.strategy_sum().unwrap();
            let (mean, max) = solver.tree.strategy_difference(&solver.tree.read_strategy(&cpu), &gpu_sum);
            assert!(mean < 1e-5 && max < 1e-3, "{} iteration {t}: mean {mean:e}, max {max:e}", spot.name);
        }
    }
}

fn small_turn() -> BenchSpot {
    BenchSpot {
        name: "Turn klein",
        oop_range: "99-22,AJs-A2s,KJs-K5s,QJs-Q8s,J8s+,T8s+,97s+,86s+,75s+,65s,54s,AJo-A8o,KTo+,QTo+,JTo",
        ip_range: "22+,A2s+,K2s+,Q5s+,J7s+,T7s+,97s+,86s+,75s+,64s+,54s,A2o+,K9o+,Q9o+,J9o+,T9o",
        board: "QsJh7d2c",
        pot: 1000,
        stack: 4000,
        turn: ("50%, 100%", "60%"),
        river: ("50%, 100%", "60%"),
    }
}
