//! Showdown judge: who wins with this board?

use rand::Rng;
use ringside_engine::outs::rank_of;
use ringside_engine::{Card, CardSet, HandRank, evaluate};

use crate::deal::{self, Rng8};
use crate::fmt;
use crate::{Answer, AnswerSpec, ChoiceOption, Exercise, Fact, Feedback, Grading, Prompt, Task, feedback};

/// The best five of up to seven cards, strongest first by rank then card.
pub fn best_five(cards: &[Card]) -> (HandRank, Vec<Card>) {
    let n = cards.len();
    let mut best: Option<(HandRank, Vec<Card>)> = None;
    for mask in 0u32..1 << n {
        if mask.count_ones() != 5 {
            continue;
        }
        let five: Vec<Card> = (0..n).filter(|i| mask >> i & 1 == 1).map(|i| cards[i]).collect();
        let rank = evaluate(CardSet::from_cards(&five));
        if best.as_ref().is_none_or(|b| rank > b.0) {
            best = Some((rank, five));
        }
    }
    let (rank, mut five) = best.expect("at least five cards");
    let count = |r: u8| five.iter().filter(|c| c.rank() == r).count();
    let counts: Vec<usize> = five.iter().map(|c| count(c.rank())).collect();
    let mut keyed: Vec<(usize, Card)> = counts.into_iter().zip(five.iter().copied()).collect();
    keyed.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.rank().cmp(&a.1.rank())).then(b.1.cmp(&a.1)));
    five = keyed.into_iter().map(|(_, c)| c).collect();
    (rank, five)
}

/// Indices of the players with the best hand.
fn winners(board: &[Card], players: &[Vec<Card>]) -> Vec<usize> {
    let ranks: Vec<HandRank> = players.iter().map(|p| rank_of(p, board)).collect();
    let best = *ranks.iter().max().expect("players");
    (0..players.len()).filter(|&i| ranks[i] == best).collect()
}

pub fn generate(level: u8, rng: &mut Rng8) -> Result<Task, String> {
    for _ in 0..100_000 {
        let n = if level == 3 { rng.random_range(3..=4) } else { 2 };
        let mut used = CardSet::EMPTY;
        let players: Vec<Vec<Card>> = (0..n).map(|_| deal::cards(rng, &mut used, 2)).collect();
        let board = deal::cards(rng, &mut used, 5);
        let ranks: Vec<HandRank> = players.iter().map(|p| rank_of(p, &board)).collect();
        let split = winners(&board, &players).len() > 1;
        let same_category = ranks[0].category() == ranks[1].category();
        let ok = match level {
            1 => !same_category,
            // Kicker and split spots, a third of them splits.
            2 => same_category && (split == rng.random_bool(1.0 / 3.0)),
            _ => true,
        };
        if !ok {
            continue;
        }
        let mut options: Vec<ChoiceOption> =
            (0..n).map(|i| ChoiceOption::new(&format!("p{i}"), &format!("Spieler {}", i + 1))).collect();
        options.push(ChoiceOption::new("split", "Pot teilen"));
        return Ok(Task {
            question: "Wer gewinnt den Pot?".into(),
            prompt: Prompt::Showdown { board, players },
            answer: AnswerSpec::Choice { options },
            grading: Grading::Exakt,
            model_note: None,
        });
    }
    Err("Kein passender Showdown gefunden".into())
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Showdown { board, players } = &ex.prompt else { unreachable!() };
    let won = winners(board, players);
    let (truth, text) = if won.len() > 1 {
        let names: Vec<String> = won.iter().map(|i| format!("Spieler {}", i + 1)).collect();
        ("split".to_string(), format!("Pot geteilt zwischen {}", names.join(" und ")))
    } else {
        (format!("p{}", won[0]), format!("Spieler {}", won[0] + 1))
    };
    let facts = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let cards: Vec<Card> = p.iter().chain(board).copied().collect();
            let (rank, five) = best_five(&cards);
            Fact::new(&format!("Spieler {}", i + 1), format!("{} ({})", fmt::category(rank.category()), fmt::cards(&five)))
        })
        .collect();
    let correct = matches!(answer, Answer::Choice(c) if *c == truth);
    Ok(feedback(correct, if correct { 1.0 } else { 0.0 }, Answer::Choice(truth), text, facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards(s: &str) -> Vec<Card> {
        CardSet::parse(s).unwrap().to_vec()
    }

    #[test]
    fn best_five_orders_groups_first() {
        let (rank, five) = best_five(&cards("KsKd7h7c2s9dAs"));
        assert_eq!(rank.category(), ringside_engine::HandCategory::TwoPair);
        assert_eq!(fmt::cards(&five), "Ks Kd 7h 7c As");
    }

    #[test]
    fn board_plays_is_a_split() {
        let board = cards("AsKsQsJsTs");
        assert_eq!(winners(&board, &[cards("2c3d"), cards("4h5h")]), vec![0, 1]);
    }
}
