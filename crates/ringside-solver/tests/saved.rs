use ringside_solver::saved::{NOT_STORED, Snapshot, good_enough, spot_key};
use ringside_solver::{Error, Node, NodeKind, Settings, Spot, Step, StreetSizes, solve};

const LIMIT: u64 = 1 << 30;

fn sizes(bet: &[f64], raise: &[f64]) -> StreetSizes {
    StreetSizes { bet: bet.to_vec(), raise: raise.to_vec() }
}

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

fn close(a: f64, b: f64, tol: f64, what: &str, path: &[Step]) {
    assert!((a - b).abs() <= tol, "{what} at {path:?}: {a} vs {b}");
}

fn same(saved: &Node, live: &Node) {
    let p = &live.path;
    assert_eq!(saved.kind, live.kind, "{p:?}");
    assert_eq!(saved.street, live.street, "{p:?}");
    assert_eq!(saved.board, live.board, "{p:?}");
    assert_eq!(saved.player, live.player, "{p:?}");
    assert_eq!(saved.cards, live.cards, "{p:?}");
    for i in 0..2 {
        close(saved.pot, live.pot, 1e-9, "pot", p);
        close(saved.bets[i], live.bets[i], 1e-9, "bets", p);
        close(saved.stacks[i], live.stacks[i], 1e-9, "stacks", p);
        close(saved.street_bets[i], live.street_bets[i], 1e-9, "street bets", p);
        close(saved.players[i].equity, live.players[i].equity, 1e-5, "player equity", p);
        close(saved.players[i].ev, live.players[i].ev, 1e-4, "player ev", p);
    }
    assert_eq!(saved.actions.len(), live.actions.len(), "{p:?}");
    for (s, l) in saved.actions.iter().zip(&live.actions) {
        assert_eq!(s.kind, l.kind, "{p:?}");
        close(s.amount, l.amount, 1e-9, "amount", p);
        close(s.frequency, l.frequency, 0.01, "frequency", p);
    }
    assert_eq!(saved.hands.len(), live.hands.len(), "{p:?}");
    let scale = live.hands.iter().map(|h| h.ev.abs()).fold(1.0f64, f64::max);
    for (s, l) in saved.hands.iter().zip(&live.hands) {
        assert_eq!(s.combo, l.combo, "{p:?}");
        close(s.weight, l.weight, 2e-5, "weight", p);
        close(s.equity, l.equity, 2e-5, "equity", p);
        close(s.ev, l.ev, scale * 1e-4, "ev", p);
        for (a, b) in s.strategy.iter().zip(&l.strategy) {
            close(*a, *b, 1.0 / 255.0 + 1e-9, "strategy", p);
        }
    }
    assert_eq!(saved.classes.len(), live.classes.len(), "{p:?}");
    for (s, l) in saved.classes.iter().zip(&live.classes) {
        assert_eq!(s.label, l.label, "{p:?}");
        close(s.equity, l.equity, 2e-5, "class equity", p);
    }
}

/// Every node a player can reach on the stored streets looks the same after saving and
/// loading; one card deeper the snapshot says what it does not hold.
#[test]
fn a_saved_solution_navigates_like_the_solved_one() {
    let spot = small_flop();
    let settings = Settings { target_pct: 1.0, max_iterations: 400, threads: 2 };
    let mut solution = solve(&spot, &settings, LIMIT, |_| {}, || false).unwrap();
    let snapshot = Snapshot::take(&mut solution, &settings).unwrap();
    let bytes = snapshot.to_bytes();
    println!("small flop: {} bytes on disk, {} bytes of tree", bytes.len(), solution.summary.memory_bytes);
    let loaded = Snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(Snapshot::meta(&bytes).unwrap(), loaded.meta);
    assert_eq!(loaded.meta.spot, spot);

    // Breadth first through the flop and onto every turn card.
    let mut queue = vec![Vec::<Step>::new()];
    let mut checked = 0;
    let mut turn_roots = 0;
    while let Some(path) = queue.pop() {
        let live = solution.node(&path).unwrap();
        same(&loaded.node(&path).unwrap(), &live);
        checked += 1;
        let dealt = path.iter().any(|s| matches!(s, Step::Card(_)));
        if dealt {
            turn_roots += 1;
            if live.kind == NodeKind::Action {
                let mut deeper = path.clone();
                deeper.push(Step::Action(0));
                assert_eq!(loaded.node(&deeper).unwrap_err(), Error::Invalid(NOT_STORED.into()));
            }
            continue;
        }
        match live.kind {
            NodeKind::Action => queue.extend((0..live.actions.len()).map(|a| [path.clone(), vec![Step::Action(a)]].concat())),
            NodeKind::Chance => queue.extend(live.cards.iter().map(|c| [path.clone(), vec![Step::Card(c.clone())]].concat())),
            NodeKind::Terminal => {}
        }
    }
    assert!(turn_roots >= 49, "only {turn_roots} turn roots");
    assert!(checked > turn_roots);
    assert!(loaded.node(&[Step::Action(9)]).is_err());
    assert!(Snapshot::from_bytes(&bytes[..bytes.len() - 10]).is_err());
}

