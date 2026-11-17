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

fn solve_with(stack: f64, ante: f64, dead: f64) -> PushFoldResult {
    let mut p = PushFoldParams::new(stack);
    p.ante_bb = ante;
    p.dead_bb = dead;
    solve_push_fold(&p).unwrap()
}

/// Bit patterns of the solver before dead money existed: (stack, ante, SB EV, hash of
/// all frequencies, iterations).
#[test]
fn zero_dead_money_reproduces_the_previous_solver() {
    let gold: [(f64, f64, u64, u64, u32); 4] = [
        (10.0, 0.0, 0xbfa73ad5dc659387, 0xd57d505804f668dc, 0x5c0),
        (10.0, 0.125, 0xbfa97752722b8801, 0xa87cc107221d315f, 0x600),
        (6.5, 0.1, 0x3f9d69a9f0f3f680, 0x9ac90960c6d5949a, 0x780),
        (17.0, 0.0, 0xbfc34da5cb7ea285, 0x720fe6cf326a1ab, 0xa80),
    ];
    for (stack, ante, ev, hash, iterations) in gold {
        let r = solve_with(stack, ante, 0.0);
        let h = r
            .shove
            .iter()
            .chain(&r.call)
            .fold(0u64, |h, f| h.wrapping_mul(1_000_003) ^ f.to_bits());
        assert_eq!(r.sb_ev_bb.to_bits(), ev, "{stack}/{ante}");
        assert_eq!(h, hash, "{stack}/{ante}");
        assert_eq!(r.iterations, iterations, "{stack}/{ante}");
    }
}

/// Single threshold classes may mix less as the BB's calls widen too (8 BB, 2.25 → 3
/// dead: 42s from pure to 25 %), but the range as a whole grows.
#[test]
fn more_dead_money_widens_sb_shoves() {
    for stack in [4.0, 8.0, 12.0, 16.0, 20.0] {
        let mut last: Option<PushFoldResult> = None;
        for dead in [0.0, 0.25, 0.5, 1.0, 1.5, 2.25, 3.0] {
            let r = solve_with(stack, 0.1, dead);
            assert!(r.exploitability_bb() < 1e-6, "{stack} BB, {dead} dead");
            if let Some(prev) = &last {
                assert!(
                    r.shove_pct >= prev.shove_pct,
                    "{stack} BB: {dead} dead shoves {:.3}% < {:.3}%",
                    r.shove_pct,
                    prev.shove_pct
                );
            }
            last = Some(r);
        }
    }
}

#[test]
fn dead_money_is_validated() {
    let mut p = PushFoldParams::new(10.0);
    p.dead_bb = -0.1;
    assert!(solve_push_fold(&p).is_err());
    p.dead_bb = f64::NAN;
    assert!(solve_push_fold(&p).is_err());
}

/// The class EVs are the equilibrium's own numbers: they average to the SB's EV, every
/// pure choice is a best response and every mixed class is indifferent.
#[test]
fn class_evs_explain_the_equilibrium() {
    for (stack, ante, dead) in [(10.0, 0.0, 0.0), (7.0, 0.12, 0.84), (15.0, 0.0, 1.0), (3.0, 0.2, 1.4)] {
        let r = solve_with(stack, ante, dead);
        let mut total = 0.0;
        for c in HandClass::all() {
            let i = c.index();
            let p = c.combo_count() as f64 / 1326.0;
            let (sb, bb) = (r.sb_class_ev[i], r.bb_class_ev[i]);
            assert!((sb.fold + 0.5 + ante).abs() < 1e-12, "SB fold {c}: {}", sb.fold);
            assert!((bb.fold + 1.0 + ante).abs() < 1e-12, "BB fold {c}: {}", bb.fold);
            total += p * (r.shove[i] * sb.act + (1.0 - r.shove[i]) * sb.fold);
            for (f, ev) in [(r.shove[i], sb), (r.call[i], bb)] {
                if f == 1.0 {
                    assert!(ev.act >= ev.fold - 1e-9, "{stack}/{dead} {c}: {ev:?}");
                } else if f == 0.0 {
                    assert!(ev.fold >= ev.act - 1e-9, "{stack}/{dead} {c}: {ev:?}");
                } else {
                    assert!((ev.act - ev.fold).abs() < 1e-7, "{stack}/{dead} mixed {c}: {ev:?}");
                }
            }
        }
        assert!((total - r.sb_ev_bb).abs() < 1e-12, "{stack}/{dead}: {total} vs {}", r.sb_ev_bb);
        let aa = freq_class("AA");
        assert!(r.sb_class_ev[aa].loss(false) > 0.0 && r.sb_class_ev[aa].loss(true) == 0.0);
    }
}

fn freq_class(class: &str) -> usize {
    class.parse::<HandClass>().unwrap().index()
}

/// The BB's call EV is its showdown share of everything in the pot, dead money included.
#[test]
fn bb_call_ev_counts_the_dead_money() {
    let r = solve_with(10.0, 0.0, 2.0);
    let aa = freq_class("AA");
    let ev = r.bb_class_ev[aa];
    // AA wins at least 80 % against any range; pot after a call is 20 + 2 BB.
    assert!(ev.act > 0.8 * 22.0 - 10.0, "{ev:?}");
    assert!(ev.act < 22.0 - 10.0, "{ev:?}");
}
