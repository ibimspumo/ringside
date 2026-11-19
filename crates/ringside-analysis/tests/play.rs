//! The estimate grades "Hände spielen" where no solver plays: a multiway move after the
//! flop gets a grade with exactly one source, the estimate, its reliability and a reason.

use std::sync::Arc;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_analysis::assess::Estimator;
use ringside_play::Hand;
use ringside_play::hand::{Source, Street, Unrated};
use ringside_play::table::Config;

/// Plays to the end with random moves; live solves are stopped, as the hero may do.
fn play_to_end(hand: &mut Hand, rng: &mut ChaCha8Rng) {
    for _ in 0..200 {
        if hand.need().is_some() {
            hand.resume(Err("Abgebrochen".into())).unwrap();
            continue;
        }
        let Some(lesson) = hand.lesson() else { break };
        let a = rng.random_range(0..lesson.choices.len());
        hand.act(a).unwrap();
    }
    assert!(hand.is_over());
}

#[test]
fn a_multiway_move_after_the_flop_is_graded_by_the_estimate() {
    let estimator = Arc::new(Estimator::new());
    let config = Config::default();
    let mut multiway = 0;
    for seed in 0..400 {
        let mut hand = Hand::start(&config, seed, None).unwrap();
        hand.set_assess(estimator.clone());
        play_to_end(&mut hand, &mut ChaCha8Rng::seed_from_u64(seed));
        let view = hand.view();
        for v in &view.verdicts {
            let l = &v.lesson;
            // Every graded move has exactly one source; a move without a grade says why.
            assert_eq!(v.judgement.is_some(), l.strategy.is_some());
            assert_eq!(v.points.is_some(), v.judgement.is_some());
            if l.strategy.is_some() {
                assert!(l.source.is_some() && l.unrated.is_none(), "{l:?}");
            } else {
                assert!(l.unrated.is_some(), "{l:?}");
            }
            if view.free.as_ref().is_some_and(|f| f.why == Unrated::Multiway) && l.street != Street::Preflop && l.choices.len() > 1 {
                assert_eq!(l.source, Some(Source::Estimate), "{l:?}");
                assert!(v.judgement.is_some(), "{l:?}");
                assert!(l.reliability.is_some_and(|r| r > 0.5 && r <= 1.0), "{l:?}");
                let why = l.why.as_deref().unwrap_or_default();
                assert!(why.starts_with("Gegen die Hände, die") && why.ends_with('.'), "{why}");
                multiway += 1;
            }
        }
        if multiway >= 6 {
            break;
        }
    }
    assert!(multiway >= 6, "only {multiway} multiway moves graded");
}
