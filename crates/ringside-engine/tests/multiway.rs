use ringside_engine::multiway::{Params, Solution, evaluate, seat_name, solve};
use ringside_engine::{PushFoldParams, solve_push_fold};

fn open_pct(s: &Solution, seat: usize) -> f64 {
    s.act_pct(s.node(seat, None, &[]).unwrap())
}

fn check_converged(s: &Solution, what: &str) {
    println!(
        "{what}: NashConv {:.2e} after {} iterations, 3-way pots {:.4}, capped {:.4}, ΣEV error {:+.4}",
        s.exploitability,
        s.iterations,
        s.approximation.multiway_probability,
        s.approximation.capped_probability,
        s.approximation.ev_sum_error
    );
    assert!(s.converged, "{what}: NashConv {} after {} iterations", s.exploitability, s.iterations);
}

/// Two players are the heads-up game, and the pairwise card removal is exact there.
#[test]
fn heads_up_matches_the_two_player_solver() {
    for (stack, ante, dead) in [(10.0, 0.0, 0.0), (5.0, 0.1, 0.0), (15.0, 0.0, 0.5), (20.0, 0.125, 0.25)] {
        let mut hu = PushFoldParams::new(stack);
        hu.ante_bb = ante;
        hu.dead_bb = dead;
        let hu = solve_push_fold(&hu).unwrap();
        let mut p = Params::equal(2, stack);
        p.ante_bb = ante;
        p.dead_bb = dead;
        p.target_exploitability = 1e-6;
        p.max_iterations = 50_000;
        let s = solve(&p).unwrap();
        check_converged(&s, &format!("HU {stack} BB, ante {ante}, dead {dead}"));
        let (shove, call) = (&s.nodes[0], &s.nodes[1]);
        assert_eq!((shove.seat, call.seat, call.shover), (0, 1, Some(0)));
        assert!((s.ev[0] - hu.sb_ev_bb).abs() < 1e-5, "{stack}: EV {} vs {}", s.ev[0], hu.sb_ev_bb);
        assert!((s.act_pct(0) - hu.shove_pct).abs() < 0.1, "{stack}: shove {} vs {}", s.act_pct(0), hu.shove_pct);
        assert!((s.act_pct(1) - hu.call_pct).abs() < 0.1, "{stack}: call {} vs {}", s.act_pct(1), hu.call_pct);
        for c in 0..169 {
            for (node, freq, ev) in [(shove, hu.shove[c], hu.sb_class_ev[c]), (call, hu.call[c], hu.bb_class_ev[c])] {
                // Same ranges; where the heads-up gap is not tiny, at most a trace of
                // the early CFR+ iterations is left in the average.
                assert_eq!(node.act[c] >= 0.5, freq >= 0.5, "{stack} BB seat {} class {c}", node.seat);
                if (ev.act - ev.fold).abs() > 1e-3 {
                    assert!((node.act[c] - freq).abs() <= 0.01, "{stack} BB seat {} class {c}", node.seat);
                }
                // The heads-up solver mixes its threshold classes exactly, CFR+ averages
                // them, which moves single class EVs of the other player a little.
                if ev.act != 0.0 || ev.fold != 0.0 {
                    assert!((node.ev_act[c] - ev.act).abs() < 5e-3, "{stack} seat {} {c}: {ev:?}", node.seat);
                    assert!((node.ev_fold[c] - ev.fold).abs() < 5e-3, "{stack} seat {} {c}: {ev:?}", node.seat);
                }
            }
        }
    }
}

#[test]
fn three_six_and_nine_players_converge() {
    for (players, stack, ante) in [(3, 10.0, 0.0), (6, 15.0, 0.125), (9, 10.0, 0.0)] {
        let mut p = Params::equal(players, stack);
        p.ante_bb = ante;
        let s = solve(&p).unwrap();
        check_converged(&s, &format!("{players} players {stack} BB ante {ante}"));
        assert!(s.approximation.multiway_monte_carlo);
        assert!(s.approximation.ev_sum_error.abs() < 0.05);
    }
}

