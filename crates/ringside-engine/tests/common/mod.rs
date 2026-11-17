//! Slow, independent reference implementations used to check the engine.
#![allow(dead_code)]

use ringside_engine::Card;

/// Naive 5-card value: (category 0..=8, tiebreak ranks high to low). Comparing the
/// tuples lexicographically orders hands.
pub fn naive5(cards: &[Card; 5]) -> (u8, [u8; 5]) {
    let mut ranks: [u8; 5] = cards.map(|c| c.rank());
    ranks.sort_unstable_by(|a, b| b.cmp(a));
    let flush = cards.iter().all(|c| c.suit() == cards[0].suit());
    let distinct = ranks.windows(2).all(|w| w[0] != w[1]);
    let straight_high = if distinct && ranks[0] - ranks[4] == 4 {
        Some(ranks[0])
    } else if ranks == [12, 3, 2, 1, 0] {
        Some(3)
    } else {
        None
    };
    // Group ranks by multiplicity, bigger groups first, then higher rank.
    let mut groups: Vec<(u8, u8)> = Vec::new();
    for &r in &ranks {
        match groups.iter_mut().find(|g| g.1 == r) {
            Some(g) => g.0 += 1,
            None => groups.push((1, r)),
        }
    }
    groups.sort_unstable_by(|a, b| b.cmp(a));
    let mut tiebreak = [0u8; 5];
    let mut i = 0;
    for &(n, r) in &groups {
        for _ in 0..n {
            tiebreak[i] = r;
            i += 1;
        }
    }
    let counts: Vec<u8> = groups.iter().map(|g| g.0).collect();
    let category = if let (Some(h), true) = (straight_high, flush) {
        return (8, [h, 0, 0, 0, 0]);
    } else if counts[0] == 4 {
        7
    } else if counts == [3, 2] {
        6
    } else if flush {
        5
    } else if let Some(h) = straight_high {
        return (4, [h, 0, 0, 0, 0]);
    } else if counts[0] == 3 {
        3
    } else if counts == [2, 2, 1] {
        2
    } else if counts[0] == 2 {
        1
    } else {
        0
    };
    (category, tiebreak)
}

/// Best naive value over all 5-card subsets.
pub fn naive_best(cards: &[Card]) -> (u8, [u8; 5]) {
    let n = cards.len();
    let mut best = (0, [0; 5]);
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                for d in c + 1..n {
                    for e in d + 1..n {
                        best =
                            best.max(naive5(&[cards[a], cards[b], cards[c], cards[d], cards[e]]));
                    }
                }
            }
        }
    }
    best
}

/// Every k-subset of `items`, in lexicographic index order.
pub fn subsets<T: Copy>(items: &[T], k: usize) -> Vec<Vec<T>> {
    let mut out = Vec::new();
    let mut cur = Vec::with_capacity(k);
    fn rec<T: Copy>(items: &[T], k: usize, start: usize, cur: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..items.len() {
            cur.push(items[i]);
            rec(items, k, i + 1, cur, out);
            cur.pop();
        }
    }
    rec(items, k, 0, &mut cur, &mut out);
    out
}

/// Brute-force equity fractions (win, tie, pot share) for fixed hole cards.
pub fn brute_force_equity(
    holes: &[[Card; 2]],
    board: &[Card],
    dead: &[Card],
) -> Vec<(f64, f64, f64)> {
    let mut used: Vec<Card> = board.iter().chain(dead).copied().collect();
    used.extend(holes.iter().flatten());
    let deck: Vec<Card> = (0..52)
        .map(Card::from_index)
        .filter(|c| !used.contains(c))
        .collect();
    let runouts = subsets(&deck, 5 - board.len());
    let mut acc = vec![(0.0, 0.0, 0.0); holes.len()];
    for runout in &runouts {
        let full: Vec<Card> = board.iter().chain(runout).copied().collect();
        let values: Vec<_> = holes
            .iter()
            .map(|h| {
                let mut seven = full.clone();
                seven.extend_from_slice(h);
                naive_best(&seven)
            })
            .collect();
        let best = *values.iter().max().unwrap();
        let winners = values.iter().filter(|v| **v == best).count();
        for (i, v) in values.iter().enumerate() {
            if *v == best {
                if winners == 1 {
                    acc[i].0 += 1.0;
                } else {
                    acc[i].1 += 1.0;
                }
                acc[i].2 += 1.0 / winners as f64;
            }
        }
    }
    let n = runouts.len() as f64;
    acc.into_iter()
        .map(|(w, t, e)| (w / n, t / n, e / n))
        .collect()
}
