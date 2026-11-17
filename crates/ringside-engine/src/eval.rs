//! 5–7 card hand evaluator.
//!
//! A `Hand` is built incrementally: every card adds a rank key (chosen so that any
//! multiset of up to 7 ranks has a unique sum), bumps a 4-bit suit counter, and sets
//! its bit in a per-suit rank mask. Evaluation is then one table lookup: the flush
//! table by rank mask when a suit counter reached 5, otherwise a perfect hash of the
//! rank-key sum. Adding a card is two integer ops, which makes board enumeration cheap.

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::cards::{Card, CardSet};

/// Rank keys such that all sums of up to 7 keys with each key used at most 4 times are
/// distinct (found by greedy search).
const RANK_KEYS: [u32; 13] = [
    1, 5, 24, 112, 521, 2247, 9244, 30823, 103066, 250154, 667453, 1526359, 3453520,
];
const SUIT_SHIFT: u32 = 32;
/// Each suit counter starts at 3 so that its top bit (8) is set at exactly 5 cards.
const SUIT_INIT: u64 = 0x3333 << SUIT_SHIFT;
const FLUSH_BITS: u64 = 0x8888 << SUIT_SHIFT;
const ROW_SHIFT: u32 = 12;

/// Strength of a 5–7 card hand: higher is better, equal means a split.
/// Values run from 1 (7-5-4-3-2) to 7462 (royal flush).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct HandRank(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum HandCategory {
    HighCard,
    Pair,
    TwoPair,
    Trips,
    Straight,
    Flush,
    FullHouse,
    Quads,
    StraightFlush,
}

impl HandCategory {
    pub const ALL: [HandCategory; 9] = [
        HandCategory::HighCard,
        HandCategory::Pair,
        HandCategory::TwoPair,
        HandCategory::Trips,
        HandCategory::Straight,
        HandCategory::Flush,
        HandCategory::FullHouse,
        HandCategory::Quads,
        HandCategory::StraightFlush,
    ];
}

/// Highest `HandRank` value of each category, in category order.
const CATEGORY_TOP: [u16; 9] = [1277, 4137, 4995, 5853, 5863, 7140, 7296, 7452, 7462];

impl HandRank {
    pub fn category(self) -> HandCategory {
        let i = CATEGORY_TOP
            .iter()
            .position(|&top| self.0 <= top)
            .unwrap_or(8);
        HandCategory::ALL[i]
    }
}

/// An incrementally built set of up to 7 cards ready for evaluation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hand {
    key: u64,
    mask: u64,
}

impl Default for Hand {
    fn default() -> Self {
        Hand::EMPTY
    }
}

impl Hand {
    pub const EMPTY: Hand = Hand {
        key: SUIT_INIT,
        mask: 0,
    };

    #[inline(always)]
    pub fn with(self, card: Card) -> Hand {
        let (r, s) = (card.rank() as u32, card.suit() as u32);
        Hand {
            key: self.key + RANK_KEYS[r as usize] as u64 + (1 << (SUIT_SHIFT + 4 * s)),
            mask: self.mask | 1 << (16 * s + r),
        }
    }

    /// Union of two disjoint hands.
    #[inline(always)]
    pub fn combine(self, other: Hand) -> Hand {
        Hand {
            key: self.key + (other.key - SUIT_INIT),
            mask: self.mask | other.mask,
        }
    }

    pub fn from_cards(cards: impl IntoIterator<Item = Card>) -> Hand {
        cards.into_iter().fold(Hand::EMPTY, Hand::with)
    }

    pub fn from_set(set: CardSet) -> Hand {
        Hand::from_cards(set.iter())
    }

    /// Requires 5 to 7 cards.
    #[inline(always)]
    pub fn rank(self) -> HandRank {
        TABLES.rank(self)
    }
}

/// Best 5-card rank of a set of 5–7 cards.
pub fn evaluate(cards: CardSet) -> HandRank {
    debug_assert!((5..=7).contains(&cards.len()));
    Hand::from_set(cards).rank()
}

pub fn evaluate_cards(cards: &[Card]) -> HandRank {
    Hand::from_cards(cards.iter().copied()).rank()
}

/// Lookup tables; built once on first use (~0.1 s in release, ~1.5 s in debug).
struct Tables {
    flush: Vec<u16>,
    offsets: Vec<u32>,
    ranks: Vec<u16>,
}

static TABLES: LazyLock<Tables> = LazyLock::new(Tables::build);

