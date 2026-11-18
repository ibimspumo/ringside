//! ICM drills: the prize value of stacks (Malmuth–Harville, [`ringside_engine::icm`]) and
//! calls against a shove near the money by the push/fold equilibrium with payouts, the
//! same engine as the push/fold tables.
//!
//! An ICM solve takes up to a second or two, so call spots come from a fixed grid of
//! tables that stay in the push/fold cache. All-ins are capped at two players: with three
//! the solves take ten times as long.

use rand::Rng;
use ringside_engine::multiway::{Params, Solution};
use ringside_engine::{CardSet, Combo, icm};

use crate::deal::{self, Rng8};
use crate::fmt::{self, dollars, pct};
use crate::multiway::{at, lookup_or_solve, position_names};
use crate::score::band;
use crate::{Answer, AnswerSpec, Exercise, Fact, Feedback, Grading, Prompt, Task, choice, feedback, number, options, strategy_text};

pub const VALUE_NOTE: &str = "ICM nach Malmuth-Harville: Jeder Platz geht mit der Wahrscheinlichkeit an einen Spieler, die seinem Anteil an den Chips entspricht.";
pub const CALL_NOTE: &str = "ICM-Gleichgewicht für reines Push/Fold (Malmuth-Harville): schieben oder folden, nach einem Shove nur callen oder folden. Höchstens zwei Spieler all-in: Nach dem ersten Call folden alle anderen.";

/// Prize pool of the call spots, in dollars.
const POOL: f64 = 1000.0;
/// Share of the pool for first, second and third place.
const SHARES: [f64; 3] = [0.5, 0.3, 0.2];
const ANTE_BB: f64 = 0.125;
/// Clear spots differ by at least this much prize equity, in dollars.
const CLEAR: f64 = 3.0;
/// Nodes reached less often than this rest on almost no hands.
const MIN_REACH: f64 = 1e-3;

/// Bubble with four players (level 2); three and five players on level 3.
const TABLES_2: [[f64; 4]; 4] = [[10.0, 10.0, 10.0, 10.0], [12.0, 8.0, 15.0, 10.0], [20.0, 10.0, 6.0, 14.0], [8.0, 12.0, 20.0, 10.0]];
const TABLES_3: [&[f64]; 4] =
    [&[12.0, 25.0, 7.0], &[20.0, 10.0, 15.0, 6.0, 25.0], &[9.0, 30.0, 14.0, 5.0], &[15.0, 6.0, 18.0, 11.0, 22.0]];

fn payouts(pool: f64) -> Vec<f64> {
    SHARES.iter().map(|s| s * pool).collect()
}

/// The equilibrium of a table, with ICM or in chips.
fn solve(stacks: &[f64], with_icm: bool) -> Result<std::sync::Arc<Solution>, String> {
    let mut p = Params::new(stacks.to_vec());
    p.ante_bb = ANTE_BB;
    p.max_all_in = 2;
    if with_icm {
        p = p.with_payouts(payouts(POOL));
    }
    lookup_or_solve(&p)
}

pub fn generate(level: u8, rng: &mut Rng8) -> Result<Task, String> {
    if level == 1 {
        return Ok(value(rng));
    }
    let stacks: Vec<f64> = if level == 2 { deal::pick(rng, &TABLES_2).to_vec() } else { deal::pick(rng, &TABLES_3).to_vec() };
    let n = stacks.len();
    let icm = solve(&stacks, true)?;
    let chips = solve(&stacks, false)?;
    for attempt in 0..20_000 {
        let shover = rng.random_range(0..n - 1);
        let hero = rng.random_range(shover + 1..n);
        let mut used = CardSet::EMPTY;
        let hand = deal::cards(rng, &mut used, 2);
        let class = Combo::new(hand[0], hand[1]).class();
        let Some(s) = icm.spot(hero, class, Some(shover), &[]) else { continue };
        if s.reach < MIN_REACH {
            continue;
        }
        let close = (s.freq > 0.0 && s.freq < 1.0) || (s.ev_act - s.ev_fold).abs() < CLEAR;
        let c = chips.spot(hero, class, Some(shover), &[]).ok_or("Chip-Lösung ohne diesen Spot")?;
        let differs = (s.freq >= 0.5) != (c.freq >= 0.5);
        let wanted = if level == 2 { !close } else { differs || (close && attempt > 5_000) };
        if !wanted {
            continue;
        }
        return Ok(call_task(stacks, hero, shover, hand));
    }
    Err("Kein passender ICM-Spot gefunden".into())
}

fn table_words(n: usize) -> String {
    if n <= SHARES.len() {
        format!("Finaltisch mit {n} Spielern, alle werden bezahlt")
    } else {
        format!("Bubble: {n} Spieler, {} werden bezahlt", SHARES.len())
    }
}

fn prize_list(payouts: &[f64]) -> String {
    payouts.iter().map(|&p| dollars(p)).collect::<Vec<_>>().join(", ")
}

