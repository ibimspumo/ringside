//! Reading stack depth: the stack in big blinds, Harrington's M (how many rounds the
//! blinds and antes take to eat the stack) and how antes lower the equity a call against
//! a shove needs.

use rand::Rng;

use crate::deal::{self, Rng8};
use crate::fmt::{self, chips, pct};
use crate::score::band;
use crate::{Answer, AnswerSpec, Exercise, Fact, Feedback, Grading, Prompt, Task, feedback, number};

/// (small blind, big blind, ante per player) in chips; the ante is the big blind's own
/// ante where `BB_ANTE` levels are used.
const LEVELS: [(f64, f64, f64); 8] = [
    (50.0, 100.0, 10.0),
    (100.0, 200.0, 25.0),
    (150.0, 300.0, 40.0),
    (200.0, 400.0, 50.0),
    (300.0, 600.0, 75.0),
    (400.0, 800.0, 100.0),
    (500.0, 1000.0, 125.0),
    (1000.0, 2000.0, 250.0),
];
const SHOVERS: [&str; 4] = ["UTG", "der Hijack", "der Cutoff", "der Button"];

pub fn generate(level: u8, rng: &mut Rng8) -> Task {
    match level {
        1 => {
            let (sb, bb, _) = *deal::pick(rng, &LEVELS);
            // Stacks in round chip amounts, 3–60 big blinds.
            let stack = (rng.random_range(3.0..60.0) * bb / 50.0).round() * 50.0;
            Task {
                question: format!(
                    "Du hast {} Chips, die Blinds stehen bei {}/{}. Wie viele Big Blinds sind das?",
                    chips(stack),
                    chips(sb),
                    chips(bb)
                ),
                prompt: Prompt::StackDepth {
                    ask: "bb".into(),
                    stack,
                    small_blind: sb,
                    big_blind: bb,
                    ante: 0.0,
                    bb_ante: false,
                    players: 9,
                    shove_bb: None,
                    shover: None,
                },
                answer: AnswerSpec::Number { unit: "bb".into(), min: 0.0, max: 100.0, step: 0.1 },
                grading: Grading::Exakt,
                model_note: None,
            }
        }
        2 => {
            let (sb, bb, ante) = *deal::pick(rng, &LEVELS);
            let bb_ante = rng.random_bool(0.4);
            let players = *deal::pick(rng, &[6u8, 8, 9]);
            let ante = if bb_ante { bb } else { ante };
            let stack = (rng.random_range(4.0..40.0) * bb / 50.0).round() * 50.0;
            let antes = if bb_ante {
                format!("Big-Blind-Ante {}", chips(ante))
            } else {
                format!("Ante {} pro Spieler", chips(ante))
            };
            Task {
                question: format!(
                    "{players} Spieler, Blinds {}/{}, {antes}. Du hast {} Chips. Wie hoch ist deine M-Zahl, also wie viele Runden hältst du ohne Gewinn noch durch?",
                    chips(sb),
                    chips(bb),
                    chips(stack)
                ),
                prompt: Prompt::StackDepth {
                    ask: "m".into(),
                    stack,
                    small_blind: sb,
                    big_blind: bb,
                    ante,
                    bb_ante,
                    players,
                    shove_bb: None,
                    shover: None,
                },
                answer: AnswerSpec::Number { unit: "Runden".into(), min: 0.0, max: 60.0, step: 0.1 },
                grading: Grading::Formel,
                model_note: None,
            }
        }
        _ => {
            let players = *deal::pick(rng, &[6u8, 8, 9]);
            let bb_ante = rng.random_bool(0.4);
            let ante = if bb_ante { 1.0 } else { *deal::pick(rng, &[0.1, 0.2, 0.25]) };
            let shove = rng.random_range(5..=25) as f64;
            let shover = deal::pick(rng, &SHOVERS).to_string();
            let antes = if bb_ante {
                "Du bist im Big Blind und zahlst dazu 1 bb Ante.".to_string()
            } else {
                format!("Du bist im Big Blind. Jeder der {players} Spieler zahlt {} Ante.", fmt::bb(ante))
            };
            Task {
                question: format!(
                    "{} geht mit {} all-in, alle anderen folden. {antes} Wie viel Equity brauchst du für den Call?",
                    capitalized(&shover),
                    fmt::bb(shove)
                ),
                prompt: Prompt::StackDepth {
                    ask: "ante_odds".into(),
                    stack: shove,
                    small_blind: 0.5,
                    big_blind: 1.0,
                    ante,
                    bb_ante,
                    players,
                    shove_bb: Some(shove),
                    shover: Some(shover),
                },
                answer: AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 },
                grading: Grading::Exakt,
                model_note: None,
            }
        }
    }
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}


/// Chips one round of blinds and antes costs.
pub fn round_cost(sb: f64, bb: f64, ante: f64, bb_ante: bool, players: u8) -> f64 {
    sb + bb + if bb_ante { ante } else { ante * players as f64 }
}

/// Harrington's zones by M.
fn zone(m: f64) -> &'static str {
    match m {
        m if m >= 20.0 => "Grün: Du kannst normal spielen.",
        m if m >= 10.0 => "Gelb: Spekulative Hände verlieren an Wert.",
        m if m >= 6.0 => "Orange: Bald heißt es All-in oder Fold.",
        m if m >= 1.0 => "Rot: Nur noch All-in oder Fold.",
        _ => "Tot: Schon eine Runde frisst den Stack.",
    }
}