/// Forces table construction so benchmarks don't time it.
pub fn init() {
    LazyLock::force(&TABLES);
}

impl Tables {
    #[inline(always)]
    fn rank(&self, hand: Hand) -> HandRank {
        let flush = hand.key & FLUSH_BITS;
        if flush != 0 {
            let suit = (flush.trailing_zeros() - SUIT_SHIFT) / 4;
            let bits = (hand.mask >> (16 * suit)) & 0x1fff;
            return HandRank(self.flush[bits as usize]);
        }
        let key = (hand.key & 0xffff_ffff) as u32;
        let idx = (key & ((1 << ROW_SHIFT) - 1)) + self.offsets[(key >> ROW_SHIFT) as usize];
        HandRank(self.ranks[idx as usize])
    }

    fn build() -> Tables {
        let ordinals = Ordinals::build();

        let mut flush = vec![0u16; 8192];
        for (bits, slot) in flush.iter_mut().enumerate() {
            let n = (bits as u32).count_ones();
            if (5..=7).contains(&n) {
                let mut counts = [0u8; 13];
                for (r, c) in counts.iter_mut().enumerate() {
                    *c = (bits >> r & 1) as u8;
                }
                *slot = ordinals.best_of(&counts, true);
            }
        }

        let mut entries: Vec<(u32, u16)> = Vec::new();
        let mut counts = [0u8; 13];
        collect_multisets(&mut counts, 0, 0, &mut |c, n| {
            if n >= 5 {
                let key: u32 = c.iter().zip(RANK_KEYS).map(|(&k, rk)| k as u32 * rk).sum();
                entries.push((key, ordinals.best_of(c, false)));
            }
        });
        let (offsets, ranks) = perfect_hash(&entries);
        Tables {
            flush,
            offsets,
            ranks,
        }
    }
}

/// Visits every rank multiset with at most 4 of a rank and at most 7 cards.
fn collect_multisets(counts: &mut [u8; 13], rank: usize, n: u8, f: &mut impl FnMut(&[u8; 13], u8)) {
    if rank == 13 {
        f(counts, n);
        return;
    }
    for k in 0..=4u8.min(7 - n) {
        counts[rank] = k;
        collect_multisets(counts, rank + 1, n + k, f);
    }
    counts[rank] = 0;
}

/// First-fit displacement: each row of 4096 keys gets an offset so all keys land in
/// distinct slots of one compact table. Free slots live in a bitset so 64 candidate
/// offsets are tested per step.
fn perfect_hash(entries: &[(u32, u16)]) -> (Vec<u32>, Vec<u16>) {
    let rows = (entries.iter().map(|e| e.0).max().unwrap_or(0) >> ROW_SHIFT) as usize + 1;
    let mut by_row: Vec<Vec<(u32, u16)>> = vec![Vec::new(); rows];
    for &(k, v) in entries {
        by_row[(k >> ROW_SHIFT) as usize].push((k & ((1 << ROW_SHIFT) - 1), v));
    }
    let mut order: Vec<usize> = (0..rows).filter(|&r| !by_row[r].is_empty()).collect();
    order.sort_by_key(|&r| std::cmp::Reverse(by_row[r].len()));

    // Bit set = slot free; slots past the end are free.
    let mut free: Vec<u64> = Vec::new();
    let word = |free: &[u64], i: usize| free.get(i).copied().unwrap_or(!0);
    let window = |free: &[u64], pos: usize| {
        let (w, b) = (pos / 64, pos % 64);
        let lo = word(free, w) >> b;
        if b == 0 {
            lo
        } else {
            lo | word(free, w + 1) << (64 - b)
        }
    };
    let mut ranks: Vec<u16> = Vec::new();
    let mut offsets = vec![0u32; rows];
    let mut first_free = 0usize;
    for r in order {
        let row = &mut by_row[r];
        row.sort_unstable();
        let low0 = row[0].0 as usize;
        let mut base = first_free.saturating_sub(low0);
        let off = loop {
            let mut fits = !0u64;
            for &(low, _) in row.iter() {
                fits &= window(&free, low as usize + base);
                if fits == 0 {
                    break;
                }
            }
            if fits != 0 {
                break base + fits.trailing_zeros() as usize;
            }
            base += 64;
        };
        for &(low, v) in row.iter() {
            let i = low as usize + off;
            if i / 64 >= free.len() {
                free.resize(i / 64 + 1, !0);
            }
            free[i / 64] &= !(1 << (i % 64));
            if i >= ranks.len() {
                ranks.resize(i + 1, 0);
            }
            ranks[i] = v;
        }
        while word(&free, first_free / 64) >> (first_free % 64) == 0 {
            first_free = (first_free / 64 + 1) * 64;
        }
        first_free += (word(&free, first_free / 64) >> (first_free % 64)).trailing_zeros() as usize;
        offsets[r] = off as u32;
    }
    // Padding keeps lookups of invalid (<5 card) keys in bounds; they return garbage.
    ranks.resize(ranks.len() + (1 << ROW_SHIFT), 0);
    (offsets, ranks)
}

