//! Practice from the playing profile: a real decision of the hero to decide again, and
//! endless variations of it. A variation keeps the hand as it was up to the decision
//! (seats, stacks, every action before) and deals the hero another hand; the analysis
//! rates it exactly as it rates a real hand, so the sources are the same.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_core::db::{Decision, Source, Status, effective_stack_bb};
use ringside_core::model::Hand;
use ringside_core::profile::{ProfileRow, facing_bet};
use ringside_core::stats::{analyze, position_labels};
use ringside_engine::{Card, CardSet};
use ringside_play::grade::judge;

use crate::grade::rating;
use crate::{analyze_hand, estimate_rest};

/// Hands dealt per variation; the closest decision among them is asked.
const CANDIDATES: usize = 4;
/// Deals tried before giving up on a spot the analysis cannot rate.
const TRIES: usize = 24;

/// The profile row of `d` in `hand`, the same way the database derives it.
pub fn row(hand: &Hand, d: &Decision) -> ProfileRow {
    let hero = hand.hero.as_deref().unwrap_or_default();
    ProfileRow {
        hand_id: hand.id,
        action_index: d.action_index,
        played_at: hand.played_at.clone(),
        spot: d.spot,
        status: d.status,
        rating: d.rating,
        ev_loss_bb: d.ev_loss_bb,
        source: d.source,
        reliability: d.reliability,
        facing_bet: facing_bet(&d.strategy),
        position: position_labels(hand).into_iter().find(|(n, _)| n == hero).map(|(_, p)| p.to_string()),
        stack_bb: effective_stack_bb(hand, hero),
        players: analyze(hand).len(),
    }
}

/// `hand` as it stood at `action_index` with the hero holding `cards`: what came later and
/// every card shown is gone.
fn dealt(hand: &Hand, action_index: usize, cards: [Card; 2]) -> Hand {
    let mut h = hand.clone();
    h.hero_cards = cards.iter().map(Card::to_string).collect();
    h.actions.truncate(action_index + 1);
    h.shown.clear();
    h.collected.clear();
    h.returned.clear();
    h.showdown = false;
    h
}

fn known(hand: &Hand) -> CardSet {
    let mut used = CardSet::EMPTY;
    let all = hand.board.iter().chain(&hand.hero_cards).chain(hand.shown.iter().flat_map(|(_, c)| c));
    for c in all.filter_map(|s| s.parse::<Card>().ok()) {
        used.insert(c);
    }
    used
}

/// How close the decision is: the EV gap between the two best actions in percent of the
/// pot, or how far the main action is from always without EVs. Smaller is closer.
fn closeness(d: &Decision) -> f64 {
    let pot = d.pot_bb.unwrap_or(1.0).max(1.0);
    let mut evs: Vec<f64> = d.strategy.iter().filter_map(|c| c.ev_bb).collect();
    if evs.len() == d.strategy.len() && evs.len() >= 2 {
        evs.sort_by(|a, b| b.total_cmp(a));
        return (evs[0] - evs[1]) / pot;
    }
    d.strategy.iter().map(|c| c.freq).fold(0.0, f64::max)
}

fn rated(d: &Decision) -> bool {
    d.status == Status::Rated && d.strategy.len() >= 2 && d.source.is_some_and(|s| s != Source::Forced)
}

/// The decision at `action_index` of `hand` as the analysis rates it right away; a spot
/// waiting for the solver is estimated instead.
fn rate_now(hand: &Hand, action_index: usize) -> Option<Decision> {
    let d = analyze_hand(hand).decisions.into_iter().find(|d| d.action_index == action_index)?;
    let d = if d.status == Status::Pending {
        estimate_rest(hand, vec![d], "Für die Übung wird der Spot geschätzt statt gerechnet.").pop()?
    } else {
        d
    };
    rated(&d).then_some(d)
}

/// A variation of the hero's decision at `action_index`: another hand for the hero, the
/// rest as it was. The same `seed` gives the same variation.
pub fn variant(hand: &Hand, action_index: usize, seed: u64) -> Result<(Hand, Decision), String> {
    if hand.actions.get(action_index).is_none_or(|a| Some(a.player.as_str()) != hand.hero.as_deref()) {
        return Err("Diese Entscheidung gibt es in der Hand nicht.".into());
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed ^ hand.id.rotate_left(17) ^ action_index as u64);
    let used = known(hand);
    let mut best: Option<(f64, Hand, Decision)> = None;
    let mut found = 0;
    for _ in 0..TRIES {
        let mut left = used;
        let mut deal = || loop {
            let c = Card::from_index(rng.random_range(0..52));
            if !left.contains(c) {
                left.insert(c);
                return c;
            }
        };
        let cards = [deal(), deal()];
        let h = dealt(hand, action_index, cards);
        let Some(d) = rate_now(&h, action_index) else { continue };
        let c = closeness(&d);
        if best.as_ref().is_none_or(|(b, _, _)| c < *b) {
            best = Some((c, h, d));
        }
        found += 1;
        if found == CANDIDATES {
            break;
        }
    }
    best.map(|(_, h, d)| (h, d)).ok_or_else(|| "Für diese Lage konnte die Analyse keine neue Hand bewerten.".into())
}

