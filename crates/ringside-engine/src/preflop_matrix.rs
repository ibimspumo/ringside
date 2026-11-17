//! Exact all-in preflop equity of every starting-hand class against every other,
//! averaged over all combo pairs that share no card.
//!
//! Instead of enumerating boards per combo pair, `compute` walks each 5-card board
//! once: all 1081 live combos are evaluated, sorted by strength, and swept from weak
//! to strong while per-class counts of weaker combos (minus those sharing a card with
//! the current one, by inclusion–exclusion) are added to the class row. Boards are
//! reduced to suit-isomorphism classes first because class equities don't change
//! under suit permutation.

use std::collections::HashMap;
use std::sync::LazyLock;

use rayon::prelude::*;

use crate::Error;
use crate::cards::{CardSet, binomial};
use crate::eval::Hand;
use crate::range::{Combo, HandClass, NUM_CLASSES, NUM_COMBOS, class_pair_count};

const MAGIC: &[u8; 4] = b"RSPM";
const VERSION: u32 = 1;
const N: usize = NUM_CLASSES;

static EMBEDDED_BYTES: &[u8] = include_bytes!("../data/preflop_169.bin");
static EMBEDDED: LazyLock<PreflopMatrix> = LazyLock::new(|| {
    PreflopMatrix::from_bytes(EMBEDDED_BYTES)
        .expect("data/preflop_169.bin is invalid; run `cargo run --release -p ringside-engine --bin gen-preflop-matrix`")
});

/// `equity(a, b)` is the all-in pot share of class `a` against class `b`.
#[derive(Clone, PartialEq)]
pub struct PreflopMatrix {
    eq: Vec<f32>,
}

impl PreflopMatrix {
    /// The table shipped in `data/preflop_169.bin`.
    pub fn embedded() -> &'static PreflopMatrix {
        &EMBEDDED
    }

    pub fn equity(&self, a: HandClass, b: HandClass) -> f64 {
        self.eq[a.index() * N + b.index()] as f64
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(12 + 4 * N * N);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(N as u32).to_le_bytes());
        for v in &self.eq {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<PreflopMatrix, Error> {
        let bad = |m: &str| Error::InvalidInput(format!("preflop matrix: {m}"));
        if bytes.len() != 12 + 4 * N * N || &bytes[..4] != MAGIC {
            return Err(bad("wrong size or magic"));
        }
        let word = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("4 bytes"));
        if word(4) != VERSION || word(8) != N as u32 {
            return Err(bad("unsupported version"));
        }
        let eq = (0..N * N)
            .map(|i| f32::from_bits(word(12 + 4 * i)))
            .collect();
        Ok(PreflopMatrix { eq })
    }

    /// Exact computation over all boards (seconds in release, far longer in debug).
    pub fn compute() -> PreflopMatrix {
        let wins = half_point_counts();
        let boards_per_pair = binomial(48, 5) as f64;
        let mut eq = vec![0f32; N * N];
        for a in HandClass::all() {
            for b in HandClass::all() {
                let i = a.index() * N + b.index();
                let pairs = class_pair_count(a, b) as f64;
                eq[i] = (wins[i] as f64 / (2.0 * pairs * boards_per_pair)) as f32;
            }
        }
        PreflopMatrix { eq }
    }
}

