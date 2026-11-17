use ringside_solver::library::format::{Stored, encode, quantize};
use ringside_solver::library::{Catalog, SpotId, catalog, check, solve_spot};
use ringside_solver::{NodeKind, Settings, Step, StreetSizes, solve};

const MEMORY: u64 = 4 << 30;

fn small() -> SpotId {
    let c = catalog();
    SpotId { scenario: c.scenario("3bp-btn-bb-40").unwrap(), flop: c.flop("Ks9h4d").unwrap_or(0) }
}

#[test]
fn quantized_frequencies_sum_to_255_and_keep_zeros() {
    for f in [vec![0.5, 0.5], vec![1.0, 0.0, 0.0], vec![0.333, 0.333, 0.334], vec![0.001, 0.999], vec![0.0, 0.0]] {
        let q = quantize(&f);
        assert_eq!(q.iter().map(|&x| x as u32).sum::<u32>(), 255, "{f:?}");
        for (x, y) in f.iter().zip(&q) {
            if *x == 0.0 && f.iter().any(|&v| v > 0.0) {
                assert_eq!(*y, 0, "{f:?}");
            }
        }
    }
}

#[test]
fn stored_spot_round_trips_and_views() {
    let c = catalog();
    let id = small();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let stored = solve_spot(c, id, threads, MEMORY, |_| {}, || false).unwrap();
    let bytes = stored.to_bytes();
    println!("{} {}: {} bytes", stored.scenario, stored.flop, bytes.len());
    let back = Stored::from_bytes(&bytes, c).unwrap();
    assert_eq!(back, stored);

    let view = ringside_solver::library::format::View::new(&back, c).unwrap();
    let root = view.node(&[]).unwrap();
    assert_eq!(root.kind, NodeKind::Action);
    let total: f64 = root.actions.iter().map(|a| a.frequency).sum();
    assert!((total - 1.0).abs() < 1e-6, "{total}");
    // Check-check reaches the turn; every turn card shows the first turn decision.
    let checks = view.node(&[Step::Action(0), Step::Action(0)]).unwrap();
    assert_eq!(checks.kind, NodeKind::Chance);
    let turn = view.node(&[Step::Action(0), Step::Action(0), Step::Card(checks.cards[0].clone())]).unwrap();
    assert_eq!(turn.kind, NodeKind::Action);
    assert!(!turn.classes.is_empty());
    assert!(view.node(&[Step::Action(0), Step::Action(0), Step::Card(checks.cards[0].clone()), Step::Action(0)]).is_err());

}

/// A catalog with one small single-raised pot, so solves take a fraction of a second.
fn tiny() -> (Catalog, SpotId) {
    let mut c = catalog().clone();
    let mut s = c.scenarios[c.scenario("srp-btn-bb-40").unwrap()].clone();
    s.id = "test".into();
    s.oop_range = "TT-77,AJs-ATs,KJs-KTs,QJs,JTs,T9s,98s,87s,AJo,KQo".into();
    s.ip_range = "AA-JJ,AQs+,KQs,AQo+,QTs,J9s,76s,65s".into();
    s.stack = 12.0;
    let one = |pct: f64| StreetSizes { bet: vec![pct], raise: Vec::new() };
    s.sizes = [one(50.0), one(75.0), one(75.0)];
    c.scenarios = vec![s];
    let flop = c.flop("Ks9h4d").unwrap_or(0);
    (c, SpotId { scenario: 0, flop })
}

#[test]
fn unconverged_flop_strategies_fail_the_check() {
    let (c, id) = tiny();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let scenario = &c.scenarios[0];
    let flop = &c.flops[id.flop];
    let good = solve_spot(&c, id, threads, MEMORY, |_| {}, || false).unwrap();
    let r = check(&good, &c, threads, MEMORY, 300, || false).unwrap();
    println!("tiny good: {:.3} % (stored {:.3} %)", r.exploitability_pct, good.exploitability_pct);
    assert!(r.exploitability_pct <= 1.0, "{}", r.exploitability_pct);

    // A flop strategy after one iteration is a well-formed spot, but far from solved.
    let rough = Settings { target_pct: 0.5, max_iterations: 1, threads };
    let mut early = solve(&scenario.spot(flop), &rough, MEMORY, |_| {}, || false).unwrap();
    let bad = encode(&mut early, &c, scenario, flop).unwrap();
    let bad = Stored::from_bytes(&bad.to_bytes(), &c).unwrap();
    let r = check(&bad, &c, threads, MEMORY, 300, || false).unwrap();
    println!("tiny bad: {:.3} % after {} iterations", r.exploitability_pct, r.iterations);
    assert!(r.exploitability_pct > 3.0, "{}", r.exploitability_pct);
}
