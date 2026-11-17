mod common;

use std::collections::HashSet;
use std::time::Instant;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use ringside_engine::eval::evaluate_cards;
use ringside_engine::{Card, CardSet, Hand, HandCategory, HandRank, evaluate};

use common::naive_best;

fn rank(s: &str) -> HandRank {
    evaluate(CardSet::parse(s).unwrap())
}

fn cards() -> Vec<Card> {
    (0..52).map(Card::from_index).collect()
}

fn category_index(c: HandCategory) -> usize {
    HandCategory::ALL.iter().position(|&x| x == c).unwrap()
}

#[test]
fn all_five_card_hands() {
    let deck = cards();
    let mut counts = [0u64; 9];
    let mut seen = vec![false; 7463];
    for a in 0..52 {
        let ha = Hand::EMPTY.with(deck[a]);
        for b in a + 1..52 {
            let hb = ha.with(deck[b]);
            for c in b + 1..52 {
                let hc = hb.with(deck[c]);
                for d in c + 1..52 {
                    let hd = hc.with(deck[d]);
                    for &card in &deck[d + 1..] {
                        let r = hd.with(card).rank();
                        counts[category_index(r.category())] += 1;
                        seen[r.0 as usize] = true;
                    }
                }
            }
        }
    }
    assert_eq!(
        counts,
        [
            1_302_540, 1_098_240, 123_552, 54_912, 10_200, 5_108, 3_744, 624, 40
        ]
    );
    assert!(!seen[0]);
    assert_eq!(seen.iter().filter(|&&s| s).count(), 7462);
}

#[test]
#[ignore = "133M hands; run with --release -- --ignored"]
fn all_seven_card_hands() {
    ringside_engine::eval::init();
    let start = Instant::now();
    let deck = cards();
    let counts = (0..52usize)
        .into_par_iter()
        .map(|a| {
            let mut counts = [0u64; 9];
            let ha = Hand::EMPTY.with(deck[a]);
            for b in a + 1..52 {
                let hb = ha.with(deck[b]);
                for c in b + 1..52 {
                    let hc = hb.with(deck[c]);
                    for d in c + 1..52 {
                        let hd = hc.with(deck[d]);
                        for e in d + 1..52 {
                            let he = hd.with(deck[e]);
                            for f in e + 1..52 {
                                let hf = he.with(deck[f]);
                                for &card in &deck[f + 1..] {
                                    counts[category_index(hf.with(card).rank().category())] += 1;
                                }
                            }
                        }
                    }
                }
            }
            counts
        })
        .reduce(
            || [0; 9],
            |mut x, y| {
                x.iter_mut().zip(y).for_each(|(a, b)| *a += b);
                x
            },
        );
    println!("7-card exhaustive: {:.3} s", start.elapsed().as_secs_f64());
    assert_eq!(
        counts,
        [
            23_294_460, 58_627_800, 31_433_400, 6_461_620, 6_180_020, 4_047_644, 3_473_184,
            224_848, 41_584
        ]
    );
}

#[test]
fn wheel_is_the_lowest_straight() {
    let wheel = rank("5d4c3h2sAd");
    assert_eq!(wheel.category(), HandCategory::Straight);
    assert!(wheel < rank("6d5c4h3s2d"));
    assert!(wheel > rank("AdAcAhKsQd"));
    // Every straight, in any suits, is at least the wheel.
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    for top in 3..13u8 {
        for _ in 0..50 {
            // top 3 wraps to 5-4-3-2-A.
            let ranks: Vec<u8> = (0..5).map(|i| (top + 13 - i) % 13).collect();
            let hand: Vec<Card> = ranks
                .iter()
                .map(|&r| Card::new(r, rng.random_range(0..4)))
                .collect();
            let r = evaluate_cards(&hand);
            if r.category() == HandCategory::Straight {
                assert!(r >= wheel);
                assert_eq!(r == wheel, top == 3);
            }
        }
    }
    // A wheel with a 6 on board plays as the six-high straight.
    assert_eq!(rank("5d4c3h2sAd6c"), rank("6c5d4c3h2s"));
}

#[test]
fn steel_wheel_is_the_lowest_straight_flush() {
    let steel = rank("5h4h3h2hAh");
    assert_eq!(steel.category(), HandCategory::StraightFlush);
    assert!(steel < rank("6s5s4s3s2s"));
    assert!(steel > rank("AsAhAdAcKs"));
    for suit in 0..4u8 {
        for top in 4..13u8 {
            let hand: Vec<Card> = (0..5).map(|i| Card::new(top - i, suit)).collect();
            assert!(evaluate_cards(&hand) > steel);
        }
    }
    assert_eq!(rank("AsKsQsJsTs").0, 7462);
}

#[test]
fn five_and_six_card_hands() {
    assert_eq!(rank("AsAhKdKc2s3d").category(), HandCategory::TwoPair);
    assert_eq!(rank("AsKsQsJs9s8d"), rank("AsKsQsJs9s"));
    assert_eq!(rank("2c2d2h3c3d").category(), HandCategory::FullHouse);
}

/// Random 7-card hands sorted by the engine must be sorted by the reference too, with
/// ties in exactly the same places.
#[test]
fn order_matches_reference_on_random_hands() {
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let mut hands: Vec<(HandRank, (u8, [u8; 5]))> = (0..200_000)
        .map(|_| {
            let mut set = 0u64;
            while set.count_ones() < 7 {
                set |= 1 << rng.random_range(0..52);
            }
            let cs = CardSet(set).to_vec();
            (evaluate(CardSet(set)), naive_best(&cs))
        })
        .collect();
    hands.sort();
    let mut categories = HashSet::new();
    for w in hands.windows(2) {
        let ((fa, na), (fb, nb)) = (w[0], w[1]);
        assert!(
            na <= nb,
            "engine {fa:?} < {fb:?} but reference {na:?} > {nb:?}"
        );
        assert_eq!(
            fa == fb,
            na == nb,
            "tie mismatch: {fa:?}/{na:?} vs {fb:?}/{nb:?}"
        );
        assert_eq!(category_index(fa.category()), na.0 as usize);
        categories.insert(na.0);
    }
    assert_eq!(categories.len(), 9);
}

/// Two hands sharing a board: frequent ties and kicker decisions.
#[test]
fn shared_board_comparisons_match_reference() {
    let mut rng = ChaCha8Rng::seed_from_u64(11);
    let (mut ties, mut kicker_decided) = (0, 0);
    for _ in 0..200_000 {
        let mut set = 0u64;
        let mut picked = Vec::with_capacity(9);
        while picked.len() < 9 {
            let c = rng.random_range(0..52u8);
            if set & 1 << c == 0 {
                set |= 1 << c;
                picked.push(Card::from_index(c));
            }
        }
        let a: Vec<Card> = picked[..7].to_vec();
        let b: Vec<Card> = picked[..5].iter().chain(&picked[7..]).copied().collect();
        let (fa, fb) = (evaluate_cards(&a), evaluate_cards(&b));
        let (na, nb) = (naive_best(&a), naive_best(&b));
        assert_eq!(fa.cmp(&fb), na.cmp(&nb), "{a:?} vs {b:?}");
        if na == nb {
            ties += 1;
        } else if na.0 == nb.0 && na.1[0] == nb.1[0] {
            kicker_decided += 1;
        }
    }
    assert!(ties > 5_000, "only {ties} ties");
    assert!(
        kicker_decided > 10_000,
        "only {kicker_decided} kicker decisions"
    );
}