#[test]
fn equal_spots_share_a_key_and_reuse_needs_the_same_precision() {
    let a = small_flop();
    let mut b = a.clone();
    b.board = "8d3cAh".into();
    // Flop card order and range notation order do not matter.
    b.oop_range = "KK,AA,QQ,JJ,AK,AQs,T9s".into();
    assert_eq!(spot_key(&a).unwrap(), spot_key(&b).unwrap());
    let mut c = a.clone();
    c.sizes[1].bet = vec![66.0];
    assert_ne!(spot_key(&a).unwrap(), spot_key(&c).unwrap());
    let mut turn = a.clone();
    turn.board = "Ah 8d 3c 2s".into();
    let mut turn2 = turn.clone();
    turn2.sizes[0].bet = vec![33.0];
    assert_eq!(spot_key(&turn).unwrap(), spot_key(&turn2).unwrap());
    let mut turn3 = turn.clone();
    turn3.board = "8d Ah 3c 2s".into();
    assert_eq!(spot_key(&turn).unwrap(), spot_key(&turn3).unwrap());
    let mut turn4 = turn.clone();
    turn4.board = "Ah 8d 2s 3c".into();
    assert_ne!(spot_key(&turn).unwrap(), spot_key(&turn4).unwrap());

    let settings = Settings { target_pct: 2.0, max_iterations: 200, threads: 1 };
    let river = Spot {
        oop_range: "77,65".into(),
        ip_range: "KQ".into(),
        board: "Ks Th 7d 4c 2s".into(),
        pot: 10.0,
        stack: 10.0,
        sizes: [sizes(&[], &[]), sizes(&[], &[]), sizes(&[100.0], &[])],
        allin_threshold: 0.0,
    };
    let mut solution = solve(&river, &settings, LIMIT, |_| {}, || false).unwrap();
    let meta = Snapshot::take(&mut solution, &settings).unwrap().meta;
    let reached = meta.summary.exploitability_pct;
    assert!(reached <= 2.0);
    assert!(good_enough(&meta, 2.0) && good_enough(&meta, 5.0));
    assert!(!good_enough(&meta, reached * 0.5), "a more exact request solves again");
}

/// Sizes of real spots, measured on demand: `cargo test --release -p ringside-solver
/// --test saved -- --ignored --nocapture`.
#[test]
#[ignore]
fn measure_snapshot_sizes() {
    let srp = |board: &str| Spot {
        oop_range: "22-QQ,A2s-AKs,K9s-KQs,Q9s-QJs,J9s-JTs,T8s-T9s,97s-98s,86s-87s,75s-76s,64s-65s,54s,ATo-AQo,KTo-KQo,QTo-QJo,JTo".into(),
        ip_range: "22-JJ,A2s-AQs,K5s-KQs,Q8s-QJs,J8s-JTs,T7s-T9s,96s-98s,85s-87s,75s-76s,64s-65s,54s,A8o-AQo,KTo-KQo,QTo-QJo,JTo".into(),
        board: board.into(),
        pot: 5.5,
        stack: 37.5,
        sizes: [sizes(&[33.0, 75.0], &[60.0]), sizes(&[66.0], &[60.0]), sizes(&[75.0], &[60.0])],
        allin_threshold: 67.0,
    };
    for board in ["Ks 7d 2c 4h 9s", "Ks 7d 2c 4h", "Ks 7d 2c"] {
        let spot = srp(board);
        let settings = Settings { target_pct: 1.0, max_iterations: 300, threads: 8 };
        let started = std::time::Instant::now();
        let mut solution = solve(&spot, &settings, 16 << 30, |_| {}, || false).unwrap();
        let solved = started.elapsed();
        let started = std::time::Instant::now();
        let snapshot = Snapshot::take(&mut solution, &settings).unwrap();
        let bytes = snapshot.to_bytes();
        let taken = started.elapsed();
        let started = std::time::Instant::now();
        Snapshot::from_bytes(&bytes).unwrap();
        println!(
            "{board}: tree {:.0} MB, saved {:.0} KB (solve {solved:.1?}, save {taken:.1?}, load {:.1?})",
            solution.summary.memory_bytes as f64 / 1e6,
            bytes.len() as f64 / 1e3,
            started.elapsed()
        );
    }
}
