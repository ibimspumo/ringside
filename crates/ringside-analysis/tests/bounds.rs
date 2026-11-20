//! Every rated decision keeps the hard bounds of `ringside_analysis::check`, over random
//! stacks and preflop lines: opens, 3-bets, limps and short all-ins with a deep caller,
//! the hero folding, calling or going all-in, each hand played to its end.

use std::collections::HashMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_analysis::check::check;
use ringside_analysis::{Status, analyze_hand, estimate_rest};
use ringside_core::model::Hand;
use ringside_core::parse_hand;

const SB: i64 = 300;
const BB: i64 = 600;
const ANTE: i64 = 75;
/// Preflop order after the blinds: seats 3 to 8, then the small and the big blind.
const ORDER: [usize; 8] = [3, 4, 5, 6, 7, 8, 1, 2];

struct Table {
    stacks: [i64; 9],
    put: HashMap<usize, i64>,
    folded: Vec<usize>,
    text: String,
}

impl Table {
    fn new(stacks: [i64; 9]) -> Table {
        let mut text = String::from(
            "PokerStars Hand #260000000777: Tournament #3600000777, $1+$0.10 USD Hold'em No Limit - Level XI (300/600) - 2026/11/20 15:31:55 ET\n\
             Table '3600000777 1' 8-max Seat #8 is the button\n",
        );
        for (s, chips) in stacks.iter().enumerate().skip(1) {
            text.push_str(&format!("Seat {s}: {} ({chips} in chips)\n", name(s)));
        }
        let mut t = Table { stacks, put: HashMap::new(), folded: Vec::new(), text };
        for s in 1..=8 {
            t.pay(s, ANTE);
            t.text.push_str(&format!("{}: posts the ante {ANTE}\n", name(s)));
        }
        t.pay(1, SB);
        t.text.push_str(&format!("{}: posts small blind {SB}\n", name(1)));
        t.pay(2, BB);
        t.text.push_str(&format!("{}: posts big blind {BB}\n", name(2)));
        t
    }

    fn pay(&mut self, s: usize, x: i64) {
        *self.put.entry(s).or_default() += x;
    }

    fn total(&self, s: usize) -> i64 {
        self.put.get(&s).copied().unwrap_or(0)
    }

    /// Chips in this street's bets (antes are dead money).
    fn bet(&self, s: usize) -> i64 {
        self.total(s) - ANTE
    }

    fn left(&self, s: usize) -> i64 {
        self.stacks[s] - self.total(s)
    }

    fn top(&self) -> i64 {
        (1..=8).map(|s| self.bet(s)).max().unwrap()
    }

    fn fold(&mut self, s: usize) {
        self.folded.push(s);
        self.text.push_str(&format!("{}: folds\n", name(s)));
    }

    fn call(&mut self, s: usize) {
        let x = (self.top() - self.bet(s)).min(self.left(s));
        if x == 0 {
            self.text.push_str(&format!("{}: checks\n", name(s)));
            return;
        }
        self.pay(s, x);
        let all_in = if self.left(s) == 0 { " and is all-in" } else { "" };
        self.text.push_str(&format!("{}: calls {x}{all_in}\n", name(s)));
    }

    /// Raises to `to` this street, or all-in when that is short of it.
    fn raise(&mut self, s: usize, to: i64) {
        let top = self.top();
        let to = to.min(self.bet(s) + self.left(s));
        if to <= top {
            self.call(s);
            return;
        }
        self.pay(s, to - self.bet(s));
        let all_in = if self.left(s) == 0 { " and is all-in" } else { "" };
        self.text.push_str(&format!("{}: raises {} to {to}{all_in}\n", name(s), to - top));
    }

    fn shove(&mut self, s: usize) {
        let to = self.bet(s) + self.left(s);
        self.raise(s, to);
    }

    /// Everybody not named folds, in order, after the last actor.
    fn rest_fold(&mut self, keep: &[usize]) {
        for s in ORDER {
            if !keep.contains(&s) && !self.folded.contains(&s) {
                self.fold(s);
            }
        }
    }

    /// Ends the hand: the uncalled part back, then a check-down to a showdown that the
    /// first player left wins.
    fn finish(mut self) -> String {
        let alive: Vec<usize> = (1..=8).filter(|s| !self.folded.contains(s)).collect();
        let mut bets: Vec<i64> = alive.iter().map(|&s| self.bet(s)).collect();
        bets.sort_unstable();
        let top = *bets.last().unwrap();
        let second = if bets.len() > 1 { bets[bets.len() - 2] } else { 0 };
        if top > second {
            let s = *alive.iter().find(|&&s| self.bet(s) == top).unwrap();
            self.put.insert(s, self.total(s) - (top - second));
            self.text.push_str(&format!("Uncalled bet ({}) returned to {}\n", top - second, name(s)));
        }
        let pot: i64 = (1..=8).map(|s| self.total(s)).sum();
        let winner = alive[0];
        if alive.len() > 1 {
            let talk: Vec<usize> = [1, 2, 3, 4, 5, 6, 7, 8].into_iter().filter(|s| alive.contains(s) && self.left(*s) > 0).collect();
            let board = ["2d 8c Th", "3s", "4c"];
            let mut shown = String::new();
            for (k, cards) in board.iter().enumerate() {
                let street = ["FLOP", "TURN", "RIVER"][k];
                if k == 0 {
                    self.text.push_str(&format!("*** {street} *** [{cards}]\n"));
                } else {
                    self.text.push_str(&format!("*** {street} *** [{shown}] [{cards}]\n"));
                }
                shown = if shown.is_empty() { cards.to_string() } else { format!("{shown} {cards}") };
                if talk.len() > 1 {
                    for &s in &talk {
                        self.text.push_str(&format!("{}: checks\n", name(s)));
                    }
                }
            }
            self.text.push_str("*** SHOW DOWN ***\n");
        }
        self.text.push_str(&format!("{} collected {pot} from pot\n*** SUMMARY ***\nTotal pot {pot} | Rake 0\n", name(winner)));
        self.text
    }
}

