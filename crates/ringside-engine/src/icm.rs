//! Malmuth–Harville ICM: places are handed out top-down, each to a remaining player
//! with probability proportional to their chips.
//!
//! Players with 0 chips are already busted: they finish below everyone with chips and
//! share the lowest places uniformly (one zero-stack player gets exactly the last
//! payout, two of them average the last two, …).

use serde::{Deserialize, Serialize};

use crate::Error;

pub const MAX_PLAYERS: usize = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IcmResult {
    /// `finish[player][place]`, place 0 = first.
    pub finish: Vec<Vec<f64>>,
    /// Expected payout per player.
    pub ev: Vec<f64>,
}

/// `payouts[place]` for place 0 = first; missing places pay nothing.
pub fn icm(stacks: &[f64], payouts: &[f64]) -> Result<IcmResult, Error> {
    let n = stacks.len();
    if !(1..=MAX_PLAYERS).contains(&n) {
        return Err(Error::InvalidInput(format!(
            "ICM needs 1 to 10 players, got {n}"
        )));
    }
    if stacks.iter().any(|s| !s.is_finite() || *s < 0.0) {
        return Err(Error::InvalidInput(
            "stacks must be finite and non-negative".into(),
        ));
    }
    if payouts.len() > n || payouts.iter().any(|p| !p.is_finite()) {
        return Err(Error::InvalidInput(
            "need at most one finite payout per player".into(),
        ));
    }

    // prob[mask] = P(the players in `mask` take exactly the top |mask| places).
    let full = 1usize << n;
    let mut prob = vec![0.0f64; full];
    prob[0] = 1.0;
    let mut finish = vec![vec![0.0f64; n]; n];
    for mask in 0..full - 1 {
        let p = prob[mask];
        if p == 0.0 {
            continue;
        }
        let place = mask.count_ones() as usize;
        let remaining = (0..n).filter(|&j| mask & 1 << j == 0);
        let chips: f64 = remaining.clone().map(|j| stacks[j]).sum();
        let left = n - place;
        for j in remaining {
            let q = if chips > 0.0 {
                stacks[j] / chips
            } else {
                1.0 / left as f64
            };
            if q > 0.0 {
                finish[j][place] += p * q;
                prob[mask | 1 << j] += p * q;
            }
        }
    }
    let ev = finish
        .iter()
        .map(|f| f.iter().zip(payouts).map(|(p, pay)| p * pay).sum())
        .collect();
    Ok(IcmResult { finish, ev })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(stacks: &[f64], payouts: &[f64]) -> Vec<f64> {
        icm(stacks, payouts).unwrap().ev
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn ev_sums_to_prize_pool() {
        let payouts = [50.0, 30.0, 20.0];
        for stacks in [
            vec![1000.0, 2000.0, 3000.0, 4000.0, 500.0],
            vec![1.0, 1.0, 1.0],
            vec![
                8000.0, 1500.0, 300.0, 200.0, 100.0, 50.0, 20.0, 10.0, 5.0, 1.0,
            ],
        ] {
            let total: f64 = ev(&stacks, &payouts).iter().sum();
            assert!(close(total, 100.0), "{stacks:?}: {total}");
            let r = icm(&stacks, &payouts).unwrap();
            for row in &r.finish {
                assert!(close(row.iter().sum(), 1.0));
            }
            for place in 0..stacks.len() {
                assert!(close(r.finish.iter().map(|f| f[place]).sum(), 1.0));
            }
        }
    }

    #[test]
    fn equal_stacks_equal_ev() {
        let e = ev(&[1500.0; 9], &[50.0, 30.0, 20.0]);
        for x in &e {
            assert!(close(*x, 100.0 / 9.0));
        }
    }

    #[test]
    fn heads_up_is_linear() {
        let e = ev(&[3000.0, 1000.0], &[70.0, 30.0]);
        assert!(close(e[0], 60.0) && close(e[1], 40.0), "{e:?}");
    }

    #[test]
    fn known_three_way_value() {
        let e = ev(&[50.0, 30.0, 20.0], &[50.0, 30.0, 20.0]);
        let second = 0.3 * 50.0 / 70.0 + 0.2 * 50.0 / 80.0;
        let third = 1.0 - 0.5 - second;
        assert!(
            close(e[0], 0.5 * 50.0 + second * 30.0 + third * 20.0),
            "{e:?}"
        );
    }

    #[test]
    fn zero_chip_players_take_the_lowest_places() {
        let e = ev(&[100.0, 0.0, 50.0], &[50.0, 30.0, 20.0]);
        assert!(close(e[1], 20.0), "{e:?}");
        let e = ev(&[0.0, 300.0, 0.0, 100.0], &[50.0, 30.0, 15.0, 5.0]);
        assert!(close(e[0], 10.0) && close(e[2], 10.0), "{e:?}");
        let e = ev(&[0.0, 0.0], &[70.0, 30.0]);
        assert!(close(e[0], 50.0) && close(e[1], 50.0));
        let e = ev(&[100.0, 0.0, 0.0], &[60.0]);
        assert!(close(e[0], 60.0) && close(e[1], 0.0));
    }

    #[test]
    fn monotonic_in_stack() {
        let payouts = [50.0, 30.0, 20.0];
        let mut last = f64::NEG_INFINITY;
        for s in (0..=40).map(|i| i as f64 * 250.0) {
            let e = ev(&[s, 2000.0, 3000.0, 1000.0], &payouts)[0];
            assert!(e >= last - 1e-12, "stack {s}: {e} < {last}");
            last = e;
        }
    }

    #[test]
    fn rejects_bad_input() {
        assert!(icm(&[], &[]).is_err());
        assert!(icm(&[1.0; 11], &[1.0]).is_err());
        assert!(icm(&[1.0, -1.0], &[1.0]).is_err());
        assert!(icm(&[1.0, 1.0], &[1.0, 1.0, 1.0]).is_err());
    }
}
