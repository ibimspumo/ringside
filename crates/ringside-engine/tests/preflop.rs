use ringside_engine::multiway;
use ringside_engine::preflop::{Kind, Params, Solution, Step, solve};
use ringside_engine::preflop_table::{self, PLAYERS, STACKS, TARGET_EXPLOITABILITY};

fn open_pct(s: &Solution, seat: usize) -> f64 {
    s.play_pct(s.open_node(seat).unwrap())
}

fn jam_pct(s: &Solution, seat: usize) -> f64 {
    s.kind_pct(s.open_node(seat).unwrap(), Kind::AllIn)
}

/// With stacks so short that every raise commits the stack, the model is the
/// push/fold game with heads-up all-ins, and both solvers agree.
#[test]
fn short_stacks_are_push_fold() {
    let mut p = Params::new(3, 4.0);
    p.limps = false;
    p.target_exploitability = 1e-4;
    let pre = solve(&p).unwrap();
    assert!(pre.converged, "NashConv {}", pre.exploitability);
    assert!(pre.nodes.iter().flat_map(|n| &n.actions).all(|a| a.kind != Kind::Raise));
    let mut q = multiway::Params::equal(3, 4.0);
    q.max_all_in = 2;
    q.target_exploitability = 1e-4;
    let pf = multiway::solve(&q).unwrap();
    for seat in 0..2 {
        let (a, b) = (jam_pct(&pre, seat), pf.act_pct(pf.node(seat, None, &[]).unwrap()));
        println!("seat {seat}: preflop model shoves {a:.2} %, push/fold {b:.2} %");
        assert!((a - b).abs() < 1.5, "seat {seat}: {a} vs {b}");
    }
    // Big blind against the small blind's shove.
    let shove = *pre.nodes[pre.open_node(1).unwrap()].actions.last().unwrap();
    let v = pre.node(2, &[Step { seat: 1, action: shove }]).unwrap();
    let (a, b) = (pre.kind_pct(v, Kind::Call), pf.act_pct(pf.node(2, Some(1), &[]).unwrap()));
    println!("BB calls the SB shove: {a:.2} % vs {b:.2} %");
    assert!((a - b).abs() < 1.5, "{a} vs {b}");
    // Deterministic: the same parameters give the same solution.
    assert_eq!(solve(&p).unwrap(), pre);
}

/// A big-blind ante adds dead money, so every seat opens wider; the model game is
/// solved to the target and chips are conserved up to the card-removal chain.
#[test]
fn ante_widens_opening_ranges() {
    let solve_with = |bb_ante: f64| {
        let mut p = Params::new(4, 30.0);
        p.bb_ante_bb = bb_ante;
        p.multiway_samples = 64;
        p.target_exploitability = 0.002;
        let s = solve(&p).unwrap();
        assert!(s.converged, "ante {bb_ante}: NashConv {}", s.exploitability);
        assert!(s.ev_sum_error.abs() < 0.02, "ante {bb_ante}: ΣEV {}", s.ev_sum_error);
        s
    };
    let (none, ante) = (solve_with(0.0), solve_with(1.0));
    for seat in 0..3 {
        let (a, b) = (open_pct(&none, seat), open_pct(&ante, seat));
        println!("seat {seat}: opens {a:.1} % without ante, {b:.1} % with BB ante");
        assert!(b > a + 1.0, "seat {seat}: {a} → {b}");
    }
}

