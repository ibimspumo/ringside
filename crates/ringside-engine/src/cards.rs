//! Cards as `0..52` (`rank * 4 + suit`) and card sets as `u64` bitmasks.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::Error;

pub const RANK_CHARS: [char; 13] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A',
];
pub const SUIT_CHARS: [char; 4] = ['c', 'd', 'h', 's'];

/// A single card. Rank 0 = deuce … 12 = ace; suit 0..4 = c, d, h, s.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Card(u8);

impl Card {
    pub const fn new(rank: u8, suit: u8) -> Card {
        assert!(rank < 13 && suit < 4);
        Card(rank * 4 + suit)
    }

    pub const fn from_index(index: u8) -> Card {
        assert!(index < 52);
        Card(index)
    }

    pub const fn index(self) -> u8 {
        self.0
    }

    pub const fn rank(self) -> u8 {
        self.0 >> 2
    }

    pub const fn suit(self) -> u8 {
        self.0 & 3
    }

    pub const fn mask(self) -> u64 {
        1 << self.0
    }
}

pub fn parse_rank(c: char) -> Option<u8> {
    let c = c.to_ascii_uppercase();
    RANK_CHARS.iter().position(|&r| r == c).map(|p| p as u8)
}

pub fn parse_suit(c: char) -> Option<u8> {
    let c = c.to_ascii_lowercase();
    SUIT_CHARS.iter().position(|&s| s == c).map(|p| p as u8)
}

impl FromStr for Card {
    type Err = Error;

    fn from_str(s: &str) -> Result<Card, Error> {
        let mut chars = s.trim().chars();
        let (Some(r), Some(su), None) = (chars.next(), chars.next(), chars.next()) else {
            return Err(Error::Parse(format!("invalid card '{s}'")));
        };
        match (parse_rank(r), parse_suit(su)) {
            (Some(r), Some(su)) => Ok(Card::new(r, su)),
            _ => Err(Error::Parse(format!("invalid card '{s}'"))),
        }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}",
            RANK_CHARS[self.rank() as usize],
            SUIT_CHARS[self.suit() as usize]
        )
    }
}

impl Serialize for Card {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Card {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Card, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// A set of cards; bit `i` is set when card index `i` is present.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct CardSet(pub u64);

impl CardSet {
    pub const EMPTY: CardSet = CardSet(0);
    pub const FULL_DECK: CardSet = CardSet((1 << 52) - 1);

    pub fn from_cards(cards: &[Card]) -> CardSet {
        CardSet(cards.iter().fold(0, |m, c| m | c.mask()))
    }

    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains(self, card: Card) -> bool {
        self.0 & card.mask() != 0
    }

    pub const fn intersects(self, other: CardSet) -> bool {
        self.0 & other.0 != 0
    }

    pub fn insert(&mut self, card: Card) {
        self.0 |= card.mask();
    }

    pub const fn union(self, other: CardSet) -> CardSet {
        CardSet(self.0 | other.0)
    }

    /// Cards of the full deck not in this set.
    pub const fn complement(self) -> CardSet {
        CardSet(!self.0 & Self::FULL_DECK.0)
    }

    pub fn iter(self) -> impl Iterator<Item = Card> {
        let mut m = self.0;
        std::iter::from_fn(move || {
            if m == 0 {
                return None;
            }
            let i = m.trailing_zeros() as u8;
            m &= m - 1;
            Some(Card(i))
        })
    }

    pub fn to_vec(self) -> Vec<Card> {
        self.iter().collect()
    }

    /// Parses concatenated or whitespace/comma separated cards: "AhKd7c", "Ah Kd 7c".
    pub fn parse(s: &str) -> Result<CardSet, Error> {
        let cleaned: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != ',')
            .collect();
        if !cleaned.len().is_multiple_of(2) || !cleaned.is_ascii() {
            return Err(Error::Parse(format!("invalid card list '{s}'")));
        }
        let mut set = CardSet::EMPTY;
        for i in (0..cleaned.len()).step_by(2) {
            let card: Card = cleaned[i..i + 2].parse()?;
            if set.contains(card) {
                return Err(Error::Parse(format!("duplicate card {card} in '{s}'")));
            }
            set.insert(card);
        }
        Ok(set)
    }
}

impl fmt::Display for CardSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cards = self.to_vec();
        cards.sort_by(|a, b| b.cmp(a));
        for c in cards {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

/// All 52 cards in index order.
pub fn deck() -> Vec<Card> {
    (0..52).map(Card).collect()
}

/// Number of k-subsets of an n-set.
pub const fn binomial(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }
    let k = if k > n - k { n - k } else { k };
    let mut r = 1u64;
    let mut i = 0;
    while i < k {
        r = r * (n - i) / (i + 1);
        i += 1;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format_round_trip() {
        for c in deck() {
            let s = c.to_string();
            assert_eq!(s.parse::<Card>().unwrap(), c);
        }
        assert_eq!("Ah".parse::<Card>().unwrap(), Card::new(12, 2));
        assert_eq!("td".parse::<Card>().unwrap(), Card::new(8, 1));
        assert!("1h".parse::<Card>().is_err());
        assert!("Ahh".parse::<Card>().is_err());
    }

    #[test]
    fn card_set_parsing() {
        let s = CardSet::parse("Ah Kd, 7c").unwrap();
        assert_eq!(s.len(), 3);
        assert_eq!(s.to_string(), "AhKd7c");
        assert!(CardSet::parse("AhAh").is_err());
        assert_eq!(CardSet::FULL_DECK.len(), 52);
        assert_eq!(binomial(52, 5), 2_598_960);
    }
}
