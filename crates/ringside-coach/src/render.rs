//! Turns parsed hands into the compact, anonymous German text the model reads.
//!
//! Only fields of the parsed hand are written, never the raw history: player names,
//! table names, tournament ids, hand ids and file paths cannot leak. The hero is "Du",
//! every opponent "Spieler N", numbered in order of appearance across the tournament so
//! the model can follow one opponent from hand to hand. Amounts are in big blinds.

use std::collections::HashMap;
use std::fmt::Write;

use ringside_core::model::{ActionKind, Hand, Street};
use ringside_core::stats::position_labels;

pub const HERO: &str = "Du";

#[derive(Default)]
pub struct Names {
    map: HashMap<String, String>,
}

impl Names {
    pub fn get(&mut self, hand: &Hand, name: &str) -> String {
        if hand.hero.as_deref() == Some(name) {
            return HERO.into();
        }
        let next = self.map.len() + 1;
        self.map.entry(name.to_string()).or_insert_with(|| format!("Spieler {next}")).clone()
    }
}

/// German number with up to `digits` decimals, trailing zeros dropped: 12,5 · 8 · 0,13.
pub fn num(v: f64, digits: usize) -> String {
    let s = format!("{v:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = if s == "-0" { "0".into() } else { s };
    s.replace('.', ",")
}

pub fn bb(amount: i64, big_blind: i64) -> String {
    format!("{} bb", num(amount as f64 / big_blind as f64, 2))
}

fn street_name(s: Street) -> &'static str {
    match s {
        Street::Preflop => "Preflop",
        Street::Flop => "Flop",
        Street::Turn => "Turn",
        Street::River => "River",
    }
}