fn value(rng: &mut Rng8) -> Task {
    let n = rng.random_range(3..=5);
    let pool = *deal::pick(rng, &[100.0, 500.0, 1000.0]);
    let stacks: Vec<f64> = (0..n).map(|_| rng.random_range(3..=40) as f64).collect();
    let ask = rng.random_range(0..n);
    let positions: Vec<String> = position_names(n).into_iter().map(String::from).collect();
    let pays = payouts(pool);
    let question = format!(
        "{}, Preise {}. {} hat {} von {} im Spiel. Wie viel Preisgeld ist sein Stack nach ICM wert?",
        table_words(n),
        prize_list(&pays),
        positions[ask],
        fmt::bb(stacks[ask]),
        fmt::bb(stacks.iter().sum())
    );
    Task {
        question,
        prompt: Prompt::Icm {
            positions,
            stacks_bb: stacks,
            payouts: pays.clone(),
            ask: Some(ask),
            hero: None,
            shover: None,
            hand: Vec::new(),
            class: None,
            ante_bb: 0.0,
        },
        answer: AnswerSpec::Number { unit: "$".into(), min: 0.0, max: pays[0], step: 1.0 },
        grading: Grading::Modell,
        model_note: Some(VALUE_NOTE.into()),
    }
}

fn call_task(stacks: Vec<f64>, hero: usize, shover: usize, hand: Vec<ringside_engine::Card>) -> Task {
    let n = stacks.len();
    let class = Combo::new(hand[0], hand[1]).class().to_string();
    let positions: Vec<String> = position_names(n).into_iter().map(String::from).collect();
    let pays = payouts(POOL);
    let folded = if hero > shover + 1 { ", alle danach folden" } else { "" };
    let question = format!(
        "{}, Preise {}. {} geht als Erster mit {} all-in{folded}. Du sitzt {} und hast {}. Callst du mit {class}?",
        table_words(n),
        prize_list(&pays),
        positions[shover],
        fmt::bb(stacks[shover]),
        at(&positions[hero]),
        fmt::bb(stacks[hero])
    );
    Task {
        question,
        prompt: Prompt::Icm {
            positions,
            stacks_bb: stacks,
            payouts: pays,
            ask: None,
            hero: Some(hero),
            shover: Some(shover),
            hand,
            class: Some(class),
            ante_bb: ANTE_BB,
        },
        answer: options(("call", "Call"), ("fold", "Fold")),
        grading: Grading::Modell,
        model_note: Some(CALL_NOTE.into()),
    }
}


pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Icm { positions, stacks_bb, payouts, ask, hero, shover, hand, .. } = &ex.prompt else { unreachable!() };
    if let Some(seat) = ask {
        return grade_value(positions, stacks_bb, payouts, *seat, answer);
    }
    let (Some(hero), Some(shover)) = (*hero, *shover) else { unreachable!() };
    let picked = choice(answer)?;
    if picked != "call" && picked != "fold" {
        return Err(format!("Unbekannte Aktion '{picked}'"));
    }
    let icm = solve(stacks_bb, true)?;
    let chips = solve(stacks_bb, false)?;
    let class = Combo::new(hand[0], hand[1]).class();
    let s = icm.spot(hero, class, Some(shover), &[]).ok_or("Diesen Spot gibt es im Push/Fold-Modell nicht")?;
    let c = chips.spot(hero, class, Some(shover), &[]).ok_or("Diesen Spot gibt es im Push/Fold-Modell nicht")?;
    let call = picked == "call";
    let correct = if call { s.freq > 0.0 } else { s.freq < 1.0 };
    let best = s.ev_act.max(s.ev_fold);
    let loss = best - if call { s.ev_act } else { s.ev_fold };
    let position = &positions[hero];
    let icm_pct = icm.act_pct(s.node);
    let chip_pct = chips.act_pct(c.node);
    let mut facts = vec![
        Fact::new("ICM-Nash-Strategie", strategy_text(s.freq, "Call", "Fold")),
        Fact::new("Preisgeld-Erwartung mit Call", dollars(s.ev_act)),
        Fact::new("Preisgeld-Erwartung mit Fold", dollars(s.ev_fold)),
        Fact::new(&format!("Call-Range {position} mit ICM"), format!("{} aller Hände", pct(icm_pct))),
        Fact::new(&format!("Call-Range {position} nur nach Chips"), format!("{} aller Hände", pct(chip_pct))),
        Fact::new("Nur nach Chips (ohne ICM)", strategy_text(c.freq, "Call", "Fold")),
    ];
    if (s.freq >= 0.5) != (c.freq >= 0.5) {
        facts.push(Fact::new(
            "ICM macht den Unterschied",
            "Ausscheiden kostet hier mehr Preisgeld, als der Gewinn an Chips bringt.",
        ));
    }
    if !correct {
        facts.push(Fact::new("Deine Wahl kostet", dollars(loss)));
    }
    let (truth, text) = if s.freq >= 0.5 { ("call", "Call") } else { ("fold", "Fold") };
    let score = if correct { 1.0 } else { 0.5 * (1.0 - loss / (0.01 * POOL)).max(0.0) };
    Ok(feedback(correct, score, Answer::Choice(truth.into()), text.into(), facts))
}

