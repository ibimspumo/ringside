//! Playing on where no solution reaches: a plain betting round for any number of players.
//! It carries a hand after the flop with three players (the solver plays heads-up only),
//! after a live solve was stopped, and preflop where the model has no decision for the
//! hero. The opponents follow [`RULE`], which the page names; nothing here grades the
//! hero (see [`crate::assess`]).

use rand::Rng;
use ringside_engine::equity::{Method, equity};
use ringside_engine::{Card, CardSet, Combo, Range};

use crate::hand::Choice;

/// The opponents' rule in plain words, as the page shows it.
pub const RULE: &str = "Die Gegner spielen hier nach einer einfachen Regel: Sie setzen und erhöhen mit starken Händen, callen, wenn ihre Gewinnchance gegen zufällige Hände den Preis deckt, und folden sonst.";

/// Bets in percent of the pot, the sizes the solver uses on the flop and later streets.
const BETS: [f64; 2] = [33.0, 75.0];
/// A raise adds this share of the pot after calling.
const RAISE: f64 = 0.75;
/// A bet or raise that leaves less than this share of the stack behind goes all in.
const ALL_IN_SHARE: f64 = 0.85;
/// Boards dealt to estimate an opponent's chance to win.
const SAMPLES: u64 = 1500;

/// One street's betting.
#[derive(Clone, Debug)]
pub struct Round {
    /// Seats in the order they act on this street.
    order: Vec<usize>,
    /// Chips in front on this street, BB.
    pub bets: Vec<f64>,
    /// Chips behind, BB.
    pub behind: Vec<f64>,
    pub folded: Vec<bool>,
    /// Acted since the last bet or raise.
    acted: Vec<bool>,
    /// Size of the last bet or raise, the least a raise adds.
    last_raise: f64,
    pub to_act: Option<usize>,
}

impl Round {
    /// A street from its start: `order` acts first to last.
    pub fn start(order: Vec<usize>, behind: Vec<f64>, folded: Vec<bool>) -> Round {
        let n = behind.len();
        let mut r = Round { order, bets: vec![0.0; n], behind, folded, acted: vec![false; n], last_raise: 1.0, to_act: None };
        r.to_act = r.next(None);
        r
    }

    /// A street already under way: `acted` seats had their turn since the last raise and
    /// `last` acted last.
    pub fn resume(order: Vec<usize>, bets: Vec<f64>, behind: Vec<f64>, folded: Vec<bool>, acted: Vec<bool>, last: Option<usize>) -> Round {
        let n = bets.len();
        let mut sorted = bets.clone();
        sorted.sort_by(|a, b| b.total_cmp(a));
        let last_raise = (sorted[0] - sorted.get(1).copied().unwrap_or(0.0)).max(1.0);
        let mut r = Round { order, bets, behind, folded, acted, last_raise, to_act: None };
        debug_assert_eq!(r.acted.len(), n);
        r.to_act = r.next(last);
        r
    }

    fn top(&self) -> f64 {
        self.bets.iter().copied().fold(0.0, f64::max)
    }

    fn can_act(&self, s: usize) -> bool {
        !self.folded[s] && self.behind[s] > 1e-9
    }

    /// The next seat after `last` that still has to act, if any.
    fn next(&self, last: Option<usize>) -> Option<usize> {
        let k = self.order.len();
        let from = last.and_then(|l| self.order.iter().position(|&s| s == l)).map_or(0, |i| i + 1);
        let top = self.top();
        // Alone with chips behind and nothing to call: nobody is left to bet against.
        let others = |s: usize| self.order.iter().any(|&o| o != s && self.can_act(o));
        (0..k).map(|i| self.order[(from + i) % k]).find(|&s| {
            self.can_act(s) && (self.bets[s] < top - 1e-9 || (!self.acted[s] && others(s)))
        })
    }

    pub fn to_call(&self, s: usize) -> f64 {
        (self.top() - self.bets[s]).min(self.behind[s]).max(0.0)
    }