/// Maps every distinct 5-card hand value to its rank 1..=7462.
struct Ordinals {
    sorted: Vec<u32>,
}

impl Ordinals {
    fn build() -> Ordinals {
        let mut scores = Vec::new();
        let mut counts = [0u8; 13];
        collect_multisets(&mut counts, 0, 0, &mut |c, n| {
            if n == 5 {
                scores.push(score5(c, false));
                if c.iter().all(|&k| k <= 1) {
                    scores.push(score5(c, true));
                }
            }
        });
        scores.sort_unstable();
        scores.dedup();
        assert_eq!(scores.len(), 7462);
        Ordinals { sorted: scores }
    }

    fn ordinal(&self, score: u32) -> u16 {
        self.sorted
            .binary_search(&score)
            .expect("valid 5-card score") as u16
            + 1
    }

    /// Best 5-card sub-multiset of `counts` (5–7 cards).
    fn best_of(&self, counts: &[u8; 13], flush: bool) -> u16 {
        let mut best = 0u32;
        let mut sub = [0u8; 13];
        fn rec(
            counts: &[u8; 13],
            sub: &mut [u8; 13],
            r: usize,
            left: u8,
            flush: bool,
            best: &mut u32,
        ) {
            if left == 0 {
                *best = (*best).max(score5(sub, flush));
                return;
            }
            if r == 13 {
                return;
            }
            for k in 0..=counts[r].min(left) {
                sub[r] = k;
                rec(counts, sub, r + 1, left - k, flush, best);
            }
            sub[r] = 0;
        }
        rec(counts, &mut sub, 0, 5, flush, &mut best);
        self.ordinal(best)
    }
}

/// Comparable score of exactly 5 ranks: category in bits 20+, tiebreak ranks below.
fn score5(counts: &[u8; 13], flush: bool) -> u32 {
    let bits: u32 = (0..13).filter(|&r| counts[r] > 0).map(|r| 1 << r).sum();
    let straight_top = match bits {
        0b1_0000_0000_1111 => Some(3),
        _ if bits.count_ones() == 5 && bits >> bits.trailing_zeros() == 0b11111 => {
            Some(bits.trailing_zeros() + 4)
        }
        _ => None,
    };
    if let Some(top) = straight_top {
        return if flush { 8 << 20 | top } else { 4 << 20 | top };
    }
    // Ranks ordered by multiplicity, then rank, high to low.
    let mut tiebreak = 0u32;
    for c in (1..=4u8).rev() {
        for r in (0..13).rev() {
            if counts[r] == c {
                tiebreak = tiebreak << 4 | r as u32;
            }
        }
    }
    let most = *counts.iter().max().unwrap_or(&0);
    let pairs = counts.iter().filter(|&&c| c == 2).count();
    let category = match (most, pairs) {
        (4, _) => 7,
        (3, 1) => 6,
        _ if flush => 5,
        (3, _) => 3,
        (2, 2) => 2,
        (2, _) => 1,
        _ => 0,
    };
    category << 20 | tiebreak
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(s: &str) -> HandRank {
        evaluate(CardSet::parse(s).unwrap())
    }

    #[test]
    fn known_hands() {
        assert_eq!(rank("AsKsQsJsTs").0, 7462);
        assert_eq!(rank("7c5d4h3s2c").0, 1);
        assert_eq!(rank("5s4s3s2sAs").category(), HandCategory::StraightFlush);
        assert_eq!(rank("5s4s3s2sAs").0, 7453);
        assert_eq!(rank("5s4d3s2sAs").0, 5854);
        assert!(rank("AhAdKcKs2c3d4h") < rank("AhAdKcKs5c3d4h"));
        assert_eq!(rank("AhAdKcKsQcQd2h"), rank("AhAdKcKsQcJd2h"));
    }
}
