use ringside_engine::{HandClass, PushFoldParams, PushFoldResult, solve_push_fold};

const MAX_GAIN: f64 = 0.001;

fn solve(stack: f64) -> PushFoldResult {
    solve_push_fold(&PushFoldParams::new(stack)).unwrap()
}

fn freq(freqs: &[f64], class: &str) -> f64 {
    freqs[class.parse::<HandClass>().unwrap().index()]
}

#[test]
fn equilibrium_is_unexploitable() {
    for stack in [2.0, 5.0, 10.0, 15.0, 20.0] {
        let r = solve(stack);
        println!(
            "{stack} BB: shove {:.2}% call {:.2}% SB EV {:+.5} BB, gains {:.1e}/{:.1e}, exact {}, {} iterations",
            r.shove_pct, r.call_pct, r.sb_ev_bb, r.sb_gain_bb, r.bb_gain_bb, r.exact, r.iterations
        );
        assert!(
            r.sb_gain_bb < MAX_GAIN && r.bb_gain_bb < MAX_GAIN,
            "{stack} BB"
        );
        assert!(r.exact, "{stack} BB: mixed frequencies not solved exactly");
        assert!(r.exploitability_bb() < 1e-9, "{stack} BB");
        let mixed = r
            .shove
            .iter()
            .chain(&r.call)
            .filter(|f| **f > 0.0 && **f < 1.0)
            .count();
        assert!(mixed <= 4, "{stack} BB: {mixed} mixed classes");
    }
}

#[test]
fn ten_bb_ranges_are_sane() {
    let r = solve(10.0);
    println!("10 BB shove: {}", r.shove_range());
    println!("10 BB call:  {}", r.call_range());
    assert!((50.0..65.0).contains(&r.shove_pct), "shove {}", r.shove_pct);
    assert!((30.0..45.0).contains(&r.call_pct), "call {}", r.call_pct);
    for hand in ["AA", "KK", "A2o", "K9o", "22", "T9s"] {
        assert_eq!(freq(&r.shove, hand), 1.0, "shove {hand}");
    }
    for hand in ["72o", "32o", "82o"] {
        assert_eq!(freq(&r.shove, hand), 0.0, "fold {hand}");
    }
    for hand in ["AA", "A2o", "55"] {
        assert_eq!(freq(&r.call, hand), 1.0, "call {hand}");
    }
    assert_eq!(freq(&r.call, "72o"), 0.0);
}

#[test]
fn tiny_stacks_shove_almost_everything() {
    for stack in [1.0, 1.25, 1.5] {
        let r = solve(stack);
        println!("{stack} BB: shove {:.2}%", r.shove_pct);
        assert!(r.shove_pct >= 95.0, "{stack} BB: {}", r.shove_pct);
    }
}

/// The equilibrium shove range is not strictly monotone: within short stack windows a
/// threshold hand can mix in more as the BB's calling range tightens (8.0 → 8.5 BB:
/// 61.86 % → 61.97 %; 17.0 → 17.5 BB: 42.92 % → 43.15 %). Those rises stay below one
/// percentage point; the trend is clearly downward.
fn check_shrinking(stacks: &[f64]) {
    let pcts: Vec<(f64, PushFoldResult)> = stacks.iter().map(|&s| (s, solve(s))).collect();
    for (i, (s, a)) in pcts.iter().enumerate() {
        assert!(a.exploitability_bb() < 1e-6, "{s} BB");
        for (t, b) in &pcts[i + 1..] {
            assert!(
                b.shove_pct <= a.shove_pct + 1.0,
                "{t} BB shoves {:.3}% vs {:.3}% at {s} BB",
                b.shove_pct,
                a.shove_pct
            );
            if t - s >= 3.0 {
                assert!(b.shove_pct < a.shove_pct, "{s} → {t} BB");
            }
        }
    }
}

#[test]
fn shove_range_shrinks_as_stacks_grow() {
    let stacks: Vec<f64> = (0..=37).map(|i| 1.5 + 0.5 * i as f64).collect();
    check_shrinking(&stacks);
}

#[test]
#[ignore = "241 solves; run with --release -- --ignored"]
fn shove_range_shrinks_on_a_fine_grid() {
    let stacks: Vec<f64> = (0..=240).map(|i| 1.0 + 0.1 * i as f64).collect();
    check_shrinking(&stacks);
}

#[test]
fn deterministic() {
    let a = solve(12.5);
    let b = solve(12.5);
    assert_eq!(a, b);
}

#[test]
fn antes_widen_ranges() {
    let plain = solve(10.0);
    let mut p = PushFoldParams::new(10.0);
    p.ante_bb = 0.125;
    let ante = solve_push_fold(&p).unwrap();
    assert!(ante.sb_gain_bb < MAX_GAIN && ante.bb_gain_bb < MAX_GAIN);
    assert!(ante.shove_pct > plain.shove_pct && ante.call_pct > plain.call_pct);
}

#[test]
fn rejects_bad_params() {
    assert!(solve_push_fold(&PushFoldParams::new(0.5)).is_err());
    let mut p = PushFoldParams::new(10.0);
    p.small_blind_bb = 0.0;
    assert!(solve_push_fold(&p).is_err());
}
