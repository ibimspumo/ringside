//! A player's share of an all-in with every hand known: each side pot gets its own
//! exact equity among the players who can win it, with the other known hands removed
//! from the deck.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cards::{Card, CardSet};
use crate::equity::{Method, equity};
use crate::range::{Combo, Range};

/// One pot of an all-in: its size (any unit) and the players, as indices into the
/// holdings, who can win it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SidePot {
    pub amount: f64,
    pub players: Vec<usize>,
}

/// Splits the chips put in into a main pot and side pots. `contributed` holds every
/// player's chips, `live` marks who is still in the hand. A live player can win every
/// pot up to their own contribution; chips of folded players fall into the pots their
/// contribution reaches. Returns `None` when chips lie above the highest live
/// contribution, which a finished hand never leaves.
pub fn side_pots(contributed: &[i64], live: &[bool]) -> Option<Vec<SidePot>> {
    let mut levels: Vec<i64> = contributed.iter().zip(live).filter(|(_, l)| **l).map(|(c, _)| *c).collect();
    levels.sort_unstable();
    levels.dedup();
    let mut pots = Vec::new();
    let mut prev = 0;
    for &level in &levels {
        let amount: i64 = contributed.iter().map(|&c| c.clamp(prev, level) - prev).sum();
        let players: Vec<usize> = (0..contributed.len()).filter(|&i| live[i] && contributed[i] >= level).collect();
        if amount > 0 {
            pots.push(SidePot { amount: amount as f64, players });
        }
        prev = level;
    }
    (contributed.iter().all(|&c| c <= prev)).then_some(pots)
}