#[test]
fn unequal_stacks_and_icm_final_table_converge() {
    let mut p = Params::new(vec![12.0, 4.0, 25.0, 8.0, 15.0, 6.0]);
    p.ante_bb = 0.1;
    check_converged(&solve(&p).unwrap(), "6 unequal stacks, ante");
    let p = Params::new(vec![14.0, 9.0, 22.0, 6.0, 11.0, 17.0, 8.0, 13.0, 20.0])
        .with_payouts(vec![30.0, 20.0, 14.0, 10.0, 8.0, 6.0, 5.0, 4.0, 3.0]);
    let s = solve(&p).unwrap();
    check_converged(&s, "9-handed final table, ICM");
    // The prize pool is all that is conserved; the pairwise model is off by a little.
    assert!(s.icm && s.approximation.ev_sum_error.abs() < 0.05, "{}", s.approximation.ev_sum_error);
}

/// Six even stacks on the bubble (five paid), as the community queue seeds it: plain
/// regret matching+ cycled here, its average stalled at three times the target.
#[test]
fn a_bubble_where_regret_matching_cycled_converges() {
    for (stack, ante, bb_ante) in [(15.0, 0.0, 1.0), (12.0, 0.125, 0.0)] {
        let p = Params { ante_bb: ante, bb_ante_bb: bb_ante, max_all_in: 2, ..Params::equal(6, stack) }
            .with_payouts(vec![5.0, 4.0, 3.0, 2.0, 1.0]);
        check_converged(&solve(&p).unwrap(), &format!("6p {stack} BB bubble"));
    }
}

/// Every configuration of the brief: 3, 6 and 9 players, 5–20 BB, with and without
/// antes, unequal stacks, ICM.
#[test]
#[ignore = "about two minutes in release; run with --release -- --ignored"]
fn exploitability_sweep() {
    for players in [3, 6, 9] {
        for stack in [5.0, 10.0, 20.0] {
            for ante in [0.0, 0.125] {
                let mut p = Params::equal(players, stack);
                p.ante_bb = ante;
                check_converged(&solve(&p).unwrap(), &format!("{players}p {stack} BB ante {ante}"));
            }
        }
        let stacks: Vec<f64> = (0..players).map(|i| 5.0 + (i * 7 % 16) as f64).collect();
        let mut p = Params::new(stacks.clone());
        p.bb_ante_bb = 1.0;
        check_converged(&solve(&p).unwrap(), &format!("{players}p {stacks:?}, BB ante"));
        let payouts: Vec<f64> = [50.0, 30.0, 20.0, 10.0, 8.0, 6.0, 5.0, 4.0, 3.0][..players].to_vec();
        let p = Params::new(stacks.clone()).with_payouts(payouts);
        check_converged(&solve(&p).unwrap(), &format!("{players}p {stacks:?}, ICM"));
    }
}

/// Ganzfried & Sandholm, "Computing an Approximate Jam/Fold Equilibrium for 3-player
/// No-Limit Texas Hold'em Tournaments" (AAMAS 2008), table 2, "single hand": three
/// stacks of 4500, blinds 300/600 (7.5 BB), no ante, chip EV.
#[test]
fn three_players_match_ganzfried_sandholm() {
    let mut p = Params::equal(3, 7.5);
    p.target_exploitability = 1e-4;
    let s = solve(&p).unwrap();
    let pct = |seat, shover, callers: &[usize]| s.act_pct(s.node(seat, shover, callers).unwrap());
    // (ours, published, tolerance in percentage points)
    let rows = [
        ("BTN jam", pct(0, None, &[]), 35.5, 1.0),
        ("SB call vs BTN", pct(1, Some(0), &[]), 23.4, 1.0),
        ("SB jam", pct(1, None, &[]), 63.3, 1.0),
        ("BB call vs BTN", pct(2, Some(0), &[]), 30.8, 1.0),
        ("BB call vs SB", pct(2, Some(1), &[]), 46.6, 1.0),
        // Their fictitious play stopped at ε = 0.01 chips; this rare node is the loosest.
        ("BB overcall", pct(2, Some(0), &[1]), 19.7, 3.0),
    ];
    for (what, ours, theirs, tol) in rows {
        println!("{what}: {ours:.1} % vs {theirs} %");
        assert!((ours - theirs).abs() <= tol, "{what}: {ours} vs {theirs}");
    }
}

