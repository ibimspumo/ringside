use std::time::Instant;

use ringside_solver::{ActionKind, Error, NodeKind, Player, Settings, Spot, Step, StreetSizes, estimate, solve, spawn};

const LIMIT: u64 = 1 << 30;

fn sizes(bet: &[f64], raise: &[f64]) -> StreetSizes {
    StreetSizes { bet: bet.to_vec(), raise: raise.to_vec() }
}

fn settings(target_pct: f64, threads: usize) -> Settings {
    Settings { target_pct, max_iterations: 3000, threads }
}

/// Pot-sized all-in on the river: OOP holds a set (77) or air (65), IP only bluff catchers (KQ).
/// The cards of the two ranges never overlap, so the textbook formulas hold exactly.
fn polarized_river() -> Spot {
    Spot {
        oop_range: "77,65".into(),
        ip_range: "KQ".into(),
        board: "Ks Th 7d 4c 2s".into(),
        pot: 10.0,
        stack: 10.0,
        sizes: [sizes(&[], &[]), sizes(&[], &[]), sizes(&[100.0], &[])],
        allin_threshold: 0.0,
    }
}

/// A small single-raised-pot flop with one size per street and no raises.
fn small_flop() -> Spot {
    Spot {
        oop_range: "AA,KK,QQ,JJ,AK,AQs,T9s".into(),
        ip_range: "88,77,AJ,AT,KQs,QJs,65s".into(),
        board: "Ah 8d 3c".into(),
        pot: 10.0,
        stack: 20.0,
        sizes: [sizes(&[50.0], &[]), sizes(&[75.0], &[]), sizes(&[100.0], &[])],
        allin_threshold: 0.0,
    }
}

#[test]
fn polarized_river_matches_the_indifference_formulas() {
    let spot = polarized_river();
    let mut solution = solve(&spot, &settings(0.05, 1), LIMIT, |_| {}, || false).unwrap();
    assert!(solution.summary.converged, "{:?}", solution.summary);

    let root = solution.node(&[]).unwrap();
    assert_eq!(root.player, Some(Player::Oop));
    let bet = root.actions.iter().position(|a| a.kind != ActionKind::Check).unwrap();
    assert_eq!(root.actions[bet].amount, 10.0);

    // OOP bets every set; air bets b / (P + b) = 1/2 as many combos as there are value combos.
    let sets = root.classes.iter().find(|c| c.label == "77").unwrap();
    assert_eq!(sets.combos, 3.0);
    assert!(sets.strategy[bet] > 0.99, "77 bets {}", sets.strategy[bet]);
    let bluffs: f64 = root.hands.iter().filter(|h| h.combo.starts_with('6')).map(|h| h.weight * h.strategy[bet]).sum();
    assert!((bluffs / 3.0 - 0.5).abs() < 0.05, "bluff to value ratio {}", bluffs / 3.0);

    // Air is indifferent between giving up (EV 0) and bluffing; the set wins P + P/(P+b) · b.
    let air = root.classes.iter().find(|c| c.label == "65o").unwrap();
    assert!(air.ev.abs() < 0.15, "air EV {}", air.ev);
    assert!((sets.ev - 15.0).abs() < 0.15, "set EV {}", sets.ev);

    // IP calls P / (P + b) = 1/2 of the time.
    let facing = solution.node(&[Step::Action(bet)]).unwrap();
    assert_eq!(facing.player, Some(Player::Ip));
    let call = facing.actions.iter().find(|a| a.kind == ActionKind::Call).unwrap();
    assert!((call.frequency - 0.5).abs() < 0.03, "call frequency {}", call.frequency);
    assert_eq!(facing.pot, 20.0);
}

#[test]
fn small_flop_converges() {
    let spot = small_flop();
    let mut seen = Vec::new();
    let started = Instant::now();
    let solution = solve(&spot, &settings(0.5, 2), LIMIT, |p| seen.push(p.exploitability_pct), || false).unwrap();
    let summary = &solution.summary;
    assert!(summary.converged, "{summary:?}");
    assert!(summary.exploitability_pct <= 0.5);
    assert!(seen[0] > 10.0 * summary.exploitability_pct, "{seen:?}");
    assert!(summary.iterations <= 500, "{summary:?}");
    eprintln!("small flop: {summary:?} in {:?}", started.elapsed());
}

#[test]
fn solves_are_deterministic() {
    let spot = Spot { board: "Ah 8d 3c Ks".into(), ..small_flop() };
    let run = || {
        let mut s = solve(&spot, &Settings { target_pct: 0.1, max_iterations: 60, threads: 2 }, LIMIT, |_| {}, || false)
            .unwrap();
        let root = s.node(&[]).unwrap();
        let after = s.node(&[Step::Action(1), Step::Action(1), Step::Card("2h".into())]).unwrap();
        (s.summary.exploitability, root, after)
    };
    let (a, b) = (run(), run());
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
}

