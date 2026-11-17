//! What the prizes mean at the hero's table: the tournament's payouts, how many players
//! are left, and the payouts a push/fold solve at this one table should use.
//!
//! # Payouts
//! A summary that lists every paid place gives the real payouts. Without one,
//! [`standard`] builds a typical structure: 15 % of the field paid (the share PokerStars
//! made common, PokerStake "PokerStars Players Championship Structure …"), paid places
//! and first prize from the "top 15 % paid" table of WSOP Online ("MTT Tournament
//! Payouts"), the smallest prize at 1.75 buy-ins (min cashes are 1.5–2 buy-ins,
//! mypokercoaching.com "Poker Tournament Payouts", shuffleup.co.uk) and a power curve
//! in between that adds up to the prize pool. PokerStars publishes no table per place,
//! so this is an assumption and always labelled as one.
//!
//! # Players left
//! Hand histories name the place of most players who bust at the hero's table, so
//! after such a hand `place − 1` players are left and before it at least `place`.
//! Each hand takes the nearest of these marks, and never fewer than the players at
//! its table.
//!
//! # ICM at one table
//! Stacks at other tables are unknown. At the final table everyone left sits at the
//! table and Malmuth–Harville ICM with the remaining payouts is exact for the model.
//! Before that the table stands in for the field, as in ICMIZER's MTT mode, which
//! fills the other tables with stacks like the ones at the hero's table: each of the
//! `n` table places stands for `L / n` of the `L` places still to be decided and pays
//! their average. Far from the money ICM barely matters and chip EV is used.

use ringside_core::model::Hand;
use ringside_core::tournament::TournamentRow;

/// Smallest prize of the standard structure, in buy-ins.
const MIN_CASH_BUY_INS: f64 = 1.75;
/// Before the money, ICM is used from this many players per paid place on.
const BUBBLE_ZONE: f64 = 1.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Summary,
    Standard,
}

/// Payouts of a tournament in percent of the prize pool, first place first.
#[derive(Clone, Debug, PartialEq)]
pub struct Prizes {
    pub pct: Vec<f64>,
    pub source: Source,
    pub entrants: usize,
    /// The prize pool in cents, when the summary gave real amounts.
    pub pool: Option<i64>,
    /// `false`: the field size is only a lower bound from the hand histories.
    pub entrants_known: bool,
}

/// Paid places and first prize in percent for "top 15 % paid", WSOP Online.
const TOP_15: [(usize, usize, f64); 30] = [
    (2, 1, 100.0),
    (10, 2, 70.0),
    (20, 3, 50.0),
    (30, 5, 37.0),
    (40, 6, 35.0),
    (50, 8, 32.0),
    (60, 9, 30.0),
    (75, 10, 29.0),
    (100, 15, 28.5),
    (125, 20, 28.0),
    (150, 25, 27.5),
    (200, 30, 27.0),
    (250, 35, 26.5),
    (300, 40, 26.0),
    (350, 50, 25.5),
    (400, 60, 25.0),
    (500, 75, 24.5),
    (600, 100, 24.0),
    (700, 125, 23.5),
    (800, 150, 23.0),
    (900, 175, 22.5),
    (1000, 200, 22.0),
    (1250, 225, 21.5),
    (1500, 250, 21.0),
    (1750, 275, 20.5),
    (2000, 300, 20.0),
    (2250, 350, 19.5),
    (2500, 400, 19.0),
    (2750, 450, 18.5),
    (3000, 500, 18.0),
];

/// The standard structure for a field of `entrants` (see the module docs).
pub fn standard(entrants: usize) -> Vec<f64> {
    let entrants = entrants.max(2);
    let (paid, first) = match TOP_15.iter().find(|(max, ..)| entrants <= *max) {
        Some(&(_, paid, first)) => (paid, first),
        None => (((entrants as f64) * 0.15).round() as usize, 17.5),
    };
    if paid == 1 {
        return vec![100.0];
    }
    // The floor never pushes the sum past the pool.
    let floor = (100.0 * MIN_CASH_BUY_INS / entrants as f64).min((100.0 - first) / (paid - 1) as f64);
    let curve = |alpha: f64| -> Vec<f64> {
        (1..=paid).map(|k| if k == 1 { first } else { floor + (first - floor) * (k as f64).powf(-alpha) }).collect()
    };
    let sum = |alpha: f64| curve(alpha).iter().sum::<f64>();
    let (mut lo, mut hi) = (0.0, 50.0);
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if sum(mid) > 100.0 { lo = mid } else { hi = mid }
    }
    let mut pct = curve(hi);
    let total: f64 = pct.iter().sum();
    pct.iter_mut().for_each(|p| *p *= 100.0 / total);
    pct
}

/// The payouts of a tournament: from its summary when complete, else the standard
/// structure for its field (known from the summary, or at least the highest place
/// seen in the hands). `None` when the field size is unknown.
pub fn prizes(row: &TournamentRow, hands: &[Hand]) -> Option<Prizes> {
    if !row.payouts.is_empty() {
        let pool: i64 = row.payouts.iter().sum();
        if pool > 0 {
            return Some(Prizes {
                pct: row.payouts.iter().map(|&a| 100.0 * a as f64 / pool as f64).collect(),
                source: Source::Summary,
                entrants: row.entrants.map_or(row.payouts.len(), |e| e as usize),
                pool: Some(pool),
                entrants_known: row.entrants.is_some(),
            });
        }
    }
    let seen = hands.iter().flat_map(|h| &h.finishes).filter_map(|f| f.place).max();
    let (entrants, known) = match (row.entrants, seen) {
        (Some(e), _) if e >= 2 => (e as usize, true),
        (_, Some(p)) if p >= 2 => (p as usize, false),
        _ => return None,
    };
    Some(Prizes { pct: standard(entrants), source: Source::Standard, entrants, pool: None, entrants_known: known })
}

