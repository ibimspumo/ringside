//! Position quiz. Labels come from the tracker's own `position_labels`, so the quiz
//! names seats exactly as the HUD and the reports do.

use rand::Rng;
use rand::seq::SliceRandom;
use ringside_core::model::{Action, ActionKind, GameKind, Hand, Seat, Street};
use ringside_core::stats::position_labels;

use crate::deal::{self, Rng8};
use crate::{Answer, AnswerSpec, ChoiceOption, Exercise, Fact, Feedback, Grading, PositionAsk, Prompt, Task, feedback};

/// The table as dealt: who sits where and who posted the blinds.
struct Table {
    size: u8,
    occupied: Vec<u8>,
    button: u8,
    small_blind: Option<u8>,
    big_blind: u8,
}

/// Next occupied seat clockwise after `seat`.
fn next(occupied: &[u8], size: u8, seat: u8) -> u8 {
    (1..=size).map(|k| (seat - 1 + k) % size + 1).find(|s| occupied.contains(s)).expect("occupied seat")
}

/// Seat labels in acting order, starting with the first to act preflop.
fn labels(t: &Table) -> Vec<(u8, &'static str)> {
    let name = |s: u8| format!("Sitz {s}");
    let post = |seat: u8, kind: ActionKind, amount: i64| Action {
        street: Street::Preflop,
        player: name(seat),
        kind,
        amount,
        to: None,
        all_in: false,
    };
    let mut actions = Vec::new();
    if let Some(sb) = t.small_blind {
        actions.push(post(sb, ActionKind::PostSmallBlind, 50));
    }
    actions.push(post(t.big_blind, ActionKind::PostBigBlind, 100));
    let hand = Hand {
        id: 0,
        game: GameKind::Tournament,
        tournament_id: None,
        variant: String::new(),
        table_name: String::new(),
        max_seats: t.size,
        button_seat: t.button,
        small_blind: 50,
        big_blind: 100,
        currency: None,
        played_at: String::new(),
        seats: t
            .occupied
            .iter()
            .map(|&s| Seat { seat: s, name: name(s), stack: 10_000, bounty: None, sitting_out: false })
            .collect(),
        hero: None,
        hero_cards: Vec::new(),
        board: Vec::new(),
        actions,
        returned: Vec::new(),
        shown: Vec::new(),
        collected: Vec::new(),
        showdown: false,
        total_pot: 0,
        rake: 0,
        bounties_won: Vec::new(),
        bounty_currency: None,
        buy_in: None,
        finishes: Vec::new(),
    };
    position_labels(&hand)
        .into_iter()
        .map(|(n, l)| (n.trim_start_matches("Sitz ").parse().expect("seat name"), l))
        .collect()
}

/// `missing`: 0 regular, 1 dead button, 2 missing small blind.
fn deal_table(rng: &mut Rng8, size: u8, players: usize, missing: u8) -> Option<Table> {
    let mut seats: Vec<u8> = (1..=size).collect();
    seats.shuffle(rng);
    let mut occupied: Vec<u8> = seats[..players].to_vec();
    occupied.sort_unstable();
    let empty: Vec<u8> = seats[players..].to_vec();
    match missing {
        0 => {
            let button = *deal::pick(rng, &occupied);
            let (small_blind, big_blind) = if players == 2 {
                (button, next(&occupied, size, button))
            } else {
                let sb = next(&occupied, size, button);
                (sb, next(&occupied, size, sb))
            };
            Some(Table { size, occupied, button, small_blind: Some(small_blind), big_blind })
        }
        1 => {
            let button = *deal::pick(rng, &empty);
            let sb = next(&occupied, size, button);
            Some(Table { size, button, small_blind: Some(sb), big_blind: next(&occupied, size, sb), occupied })
        }
        _ => {
            let button = *deal::pick(rng, &occupied);
            let sb_seat = button % size + 1;
            if occupied.contains(&sb_seat) {
                return None;
            }
            Some(Table { size, button, small_blind: None, big_blind: next(&occupied, size, button), occupied })
        }
    }
}