/// `count[a * 169 + b]` = Σ over disjoint (combo of a, combo of b, board) of
/// 2 for a win, 1 for a split.
fn half_point_counts() -> Vec<u64> {
    struct Live {
        hand: Hand,
        mask: u64,
        class: u16,
        c1: u16,
        c2: u16,
    }
    let combos: Vec<Live> = Combo::all()
        .map(|c| {
            let (a, b) = c.cards();
            Live {
                hand: Hand::from_cards([a, b]),
                mask: c.mask().0,
                class: c.class().index() as u16,
                c1: a.index() as u16,
                c2: b.index() as u16,
            }
        })
        .collect();

    struct Scratch {
        counts: Vec<u64>,
        sorted: Vec<(u16, u16)>,
        below: Vec<i32>,
        below_card: Vec<i32>,
        group: Vec<i32>,
        group_card: Vec<i32>,
    }
    let new_scratch = || Scratch {
        counts: vec![0; N * N],
        sorted: Vec::with_capacity(NUM_COMBOS),
        below: vec![0; N],
        below_card: vec![0; 52 * N],
        group: vec![0; N],
        group_card: vec![0; 52 * N],
    };

    canonical_boards()
        .into_par_iter()
        .fold(new_scratch, |mut s, (board, weight)| {
            let board_hand = Hand::from_set(CardSet(board));
            s.sorted.clear();
            for (i, c) in combos.iter().enumerate() {
                if c.mask & board == 0 {
                    s.sorted
                        .push((c.hand.combine(board_hand).rank().0, i as u16));
                }
            }
            s.sorted.sort_unstable();
            s.below.fill(0);
            s.below_card.fill(0);
            let weight = weight as u64;
            let mut start = 0;
            while start < s.sorted.len() {
                let rank = s.sorted[start].0;
                let end = start + s.sorted[start..].iter().take_while(|e| e.0 == rank).count();
                let group = &s.sorted[start..end];
                for &(_, i) in group {
                    let c = &combos[i as usize];
                    let k = c.class as usize;
                    s.group[k] += 1;
                    s.group_card[c.c1 as usize * N + k] += 1;
                    s.group_card[c.c2 as usize * N + k] += 1;
                }
                for &(_, i) in group {
                    let c = &combos[i as usize];
                    let (x, y) = (c.c1 as usize * N, c.c2 as usize * N);
                    // `group - gx - gy` subtracts `c` itself twice but counts it once;
                    // the temporary +1 makes its own class come out right.
                    s.group[c.class as usize] += 1;
                    let row = &mut s.counts[c.class as usize * N..][..N];
                    let (bx, by) = (&s.below_card[x..x + N], &s.below_card[y..y + N]);
                    let (gx, gy) = (&s.group_card[x..x + N], &s.group_card[y..y + N]);
                    for b in 0..N {
                        let wins = s.below[b] - bx[b] - by[b];
                        let ties = s.group[b] - gx[b] - gy[b];
                        row[b] += weight * (2 * wins + ties) as u64;
                    }
                    s.group[c.class as usize] -= 1;
                }
                for &(_, i) in group {
                    let c = &combos[i as usize];
                    let k = c.class as usize;
                    s.group[k] -= 1;
                    s.group_card[c.c1 as usize * N + k] -= 1;
                    s.group_card[c.c2 as usize * N + k] -= 1;
                    s.below[k] += 1;
                    s.below_card[c.c1 as usize * N + k] += 1;
                    s.below_card[c.c2 as usize * N + k] += 1;
                }
                start = end;
            }
            s
        })
        .map(|s| s.counts)
        .reduce(
            || vec![0; N * N],
            |mut a, b| {
                a.iter_mut().zip(b).for_each(|(x, y)| *x += y);
                a
            },
        )
}

/// One representative per suit-isomorphism class of 5-card boards, with the number
/// of boards in the class. Two boards are isomorphic iff their multisets of per-suit
/// rank masks agree.
fn canonical_boards() -> Vec<(u64, u32)> {
    let mut classes: HashMap<u64, u32> = HashMap::new();
    let mut idx = [0u8; 5];
    fn next(idx: &mut [u8; 5]) -> bool {
        for i in (0..5).rev() {
            if (idx[i] as usize) < 52 - (5 - i) {
                idx[i] += 1;
                for j in i + 1..5 {
                    idx[j] = idx[j - 1] + 1;
                }
                return true;
            }
        }
        false
    }
    for (i, v) in idx.iter_mut().enumerate() {
        *v = i as u8;
    }
    loop {
        let mut suits = [0u64; 4];
        for &c in &idx {
            suits[(c & 3) as usize] |= 1 << (c >> 2);
        }
        suits.sort_unstable_by(|a, b| b.cmp(a));
        let key = suits[0] << 39 | suits[1] << 26 | suits[2] << 13 | suits[3];
        *classes.entry(key).or_default() += 1;
        if !next(&mut idx) {
            break;
        }
    }
    let mut out: Vec<(u64, u32)> = classes
        .into_iter()
        .map(|(key, count)| {
            let mut board = 0u64;
            for suit in 0..4 {
                let ranks = key >> (39 - 13 * suit) & 0x1fff;
                for r in 0..13 {
                    if ranks >> r & 1 == 1 {
                        board |= 1 << (r * 4 + suit);
                    }
                }
            }
            (board, count)
        })
        .collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_boards_cover_every_board() {
        let boards = canonical_boards();
        let total: u64 = boards.iter().map(|b| b.1 as u64).sum();
        assert_eq!(total, binomial(52, 5));
        assert!(boards.iter().all(|b| b.0.count_ones() == 5));
    }

    #[test]
    fn bytes_round_trip() {
        let m = PreflopMatrix {
            eq: (0..N * N).map(|i| i as f32 / 1e5).collect(),
        };
        assert!(PreflopMatrix::from_bytes(&m.to_bytes()).unwrap() == m);
        assert!(PreflopMatrix::from_bytes(&[0; 12]).is_err());
    }
}