/// The outcome of deciding `d` again with `choice`.
#[derive(Clone, Debug, PartialEq)]
pub struct Answered {
    /// `d` with `choice` as the played action, rated like the analysis rates it.
    pub decision: Decision,
    /// Points out of 1 as in the practice.
    pub score: f64,
}

/// Grades `choice` (index into the strategy) against the recommendation of `d`.
pub fn answer(d: &Decision, choice: usize) -> Result<Answered, String> {
    if choice >= d.strategy.len() {
        return Err("Diese Aktion gibt es hier nicht.".into());
    }
    let pot = d.pot_bb.ok_or("Für diesen Spot fehlt der Pot.")?;
    let freqs: Vec<f64> = d.strategy.iter().map(|c| c.freq).collect();
    let evs: Option<Vec<f64>> = d.strategy.iter().map(|c| c.ev_bb).collect();
    let j = judge(&freqs, evs.as_deref(), choice, pot);
    let mut out = d.clone();
    for (i, c) in out.strategy.iter_mut().enumerate() {
        c.played = i == choice;
    }
    out.played = d.strategy[choice].label.clone();
    out.played_freq = Some(j.frequency);
    out.rating = Some(rating(j.grade));
    out.ev_loss_bb = Some(j.loss_bb.unwrap_or(0.0).max(0.0));
    Ok(Answered { decision: out, score: j.grade.points() / 100.0 })
}

#[cfg(test)]
mod tests {
    use ringside_core::{parse_hand, split_hands};
    use ringside_core::profile::kind_of;

    use super::*;

    fn fixtures() -> Vec<Hand> {
        let text = include_str!("../tests/fixtures/hands.txt");
        split_hands(text).into_iter().filter_map(|raw| parse_hand(raw).ok()).collect()
    }

    /// Every rated decision of the fixture hands with its variations: the same kind of spot,
    /// another hand for the hero, no card twice, the same variation for the same seed and a
    /// grade for every action that can be chosen.
    #[test]
    fn variations_stay_in_their_kind_of_spot() {
        let mut checked = 0;
        for hand in fixtures() {
            for d in analyze_hand(&hand).decisions.iter().filter(|d| rated(d)) {
                let kind = kind_of(&row(&hand, d)).expect("rated decisions have a kind");
                let seed = hand.id ^ d.action_index as u64;
                let Ok((h, v)) = variant(&hand, d.action_index, seed) else { continue };
                assert_eq!(kind_of(&row(&h, &v)), Some(kind), "hand {} @{}", hand.id, d.action_index);
                assert_ne!(h.hero_cards, hand.hero_cards);
                let mut all: Vec<&String> = h.hero_cards.iter().chain(&h.board).collect();
                all.sort();
                all.dedup();
                assert_eq!(all.len(), h.hero_cards.len() + h.board.len(), "no card twice");
                if checked == 0 {
                    let (h2, v2) = variant(&hand, d.action_index, seed).unwrap();
                    assert_eq!((h2.hero_cards, v2), (h.hero_cards.clone(), v.clone()));
                }
                for c in 0..v.strategy.len() {
                    let a = answer(&v, c).unwrap();
                    assert!(a.decision.ev_loss_bb.unwrap() >= 0.0 && (0.0..=1.0).contains(&a.score));
                    assert_eq!(a.decision.strategy.iter().filter(|x| x.played).count(), 1);
                }
                checked += 1;
            }
        }
        assert!(checked >= 8, "only {checked} variations");
    }

    /// Choosing again what was played back then gives the stored grade.
    #[test]
    fn answering_the_played_action_repeats_the_analysis() {
        for hand in fixtures() {
            for d in analyze_hand(&hand).decisions.iter().filter(|d| rated(d)) {
                let Some(played) = d.strategy.iter().position(|c| c.played) else { continue };
                let a = answer(d, played).unwrap();
                assert_eq!(a.decision.rating, d.rating, "hand {} @{}", hand.id, d.action_index);
            }
        }
    }
}
