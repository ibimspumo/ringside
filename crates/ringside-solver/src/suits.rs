//! Suit renamings: a spot on A♠K♠2♦ is the spot on A♥K♥2♣ with the suits renamed, as
//! long as both ranges weigh every suit alike. A renaming is a new suit per old suit.

use ringside_engine::{Card, CardSet, Combo, Range};

pub type Renaming = [u8; 4];

/// All 24 renamings, the identity first.
pub fn all() -> Vec<Renaming> {
    let mut out = Vec::with_capacity(24);
    for a in 0..4u8 {
        for b in (0..4).filter(|&b| b != a) {
            for c in (0..4).filter(|&c| c != a && c != b) {
                out.push([a, b, c, 6 - a - b - c]);
            }
        }
    }
    out
}

pub fn rename(card: Card, r: &Renaming) -> Card {
    Card::new(card.rank(), r[card.suit() as usize])
}

/// The renaming that turns `from` into `to`: the flop as a set, turn and river in place.
pub fn between(from: &[Card], to: &[Card]) -> Option<Renaming> {
    if from.len() != to.len() || from.len() < 3 {
        return None;
    }
    let flop = CardSet::from_cards(&to[..3]);
    all().into_iter().find(|r| {
        let renamed: Vec<Card> = from.iter().map(|&c| rename(c, r)).collect();
        CardSet::from_cards(&renamed[..3]) == flop && renamed[3..] == to[3..]
    })
}

/// Whether `range` weighs every hand that does not touch `dead` like its renamed hand.
pub fn keeps(range: &Range, dead: CardSet, r: &Renaming) -> bool {
    range.live_combos(dead).into_iter().all(|(combo, w)| {
        let (a, b) = combo.cards();
        range.weight(Combo::new(rename(a, r), rename(b, r))) == w
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards(text: &str) -> Vec<Card> {
        (0..text.len() / 2).map(|i| text[2 * i..2 * i + 2].parse().unwrap()).collect()
    }

    #[test]
    fn renamings_respect_the_flop_as_a_set_and_the_turn_in_place() {
        assert_eq!(all().len(), 24);
        assert_eq!(all()[0], [0, 1, 2, 3]);
        let r = between(&cards("AhKh2d"), &cards("2cAsKs")).expect("a renaming");
        let renamed: Vec<Card> = cards("AhKh2d").iter().map(|&c| rename(c, &r)).collect();
        assert_eq!(CardSet::from_cards(&renamed), CardSet::from_cards(&cards("AsKs2c")));
        assert!(between(&cards("AhKh2d5c"), &cards("AsKs2c5d")).is_some());
        assert!(between(&cards("AhKh2d5c"), &cards("AsKs5d2c")).is_none(), "the turn is not a flop card");
        assert!(between(&cards("AhKh2d"), &cards("AsKd2c")).is_none(), "a flush draw is not two suits");
        let range = Range::parse("AK,QQ").unwrap();
        assert!(all().iter().all(|r| keeps(&range, CardSet::default(), r)));
        let one = Range::parse("AK,QhQd").unwrap();
        assert!(!keeps(&one, CardSet::default(), &between(&cards("2h3d4c"), &cards("2s3d4c")).unwrap()));
    }
}