fn grade_value(positions: &[String], stacks: &[f64], payouts: &[f64], seat: usize, answer: &Answer) -> Result<Feedback, String> {
    let r = icm(stacks, payouts).map_err(|e| e.to_string())?;
    let pool: f64 = payouts.iter().sum();
    let total: f64 = stacks.iter().sum();
    let truth = r.ev[seat];
    let share = stacks[seat] / total * pool;
    let x = number(answer)?;
    let err = x - truth;
    let (full, zero) = (0.025 * pool, 0.12 * pool);
    let all = positions
        .iter()
        .zip(&r.ev)
        .map(|(p, v)| format!("{p} {}", dollars(*v)))
        .collect::<Vec<_>>()
        .join(" · ");
    let mut facts = vec![
        Fact::new(&format!("ICM-Wert {}", positions[seat]), dollars(truth)),
        Fact::new("Nur nach Chip-Anteil", dollars(share)).with(format!(
            "{} ÷ {} × {}",
            fmt::short(stacks[seat]),
            fmt::short(total),
            dollars(pool)
        )),
        Fact::new("Alle Stacks nach ICM", all).with(format!("zusammen {}, das ganze Preisgeld", dollars(r.ev.iter().sum()))),
    ];
    facts.push(if truth < share {
        Fact::new("Warum weniger", "Mehr als den ersten Preis kann ein großer Stack nicht gewinnen.")
    } else {
        Fact::new("Warum mehr", "Auch ein kurzer Stack landet oft noch auf einem bezahlten Platz.")
    });
    Ok(feedback(err.abs() <= full, band(err, full, zero), Answer::Number(truth), dollars(truth), facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ICM values of every table add up to the prize pool, and equal stacks share it equally.
    #[test]
    fn icm_values_sum_to_the_prize_pool() {
        for stacks in TABLES_3.iter().map(|t| t.to_vec()).chain(TABLES_2.iter().map(|t| t.to_vec())) {
            let r = icm(&stacks, &payouts(POOL)).unwrap();
            assert!((r.ev.iter().sum::<f64>() - POOL).abs() < 1e-6, "{stacks:?}");
        }
        let r = icm(&[10.0; 4], &payouts(POOL)).unwrap();
        assert!(r.ev.iter().all(|v| (v - 250.0).abs() < 1e-9));
        // Heads-up the value is exactly second place plus the chip share of the difference.
        let r = icm(&[30.0, 10.0], &[700.0, 300.0]).unwrap();
        assert!((r.ev[0] - (300.0 + 0.75 * 400.0)).abs() < 1e-9);
    }

    /// Level 1 asks the engine's ICM value; the shown values add up to the prize pool.
    #[test]
    fn value_questions_match_the_engine() {
        for seed in 0..40 {
            let ex = crate::generate(crate::Kind::IcmCall, 1, seed).unwrap();
            let Prompt::Icm { stacks_bb, payouts, ask: Some(seat), .. } = &ex.prompt else { panic!() };
            assert!((3..=5).contains(&stacks_bb.len()) && stacks_bb.iter().all(|&s| s > 0.0));
            let r = icm(stacks_bb, payouts).unwrap();
            let (_, fb) = crate::check(crate::Kind::IcmCall, 1, seed, &Answer::Number(0.0)).unwrap();
            assert_eq!(fb.truth, Answer::Number(r.ev[*seat]));
            assert!((r.ev.iter().sum::<f64>() - payouts.iter().sum::<f64>()).abs() < 1e-6);
        }
    }

    /// Calls are graded by the ICM equilibrium of exactly that table; level 3 asks spots
    /// where ICM and chips disagree or that are close.
    #[test]
    fn calls_follow_the_icm_equilibrium() {
        for level in 2..=3 {
            let ex = crate::generate(crate::Kind::IcmCall, level, 0).unwrap();
            let Prompt::Icm { stacks_bb, payouts, hero: Some(hero), shover: Some(shover), hand, .. } = &ex.prompt else {
                panic!()
            };
            assert!(shover < hero && *hero < stacks_bb.len());
            assert_eq!(payouts.iter().sum::<f64>(), POOL);
            let class = Combo::new(hand[0], hand[1]).class();
            let s = solve(stacks_bb, true).unwrap().spot(*hero, class, Some(*shover), &[]).unwrap();
            let c = solve(stacks_bb, false).unwrap().spot(*hero, class, Some(*shover), &[]).unwrap();
            let close = (s.freq > 0.0 && s.freq < 1.0) || (s.ev_act - s.ev_fold).abs() < CLEAR;
            if level == 2 {
                assert!(!close);
            } else {
                assert!(close || (s.freq >= 0.5) != (c.freq >= 0.5));
            }
            let (_, fb) = crate::check(crate::Kind::IcmCall, level, 0, &Answer::Choice("call".into())).unwrap();
            assert_eq!(fb.correct, s.freq > 0.0);
            assert_eq!(fb.truth, Answer::Choice(if s.freq >= 0.5 { "call" } else { "fold" }.into()));
        }
    }
}