/// trainingsharks.com/push-fold/icm: blinds 0.5/1, no ante, equal 10 BB stacks,
/// Malmuth–Harville ICM. Their model treats folded players as random hands and a
/// split as half a win; with those two choices this solver gives 64.6 / 23.2 / 11.6 /
/// 2.7 %. The defaults here (bunching after folds, exact ties) move the numbers by up
/// to one percentage point.
#[test]
fn icm_spots_match_trainingsharks() {
    for (players, payouts, sb, bb) in [
        (3, vec![50.0, 30.0, 20.0], 64.5, 23.3),
        (4, vec![50.0, 30.0, 20.0], 100.0, 11.6),
        (4, vec![1.0, 1.0, 1.0], 100.0, 2.7),
    ] {
        let s = solve(&Params::equal(players, 10.0).with_payouts(payouts.clone())).unwrap();
        let seat = players - 2;
        let (our_sb, our_bb) = (open_pct(&s, seat), s.act_pct(s.node(seat + 1, Some(seat), &[]).unwrap()));
        println!("{players} players {payouts:?}: SB {our_sb:.1} vs {sb}, BB {our_bb:.1} vs {bb}");
        assert!((our_sb - sb).abs() <= 1.0 && (our_bb - bb).abs() <= 1.0);
    }
}

#[test]
fn later_positions_shove_wider() {
    let s = solve(&Params::equal(6, 10.0)).unwrap();
    let pcts: Vec<f64> = (0..5).map(|seat| open_pct(&s, seat)).collect();
    println!("6 players 10 BB open shoves: {pcts:.1?}");
    assert!(pcts.windows(2).all(|w| w[0] < w[1]), "{pcts:?}");
}

/// As heads-up, single threshold classes may mix a little more at a deeper stack, so
/// the range as a whole is compared, with the same one-point allowance.
#[test]
fn shorter_stacks_shove_wider() {
    let solutions: Vec<Solution> = [6.0, 10.0, 15.0, 20.0].iter().map(|&b| solve(&Params::equal(6, b)).unwrap()).collect();
    for seat in 0..5 {
        let pcts: Vec<f64> = solutions.iter().map(|s| open_pct(s, seat)).collect();
        println!("{} open shove at 6/10/15/20 BB: {pcts:.1?}", seat_name(6, seat));
        assert!(pcts.windows(2).all(|w| w[1] < w[0] + 1.0), "{pcts:?}");
        assert!(pcts[3] < pcts[0], "{pcts:?}");
    }
}

/// Without card removal every player sees the same deal distribution, so chips are
/// conserved exactly; winner-take-all ICM is linear in chips and must give the same
/// strategies as chip EV, with EVs mapped affinely.
#[test]
fn conservation_and_linear_icm() {
    let mut p = Params::new(vec![10.0, 14.0, 6.0, 12.0]);
    p.ante_bb = 0.1;
    p.card_removal = false;
    let chips = solve(&p).unwrap();
    // Left: the f32 equity matrix (antisymmetric to 1e-7) and f32 showdown tables.
    assert!(chips.approximation.ev_sum_error.abs() < 1e-6, "{}", chips.approximation.ev_sum_error);

    let mut p = Params::new(vec![10.0, 14.0, 6.0, 12.0]);
    p.ante_bb = 0.1;
    p.target_exploitability = 1e-4;
    let chips = solve(&p).unwrap();
    let total: f64 = p.stacks_bb.iter().sum();
    let mut icm = p.clone().with_payouts(vec![1.0]);
    icm.target_exploitability = p.target_exploitability / total;
    let icm = solve(&icm).unwrap();
    // Rounding lets mixed classes drift apart; pure decisions with a clear gap and
    // the ranges as a whole agree.
    for (i, (a, b)) in chips.nodes.iter().zip(&icm.nodes).enumerate() {
        assert!((chips.act_pct(i) - icm.act_pct(i)).abs() < 0.3, "node {i}");
        for c in 0..169 {
            if (a.ev_act[c] - a.ev_fold[c]).abs() > 1e-2 && a.reach > 1e-3 {
                assert!((a.act[c] - b.act[c]).abs() < 1e-3, "node {i} class {c}: {} vs {}", a.act[c], b.act[c]);
            }
        }
    }
    for (seat, (e_chips, e_icm)) in chips.ev.iter().zip(&icm.ev).enumerate() {
        let mapped = (p.stacks_bb[seat] + e_chips) / total;
        assert!((mapped - e_icm).abs() < 1e-4, "seat {seat}: {mapped} vs {e_icm}");
    }
}