/// One hand as text, headed by `label` (e.g. "H3"). `None` for hands the coach cannot
/// read (no big blind, no hero cards).
pub fn render_hand(hand: &Hand, label: &str, names: &mut Names) -> Option<String> {
    let big = hand.big_blind;
    if big <= 0 || hand.hero.is_none() || hand.hero_cards.is_empty() {
        return None;
    }
    let mut out = String::new();
    let ante = hand.actions.iter().filter(|a| a.kind == ActionKind::PostAnte).map(|a| a.amount).max().unwrap_or(0);
    let players = hand.active_players().count();
    let _ = write!(out, "{label} · Blinds {}/{}", hand.small_blind / 100, big / 100);
    if ante > 0 {
        let _ = write!(out, ", Ante {} ({})", ante / 100, bb(ante, big));
    }
    let _ = writeln!(out, " · {players} Spieler · deine Karten {}", hand.hero_cards.join(" "));

    let positions = position_labels(hand);
    let position = |name: &str| positions.iter().find(|(n, _)| n == name).map(|(_, p)| *p);
    let seats: Vec<String> = hand
        .active_players()
        .map(|s| {
            let who = names.get(hand, &s.name);
            match position(&s.name) {
                Some(p) => format!("{who} ({p}) {}", bb(s.stack, big)),
                None => format!("{who} {}", bb(s.stack, big)),
            }
        })
        .collect();
    let _ = writeln!(out, "Stacks: {}", seats.join(" · "));

    let mut street = Street::Preflop;
    let mut line: Vec<String> = Vec::new();
    let antes: i64 = hand.actions.iter().filter(|a| a.kind == ActionKind::PostAnte).map(|a| a.amount).sum();
    let mut pot = antes;
    let flush = |out: &mut String, street: Street, line: &mut Vec<String>, pot_before: i64| {
        if line.is_empty() {
            return;
        }
        let board = match street {
            Street::Preflop => String::new(),
            Street::Flop => format!(" [{}]", hand.board.get(..3).map(|b| b.join(" ")).unwrap_or_default()),
            Street::Turn => format!(" [{}]", hand.board.get(3).cloned().unwrap_or_default()),
            Street::River => format!(" [{}]", hand.board.get(4).cloned().unwrap_or_default()),
        };
        let pot = if street == Street::Preflop { String::new() } else { format!(" (Pot {})", bb(pot_before, big)) };
        let _ = writeln!(out, "{}{board}{pot}: {}", street_name(street), line.join(", "));
        line.clear();
    };
    let mut street_start_pot = antes;
    for a in hand.actions.iter().filter(|a| a.kind != ActionKind::PostAnte) {
        if a.street != street {
            flush(&mut out, street, &mut line, street_start_pot);
            street = a.street;
            street_start_pot = pot;
        }
        pot += a.amount;
        let who = names.get(hand, &a.player);
        let you = who == HERO;
        let verb = |du: &'static str, er: &'static str| if you { du } else { er };
        let all_in = if a.all_in { " all-in" } else { "" };
        let text = match a.kind {
            ActionKind::PostSmallBlind => format!("{who} SB {}", bb(a.amount, big)),
            ActionKind::PostBigBlind => format!("{who} BB {}", bb(a.amount, big)),
            ActionKind::PostBoth => format!("{who} SB+BB {}", bb(a.amount, big)),
            ActionKind::PostStraddle => format!("{who} Straddle {}{all_in}", bb(a.amount, big)),
            ActionKind::PostDead => format!("{who} totes Blind {}", bb(a.amount, big)),
            ActionKind::PostAnte => continue,
            ActionKind::Fold => format!("{who} {}", verb("foldest", "foldet")),
            ActionKind::Check => format!("{who} {}", verb("checkst", "checkt")),
            ActionKind::Call => format!("{who} {} {}{all_in}", verb("callst", "callt"), bb(a.amount, big)),
            ActionKind::Bet => format!("{who} setzt {}{all_in}", bb(a.amount, big)),
            ActionKind::Raise => {
                format!("{who} {} auf {}{all_in}", verb("erhöhst", "erhöht"), bb(a.to.unwrap_or(a.amount), big))
            }
        };
        line.push(text);
    }
    flush(&mut out, street, &mut line, street_start_pot);
    // Board cards dealt after everyone was all-in.
    let dealt = match street {
        Street::Preflop => 0,
        Street::Flop => 3,
        Street::Turn => 4,
        Street::River => 5,
    };
    if hand.board.len() > dealt {
        let _ = writeln!(out, "Restliches Board: {}", hand.board[dealt..].join(" "));
    }
    for (name, amount) in &hand.returned {
        let to = names.get(hand, name);
        let to = if to == HERO { "dich".to_string() } else { to };
        let _ = writeln!(out, "Zurück an {to}: {}", bb(*amount, big));
    }
    let shown: Vec<String> = hand
        .shown
        .iter()
        .filter(|(n, _)| hand.hero.as_deref() != Some(n.as_str()))
        .map(|(n, c)| format!("{} zeigt {}", names.get(hand, n), c.join(" ")))
        .collect();
    if !shown.is_empty() {
        let _ = writeln!(out, "Showdown: {}", shown.join(", "));
    }
    let won: Vec<String> = hand
        .collected
        .iter()
        .map(|(n, a)| {
            let who = names.get(hand, n);
            let verb = if who == HERO { "gewinnst" } else { "gewinnt" };
            format!("{who} {verb} {}", bb(*a, big))
        })
        .collect();
    let hero = hand.hero.as_deref().unwrap_or_default();
    let net = hand.net(hero);
    let _ = write!(out, "Ergebnis: {}; du {}{}", won.join(", "), if net > 0 { "+" } else { "" }, bb(net, big));
    if let Some(f) = hand.finishes.iter().find(|f| f.player == hero) {
        match f.place {
            Some(p) => {
                let _ = write!(out, "; du scheidest auf Platz {p} aus");
            }
            None => out.push_str("; du scheidest aus"),
        }
    }
    out.push('\n');
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HAND: &str = "PokerStars Hand #260000000103: Tournament #4046800001, $5+$0.50 USD Hold'em No Limit - Level VII (100/200) - 2026/11/16 20:43:00 CET [2026/11/16 14:43:00 ET]
Table '4046800001 3' 6-max Seat #3 is the button
Seat 1: thoralf69 (1600 in chips)
Seat 2: MaxMuster (5200 in chips)
Seat 3: ErikaBeispiel (3100 in chips)
Seat 4: Anon_Fish (4400 in chips)
thoralf69: posts the ante 20
MaxMuster: posts the ante 20
ErikaBeispiel: posts the ante 20
Anon_Fish: posts the ante 20
Anon_Fish: posts small blind 100
thoralf69: posts big blind 200
*** HOLE CARDS ***
Dealt to thoralf69 [Kh 6d]
MaxMuster: raises 400 to 600
ErikaBeispiel: folds
Anon_Fish: folds
thoralf69: raises 980 to 1580 and is all-in
MaxMuster: calls 980
*** FLOP *** [9c 4h 2s]
*** TURN *** [9c 4h 2s] [Kd]
*** RIVER *** [9c 4h 2s Kd] [3c]
*** SHOW DOWN ***
thoralf69: shows [Kh 6d] (a pair of Kings)
MaxMuster: shows [Ah Qd] (high card Ace)
thoralf69 collected 3340 from pot
*** SUMMARY ***
Total pot 3340 | Rake 0
Board [9c 4h 2s Kd 3c]
Seat 1: thoralf69 (big blind) showed [Kh 6d] and won (3340)
";

    #[test]
    fn hides_every_name_and_id() {
        let hand = ringside_core::parse_hand(HAND).unwrap();
        let mut names = Names::default();
        let text = render_hand(&hand, "H1", &mut names).unwrap();
        for secret in ["thoralf69", "MaxMuster", "ErikaBeispiel", "Anon_Fish", "260000000103", "4046800001", "Table"] {
            assert!(!text.contains(secret), "{secret} steht im Text:\n{text}");
        }
        assert!(text.contains("Du (BB)"), "{text}");
        assert!(text.contains("Spieler 1 erhöht auf 3 bb"), "{text}");
        assert!(text.contains("Du erhöhst auf 7,9 bb all-in"), "{text}");
        assert!(text.contains("Du gewinnst 16,7 bb"), "{text}");
        assert!(text.contains("Spieler 1 zeigt Ah Qd"), "{text}");
        assert!(text.contains("Restliches Board: 9c 4h 2s Kd 3c"), "{text}");
    }

    #[test]
    fn opponents_keep_their_number_across_hands() {
        let hand = ringside_core::parse_hand(HAND).unwrap();
        let mut names = Names::default();
        let first = names.get(&hand, "MaxMuster");
        names.get(&hand, "ErikaBeispiel");
        assert_eq!(names.get(&hand, "MaxMuster"), first);
        assert_eq!(names.get(&hand, "thoralf69"), HERO);
    }

    #[test]
    fn german_numbers() {
        assert_eq!(num(12.5, 2), "12,5");
        assert_eq!(num(8.0, 2), "8");
        assert_eq!(num(-0.001, 2), "0");
        assert_eq!(num(0.125, 3), "0,125");
    }
}