/// (needed equity with the antes, without them), in percent, and the pot after the call.
pub fn ante_odds(shove: f64, ante: f64, bb_ante: bool, players: u8) -> (f64, f64, f64) {
    let call = shove - 1.0;
    let antes = if bb_ante { ante } else { ante * players as f64 };
    let pot = 2.0 * shove + 0.5 + antes;
    (100.0 * call / pot, 100.0 * call / (2.0 * shove + 0.5), pot)
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::StackDepth { ask, stack, small_blind, big_blind, ante, bb_ante, players, shove_bb, .. } = &ex.prompt else {
        unreachable!()
    };
    let x = number(answer)?;
    match ask.as_str() {
        "bb" => {
            let truth = stack / big_blind;
            let facts = vec![
                Fact::new("Stack in Big Blinds", fmt::bb(truth)).with(format!("{} ÷ {}", chips(*stack), chips(*big_blind))),
                Fact::new("Warum in bb denken", "Ob ein Stack kurz oder tief ist, hängt an den Blinds, nicht an den Chips."),
            ];
            let err = (x - truth) / truth;
            Ok(feedback(err.abs() <= 0.03, band(err, 0.03, 0.25), Answer::Number(round1(truth)), fmt::bb(truth), facts))
        }
        "m" => {
            let cost = round_cost(*small_blind, *big_blind, *ante, *bb_ante, *players);
            let m = stack / cost;
            let antes = if *bb_ante { chips(*ante) } else { format!("{} × {}", players, chips(*ante)) };
            let facts = vec![
                Fact::new("Eine Runde kostet", format!("{} Chips", chips(cost)))
                    .with(format!("{} + {} + {antes}", chips(*small_blind), chips(*big_blind))),
                Fact::new("M-Zahl", fmt::num(m, 1)).with(format!("{} ÷ {}", chips(*stack), chips(cost))),
                Fact::new("Zone", zone(m)),
                Fact::new("Zum Vergleich in bb", fmt::bb(stack / big_blind)),
            ];
            let err = (x - m) / m;
            Ok(feedback(err.abs() <= 0.05, band(err, 0.05, 0.3), Answer::Number(round1(m)), format!("{} Runden", fmt::num(m, 1)), facts))
        }
        _ => {
            let shove = shove_bb.ok_or("Shove fehlt")?;
            let (need, plain, pot) = ante_odds(shove, *ante, *bb_ante, *players);
            let antes = if *bb_ante { fmt::short(*ante) } else { format!("{} × {}", players, fmt::short(*ante)) };
            let facts = vec![
                Fact::new("Dein Call", fmt::bb(shove - 1.0)).with(format!("{} − 1 (dein Big Blind liegt schon drin)", fmt::short(shove))),
                Fact::new("Pot nach dem Call", fmt::bb(pot))
                    .with(format!("{} + {} + 0,5 + {antes}", fmt::short(shove), fmt::short(shove))),
                Fact::new("Benötigte Equity", pct(need)).with(format!("{} ÷ {}", fmt::short(shove - 1.0), fmt::short(pot))),
                Fact::new("Ohne Antes wären es", pct(plain)),
                Fact::new("Die Antes sparen dir", format!("{} Prozentpunkte", fmt::num(plain - need, 1))),
            ];
            let err = x - need;
            Ok(feedback(err.abs() <= 1.5, band(err, 1.5, 8.0), Answer::Number(round1(need)), pct(need), facts))
        }
    }
}

/// The truth as the number field can show it, so that entering it scores fully.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_examples() {
        // 9 players, 100/200, ante 25: one round costs 525 chips, 10.500 chips are M 20.
        let cost = round_cost(100.0, 200.0, 25.0, false, 9);
        assert_eq!(cost, 525.0);
        assert_eq!(10_500.0 / cost, 20.0);
        assert_eq!(round_cost(100.0, 200.0, 200.0, true, 9), 500.0);
        // 10 bb shove into a big blind with 9 × 0,125 ante: 9 ÷ 21,625.
        let (need, plain, pot) = ante_odds(10.0, 0.125, false, 9);
        assert_eq!(pot, 21.625);
        assert!((need - 900.0 / 21.625).abs() < 1e-9);
        assert!((plain - 900.0 / 20.5).abs() < 1e-9);
        assert!(need < plain);
    }

    /// The truth of every generated question is the formula's result.
    #[test]
    fn generated_questions_follow_the_formulas() {
        for level in 1..=3 {
            for seed in 0..60 {
                let ex = crate::generate(crate::Kind::StackDepth, level, seed).unwrap();
                let Prompt::StackDepth { stack, small_blind, big_blind, ante, bb_ante, players, shove_bb, .. } = &ex.prompt else {
                    unreachable!()
                };
                assert!(*stack > 0.0 && small_blind < big_blind);
                let want = match level {
                    1 => stack / big_blind,
                    2 => stack / round_cost(*small_blind, *big_blind, *ante, *bb_ante, *players),
                    _ => ante_odds(shove_bb.unwrap(), *ante, *bb_ante, *players).0,
                };
                let (_, fb) = crate::check(crate::Kind::StackDepth, level, seed, &Answer::Number(0.0)).unwrap();
                let Answer::Number(t) = fb.truth else { panic!() };
                assert!((t - want).abs() <= 0.05 + 1e-9, "{level} {seed}: {t} against {want}");
            }
        }
    }
}
