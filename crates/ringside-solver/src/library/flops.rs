//! A representative flop subset, derived from the 1755 strategically distinct flops.
//!
//! Two flops that differ only by a permutation of suits (A♠K♠2♦ and A♥K♥2♣) have the
//! same solution with the suits swapped, so the 22 100 flops fall into 1755 classes
//! ([`classes`]); each class weighs the number of flops it stands for. Solving all
//! classes is the full flop library; [`subset`] picks `n` of them by proportional
//! stratified systematic sampling:
//! 1. Strata by suit and rank structure: unpaired rainbow, unpaired two-tone, unpaired
//!    monotone, paired, trips. Each stratum gets seats in proportion to the flops it
//!    holds (largest remainder), at least one.
//! 2. Inside a stratum the classes are ordered by ranks, highest card first, and the
//!    class at every `(k + ½)·W/n` of the cumulative weight `W` is taken. Each pick
//!    stands for `W/n` flops, so weighted sums over the subset estimate sums over all
//!    flops without bias, and high, middle and low boards, connected and disconnected
//!    ones, appear in their real proportions.
//!
//! The procedure is deterministic: the same `n` always gives the same list.

use ringside_engine::Card;

pub const ALL_FLOPS: u32 = 22_100;
pub const CLASSES: usize = 1755;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Texture {
    Rainbow,
    TwoTone,
    Monotone,
    Paired,
    Trips,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlopClass {
    /// Canonical representative, highest card first, suits assigned in order of first
    /// appearance (s, h, d, c), e.g. "AsKs2h".
    pub cards: [Card; 3],
    /// Flops of the full deck in this class.
    pub count: u32,
    pub texture: Texture,
}

impl FlopClass {
    pub fn text(&self) -> String {
        self.cards.iter().map(|c| c.to_string()).collect()
    }
}

/// The canonical form of a flop: of all 24 suit renamings, the one that sorts first with
/// cards ordered highest rank first and suits in the order s, h, d, c.
pub fn canonical(cards: [Card; 3]) -> [Card; 3] {
    let key = |c: Card| (12 - c.rank(), 3 - c.suit());
    let mut best: Option<[Card; 3]> = None;
    for perm in crate::suits::all() {
        let mut renamed = cards.map(|c| crate::suits::rename(c, &perm));
        renamed.sort_by_key(|&c| key(c));
        if best.is_none_or(|b| renamed.map(key) < b.map(key)) {
            best = Some(renamed);
        }
    }
    best.expect("24 permutations")
}

fn texture(c: &[Card; 3]) -> Texture {
    let ranks = [c[0].rank(), c[1].rank(), c[2].rank()];
    let suits = [c[0].suit(), c[1].suit(), c[2].suit()];
    if ranks[0] == ranks[2] {
        return Texture::Trips;
    }
    if ranks[0] == ranks[1] || ranks[1] == ranks[2] {
        return Texture::Paired;
    }
    match (suits[0] == suits[1]) as u8 + (suits[1] == suits[2]) as u8 + (suits[0] == suits[2]) as u8 {
        0 => Texture::Rainbow,
        3 => Texture::Monotone,
        _ => Texture::TwoTone,
    }
}

/// Ranks and suits of a canonical flop, for sorting.
type Key = [(u8, u8); 3];

/// The 1755 classes, ordered by texture, then ranks (highest first), then suit pattern.
pub fn classes() -> Vec<FlopClass> {
    let mut counts: std::collections::BTreeMap<Key, ([Card; 3], u32)> = Default::default();
    for a in 0..52u8 {
        for b in a + 1..52 {
            for c in b + 1..52 {
                let flop = canonical([Card::from_index(a), Card::from_index(b), Card::from_index(c)]);
                let key = flop.map(|c| (12 - c.rank(), 3 - c.suit()));
                counts.entry(key).or_insert((flop, 0)).1 += 1;
            }
        }
    }
    let mut out: Vec<FlopClass> =
        counts.into_values().map(|(cards, count)| FlopClass { cards, count, texture: texture(&cards) }).collect();
    out.sort_by_key(|f| (f.texture, f.cards.map(|c| (12 - c.rank(), 3 - c.suit()))));
    out
}

/// A flop of the subset and how many flops of the full deck it stands for.
#[derive(Clone, Debug, PartialEq)]
pub struct Pick {
    pub flop: FlopClass,
    pub weight: f64,
}

/// `n` representative flops (at least one per texture, so `n ≥ 5`), see the module docs.
pub fn subset(n: usize) -> Vec<Pick> {
    assert!((5..=CLASSES).contains(&n), "between 5 and {CLASSES} flops");
    let all = classes();
    let textures = [Texture::Rainbow, Texture::TwoTone, Texture::Monotone, Texture::Paired, Texture::Trips];
    let weight_of = |t: Texture| all.iter().filter(|f| f.texture == t).map(|f| f.count as f64).sum::<f64>();
    let weights: Vec<f64> = textures.iter().map(|&t| weight_of(t)).collect();

    // Largest remainder over the seats left after the one each stratum is guaranteed.
    let spare = (n - textures.len()) as f64;
    let quota: Vec<f64> = weights.iter().map(|w| w / ALL_FLOPS as f64 * spare).collect();
    let mut seats: Vec<usize> = quota.iter().map(|q| 1 + q.floor() as usize).collect();
    let mut order: Vec<usize> = (0..textures.len()).collect();
    order.sort_by(|&a, &b| (quota[b] - quota[b].floor()).total_cmp(&(quota[a] - quota[a].floor())).then(a.cmp(&b)));
    for &i in order.iter().take(n - seats.iter().sum::<usize>()) {
        seats[i] += 1;
    }

    let mut out = Vec::with_capacity(n);
    for (i, &t) in textures.iter().enumerate() {
        let stratum: Vec<&FlopClass> = all.iter().filter(|f| f.texture == t).collect();
        let step = weights[i] / seats[i] as f64;
        let mut cumulative = 0.0;
        let mut k = 0;
        for f in stratum {
            cumulative += f.count as f64;
            while k < seats[i] && (k as f64 + 0.5) * step < cumulative {
                out.push(Pick { flop: f.clone(), weight: step });
                k += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flop(s: &str) -> [Card; 3] {
        let c: Vec<Card> = (0..3).map(|i| s[2 * i..2 * i + 2].parse().unwrap()).collect();
        [c[0], c[1], c[2]]
    }

    #[test]
    fn there_are_1755_classes_covering_every_flop() {
        let all = classes();
        assert_eq!(all.len(), CLASSES);
        assert_eq!(all.iter().map(|f| f.count).sum::<u32>(), ALL_FLOPS);
        let count = |t: Texture| all.iter().filter(|f| f.texture == t).count();
        assert_eq!(
            [Texture::Rainbow, Texture::TwoTone, Texture::Monotone, Texture::Paired, Texture::Trips].map(count),
            [286, 858, 286, 312, 13]
        );
    }

    #[test]
    fn suit_permutations_share_a_class() {
        let a = canonical(flop("AhKh2c"));
        assert_eq!(a, canonical(flop("KsAs2d")));
        assert_eq!(a, canonical(flop("2hAcKc")));
        assert_ne!(a, canonical(flop("AhKc2c")));
        assert_eq!(canonical(flop("7c7d2c")), canonical(flop("7h7s2s")));
        assert_ne!(canonical(flop("7c7d2c")), canonical(flop("7h7s2c")));
    }

    #[test]
    fn subset_is_proportional_and_unique() {
        let picks = subset(184);
        assert_eq!(picks.len(), 184);
        let total: f64 = picks.iter().map(|p| p.weight).sum();
        assert!((total - ALL_FLOPS as f64).abs() < 1e-6, "{total}");
        let mut names: Vec<String> = picks.iter().map(|p| p.flop.text()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 184, "no flop twice");
        let share = |t: Texture| picks.iter().filter(|p| p.flop.texture == t).count();
        assert_eq!(
            [Texture::Rainbow, Texture::TwoTone, Texture::Monotone, Texture::Paired, Texture::Trips].map(share),
            [57, 84, 10, 31, 2]
        );
        assert_eq!(subset(184), picks, "deterministic");
    }
}
