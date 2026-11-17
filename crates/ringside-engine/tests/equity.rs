mod common;

use ringside_engine::{Card, CardSet, EquityResult, Method, Range, equity};

use common::brute_force_equity;

fn ranges(specs: &[&str]) -> Vec<Range> {
    specs.iter().map(|s| Range::parse(s).unwrap()).collect()
}

fn set(s: &str) -> CardSet {
    CardSet::parse(s).unwrap()
}

fn exact(specs: &[&str], board: &str, dead: &str) -> EquityResult {
    let r = equity(&ranges(specs), set(board), set(dead), Method::Exact).unwrap();
    assert!(r.exact);
    r
}

fn hole(s: &str) -> [Card; 2] {
    let v = set(s).to_vec();
    [v[0], v[1]]
}

fn assert_sums_to_100(r: &EquityResult) {
    let total: f64 = r.players.iter().map(|p| p.equity_pct).sum();
    assert!((total - 100.0).abs() < 1e-9, "equities sum to {total}");
}

#[test]
fn exact_matches_brute_force_on_fixed_hands() {
    let spots: [(&[&str], &str, &str); 5] = [
        (&["AhKh", "QsQd"], "Jh7h2c", ""),
        (&["AhKh", "QsQd", "9c8c"], "Th7c2c", ""),
        (&["5c4c", "AsAd", "KhKs", "7d6d"], "8c3c2h", "Kd"),
        (&["AhAs", "AdAc"], "KdQh3s", ""),
        (&["2c2d", "7h6h", "JsTs", "AcKd"], "9s8h3d4c", "Qc"),
    ];
    for (players, board, dead) in spots {
        let r = exact(players, board, dead);
        assert_sums_to_100(&r);
        let holes: Vec<[Card; 2]> = players.iter().map(|p| hole(p)).collect();
        let reference = brute_force_equity(&holes, &set(board).to_vec(), &set(dead).to_vec());
        for (p, (w, t, e)) in r.players.iter().zip(reference) {
            assert!((p.win_pct - 100.0 * w).abs() < 1e-9, "{players:?} win");
            assert!((p.tie_pct - 100.0 * t).abs() < 1e-9, "{players:?} tie");
            assert!(
                (p.equity_pct - 100.0 * e).abs() < 1e-9,
                "{players:?} equity"
            );
        }
    }
}

#[test]
fn exact_matches_brute_force_with_ranges_and_dead_cards() {
    let specs = ["AA, KK, AKs", "QQ-TT, AQs", "JhTh, 9h8h"];
    let board = "Ah7h2d";
    let dead = "Kc";
    let r = exact(&specs, board, dead);
    assert_sums_to_100(&r);

    // Weighted brute force over all non-conflicting holdings.
    let known = set(board).union(set(dead));
    let live: Vec<Vec<_>> = ranges(&specs)
        .iter()
        .map(|r| r.live_combos(known))
        .collect();
    let mut acc = [0.0f64; 3];
    let mut total = 0.0;
    for &(a, wa) in &live[0] {
        for &(b, wb) in &live[1] {
            for &(c, wc) in &live[2] {
                let masks = [a.mask(), b.mask(), c.mask()];
                if masks[0].intersects(masks[1])
                    || masks[0].intersects(masks[2])
                    || masks[1].intersects(masks[2])
                {
                    continue;
                }
                let w = (wa * wb * wc) as f64;
                let holes = [a, b, c].map(|x| {
                    let (p, q) = x.cards();
                    [p, q]
                });
                let eq = brute_force_equity(&holes, &set(board).to_vec(), &set(dead).to_vec());
                for i in 0..3 {
                    acc[i] += w * eq[i].2;
                }
                total += w;
            }
        }
    }
    for i in 0..3 {
        let expected = 100.0 * acc[i] / total;
        assert!(
            (r.players[i].equity_pct - expected).abs() < 1e-9,
            "player {i}: {} vs {expected}",
            r.players[i].equity_pct
        );
    }
}

#[test]
fn weights_change_the_mix() {
    let full = exact(&["AA, 72o", "KK"], "", "").players[0].equity_pct;
    let half = exact(&["AA, 72o:0.5", "KK"], "", "").players[0].equity_pct;
    let aa_only = exact(&["AA", "KK"], "", "").players[0].equity_pct;
    assert!(full < half && half < aa_only, "{full} {half} {aa_only}");
}

#[test]
fn symmetric_spots_split_evenly() {
    for (players, board) in [
        (["AhKd", "AsKc"], ""),
        (["QQ", "QQ"], "2c7d9h"),
        (["random", "random"], "AsKsQs7d2c"),
    ] {
        let r = exact(&players, board, "");
        assert_sums_to_100(&r);
        assert!(
            (r.players[0].equity_pct - 50.0).abs() < 1e-9,
            "{players:?}: {:?}",
            r.players
        );
        assert!((r.players[0].win_pct - r.players[1].win_pct).abs() < 1e-9);
    }
}

