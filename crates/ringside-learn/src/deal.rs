//! Seeded randomness: the same (kind, level, seed) always deals the same exercise.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::{Card, CardSet};

use crate::Kind;

pub type Rng8 = ChaCha8Rng;

pub fn rng(kind: Kind, level: u8, seed: u64) -> Rng8 {
    let k = Kind::ALL.iter().position(|&x| x == kind).unwrap_or(0) as u64;
    // splitmix64 of the seed, so neighbouring seeds don't share streams.
    let mut z = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^= z >> 31;
    ChaCha8Rng::seed_from_u64(z ^ (k << 56) ^ ((level as u64) << 48))
}

/// A random card not in `used`, which then contains it.
pub fn card(rng: &mut Rng8, used: &mut CardSet) -> Card {
    loop {
        let c = Card::from_index(rng.random_range(0..52));
        if !used.contains(c) {
            used.insert(c);
            return c;
        }
    }
}

pub fn cards(rng: &mut Rng8, used: &mut CardSet, n: usize) -> Vec<Card> {
    (0..n).map(|_| card(rng, used)).collect()
}

pub fn pick<'a, T>(rng: &mut Rng8, items: &'a [T]) -> &'a T {
    &items[rng.random_range(0..items.len())]
}