/// Players still in the tournament when each hand starts, if the hands tell.
pub fn players_left(hands: &[Hand]) -> Vec<Option<usize>> {
    let marks: Vec<Option<(i64, i64)>> = hands
        .iter()
        .map(|h| {
            let places: Vec<i64> = h.finishes.iter().filter_map(|f| f.place).collect();
            Some((*places.iter().min()?, *places.iter().max()?))
        })
        .collect();
    (0..hands.len())
        .map(|i| {
            let before = (0..i).rev().find_map(|k| marks[k].map(|(min, _)| (i - k, min - 1)));
            let from = (i..hands.len()).find_map(|k| marks[k].map(|(_, max)| (k - i, max)));
            let left = match (before, from) {
                (Some(b), Some(f)) => if f.0 <= b.0 { f.1 } else { b.1 },
                (Some(b), None) => b.1,
                (None, Some(f)) => f.1,
                (None, None) => return None,
            };
            Some((left.max(1) as usize).max(hands[i].active_players().count()))
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Far from the money: chip EV.
    Early,
    /// Close to the money: ICM, approximated at the table.
    Bubble,
    /// In the money before the final table: ICM, approximated at the table.
    InTheMoney,
    /// Everyone left sits at this table: exact ICM.
    FinalTable,
}

/// The prize situation of one hand.
#[derive(Clone, Debug, PartialEq)]
pub struct Situation {
    pub phase: Phase,
    pub left: usize,
    pub paid: usize,
    /// Payouts for the players at the table, percent of the prize pool, first first;
    /// empty with chip EV.
    pub table: Vec<f64>,
}

impl Situation {
    pub fn icm(&self) -> bool {
        !self.table.is_empty()
    }
}

/// The phase and table payouts for `left` players in the tournament and `seated` at the table.
pub fn situation(prizes: &Prizes, left: usize, seated: usize) -> Situation {
    let paid = prizes.pct.len();
    let left = left.max(seated);
    let phase = if left <= seated {
        Phase::FinalTable
    } else if left <= paid {
        Phase::InTheMoney
    } else if (left as f64) <= (BUBBLE_ZONE * paid as f64).max((paid + seated) as f64) {
        Phase::Bubble
    } else {
        Phase::Early
    };
    let table = if phase == Phase::Early { Vec::new() } else { table_payouts(&prizes.pct, left, seated) };
    Situation { phase, left, paid, table }
}

/// Table place `j` of `seated` pays the average of field places `(j − 1)·L/n … j·L/n`.
fn table_payouts(pct: &[f64], left: usize, seated: usize) -> Vec<f64> {
    let prize = |place: usize| pct.get(place).copied().unwrap_or(0.0);
    let width = left as f64 / seated as f64;
    (0..seated)
        .map(|j| {
            let (a, b) = (j as f64 * width, (j + 1) as f64 * width);
            let mut total = 0.0;
            let mut x = a;
            while x < b - 1e-9 {
                let place = x.floor() as usize;
                let end = ((place + 1) as f64).min(b);
                total += prize(place) * (end - x);
                x = end;
            }
            total / width
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_structure_adds_up_and_falls() {
        for entrants in [2, 6, 9, 18, 45, 180, 1000, 5000] {
            let pct = standard(entrants);
            let total: f64 = pct.iter().sum();
            assert!((total - 100.0).abs() < 1e-9, "{entrants}: {total}");
            assert!(pct.windows(2).all(|w| w[0] >= w[1] - 1e-12), "{entrants}: {pct:?}");
        }
        let big = standard(1000);
        assert_eq!(big.len(), 200);
        assert!((big[0] - 22.0).abs() < 0.5, "{}", big[0]);
        // The smallest prize near 1.75 buy-ins (a buy-in is 0.1 % of this pool).
        assert!((0.15..0.2).contains(big.last().unwrap()), "{}", big.last().unwrap());
        assert_eq!(standard(9).len(), 2);
    }

    #[test]
    fn table_payouts_split_the_field() {
        let pct = [50.0, 30.0, 20.0];
        // Final table: the remaining payouts themselves.
        assert_eq!(table_payouts(&pct, 3, 3), vec![50.0, 30.0, 20.0]);
        // Six left, three seated: each table place stands for two field places.
        assert_eq!(table_payouts(&pct, 6, 3), vec![40.0, 10.0, 0.0]);
        let p = Prizes { pct: pct.to_vec(), source: Source::Standard, entrants: 9, pool: None, entrants_known: true };
        assert_eq!(situation(&p, 3, 3).phase, Phase::FinalTable);
        assert_eq!(situation(&p, 5, 3).phase, Phase::Bubble);
        let field = Prizes { pct: standard(1000), source: Source::Standard, entrants: 1000, pool: None, entrants_known: true };
        assert_eq!(situation(&field, 600, 8).phase, Phase::Early);
        assert!(!situation(&field, 600, 8).icm());
        assert_eq!(situation(&field, 230, 8).phase, Phase::Bubble);
        assert_eq!(situation(&field, 120, 8).phase, Phase::InTheMoney);
    }
}
