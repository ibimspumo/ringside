//! All-in equity of 2–9 players holding specific hands or ranges.
//!
//! With ranges, every combination of non-conflicting holdings is weighted by the
//! product of the players' combo weights; boards are uniform over the remaining deck.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cards::{Card, CardSet, binomial};
use crate::eval::Hand;
use crate::range::{Combo, Range};

pub const MAX_PLAYERS: usize = 9;
/// lcm(1..=9): a pot split k ways gives each winner `SHARE / k` exactly.
const SHARE: u64 = 2520;
const MC_CHUNK: u64 = 1 << 14;
/// Below this many boards per holding, enumerate on one thread.
const PARALLEL_BOARDS: u64 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Method {
    /// Exact when at most `max_exact_showdowns` (an upper bound counting conflicting
    /// holdings too) would be evaluated, otherwise Monte Carlo.
    Auto {
        max_exact_showdowns: u64,
        samples: u64,
        seed: u64,
    },
    Exact,
    MonteCarlo {
        samples: u64,
        seed: u64,
    },
}

impl Default for Method {
    fn default() -> Self {
        Method::Auto {
            max_exact_showdowns: 2_000_000_000,
            samples: 10_000_000,
            seed: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerEquity {
    /// Share of showdowns won alone.
    pub win_pct: f64,
    /// Share of showdowns where the pot is split with this player in it.
    pub tie_pct: f64,
    /// Pot share: wins plus each split counted as 1/k.
    pub equity_pct: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquityResult {
    pub players: Vec<PlayerEquity>,
    pub exact: bool,
    /// Showdowns enumerated (exact) or samples drawn (Monte Carlo).
    pub showdowns: u64,
}

pub fn equity(
    players: &[Range],
    board: CardSet,
    dead: CardSet,
    method: Method,
) -> Result<EquityResult, Error> {
    let spot = Spot::new(players, board, dead)?;
    match method {
        Method::Exact => spot.exact(),
        Method::Auto {
            max_exact_showdowns,
            ..
        } if spot.exact_upper_bound() <= max_exact_showdowns as f64 => spot.exact(),
        Method::Auto { samples, seed, .. } | Method::MonteCarlo { samples, seed } => {
            spot.monte_carlo(samples, seed)
        }
    }
}

/// Showdown tallies of one holding (or one Monte Carlo chunk).
#[derive(Clone, Copy, Default)]
struct Tally {
    win: [u64; MAX_PLAYERS],
    tie: [u64; MAX_PLAYERS],
    /// In units of `1 / SHARE` pots.
    eq: [u64; MAX_PLAYERS],
    showdowns: u64,
}

impl Tally {
    fn merge(mut self, o: Tally) -> Tally {
        for i in 0..MAX_PLAYERS {
            self.win[i] += o.win[i];
            self.tie[i] += o.tie[i];
            self.eq[i] += o.eq[i];
        }
        self.showdowns += o.showdowns;
        self
    }

    #[inline(always)]
    fn showdown(&mut self, hands: &[Hand], board: Hand) {
        let mut ranks = [0u16; MAX_PLAYERS];
        let mut best = 0;
        for (r, h) in ranks.iter_mut().zip(hands) {
            *r = h.combine(board).rank().0;
            best = best.max(*r);
        }
        let ranks = &ranks[..hands.len()];
        let winners = ranks.iter().filter(|&&r| r == best).count();
        for (i, &r) in ranks.iter().enumerate() {
            if r == best {
                if winners == 1 {
                    self.win[i] += 1;
                } else {
                    self.tie[i] += 1;
                }
                self.eq[i] += SHARE / winners as u64;
            }
        }
        self.showdowns += 1;
    }
}

/// Weighted sums over holdings, as fractions of `total` weight.
#[derive(Clone, Copy, Default)]
struct Weighted {
    win: [f64; MAX_PLAYERS],
    tie: [f64; MAX_PLAYERS],
    eq: [f64; MAX_PLAYERS],
    total: f64,
    showdowns: u64,
}

impl Weighted {
    fn add(mut self, t: &Tally, w: f64) -> Weighted {
        for i in 0..MAX_PLAYERS {
            self.win[i] += w * t.win[i] as f64;
            self.tie[i] += w * t.tie[i] as f64;
            self.eq[i] += w * t.eq[i] as f64 / SHARE as f64;
        }
        self.total += w * t.showdowns as f64;
        self.showdowns += t.showdowns;
        self
    }

    fn merge(mut self, o: Weighted) -> Weighted {
        for i in 0..MAX_PLAYERS {
            self.win[i] += o.win[i];
            self.tie[i] += o.tie[i];
            self.eq[i] += o.eq[i];
        }
        self.total += o.total;
        self.showdowns += o.showdowns;
        self
    }
}

struct Spot {
    /// Live combos per player with their weights.
    live: Vec<Vec<(Combo, f64)>>,
    board: CardSet,
    known: CardSet,
}

impl Spot {
    fn new(players: &[Range], board: CardSet, dead: CardSet) -> Result<Spot, Error> {
        let invalid = |m: String| Err(Error::InvalidInput(m));
        if !(2..=MAX_PLAYERS).contains(&players.len()) {
            return invalid(format!("need 2 to 9 players, got {}", players.len()));
        }
        if ![0, 3, 4, 5].contains(&board.len()) {
            return invalid(format!(
                "board must have 0, 3, 4 or 5 cards, got {}",
                board.len()
            ));
        }
        if board.intersects(dead) {
            return invalid("board and dead cards overlap".into());
        }
        let known = board.union(dead);
        if 52 - known.len() < 2 * players.len() as u32 + (5 - board.len()) {
            return invalid("not enough cards left in the deck".into());
        }
        let live: Vec<Vec<(Combo, f64)>> = players
            .iter()
            .map(|r| {
                r.live_combos(known)
                    .into_iter()
                    .map(|(c, w)| (c, w as f64))
                    .collect()
            })
            .collect();
        if let Some(i) = live.iter().position(Vec::is_empty) {
            return invalid(format!("player {} has no live combos", i + 1));
        }
        Ok(Spot { live, board, known })
    }

    fn missing(&self) -> u32 {
        5 - self.board.len()
    }

    fn boards_per_holding(&self) -> u64 {
        let rest = 52 - self.known.len() - 2 * self.live.len() as u32;
        binomial(rest as u64, self.missing() as u64)
    }

    fn exact_upper_bound(&self) -> f64 {
        let holdings: f64 = self.live.iter().map(|l| l.len() as f64).product();
        holdings * self.boards_per_holding() as f64
    }

    fn exact(&self) -> Result<EquityResult, Error> {
        let (l0, l1) = (&self.live[0], &self.live[1]);
        let heads: Vec<(usize, usize)> = (0..l0.len())
            .flat_map(|i| (0..l1.len()).map(move |j| (i, j)))
            .filter(|&(i, j)| !l0[i].0.mask().intersects(l1[j].0.mask()))
            .collect();
        let board_hand = Hand::from_set(self.board);
        let sum = heads
            .par_iter()
            .map(|&(i, j)| {
                let mut holes = [Combo::from_index(0); MAX_PLAYERS];
                holes[0] = l0[i].0;
                holes[1] = l1[j].0;
                let used = self.known.union(l0[i].0.mask()).union(l1[j].0.mask());
                self.exact_rest(2, &mut holes, used, l0[i].1 * l1[j].1, board_hand)
            })
            .reduce(Weighted::default, Weighted::merge);
        if sum.total == 0.0 {
            return Err(Error::InvalidInput(
                "no holding without card conflicts".into(),
            ));
        }
        Ok(self.result(&sum, true))
    }

    fn exact_rest(
        &self,
        p: usize,
        holes: &mut [Combo; MAX_PLAYERS],
        used: CardSet,
        w: f64,
        board: Hand,
    ) -> Weighted {
        let n = self.live.len();
        if p == n {
            let mut hands = [Hand::EMPTY; MAX_PLAYERS];
            for (h, c) in hands.iter_mut().zip(&holes[..n]) {
                let (a, b) = c.cards();
                *h = board.with(a).with(b);
            }
            let avail: Vec<Card> = used.complement().iter().collect();
            let tally = enumerate_boards(&avail, self.missing() as usize, &hands[..n]);
            return Weighted::default().add(&tally, w);
        }
        let mut acc = Weighted::default();
        for &(c, cw) in &self.live[p] {
            if c.mask().intersects(used) {
                continue;
            }
            holes[p] = c;
            acc = acc.merge(self.exact_rest(p + 1, holes, used.union(c.mask()), w * cw, board));
        }
        acc
    }

    fn monte_carlo(&self, samples: u64, seed: u64) -> Result<EquityResult, Error> {
        if samples == 0 {
            return Err(Error::InvalidInput("need at least one sample".into()));
        }
        let cumulative: Vec<Vec<f64>> = self
            .live
            .iter()
            .map(|l| {
                l.iter()
                    .scan(0.0, |s, (_, w)| {
                        *s += w;
                        Some(*s)
                    })
                    .collect()
            })
            .collect();
        let chunks = samples.div_ceil(MC_CHUNK);
        let board_hand = Hand::from_set(self.board);
        let missing = self.missing();
        let n = self.live.len();
        let tally = (0..chunks)
            .into_par_iter()
            .map(|chunk| {
                let mut rng = ChaCha8Rng::seed_from_u64(seed);
                rng.set_stream(chunk);
                let count = MC_CHUNK.min(samples - chunk * MC_CHUNK);
                let mut tally = Tally::default();
                let mut hands = [Hand::EMPTY; MAX_PLAYERS];
                for _ in 0..count {
                    let mut used =
                        self.sample_holdings(&mut rng, &cumulative, &mut hands, board_hand)?;
                    let mut runout = Hand::EMPTY;
                    for _ in 0..missing {
                        let card = loop {
                            let c = Card::from_index(rng.random_range(0..52));
                            if !used.contains(c) {
                                break c;
                            }
                        };
                        used.insert(card);
                        runout = runout.with(card);
                    }
                    tally.showdown(&hands[..n], runout);
                }
                Ok(tally)
            })
            .try_reduce(Tally::default, |a, b| Ok(a.merge(b)))?;
        Ok(self.result(&Weighted::default().add(&tally, 1.0), false))
    }

    /// Draws one non-conflicting holding by rejection, which samples exactly the
    /// weight-product distribution the exact path enumerates.
    fn sample_holdings(
        &self,
        rng: &mut ChaCha8Rng,
        cumulative: &[Vec<f64>],
        hands: &mut [Hand; MAX_PLAYERS],
        board: Hand,
    ) -> Result<CardSet, Error> {
        'attempt: for _ in 0..1_000_000 {
            let mut used = self.known;
            for (p, cum) in cumulative.iter().enumerate() {
                let total = *cum.last().expect("non-empty");
                let x = rng.random::<f64>() * total;
                let i = cum.partition_point(|&c| c <= x).min(cum.len() - 1);
                let combo = self.live[p][i].0;
                if combo.mask().intersects(used) {
                    continue 'attempt;
                }
                used = used.union(combo.mask());
                let (a, b) = combo.cards();
                hands[p] = board.with(a).with(b);
            }
            return Ok(used);
        }
        Err(Error::InvalidInput(
            "ranges conflict too much to sample a holding".into(),
        ))
    }

    fn result(&self, s: &Weighted, exact: bool) -> EquityResult {
        let pct = |x: f64| 100.0 * x / s.total;
        EquityResult {
            players: (0..self.live.len())
                .map(|i| PlayerEquity {
                    win_pct: pct(s.win[i]),
                    tie_pct: pct(s.tie[i]),
                    equity_pct: pct(s.eq[i]),
                })
                .collect(),
            exact,
            showdowns: s.showdowns,
        }
    }
}

/// Showdowns over every `k`-card runout from `avail`; `hands` already hold hole
/// cards and the known board.
fn enumerate_boards(avail: &[Card], k: usize, hands: &[Hand]) -> Tally {
    if k > 0 && binomial(avail.len() as u64, k as u64) >= PARALLEL_BOARDS {
        return (0..=avail.len() - k)
            .into_par_iter()
            .map(|i| {
                let mut t = Tally::default();
                enumerate_rec(
                    &avail[i + 1..],
                    k - 1,
                    Hand::EMPTY.with(avail[i]),
                    hands,
                    &mut t,
                );
                t
            })
            .reduce(Tally::default, Tally::merge);
    }
    let mut t = Tally::default();
    enumerate_rec(avail, k, Hand::EMPTY, hands, &mut t);
    t
}

fn enumerate_rec(avail: &[Card], k: usize, runout: Hand, hands: &[Hand], t: &mut Tally) {
    if k == 0 {
        t.showdown(hands, runout);
        return;
    }
    for i in 0..=avail.len() - k {
        enumerate_rec(&avail[i + 1..], k - 1, runout.with(avail[i]), hands, t);
    }
}
