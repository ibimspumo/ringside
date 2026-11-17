//! Outs: the cards that change who wins once the next board card is dealt.
//!
//! Against a known hand an out is a card after which the hero is strictly ahead; a card
//! after which the hands split is a half out. Every other unseen card is neither.
//! Against an unknown hand there is no "ahead", so [`outs_to`] counts the cards that
//! bring the hero to a target category with at least one hole card playing.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cards::{Card, CardSet};
use crate::eval::{Hand, HandCategory, HandRank};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutsResult {
    /// In card index order.
    pub outs: Vec<Card>,
    pub half_outs: Vec<Card>,
}

impl OutsResult {
    /// Outs plus half the half outs, the number usually quoted.
    pub fn effective(&self) -> f64 {
        self.outs.len() as f64 + 0.5 * self.half_outs.len() as f64
    }
}

fn check(hero: [Card; 2], villain: Option<[Card; 2]>, board: &[Card]) -> Result<CardSet, Error> {
    if !(3..=4).contains(&board.len()) {
        return Err(Error::InvalidInput("outs need a flop or a turn board".into()));
    }
    let mut all: Vec<Card> = hero.to_vec();
    all.extend(villain.iter().flatten());
    all.extend(board);
    let set = CardSet::from_cards(&all);
    if set.len() as usize != all.len() {
        return Err(Error::InvalidInput("duplicate card".into()));
    }
    Ok(set)
}

/// With `villain`, the cards that put the hero strictly ahead (outs) or level (half
/// outs) after the next card, judged on the hands as they stand then; it is meant for
/// a hero who is behind now. Without `villain`, [`outs_to`] the next better category
/// than the hero holds now.
pub fn outs(hero: [Card; 2], villain: Option<[Card; 2]>, board: &[Card]) -> Result<OutsResult, Error> {
    let Some(villain) = villain else {
        let mut cards = hero.to_vec();
        cards.extend(board);
        let now = category(&cards);
        let Some(next) = HandCategory::ALL.iter().find(|&&c| c > now) else {
            check(hero, None, board)?;
            return Ok(OutsResult::default());
        };
        return outs_to(hero, board, *next);
    };
    let seen = check(hero, Some(villain), board)?;
    let board_hand = Hand::from_cards(board.iter().copied());
    let (h, v) = (
        Hand::from_cards(hero).combine(board_hand),
        Hand::from_cards(villain).combine(board_hand),
    );
    let mut result = OutsResult::default();
    for card in seen.complement().iter() {
        let (hr, vr) = (h.with(card).rank(), v.with(card).rank());
        if hr > vr {
            result.outs.push(card);
        } else if hr == vr {
            result.half_outs.push(card);
        }
    }
    Ok(result)
}

/// Cards that give the hero at least `target` with a hole card playing: the hand must
/// beat what the board plays on its own. `half_outs` stays empty.
pub fn outs_to(hero: [Card; 2], board: &[Card], target: HandCategory) -> Result<OutsResult, Error> {
    let seen = check(hero, None, board)?;
    let mut result = OutsResult::default();
    for card in seen.complement().iter() {
        let mut cards = board.to_vec();
        cards.push(card);
        let board_only = category(&cards);
        cards.extend(hero);
        let with_hero = category(&cards);
        if with_hero >= target && with_hero > board_only {
            result.outs.push(card);
        }
    }
    Ok(result)
}

/// Category of any 1–7 cards; below five cards only rank groups can count.
pub fn category(cards: &[Card]) -> HandCategory {
    if cards.len() >= 5 {
        return Hand::from_cards(cards.iter().copied()).rank().category();
    }
    let mut counts = [0u8; 13];
    for c in cards {
        counts[c.rank() as usize] += 1;
    }
    let max = counts.iter().copied().max().unwrap_or(0);
    let pairs = counts.iter().filter(|&&n| n == 2).count();
    match (max, pairs) {
        (4, _) => HandCategory::Quads,
        (3, _) => HandCategory::Trips,
        (2, 2) => HandCategory::TwoPair,
        (2, _) => HandCategory::Pair,
        _ => HandCategory::HighCard,
    }
}

/// Rank of the best hand from hole cards and a full or partial (≥ 3 cards) board.
pub fn rank_of(hole: &[Card], board: &[Card]) -> HandRank {
    Hand::from_cards(hole.iter().chain(board).copied()).rank()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards(s: &str) -> Vec<Card> {
        let mut v = Vec::new();
        for i in (0..s.len()).step_by(2) {
            v.push(s[i..i + 2].parse().unwrap());
        }
        v
    }

    fn hole(s: &str) -> [Card; 2] {
        let v = cards(s);
        [v[0], v[1]]
    }

    #[test]
    fn nut_flush_draw_with_overcards_against_top_pair() {
        let r = outs(hole("AhKh"), Some(hole("QsJd")), &cards("Qh7h2c")).unwrap();
        // 9 hearts, 3 aces, 3 kings.
        assert_eq!(r.outs.len(), 15, "{r:?}");
        assert!(r.half_outs.is_empty());
    }

    #[test]
    fn kicker_battles_give_half_outs() {
        // Both play K-K-A-7; a seven or a deuce counterfeits the kickers.
        let r = outs(hole("Ac3d"), Some(hole("As4h")), &cards("KcKd7h2s")).unwrap();
        let has = |v: &[Card], c: &str| v.contains(&c.parse().unwrap());
        assert!(has(&r.outs, "3c") && has(&r.outs, "3h") && has(&r.outs, "3s"));
        assert!(has(&r.half_outs, "7c") && has(&r.half_outs, "2c") && has(&r.half_outs, "6d"));
        assert!(!has(&r.outs, "4c") && !has(&r.half_outs, "4c"));
        assert_eq!(r.effective(), r.outs.len() as f64 + r.half_outs.len() as f64 / 2.0);
    }

    #[test]
    fn target_outs_need_a_hole_card() {
        // A straight draw: any 9 or 4 makes a straight with the hole cards.
        let r = outs_to(hole("6c5d"), &cards("8h7s2c"), HandCategory::Straight).unwrap();
        assert_eq!(r.outs.len(), 8, "{r:?}");
        // A board pair does not improve the hero on its own.
        let r = outs_to(hole("AcKd"), &cards("8h7s2c"), HandCategory::Pair).unwrap();
        assert_eq!(r.outs.len(), 6, "{r:?}");
        // Without a villain: the next category up from a pair is two pair or better.
        let r = outs(hole("AcKd"), None, &cards("Ah7s2c")).unwrap();
        assert_eq!(r.outs.len(), 3 + 2 + 3 + 3, "{r:?}");
    }

    #[test]
    fn rejects_bad_boards() {
        assert!(outs(hole("AcKd"), None, &cards("Ah7s")).is_err());
        assert!(outs(hole("AcKd"), Some(hole("AcQd")), &cards("Ah7s2c")).is_err());
        assert!(outs(hole("AcKd"), None, &cards("Ah7s2c3c4c")).is_err());
    }

    #[test]
    fn small_categories() {
        assert_eq!(category(&cards("KcKdKh2s")), HandCategory::Trips);
        assert_eq!(category(&cards("KcKd2h2s")), HandCategory::TwoPair);
        assert_eq!(category(&cards("Kc9d2h")), HandCategory::HighCard);
    }
}