fn name(s: usize) -> &'static str {
    ["", "Small", "Big", "Under", "Next", "Middle", "Hijack", "Cutoff", "Button"][s]
}

/// A random hand played to its end.
fn deal(rng: &mut ChaCha8Rng) -> String {
    let mut stacks = [0i64; 9];
    for s in stacks.iter_mut().skip(1) {
        // 3 to 120 BB, a few very short.
        let bb = if rng.random_bool(0.15) { rng.random_range(2.0..8.0) } else { rng.random_range(8.0..120.0) };
        *s = (bb * BB as f64) as i64 + ANTE + BB;
    }
    let cards = ["As 7s", "Ah Ad", "9s 9h", "Kh Qc", "7d 2c", "As 2s", "Th 9h", "Jc Jd", "Ac 5d", "Qd Js", "6c 5c", "Kd 4h"];
    let hero_cards = cards[rng.random_range(0..cards.len())];
    let size = |rng: &mut ChaCha8Rng, of: i64| (of as f64 * rng.random_range(2.0..3.5)) as i64;
    let template = rng.random_range(0..4);
    let (hero, mut t) = match template {
        // An open, the hero in the big blind.
        0 => {
            let mut t = Table::new(stacks);
            let opener = ORDER[rng.random_range(0..6)];
            for s in ORDER.into_iter().take_while(|&s| s != opener) {
                t.fold(s);
            }
            if rng.random_bool(0.2) { t.shove(opener) } else { t.raise(opener, size(rng, BB)) }
            t.rest_fold(&[opener, 2]);
            (2, t)
        }
        // The hero opens early, a later seat 3-bets or shoves.
        1 => {
            let mut t = Table::new(stacks);
            t.raise(3, size(rng, BB));
            let raiser = [5, 6, 7, 8, 1, 2][rng.random_range(0..6)];
            for s in ORDER.into_iter().skip(1).take_while(|&s| s != raiser) {
                t.fold(s);
            }
            let to = t.top() * 3;
            if rng.random_bool(0.35) { t.shove(raiser) } else { t.raise(raiser, to) }
            t.rest_fold(&[3, raiser]);
            (3, t)
        }
        // A short all-in, a deep caller, the hero in the big blind.
        2 => {
            let mut t = Table::new(stacks);
            t.stacks[3] = (rng.random_range(1.5..10.0) * BB as f64) as i64 + ANTE;
            t.text = t.text.replace(&format!("Seat 3: Under ({} in chips)", stacks[3]), &format!("Seat 3: Under ({} in chips)", t.stacks[3]));
            t.shove(3);
            t.fold(4);
            t.fold(5);
            t.call(6);
            t.rest_fold(&[3, 6, 2]);
            (2, t)
        }
        // A limp, the hero in the small blind.
        _ => {
            let mut t = Table::new(stacks);
            let limper = ORDER[rng.random_range(0..6)];
            for s in ORDER.into_iter().take_while(|&s| s != limper) {
                t.fold(s);
            }
            t.call(limper);
            for s in ORDER.into_iter().skip_while(|&s| s != limper).skip(1).take_while(|&s| s != 1) {
                t.fold(s);
            }
            (1, t)
        }
    };
    // The hole cards go right after the blinds.
    let at = t.text.find(&format!("{}: posts big blind", name(2))).unwrap();
    let end = at + t.text[at..].find('\n').unwrap() + 1;
    t.text.insert_str(end, &format!("*** HOLE CARDS ***\nDealt to {} [{hero_cards}]\n", name(hero)));
    // The hero's decision, then the players behind.
    match rng.random_range(0..3) {
        0 => t.fold(hero),
        1 => t.call(hero),
        _ => t.shove(hero),
    }
    if !t.folded.contains(&hero) {
        if template == 3 {
            if t.top() > t.bet(2) && rng.random_bool(0.5) {
                t.fold(2);
            } else {
                t.call(2);
            }
            let limper = (1..=8).find(|s| ![1, 2].contains(s) && !t.folded.contains(s));
            if let Some(l) = limper.filter(|&l| t.bet(l) < t.top()) {
                t.call(l);
            }
        } else {
            let others: Vec<usize> = (1..=8).filter(|&s| s != hero && !t.folded.contains(&s)).collect();
            for s in others {
                if t.bet(s) < t.top() && t.left(s) > 0 {
                    t.call(s);
                }
            }
        }
    }
    t.finish()
}

#[test]
fn rated_decisions_keep_the_bounds_of_the_chips() {
    let mut rng = ChaCha8Rng::seed_from_u64(23);
    let (mut rated, mut hands) = (0, 0);
    for _ in 0..400 {
        let text = deal(&mut rng);
        let hand: Hand = parse_hand(&text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
        hands += 1;
        let a = analyze_hand(&hand);
        let decisions = estimate_rest(&hand, a.decisions, "geschätzt");
        for d in decisions.iter().filter(|d| d.status == Status::Rated) {
            rated += 1;
            let found = check(&hand, d);
            assert!(found.is_empty(), "{found:?}\n{:?} {} {:?}\n{text}", d.spot, d.played, d.strategy);
        }
    }
    assert!(rated > hands, "only {rated} rated decisions in {hands} hands");
}
