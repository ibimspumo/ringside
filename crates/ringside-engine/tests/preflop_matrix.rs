use std::time::Instant;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::{CardSet, HandClass, Method, PreflopMatrix, Range, equity};

fn class(s: &str) -> HandClass {
    s.parse().unwrap()
}

#[test]
fn matrix_is_antisymmetric() {
    let m = PreflopMatrix::embedded();
    for a in HandClass::all() {
        for b in HandClass::all() {
            let sum = m.equity(a, b) + m.equity(b, a);
            assert!((sum - 1.0).abs() < 1e-6, "{a} vs {b}: {sum}");
        }
        assert!((m.equity(a, a) - 0.5).abs() < 1e-6);
    }
}

#[test]
fn known_values() {
    let m = PreflopMatrix::embedded();
    let aa_kk = m.equity(class("AA"), class("KK"));
    assert!((aa_kk - 0.82).abs() < 0.005, "AA vs KK {aa_kk}");
    let aks_qq = m.equity(class("AKs"), class("QQ"));
    assert!((0.45..0.47).contains(&aks_qq), "AKs vs QQ {aks_qq}");
    let best_vs_worst = m.equity(class("AA"), class("72o"));
    assert!(best_vs_worst > 0.85, "{best_vs_worst}");
}

/// Recomputes random entries through the equity module's exact class-vs-class
/// enumeration, an independent code path.
#[test]
#[ignore = "slow; run with --release -- --ignored"]
fn random_entries_match_exact_enumeration() {
    let m = PreflopMatrix::embedded();
    let mut rng = ChaCha8Rng::seed_from_u64(2024);
    let mut pairs: Vec<(HandClass, HandClass)> = (0..22)
        .map(|_| {
            (
                HandClass::from_index(rng.random_range(0..169)),
                HandClass::from_index(rng.random_range(0..169)),
            )
        })
        .collect();
    pairs.push((class("AKs"), class("AKo")));
    pairs.push((class("72o"), class("72o")));
    let start = Instant::now();
    for (a, b) in pairs {
        let players = [
            Range::parse(&a.to_string()).unwrap(),
            Range::parse(&b.to_string()).unwrap(),
        ];
        let r = equity(&players, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).unwrap();
        let exact = r.players[0].equity_pct / 100.0;
        let table = m.equity(a, b);
        println!("{a} vs {b}: exact {exact:.7} table {table:.7}");
        assert!(
            (exact - table).abs() < 1e-6,
            "{a} vs {b}: exact {exact} table {table}"
        );
    }
    println!(
        "24 entries recomputed in {:.2} s",
        start.elapsed().as_secs_f64()
    );
}

#[test]
#[ignore = "slow in debug; run with --release -- --ignored"]
fn embedded_file_matches_fresh_computation() {
    let start = Instant::now();
    let fresh = PreflopMatrix::compute();
    println!("matrix computed in {:.2} s", start.elapsed().as_secs_f64());
    assert!(
        &fresh == PreflopMatrix::embedded(),
        "data/preflop_169.bin is stale"
    );
}

#[test]
fn equity_against_a_random_hand() {
    let m = PreflopMatrix::embedded();
    let aa = m.equity_vs_random(class("AA"));
    assert!((aa - 0.852).abs() < 0.001, "AA {aa}");
    let worst = HandClass::all()
        .min_by(|a, b| m.equity_vs_random(*a).total_cmp(&m.equity_vs_random(*b)))
        .unwrap();
    assert_eq!(worst.to_string(), "32o");
    // Over all deals the two hands share the pot evenly.
    let mean: f64 = HandClass::all()
        .map(|c| c.combo_count() as f64 / 1326.0 * m.equity_vs_random(c))
        .sum();
    assert!((mean - 0.5).abs() < 1e-6, "{mean}");
    // An independent Monte Carlo run against the full range.
    let players = [Range::parse("T9s").unwrap(), Range::all()];
    let mc = equity(
        &players,
        CardSet::EMPTY,
        CardSet::EMPTY,
        Method::MonteCarlo { samples: 400_000, seed: 3 },
    )
    .unwrap();
    let t9s = m.equity_vs_random(class("T9s"));
    let got = mc.players[0].equity_pct / 100.0;
    assert!((got - t9s).abs() < 0.004, "T9s matrix {t9s} vs MC {got}");
}