    /// What `s` may do with `pot` in the middle (bets in front included).
    pub fn choices(&self, s: usize, pot: f64) -> Vec<Choice> {
        let before = self.bets[s];
        let all = before + self.behind[s];
        let call = self.to_call(s);
        let contested = self.order.iter().any(|&o| o != s && self.can_act(o));
        let mut out = Vec::new();
        let sized = |to: f64, kind: &'static str, pct: f64| -> Option<Choice> {
            (to < before + ALL_IN_SHARE * (all - before)).then_some(Choice { kind, to_bb: to, add_bb: to - before, pct_pot: Some(pct) })
        };
        if call > 1e-9 {
            out.push(Choice { kind: "fold", to_bb: before, add_bb: 0.0, pct_pot: None });
            out.push(Choice { kind: "call", to_bb: before + call, add_bb: call, pct_pot: None });
            if contested && self.behind[s] > call + 1e-9 {
                let top = self.top();
                let add = (RAISE * (pot + call)).max(self.last_raise);
                out.extend(sized(round2(top + add), "raise", add / (pot + call) * 100.0));
                out.push(Choice { kind: "all_in", to_bb: all, add_bb: all - before, pct_pot: None });
            }
        } else {
            out.push(Choice { kind: "check", to_bb: before, add_bb: 0.0, pct_pot: None });
            if contested {
                for pct in BETS {
                    let to = round2((pct / 100.0 * pot).max(1.0));
                    out.extend(sized(before + to, "bet", pct).filter(|c| out.iter().all(|o| (o.to_bb - c.to_bb).abs() > 1e-9)));
                }
                out.push(Choice { kind: "all_in", to_bb: all, add_bb: all - before, pct_pot: Some(self.behind[s] / pot * 100.0) });
            }
        }
        out
    }

    /// `s` takes `c`; returns the chips it puts in.
    pub fn play(&mut self, s: usize, c: &Choice) -> f64 {
        let top = self.top();
        let add = c.add_bb.min(self.behind[s]);
        if c.kind == "fold" {
            self.folded[s] = true;
        }
        self.bets[s] += add;
        self.behind[s] -= add;
        if self.bets[s] > top + 1e-9 {
            self.last_raise = self.last_raise.max(self.bets[s] - top);
            self.acted.iter_mut().for_each(|a| *a = false);
        }
        self.acted[s] = true;
        self.to_act = self.next(Some(s));
        add
    }
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/// An opponent's share of the pot against `opponents` random hands on `board`.
pub fn strength(hole: [Card; 2], board: &[Card], opponents: usize, seed: u64) -> f64 {
    let mut players = vec![Range::single(Combo::new(hole[0], hole[1]))];
    players.extend((0..opponents.max(1)).map(|_| Range::all()));
    let method = Method::MonteCarlo { samples: SAMPLES, seed };
    equity(&players, CardSet::from_cards(board), CardSet::EMPTY, method).map_or(0.0, |r| r.players[0].equity_pct / 100.0)
}

/// [`RULE`]: the move an opponent with `strength` (its share of the pot against random
/// hands) makes among `choices`, with `players` in the hand.
pub fn rule(strength: f64, players: usize, choices: &[Choice], pot: f64, rng: &mut impl Rng) -> usize {
    let fair = 1.0 / players.max(2) as f64;
    let find = |kind: &str| choices.iter().position(|c| c.kind == kind);
    let strong = strength >= (fair * 1.6).min(0.8);
    // The aggressive move: the smaller sized one, all-in where there is none.
    let aggressive = choices.iter().position(|c| c.kind == "bet" || c.kind == "raise").or_else(|| find("all_in"));
    match find("call") {
        Some(call) => {
            let price = choices[call].add_bb / (pot + choices[call].add_bb);
            if strong && strength >= price && rng.random::<f64>() < 0.5 {
                aggressive.unwrap_or(call)
            } else if strength >= price {
                call
            } else {
                0
            }
        }
        None if strong => aggressive.unwrap_or(0),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(n: usize) -> Round {
        Round::start((0..n).collect(), vec![20.0; n], vec![false; n])
    }

    #[test]
    fn a_street_ends_once_everyone_matched_the_bet() {
        let mut r = round(3);
        assert_eq!(r.to_act, Some(0));
        let check = r.choices(0, 6.0)[0].clone();
        r.play(0, &check);
        let bet = r.choices(1, 6.0).into_iter().find(|c| c.kind == "bet").unwrap();
        assert!((bet.to_bb - 1.98).abs() < 1e-9);
        r.play(1, &bet);
        assert_eq!(r.to_act, Some(2));
        let fold = r.choices(2, 8.0)[0].clone();
        r.play(2, &fold);
        // The first player checked before the bet and has to answer it.
        assert_eq!(r.to_act, Some(0));
        let call = r.choices(0, 8.0)[1].clone();
        assert_eq!(call.kind, "call");
        r.play(0, &call);
        assert_eq!(r.to_act, None);
        assert_eq!(r.bets, vec![1.98, 1.98, 0.0]);
    }

    #[test]
    fn nobody_bets_into_players_who_are_all_in() {
        let mut r = Round::start(vec![0, 1], vec![10.0, 30.0], vec![false; 2]);
        let shove = r.choices(0, 4.0).into_iter().find(|c| c.kind == "all_in").unwrap();
        r.play(0, &shove);
        let answers: Vec<&str> = r.choices(1, 14.0).iter().map(|c| c.kind).collect();
        assert_eq!(answers, ["fold", "call"]);
        let call = r.choices(1, 14.0)[1].clone();
        r.play(1, &call);
        assert_eq!(r.to_act, None);
    }

    #[test]
    fn the_rule_calls_when_the_price_is_right() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        let choices = vec![
            Choice { kind: "fold", to_bb: 0.0, add_bb: 0.0, pct_pot: None },
            Choice { kind: "call", to_bb: 5.0, add_bb: 5.0, pct_pot: None },
        ];
        // 5 into 10: a call needs a third of the pot.
        assert_eq!(rule(0.30, 2, &choices, 10.0, &mut rng), 0);
        assert_eq!(rule(0.40, 2, &choices, 10.0, &mut rng), 1);
        let card = |c: &str| c.parse().unwrap();
        let aces = strength([card("As"), card("Ah")], &[], 1, 7);
        assert!((0.8..0.9).contains(&aces), "{aces}");
        // A set on the flop against two random hands, and air.
        let board = [card("7d"), card("7c"), card("2s")];
        let set = strength([card("7s"), card("Kd")], &board, 2, 7);
        let air = strength([card("4h"), card("3h")], &board, 2, 7);
        assert!(set > 0.85 && air < 0.3, "set {set}, air {air}");
    }

    use rand::SeedableRng;
}