pub fn position(level: u8, rng: &mut Rng8) -> Task {
    let (table, missing) = loop {
        let (size, players, missing) = match level {
            1 => {
                let size = if rng.random_bool(0.5) { 6 } else { 9 };
                (size, size as usize, 0)
            }
            2 => {
                let size = rng.random_range(2..=10u8);
                let players = rng.random_range((size as usize).saturating_sub(3).max(2)..=size as usize);
                (size, players, 0)
            }
            _ => {
                let size = rng.random_range(4..=10u8);
                let players = rng.random_range(3..size as usize);
                (size, players, rng.random_range(1..=2u8))
            }
        };
        if let Some(t) = deal_table(rng, size, players, missing) {
            break (t, missing);
        }
    };
    let labels = labels(&table);
    let (seat, label) = *deal::pick(rng, &labels);
    let ask_label = level == 1 || rng.random_bool(0.5);
    let dead_button = missing == 1;
    let blinds = missing > 0;
    let (question, ask, answer) = if ask_label {
        (
            format!("Welcher Sitz ist {label}?"),
            PositionAsk::SeatOf { label: label.into() },
            AnswerSpec::Seat { seats: table.occupied.clone() },
        )
    } else {
        (
            format!("Welche Position hat Sitz {seat}?"),
            PositionAsk::LabelOf { seat },
            AnswerSpec::Choice { options: labels.iter().map(|(_, l)| ChoiceOption::new(l, l)).collect() },
        )
    };
    let note = match missing {
        1 => " Der Button liegt auf einem leeren Sitz (toter Button).",
        2 => " Der Small Blind fehlt in dieser Hand.",
        _ => "",
    };
    Task {
        question: format!("{question}{note}"),
        prompt: Prompt::Position {
            table_size: table.size,
            occupied: table.occupied.clone(),
            button: table.button,
            dead_button,
            small_blind: if blinds { table.small_blind } else { None },
            big_blind: blinds.then_some(table.big_blind),
            ask,
        },
        answer,
        grading: Grading::Exakt,
        model_note: None,
    }
}

pub fn grade_position(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Position { table_size, occupied, button, dead_button, small_blind, big_blind, ask } = &ex.prompt
    else {
        unreachable!()
    };
    let (sb, bb) = match big_blind {
        Some(bb) => (*small_blind, *bb),
        None if occupied.len() == 2 => (Some(*button), next(occupied, *table_size, *button)),
        None => {
            let sb = next(occupied, *table_size, *button);
            (Some(sb), next(occupied, *table_size, sb))
        }
    };
    let table = Table { size: *table_size, occupied: occupied.clone(), button: *button, small_blind: sb, big_blind: bb };
    let labels = labels(&table);
    let order = labels.iter().map(|(s, l)| format!("{l}: Sitz {s}")).collect::<Vec<_>>().join(" · ");
    let mut facts = vec![Fact::new("Reihenfolge preflop", order)];
    if *dead_button {
        facts.push(Fact::new(
            "Toter Button",
            "Der Button liegt auf einem leeren Sitz; der Spieler davor handelt nach dem Flop zuletzt und gilt als BTN.",
        ));
    } else if sb.is_none() {
        facts.push(Fact::new("Kein Small Blind", "Nur der Big Blind wurde gesetzt; der Button bleibt, wo er ist."));
    } else if occupied.len() == 2 {
        facts.push(Fact::new("Heads-up", "Der Button setzt den Small Blind und handelt preflop zuerst."));
    }
    let (correct, truth, text) = match ask {
        PositionAsk::SeatOf { label } => {
            let seat = labels.iter().find(|(_, l)| l == label).map(|(s, _)| *s).ok_or("Position fehlt am Tisch")?;
            (matches!(answer, Answer::Seat(s) if *s == seat), Answer::Seat(seat), format!("Sitz {seat}"))
        }
        PositionAsk::LabelOf { seat } => {
            let label = labels.iter().find(|(s, _)| s == seat).map(|(_, l)| *l).ok_or("Sitz ist leer")?;
            (matches!(answer, Answer::Choice(c) if c == label), Answer::Choice(label.into()), label.to_string())
        }
    };
    Ok(feedback(correct, if correct { 1.0 } else { 0.0 }, truth, text, facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_nine_max_labels() {
        let t = Table { size: 9, occupied: (1..=9).collect(), button: 9, small_blind: Some(1), big_blind: 2 };
        let l = labels(&t);
        assert_eq!(l.first(), Some(&(3, "UTG")));
        assert_eq!(l.last(), Some(&(2, "BB")));
        assert!(l.contains(&(9, "BTN")) && l.contains(&(8, "CO")) && l.contains(&(1, "SB")));
    }

    #[test]
    fn dead_button_and_missing_small_blind() {
        // Button on empty seat 4: seat 5 posts the small blind, seat 3 acts last.
        let occupied = vec![1, 3, 5, 6];
        let t = Table { size: 6, occupied: occupied.clone(), button: 4, small_blind: Some(5), big_blind: 6 };
        assert_eq!(labels(&t), vec![(1, "CO"), (3, "BTN"), (5, "SB"), (6, "BB")]);
        let t = Table { size: 6, occupied, button: 3, small_blind: None, big_blind: 5 };
        assert_eq!(labels(&t), vec![(6, "HJ"), (1, "CO"), (3, "BTN"), (5, "BB")]);
    }

    #[test]
    fn heads_up_button_is_the_small_blind() {
        let t = Table { size: 6, occupied: vec![2, 5], button: 5, small_blind: Some(5), big_blind: 2 };
        assert_eq!(labels(&t), vec![(5, "BTN"), (2, "BB")]);
    }
}
