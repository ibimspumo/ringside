mod common;

use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::{Card, CardSet, outs};

use common::naive_best;

/// On random flops and turns the outs partition the unseen cards exactly as an
/// independent naive evaluator ranks the two hands after each card.
#[test]
fn outs_match_a_naive_evaluator() {
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    let mut deck: Vec<Card> = ringside_engine::cards::deck();
    for round in 0..400 {
        deck.shuffle(&mut rng);
        let (hero, villain) = ([deck[0], deck[1]], [deck[2], deck[3]]);
        let board = &deck[4..if round % 2 == 0 { 7 } else { 8 }];
        let r = outs(hero, Some(villain), board).unwrap();
        let seen = CardSet::from_cards(&deck[..4 + board.len()]);
        let mut expected = (Vec::new(), Vec::new());
        for card in seen.complement().iter() {
            let mut h: Vec<Card> = hero.into_iter().chain(board.iter().copied()).collect();
            let mut v: Vec<Card> = villain.into_iter().chain(board.iter().copied()).collect();
            h.push(card);
            v.push(card);
            match naive_best(&h).cmp(&naive_best(&v)) {
                std::cmp::Ordering::Greater => expected.0.push(card),
                std::cmp::Ordering::Equal => expected.1.push(card),
                std::cmp::Ordering::Less => {}
            }
        }
        assert_eq!((r.outs, r.half_outs), expected, "{hero:?} vs {villain:?} on {board:?}");
    }
}
