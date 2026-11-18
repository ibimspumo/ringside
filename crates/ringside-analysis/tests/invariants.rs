//! What every rated decision promises (see `grade::check`), over random strategies and
//! over every decision of the example hands.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_analysis::grade::{check, verdict};
use ringside_analysis::{Choice, Decision, Rating, Source, SpotType, Status, analyze_hand};
use ringside_core::model::Street;
use ringside_core::{parse_hand, split_hands};
use ringside_play::grade::{BEST_PCT, MIX_MIN_FREQUENCY, grade_of_loss};

fn random_strategy(rng: &mut ChaCha8Rng) -> Vec<Choice> {
    let k = rng.random_range(2..=6);
    let mut freqs: Vec<f64> = (0..k).map(|_| if rng.random_bool(0.3) { 0.0 } else { rng.random::<f64>() }).collect();
    if freqs.iter().all(|&f| f == 0.0) {
        freqs[0] = 1.0;
    }
    let total: f64 = freqs.iter().sum();
    let played = rng.random_range(0..k);
    let scale = [0.1, 1.0, 10.0, 50.0][rng.random_range(0..4)];
    (0..k)
        .map(|i| Choice {
            label: format!("A{i}"),
            freq: freqs[i] / total,
            ev_bb: Some((rng.random::<f64>() - 0.5) * scale),
            played: i == played,
            kind: "call".into(),
        })
        .collect()
}

fn decision(strategy: Vec<Choice>, pot: f64) -> Decision {
    let v = verdict(&strategy, pot).unwrap();
    Decision {
        hand_id: 1,
        action_index: 0,
        street: Street::Flop,
        spot: SpotType::Flop,
        status: Status::Rated,
        played: "x".into(),
        best: None,
        played_freq: Some(v.freq),
        ev_loss_bb: v.loss_bb,
        rating: Some(v.rating),
        source: Some(Source::Solver),
        why: String::new(),
        note: String::new(),
        reason: None,
        solver: None,
        engine: String::new(),
        pot_bb: Some(pot),
        reliability: None,
        strategy,
    }
}

#[test]
fn random_verdicts_keep_every_promise() {
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    for _ in 0..50_000 {
        let strategy = random_strategy(&mut rng);
        let pot = [0.5, 1.5, 4.0, 20.0, 200.0][rng.random_range(0..5)];
        let d = decision(strategy, pot);
        check(&d).unwrap();
        let (loss, freq, r) = (d.ev_loss_bb.unwrap(), d.played_freq.unwrap(), d.rating.unwrap());
        // A mix is never a mistake.
        if freq >= MIX_MIN_FREQUENCY {
            assert!(r <= Rating::Mix && loss == 0.0, "{d:?}");
        } else if loss / pot * 100.0 > BEST_PCT {
            assert_eq!(r, ringside_analysis::grade::rating(grade_of_loss(loss / pot * 100.0)), "{d:?}");
        }
    }
}

#[test]
fn a_larger_loss_never_rates_better() {
    let mut rng = ChaCha8Rng::seed_from_u64(11);
    for _ in 0..10_000 {
        let pot = rng.random_range(1.0..100.0);
        let a = rng.random_range(0.0..pot);
        let b = a + rng.random_range(0.0..pot);
        let rate = |loss: f64| {
            let s = vec![
                Choice { label: "A".into(), freq: 0.97, ev_bb: Some(loss), played: false, kind: "fold".into() },
                Choice { label: "B".into(), freq: 0.03, ev_bb: Some(0.0), played: true, kind: "call".into() },
            ];
            verdict(&s, pot).unwrap().rating
        };
        assert!(rate(a) <= rate(b), "{a} {b} {pot}");
    }
}

#[test]
fn the_best_action_costs_nothing() {
    let mut rng = ChaCha8Rng::seed_from_u64(13);
    for _ in 0..10_000 {
        let mut s = random_strategy(&mut rng);
        let top = (0..s.len()).max_by(|&a, &b| s[a].ev_bb.unwrap().total_cmp(&s[b].ev_bb.unwrap())).unwrap();
        for (i, c) in s.iter_mut().enumerate() {
            c.played = i == top;
        }
        assert_eq!(verdict(&s, 10.0).unwrap().loss_bb, Some(0.0));
    }
}

/// Anonymised hands of every kind the analysis meets: push/fold, model, limps, short
/// tables, multiway pots, forced moves.
const FIXTURES: &str = include_str!("fixtures/hands.txt");

#[test]
fn every_decision_of_the_example_hands_keeps_every_promise() {
    let mut seen = 0;
    for raw in split_hands(FIXTURES) {
        let hand = parse_hand(raw).unwrap_or_else(|e| panic!("{e:?}\n{raw}"));
        for d in analyze_hand(&hand).decisions {
            check(&d).unwrap();
            seen += 1;
        }
    }
    assert!(seen > 0);
}