/// The shipped tables: every depth is there and solved to the target, later seats
/// open wider, shorter stacks open all in more often, and the postflop model realizes
/// equity in the range published for an opener in position against the big blind.
#[test]
fn embedded_tables() {
    for &n in &PLAYERS {
        let entries = preflop_table::embedded_entries(n);
        assert_eq!(entries.len(), STACKS.len(), "{n} players");
        let info = preflop_table::file_info(preflop_table::embedded_file(n).unwrap()).unwrap();
        assert_eq!((info.players, info.entries), (n, STACKS.len()));
        let mut jams = Vec::new();
        for ((p, s), &stack) in entries.iter().zip(&STACKS) {
            let what = format!("{n} players {stack} BB");
            assert_eq!(*p, preflop_table::params(n, stack), "{what}");
            assert!(s.converged && s.exploitability < TARGET_EXPLOITABILITY, "{what}: {}", s.exploitability);
            // Later seats open wider: the button (n − 3) at least half as wide again as the
            // first seat, and no seat much narrower than the one before.
            let opens: Vec<f64> = (0..n - 2).map(|seat| open_pct(s, seat)).collect();
            assert!(opens[n - 3] > 1.5 * opens[0], "{what}: {opens:?}");
            for w in opens.windows(2) {
                assert!(w[1] > w[0] - 5.0, "{what}: {opens:?}");
            }
            jams.push((0..n - 1).map(|seat| jam_pct(s, seat)).sum::<f64>());
            // The most frequent single-raised pot of the big blind against an opener in position.
            let flop = s
                .realized
                .iter()
                .find(|r| r.seats[0] == n - 1 && r.seats[1] < n - 2 && r.line.len() == 2)
                .unwrap_or_else(|| panic!("{what}: no big blind defence"));
            assert!((0.65..0.9).contains(&flop.realization[0]), "{what}: BB realizes {}", flop.realization[0]);
            assert!((1.05..1.3).contains(&flop.realization[1]), "{what}: opener realizes {}", flop.realization[1]);
        }
        println!("{n} players, open all-in % summed over seats by stack: {jams:.1?}");
        assert!(jams[0] > jams[2] && jams[2] > jams[5], "{n} players: {jams:?}");
        assert_eq!(*jams.last().unwrap(), 0.0, "{n} players: open shoves at 100 BB");
    }
}

/// Comparisons for the docs; run with
/// `cargo test --release -p ringside-engine --test preflop -- --ignored --nocapture`.
#[test]
#[ignore = "measurement, about a minute in release"]
fn comparisons() {
    use ringside_engine::multiway::position_names;
    use ringside_engine::pushfold_table::{self, Ante};
    let opens = |s: &Solution, n: usize| -> String {
        let names = position_names(n);
        (0..n - 1).map(|seat| format!("{} {:.1}", names[seat], open_pct(s, seat))).collect::<Vec<_>>().join(", ")
    };
    // Published 6-max 100 BB cash charts (no ante): UTG ~14–18, HJ ~19–23, CO ~25–30, BTN ~40–45 %.
    let cash = solve(&Params::new(6, 100.0)).unwrap();
    println!("6-max 100 BB, no ante (NashConv {:.4}): {}", cash.exploitability, opens(&cash, 6));
    // Published 8-max 40 BB chip EV (ThinkGTO): UTG 17.3, LJ 23.6, HJ 28.5, CO 36.9, BTN 52 %.
    for (n, stack) in [(8, 40.0), (9, 40.0), (6, 40.0)] {
        let s = preflop_table::chart(n, stack).unwrap();
        println!("{n}-max {stack} BB, BB ante: {}", opens(&s, n));
    }
    // Near push/fold: open shoves and total opens at 15 BB against the push/fold tables.
    for &n in &PLAYERS {
        let s = preflop_table::chart(n, 15.0).unwrap();
        let pf = pushfold_table::chart(Ante::BigBlind, n, 15.0).unwrap();
        let names = position_names(n);
        let rows: Vec<String> = (0..n - 1)
            .map(|seat| {
                let shove = pf.act_pct(pf.node(seat, None, &[]).unwrap());
                format!("{} {:.1}/{:.1} vs {:.1}", names[seat], jam_pct(&s, seat), open_pct(&s, seat), shove)
            })
            .collect();
        println!("{n}-max 15 BB, model all-in/any open vs push/fold shove: {}", rows.join(", "));
    }
}