#[test]
fn navigates_actions_and_cards() {
    let spot = Spot { board: "Ah 8d 3c Ks".into(), ..small_flop() };
    let mut s = solve(&spot, &settings(5.0, 1), LIMIT, |_| {}, || false).unwrap();
    let root = s.node(&[]).unwrap();
    assert_eq!(root.kind, NodeKind::Action);
    assert_eq!(root.board, ["Ah", "8d", "3c", "Ks"]);
    let total: f64 = root.actions.iter().map(|a| a.frequency).sum();
    assert!((total - 1.0).abs() < 1e-6);
    for hand in &root.hands {
        assert!((hand.strategy.iter().sum::<f64>() - 1.0).abs() < 1e-3, "{hand:?}");
        assert!((0.0..=1.0).contains(&hand.equity));
    }

    let river = s.node(&[Step::Action(0), Step::Action(0)]).unwrap();
    assert_eq!(river.kind, NodeKind::Chance);
    assert_eq!(river.cards.len(), 48);
    assert!(!river.cards.contains(&"Ks".to_string()));
    let dealt = s.node(&[Step::Action(0), Step::Action(0), Step::Card("Kd".into())]).unwrap();
    assert_eq!(dealt.board.len(), 5);
    assert_eq!(dealt.player, Some(Player::Oop));

    let fold = s.node(&[Step::Action(1), Step::Action(0)]).unwrap();
    assert_eq!(fold.kind, NodeKind::Terminal);
    for bad in [vec![Step::Card("2h".into())], vec![Step::Action(9)], vec![Step::Action(1), Step::Action(0), Step::Action(0)]] {
        assert!(matches!(s.node(&bad), Err(Error::Invalid(_))), "{bad:?}");
    }
    assert!(matches!(
        s.node(&[Step::Action(0), Step::Action(0), Step::Card("Ks".into())]),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn rejects_bad_spots_with_a_message() {
    let cases = [
        Spot { board: "Ah 8d".into(), ..small_flop() },
        Spot { board: "Ah Ah 3c".into(), ..small_flop() },
        Spot { oop_range: "AX".into(), ..small_flop() },
        Spot { ip_range: "AhAd".into(), board: "Ah Ad 3c".into(), ..small_flop() },
        Spot { pot: 0.0, ..small_flop() },
        Spot { sizes: [sizes(&[10.0, 20.0, 30.0, 40.0, 50.0], &[]), sizes(&[], &[]), sizes(&[], &[])], ..small_flop() },
    ];
    for spot in cases {
        match estimate(&spot, LIMIT, 1) {
            Err(Error::Invalid(m)) => assert!(!m.is_empty()),
            other => panic!("{spot:?} gave {other:?}"),
        }
    }
}

#[test]
fn memory_limit_is_checked_before_allocating() {
    let spot = small_flop();
    let e = estimate(&spot, LIMIT, 4).unwrap();
    assert!(e.fits && !e.compressed && e.memory_bytes > 0 && e.seconds > 0.0);
    // AA 3, KK/QQ/JJ 6 each, AK 12, AQs 3, T9s 4 against 88 3, 77 6, AJ/AT 12 each, three suited hands 4 each.
    assert_eq!(e.combos, [40.0, 45.0]);

    let tight = estimate(&spot, e.memory_bytes - 1, 4).unwrap();
    assert!(tight.compressed && tight.fits && tight.memory_bytes < e.memory_bytes);

    let limit = tight.memory_bytes / 2;
    match solve(&spot, &settings(1.0, 1), limit, |_| {}, || false) {
        Err(Error::TooLarge { needed, limit: l }) => assert!(needed > l && l == limit),
        other => panic!("expected TooLarge, got {:?}", other.map(|s| s.summary)),
    }
}

#[test]
fn stops_when_asked() {
    let result = solve(&small_flop(), &settings(0.01, 1), LIMIT, |_| {}, || true);
    assert!(matches!(result, Err(Error::Cancelled)));

    let (tx, rx) = std::sync::mpsc::channel();
    let job = spawn(small_flop(), settings(0.01, 1), LIMIT, |_| {}, || false, move |r| tx.send(r.map(|s| s.summary)).unwrap());
    job.cancel();
    job.wait();
    assert!(matches!(rx.recv().unwrap(), Err(Error::Cancelled)));
}

/// BTN against BB in a single-raised pot at 40 BB with two sizes per street. Run with
/// `cargo test --release -p ringside-solver -- --ignored --nocapture` for timings.
#[test]
#[ignore]
fn realistic_flop_spot() {
    let btn = "22+,A2s+,K2s+,Q4s+,J6s+,T6s+,96s+,85s+,75s+,64s+,54s,A2o+,K8o+,Q9o+,J9o+,T8o+,98o";
    let bb = "22-99,A2s-AJs,K2s-KJs,Q2s-QJs,J4s+,T6s+,96s+,85s+,74s+,63s+,53s+,43s,A2o-AJo,K8o-KJo,Q9o+,J9o+,T8o+,98o,87o";
    let street = || sizes(&[33.0, 75.0], &[60.0]);
    let spot = Spot {
        oop_range: bb.into(),
        ip_range: btn.into(),
        board: "Ks 7d 2c".into(),
        pot: 5.5,
        stack: 37.5,
        sizes: [street(), sizes(&[50.0, 100.0], &[60.0]), sizes(&[50.0, 100.0], &[60.0])],
        allin_threshold: 150.0,
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let e = estimate(&spot, 8 << 30, threads).unwrap();
    eprintln!("estimate: {e:?}");
    let started = Instant::now();
    let mut last = 0;
    let mut s = solve(&spot, &Settings { target_pct: 0.5, max_iterations: 1000, threads }, 8 << 30, |p| {
        if p.elapsed_ms >= last + 2000 {
            last = p.elapsed_ms;
            eprintln!("  {p:?}");
        }
    }, || false)
    .unwrap();
    eprintln!("solved: {:?} in {:?}", s.summary, started.elapsed());
    let t = Instant::now();
    let root = s.node(&[]).unwrap();
    eprintln!("root node in {:?}: {:?}", t.elapsed(), root.actions);
}
