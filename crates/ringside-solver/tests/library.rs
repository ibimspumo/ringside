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

#[test]
fn a_renamed_spot_is_the_same_spot_with_other_suits() {
    use ringside_solver::library::format::View;
    use ringside_solver::suits::between as suit_map;
    let (mut c, id) = tiny();
    let flop = c.flops[id.flop].clone();
    // Ks9h4d with spades and diamonds swapped.
    let other = "Kd9h4s".to_string();
    c.flops.push(other.clone());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let stored = solve_spot(&c, id, threads, MEMORY, |_| {}, || false).unwrap();
    let scenario = &c.scenarios[0];
    let renamed = stored.renamed(scenario, &other).unwrap();
    assert_eq!(renamed.flop, other);
    assert_eq!(Stored::from_bytes(&renamed.to_bytes(), &c).unwrap(), renamed, "the renamed spot reads back");
    assert_eq!(renamed.renamed(scenario, &flop).unwrap(), stored, "renaming back gives the original");

    let parse = |s: &str| -> Vec<ringside_engine::Card> { (0..s.len() / 2).map(|i| s[2 * i..2 * i + 2].parse().unwrap()).collect() };
    let perm = suit_map(&parse(&flop), &parse(&other)).unwrap();
    let swap = |text: &str| -> String {
        parse(text).iter().map(|c| ringside_engine::Card::new(c.rank(), perm[c.suit() as usize]).to_string()).collect()
    };
    let a = View::new(&stored, &c).unwrap();
    let b = View::new(&renamed, &c).unwrap();
    let path = [Step::Action(0)];
    let (na, nb) = (a.node(&path).unwrap(), b.node(&path).unwrap());
    assert_eq!(na.hands.len(), nb.hands.len());
    for h in &na.hands {
        let mapped = swap(&h.combo);
        let twin = nb.hands.iter().find(|x| parse(&x.combo).iter().all(|c| parse(&mapped).contains(c))).unwrap();
        assert_eq!(twin.strategy, h.strategy, "{} vs {}", h.combo, twin.combo);
        assert_eq!(twin.ev, h.ev);
    }
    let checks = a.node(&[Step::Action(0), Step::Action(0)]).unwrap();
    let card = checks.cards[5].clone();
    let ta = a.node(&[Step::Action(0), Step::Action(0), Step::Card(card.clone())]).unwrap();
    let tb = b.node(&[Step::Action(0), Step::Action(0), Step::Card(swap(&card))]).unwrap();
    assert_eq!(ta.classes.len(), tb.classes.len());
    for (x, y) in ta.classes.iter().zip(&tb.classes) {
        assert_eq!((x.class, &x.strategy), (y.class, &y.strategy));
    }
    assert!(stored.renamed(scenario, "AsKh2d").is_err(), "another flop is no renaming");
}