/// Exact equity of each holding in percent, all drawn from one deck.
pub fn exact_equity(holdings: &[Vec<Card>], board: &[Card], dead: CardSet) -> Result<Vec<f64>, Error> {
    let ranges = holdings
        .iter()
        .map(|h| match h.as_slice() {
            [a, b] => Ok(Range::single(Combo::new(*a, *b))),
            _ => Err(Error::InvalidInput("every holding needs two cards".into())),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let r = equity(&ranges, CardSet::from_cards(board), dead, Method::Exact)?;
    Ok(r.players.iter().map(|p| p.equity_pct).collect())
}

/// Expected share of player `who` in all pots together and in each pot, in percent.
pub fn pot_shares(holdings: &[Vec<Card>], board: &[Card], pots: &[SidePot], who: usize) -> Result<(f64, Vec<f64>), Error> {
    let total: f64 = pots.iter().map(|p| p.amount).sum();
    if total <= 0.0 {
        return Err(Error::InvalidInput("empty pot".into()));
    }
    let mut won = 0.0;
    let mut per_pot = Vec::with_capacity(pots.len());
    for pot in pots {
        let share = if !pot.players.contains(&who) {
            0.0
        } else if pot.players.len() == 1 {
            100.0
        } else {
            let in_pot: Vec<Vec<Card>> = pot.players.iter().map(|&i| holdings[i].clone()).collect();
            let dead: Vec<Card> =
                (0..holdings.len()).filter(|i| !pot.players.contains(i)).flat_map(|i| holdings[i].clone()).collect();
            let eq = exact_equity(&in_pot, board, CardSet::from_cards(&dead))?;
            eq[pot.players.iter().position(|&i| i == who).expect("checked above")]
        };
        won += share / 100.0 * pot.amount;
        per_pot.push(share);
    }
    Ok((100.0 * won / total, per_pot))
}

/// Expected result of an all-in for player `who`, in the unit of `contributed`: equity
/// in each pot × the pot after rake − own chips. Rake comes off every pot in proportion
/// to its size.
pub fn all_in_ev(
    holdings: &[Vec<Card>],
    board: &[Card],
    contributed: &[i64],
    live: &[bool],
    rake: i64,
    who: usize,
) -> Result<AllInEv, Error> {
    let pots = side_pots(contributed, live).ok_or_else(|| Error::InvalidInput("chips above every live player".into()))?;
    // Holdings are indexed like `contributed`; folded players hold nothing.
    let live_idx: Vec<usize> = (0..contributed.len()).filter(|&i| live[i]).collect();
    let live_hands: Vec<Vec<Card>> = live_idx.iter().map(|&i| holdings[i].clone()).collect();
    let local = |i: usize| live_idx.iter().position(|&j| j == i).expect("pot players are live");
    let local_pots: Vec<SidePot> =
        pots.iter().map(|p| SidePot { amount: p.amount, players: p.players.iter().map(|&i| local(i)).collect() }).collect();
    let who_local = live_idx.iter().position(|&j| j == who).ok_or_else(|| Error::InvalidInput("player folded".into()))?;
    let (share_pct, _) = pot_shares(&live_hands, board, &local_pots, who_local)?;
    let pot: i64 = contributed.iter().sum();
    let ev = share_pct / 100.0 * (pot - rake) as f64 - contributed[who] as f64;
    Ok(AllInEv { share_pct, pot, rake, ev, pots })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllInEv {
    /// Share of all pots before rake, in percent.
    pub share_pct: f64,
    pub pot: i64,
    pub rake: i64,
    /// Expected result in chips.
    pub ev: f64,
    /// Pots with players indexed like `contributed`.
    pub pots: Vec<SidePot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(s: &str) -> Vec<Card> {
        CardSet::parse(s).unwrap().to_vec()
    }

    #[test]
    fn side_pots_follow_contributions() {
        // A folded 10, short all-in 30, two players with 100.
        let pots = side_pots(&[10, 30, 100, 100], &[false, true, true, true]).unwrap();
        assert_eq!(pots, vec![
            SidePot { amount: 100.0, players: vec![1, 2, 3] },
            SidePot { amount: 140.0, players: vec![2, 3] },
        ]);
        assert!(side_pots(&[50, 30, 30], &[false, true, true]).is_none());
    }

    /// Two players, river complete: the better hand wins everything, the EV is the
    /// pot minus rake minus the own stake.
    #[test]
    fn heads_up_on_the_river_board() {
        let hands = [h("AsAh"), h("KsKh")];
        let board = h("2c7d9hTs3c");
        let r = all_in_ev(&hands, &board, &[1000, 1000], &[true, true], 50, 0).unwrap();
        assert_eq!(r.share_pct, 100.0);
        assert!((r.ev - (2000.0 - 50.0 - 1000.0)).abs() < 1e-9);
        let r = all_in_ev(&hands, &board, &[1000, 1000], &[true, true], 50, 1).unwrap();
        assert!((r.ev + 1000.0).abs() < 1e-9);
    }

    /// AK against AK on a rainbow flop: no flush can come, so every board splits and
    /// each gets half the pot after rake: 0.5 × (1000 − 20) − 500 = −10.
    #[test]
    fn split_pot_with_rake() {
        let hands = [h("AsKd"), h("AhKc")];
        let board = h("2c7d9h");
        let r = all_in_ev(&hands, &board, &[500, 500], &[true, true], 20, 0).unwrap();
        assert!((r.share_pct - 50.0).abs() < 1e-9, "{r:?}");
        assert!((r.ev + 10.0).abs() < 1e-9, "{r:?}");
    }

    /// Short all-in against two covering stacks: hero wins the main pot only.
    #[test]
    fn side_pot_hand_calculation() {
        // Hero (index 1) has the nuts on a full board, all-in for 300; two others put in 700.
        let hands = [vec![], h("AhKh"), h("2s2d"), h("3s3d")];
        let board = h("QhJhTh4c5d");
        let contrib = [100, 300, 700, 700];
        let live = [false, true, true, true];
        let r = all_in_ev(&hands, &board, &contrib, &live, 0, 1).unwrap();
        // Main pot: 100 + 3 × 300 = 1000; side pot 800 goes to 3s3d.
        assert!((r.ev - (1000.0 - 300.0)).abs() < 1e-9, "{r:?}");
        assert!((r.share_pct - 100.0 * 1000.0 / 1800.0).abs() < 1e-9);
        let r = all_in_ev(&hands, &board, &contrib, &live, 0, 3).unwrap();
        assert!((r.ev - (800.0 - 700.0)).abs() < 1e-9, "{r:?}");
    }
}