#[test]
fn deterministic() {
    let mut p = Params::new(vec![9.0, 12.0, 7.0, 15.0, 10.0]);
    p.ante_bb = 0.12;
    assert_eq!(solve(&p).unwrap(), solve(&p).unwrap());
}

#[test]
fn spots_explain_the_strategy() {
    let s = solve(&Params::equal(6, 10.0)).unwrap();
    // Folded to the big blind's decision after an UTG shove and a call.
    let aa = s.spot(5, "AA".parse().unwrap(), Some(0), &[2]).unwrap();
    assert_eq!(aa.freq, 1.0);
    assert!(aa.ev_act > aa.ev_fold && aa.reach > 0.0, "{aa:?}");
    let trash = s.spot(0, "72o".parse().unwrap(), None, &[]).unwrap();
    assert_eq!(trash.freq, 0.0);
    assert!((trash.ev_fold - 0.0).abs() < 1e-12, "UTG folds for free: {trash:?}");
    for (i, node) in s.nodes.iter().enumerate() {
        for c in 0..169 {
            let (f, gap) = (node.act[c], node.ev_act[c] - node.ev_fold[c]);
            if node.reach > 1e-3 && f == 1.0 {
                assert!(gap > -1e-2, "node {i} class {c}: shoves at a loss of {gap}");
            }
        }
    }
    assert!(s.spot(5, "AA".parse().unwrap(), Some(5), &[]).is_none());
}

#[test]
fn results_serialize() {
    let s = solve(&Params::equal(3, 8.0)).unwrap();
    let text = serde_json::to_string(&s).unwrap();
    let back: Solution = serde_json::from_str(&text).unwrap();
    assert_eq!(back.nodes.len(), s.nodes.len());
    // serde_json's default float parser may be off by one ulp.
    for (a, b) in back.nodes.iter().zip(&s.nodes) {
        assert!(a.act.iter().zip(&b.act).all(|(x, y)| (x - y).abs() <= 1e-15));
        assert_eq!((a.seat, a.shover, &a.callers), (b.seat, b.shover, &b.callers));
    }
    let p: Params = serde_json::from_str(r#"{"stacks_bb":[10,10,10],"small_blind_bb":0.5,"target_exploitability":0.001,"max_iterations":100}"#).unwrap();
    assert_eq!((p.max_all_in, p.multiway_samples, p.card_removal), (3, 1024, true));
}

#[test]
fn rejects_bad_params() {
    assert!(solve(&Params::equal(1, 10.0)).is_err());
    assert!(solve(&Params::equal(10, 10.0)).is_err());
    let mut p = Params::equal(3, 10.0);
    p.stacks_bb[2] = 0.8;
    assert!(solve(&p).is_err(), "big blind can't be posted");
    let mut p = Params::equal(3, 10.0);
    p.max_all_in = 1;
    assert!(solve(&p).is_err());
    let p = Params::equal(3, 10.0).with_payouts(vec![1.0; 4]);
    assert!(solve(&p).is_err());
}

/// Checking a strategy from elsewhere: the solver's own answer passes with the same
/// exploitability, a tampered one shows the loss, malformed input is refused.
#[test]
fn evaluate_measures_a_given_strategy() {
    let p = Params::equal(4, 10.0);
    let s = solve(&p).unwrap();
    let act: Vec<Vec<f64>> = s.nodes.iter().map(|n| n.act.clone()).collect();
    let same = evaluate(&p, &act).unwrap();
    assert!((same.exploitability - s.exploitability).abs() < 1e-9, "{} vs {}", same.exploitability, s.exploitability);
    assert_eq!((same.iterations, same.converged), (0, true));

    let mut tampered = act.clone();
    let aa = ringside_engine::range::HandClass::all().position(|c| c.to_string() == "AA").unwrap();
    tampered[0][aa] = 0.0; // the first seat folds aces
    let worse = evaluate(&p, &tampered).unwrap();
    assert!(worse.exploitability > s.exploitability + 1e-3, "{}", worse.exploitability);
    assert!(!worse.converged);

    assert!(evaluate(&p, &act[1..]).is_err());
    let mut out_of_range = act;
    out_of_range[0][0] = 1.5;
    assert!(evaluate(&p, &out_of_range).is_err());
}