#[test]
fn classic_preflop_values() {
    let r = exact(&["AhAs", "KdKc"], "", "");
    assert_sums_to_100(&r);
    let aa = r.players[0].equity_pct;
    assert!((81.0..83.0).contains(&aa), "AA vs KK: {aa}");
    assert_eq!(r.showdowns, 1_712_304);

    let r = exact(&["AKs", "QQ"], "", "");
    assert_sums_to_100(&r);
    let ak = r.players[0].equity_pct;
    assert!((45.0..47.0).contains(&ak), "AKs vs QQ: {ak}");
}

#[test]
#[ignore = "naive evaluator over 1.7M boards; run with --release -- --ignored"]
fn preflop_matches_brute_force() {
    for (a, b) in [("AhAs", "KdKc"), ("AhKh", "QsQd"), ("7c2d", "AsKs")] {
        let r = exact(&[a, b], "", "");
        let reference = brute_force_equity(&[hole(a), hole(b)], &[], &[]);
        for (p, (w, t, e)) in r.players.iter().zip(reference) {
            assert!((p.win_pct - 100.0 * w).abs() < 1e-9, "{a} vs {b}");
            assert!((p.tie_pct - 100.0 * t).abs() < 1e-9, "{a} vs {b}");
            assert!((p.equity_pct - 100.0 * e).abs() < 1e-9, "{a} vs {b}");
        }
        println!("{a} vs {b}: {:.4} %", r.players[0].equity_pct);
    }
}

#[test]
fn monte_carlo_agrees_with_exact() {
    let specs = [
        "22+, AJs+, KQs, AQo+",
        "JTs, T9s, 98s, 87s, KQo",
        "A2s+, K9s+, 55+, ATo+",
    ];
    let board = set("Ts9h3c");
    let rs = ranges(&specs);
    let ex = equity(&rs, board, CardSet::EMPTY, Method::Exact).unwrap();
    let samples = 400_000;
    let mc = equity(
        &rs,
        board,
        CardSet::EMPTY,
        Method::MonteCarlo { samples, seed: 42 },
    )
    .unwrap();
    assert!(!mc.exact);
    assert_eq!(mc.showdowns, samples);
    assert_sums_to_100(&ex);
    assert_sums_to_100(&mc);
    for (e, m) in ex.players.iter().zip(&mc.players) {
        // Five standard errors of a proportion.
        let p = e.equity_pct / 100.0;
        let tol = 100.0 * 5.0 * (p * (1.0 - p) / samples as f64).sqrt();
        assert!(
            (e.equity_pct - m.equity_pct).abs() < tol,
            "exact {e:?} vs mc {m:?}"
        );
        let pw = e.win_pct / 100.0;
        let tol_w = 100.0 * 5.0 * (pw * (1.0 - pw) / samples as f64).sqrt();
        assert!((e.win_pct - m.win_pct).abs() < tol_w);
    }
    let again = equity(
        &rs,
        board,
        CardSet::EMPTY,
        Method::MonteCarlo { samples, seed: 42 },
    )
    .unwrap();
    assert_eq!(again, mc, "Monte Carlo must be reproducible for a seed");
}

#[test]
fn auto_picks_exact_for_small_work() {
    let rs = ranges(&["AhKh", "QsQd"]);
    let auto = Method::Auto {
        max_exact_showdowns: 10_000_000,
        samples: 1000,
        seed: 1,
    };
    assert!(
        equity(&rs, set("2c3d4h"), CardSet::EMPTY, auto)
            .unwrap()
            .exact
    );
    let rs = ranges(&["random", "random", "random"]);
    assert!(
        !equity(&rs, CardSet::EMPTY, CardSet::EMPTY, auto)
            .unwrap()
            .exact
    );
}

#[test]
fn rejects_invalid_spots() {
    let two = ranges(&["AA", "KK"]);
    assert!(equity(&two[..1], CardSet::EMPTY, CardSet::EMPTY, Method::Exact).is_err());
    assert!(equity(&two, set("2c3d"), CardSet::EMPTY, Method::Exact).is_err());
    assert!(equity(&two, set("2c3d4h"), set("2c"), Method::Exact).is_err());
    assert!(equity(&two, CardSet::EMPTY, set("AsAhAdAc"), Method::Exact).is_err());
    let blocked = ranges(&["AhAs", "AhAs"]);
    assert!(equity(&blocked, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).is_err());
    let ten = vec![Range::all(); 10];
    assert!(equity(&ten, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).is_err());
}

#[test]
fn nine_players_monte_carlo() {
    let rs = vec![Range::all(); 9];
    let r = equity(
        &rs,
        CardSet::EMPTY,
        CardSet::EMPTY,
        Method::MonteCarlo {
            samples: 90_000,
            seed: 3,
        },
    )
    .unwrap();
    assert_sums_to_100(&r);
    for p in &r.players {
        assert!((p.equity_pct - 100.0 / 9.0).abs() < 1.0, "{p:?}");
    }
}
