//! Hole-card combos, the 169 starting-hand classes and weighted ranges.

use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::cards::{Card, CardSet, RANK_CHARS, parse_rank};

pub const NUM_COMBOS: usize = 1326;
pub const NUM_CLASSES: usize = 169;

/// Two hole cards, stored as an index `0..1326` (high card index first).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Combo(u16);

static COMBO_CARDS: LazyLock<Vec<(Card, Card)>> = LazyLock::new(|| {
    let mut v = Vec::with_capacity(NUM_COMBOS);
    for hi in 1..52u8 {
        for lo in 0..hi {
            v.push((Card::from_index(hi), Card::from_index(lo)));
        }
    }
    v
});

impl Combo {
    pub fn new(a: Card, b: Card) -> Combo {
        assert_ne!(a, b, "a combo needs two different cards");
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        let (hi, lo) = (hi.index() as u16, lo.index() as u16);
        Combo(hi * (hi - 1) / 2 + lo)
    }

    pub fn from_index(i: usize) -> Combo {
        assert!(i < NUM_COMBOS);
        Combo(i as u16)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Cards with the higher card index first.
    pub fn cards(self) -> (Card, Card) {
        COMBO_CARDS[self.0 as usize]
    }

    pub fn mask(self) -> CardSet {
        let (a, b) = self.cards();
        CardSet(a.mask() | b.mask())
    }

    pub fn class(self) -> HandClass {
        let (a, b) = self.cards();
        HandClass::from_ranks(a.rank(), b.rank(), a.suit() == b.suit())
    }

    pub fn all() -> impl Iterator<Item = Combo> {
        (0..NUM_COMBOS as u16).map(Combo)
    }
}

impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (a, b) = self.cards();
        write!(f, "{a}{b}")
    }
}

impl FromStr for Combo {
    type Err = Error;
    fn from_str(s: &str) -> Result<Combo, Error> {
        let set = CardSet::parse(s)?;
        let cards = set.to_vec();
        match cards.as_slice() {
            [a, b] => Ok(Combo::new(*a, *b)),
            _ => Err(Error::Parse(format!("'{s}' is not two cards"))),
        }
    }
}

/// One of the 169 strategically distinct starting hands, laid out like the usual
/// 13×13 grid: `row * 13 + col` with ace = 0, pairs on the diagonal, suited hands
/// above it (row < col) and offsuit hands below.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct HandClass(u8);

impl HandClass {
    /// `hi`/`lo` are card ranks (0 = deuce); order doesn't matter.
    pub fn from_ranks(a: u8, b: u8, suited: bool) -> HandClass {
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        let (ghi, glo) = (12 - hi, 12 - lo);
        let id = if hi == lo || suited {
            ghi * 13 + glo
        } else {
            glo * 13 + ghi
        };
        HandClass(id)
    }

    pub fn from_index(i: usize) -> HandClass {
        assert!(i < NUM_CLASSES);
        HandClass(i as u8)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn all() -> impl Iterator<Item = HandClass> {
        (0..NUM_CLASSES as u8).map(HandClass)
    }

    /// (high rank, low rank) with 0 = deuce.
    pub fn ranks(self) -> (u8, u8) {
        let (row, col) = (self.0 / 13, self.0 % 13);
        let (a, b) = (12 - row, 12 - col);
        (a.max(b), a.min(b))
    }

    pub fn is_pair(self) -> bool {
        self.0 / 13 == self.0 % 13
    }

    pub fn is_suited(self) -> bool {
        self.0 / 13 < self.0 % 13
    }

    /// 6 for pairs, 4 suited, 12 offsuit.
    pub fn combo_count(self) -> u32 {
        if self.is_pair() {
            6
        } else if self.is_suited() {
            4
        } else {
            12
        }
    }

    pub fn combos(self) -> &'static [Combo] {
        &CLASS_COMBOS[self.index()]
    }
}

impl fmt::Display for HandClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (hi, lo) = self.ranks();
        let (h, l) = (RANK_CHARS[hi as usize], RANK_CHARS[lo as usize]);
        if self.is_pair() {
            write!(f, "{h}{l}")
        } else if self.is_suited() {
            write!(f, "{h}{l}s")
        } else {
            write!(f, "{h}{l}o")
        }
    }
}

impl FromStr for HandClass {
    type Err = Error;
    fn from_str(s: &str) -> Result<HandClass, Error> {
        match parse_class_spec(s)? {
            ClassSpec { hi, lo, suit: None } if hi == lo => {
                Ok(HandClass::from_ranks(hi, lo, false))
            }
            ClassSpec {
                hi,
                lo,
                suit: Some(suited),
            } => Ok(HandClass::from_ranks(hi, lo, suited)),
            _ => Err(Error::Parse(format!("'{s}' is not a single hand class"))),
        }
    }
}

impl Serialize for HandClass {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for HandClass {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<HandClass, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

static CLASS_COMBOS: LazyLock<Vec<Vec<Combo>>> = LazyLock::new(|| {
    let mut v = vec![Vec::new(); NUM_CLASSES];
    for c in Combo::all() {
        v[c.class().index()].push(c);
    }
    v
});

static PAIR_COUNTS: LazyLock<Vec<[u16; NUM_CLASSES]>> = LazyLock::new(|| {
    let mut t = vec![[0u16; NUM_CLASSES]; NUM_CLASSES];
    for a in Combo::all() {
        for b in Combo::all() {
            if !a.mask().intersects(b.mask()) {
                t[a.class().index()][b.class().index()] += 1;
            }
        }
    }
    t
});

/// Number of ordered combo pairs (a ∈ `a`, b ∈ `b`) that share no card.
pub fn class_pair_count(a: HandClass, b: HandClass) -> u32 {
    PAIR_COUNTS[a.index()][b.index()] as u32
}

/// A weighted set of combos; weights are in `0..=1`.
#[derive(Clone, PartialEq)]
pub struct Range {
    weights: Box<[f32; NUM_COMBOS]>,
}

impl fmt::Debug for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Range({self})")
    }
}

impl Default for Range {
    fn default() -> Self {
        Range::empty()
    }
}

impl Range {
    pub fn empty() -> Range {
        Range {
            weights: Box::new([0.0; NUM_COMBOS]),
        }
    }

    pub fn all() -> Range {
        Range {
            weights: Box::new([1.0; NUM_COMBOS]),
        }
    }

    pub fn single(combo: Combo) -> Range {
        let mut r = Range::empty();
        r.set_combo(combo, 1.0);
        r
    }

    pub fn parse(s: &str) -> Result<Range, Error> {
        s.parse()
    }

    pub fn weight(&self, combo: Combo) -> f32 {
        self.weights[combo.index()]
    }

    pub fn set_combo(&mut self, combo: Combo, weight: f32) {
        self.weights[combo.index()] = weight;
    }

    pub fn set_class(&mut self, class: HandClass, weight: f32) {
        for &c in class.combos() {
            self.set_combo(c, weight);
        }
    }

    /// Combos with a positive weight.
    pub fn combos(&self) -> impl Iterator<Item = (Combo, f32)> + '_ {
        self.weights
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > 0.0)
            .map(|(i, &w)| (Combo(i as u16), w))
    }

    /// Combos with a positive weight that don't touch `dead`.
    pub fn live_combos(&self, dead: CardSet) -> Vec<(Combo, f32)> {
        self.combos()
            .filter(|(c, _)| !c.mask().intersects(dead))
            .collect()
    }

    /// Copy without the combos that touch `dead`.
    pub fn without(&self, dead: CardSet) -> Range {
        let mut r = self.clone();
        for c in Combo::all() {
            if c.mask().intersects(dead) {
                r.set_combo(c, 0.0);
            }
        }
        r
    }

    /// Number of combos with positive weight.
    pub fn combo_count(&self) -> usize {
        self.combos().count()
    }

    /// Sum of weights, i.e. the effective number of combos.
    pub fn weighted_count(&self) -> f64 {
        self.weights.iter().map(|&w| w as f64).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.combos().next().is_none()
    }

    /// The weight shared by every combo of `class`, if they all agree.
    fn class_weight(&self, class: HandClass) -> Option<f32> {
        let combos = class.combos();
        let w = self.weight(combos[0]);
        combos.iter().all(|&c| self.weight(c) == w).then_some(w)
    }
}

impl FromStr for Range {
    type Err = Error;

    fn from_str(s: &str) -> Result<Range, Error> {
        let mut range = Range::empty();
        for token in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let (item, weight) = match token.split_once(':') {
                Some((item, w)) => {
                    let w: f32 = w
                        .trim()
                        .parse()
                        .map_err(|_| Error::Parse(format!("invalid weight in '{token}'")))?;
                    if !(0.0..=1.0).contains(&w) {
                        return Err(Error::Parse(format!("weight out of 0..=1 in '{token}'")));
                    }
                    (item.trim(), w)
                }
                None => (token, 1.0),
            };
            let lower = item.to_ascii_lowercase();
            if lower == "random" || lower == "100%" || lower == "any" {
                range.weights.fill(weight);
                continue;
            }
            if item.len() == 4
                && let Ok(combo) = item.parse::<Combo>()
            {
                range.set_combo(combo, weight);
                continue;
            }
            for class in expand_classes(item)? {
                range.set_class(class, weight);
            }
        }
        Ok(range)
    }
}

/// `hi >= lo`; `suit` is `None` for pairs and for "AK" (both suited and offsuit).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ClassSpec {
    hi: u8,
    lo: u8,
    suit: Option<bool>,
}

impl ClassSpec {
    fn classes(self) -> Vec<HandClass> {
        match self.suit {
            _ if self.hi == self.lo => vec![HandClass::from_ranks(self.hi, self.lo, false)],
            Some(s) => vec![HandClass::from_ranks(self.hi, self.lo, s)],
            None => vec![
                HandClass::from_ranks(self.hi, self.lo, true),
                HandClass::from_ranks(self.hi, self.lo, false),
            ],
        }
    }
}

fn parse_class_spec(s: &str) -> Result<ClassSpec, Error> {
    let err = || Error::Parse(format!("invalid hand '{s}'"));
    let chars: Vec<char> = s.chars().collect();
    let (a, b) = match chars.as_slice() {
        [a, b] | [a, b, _] => (
            parse_rank(*a).ok_or_else(err)?,
            parse_rank(*b).ok_or_else(err)?,
        ),
        _ => return Err(err()),
    };
    let suit = match chars.get(2).map(|c| c.to_ascii_lowercase()) {
        None => None,
        Some('s') => Some(true),
        Some('o') => Some(false),
        Some(_) => return Err(err()),
    };
    if a == b && suit.is_some() {
        return Err(err());
    }
    Ok(ClassSpec {
        hi: a.max(b),
        lo: a.min(b),
        suit,
    })
}

fn expand_classes(item: &str) -> Result<Vec<HandClass>, Error> {
    let err = || Error::Parse(format!("invalid range item '{item}'"));
    if let Some(base) = item.strip_suffix('+') {
        let spec = parse_class_spec(base)?;
        let specs: Vec<ClassSpec> = if spec.hi == spec.lo {
            (spec.lo..13)
                .map(|r| ClassSpec {
                    hi: r,
                    lo: r,
                    suit: None,
                })
                .collect()
        } else {
            (spec.lo..spec.hi)
                .map(|lo| ClassSpec { lo, ..spec })
                .collect()
        };
        return Ok(specs.into_iter().flat_map(ClassSpec::classes).collect());
    }
    if let Some((from, to)) = item.split_once('-') {
        let (a, b) = (parse_class_spec(from.trim())?, parse_class_spec(to.trim())?);
        let specs: Vec<ClassSpec> = if a.hi == a.lo && b.hi == b.lo {
            (a.lo.min(b.lo)..=a.lo.max(b.lo))
                .map(|r| ClassSpec {
                    hi: r,
                    lo: r,
                    suit: None,
                })
                .collect()
        } else if a.hi == b.hi && a.suit == b.suit && a.hi != a.lo && b.hi != b.lo {
            (a.lo.min(b.lo)..=a.lo.max(b.lo))
                .map(|lo| ClassSpec { lo, ..a })
                .collect()
        } else {
            return Err(err());
        };
        return Ok(specs.into_iter().flat_map(ClassSpec::classes).collect());
    }
    Ok(parse_class_spec(item)?.classes())
}

fn fmt_weight(w: f32) -> String {
    if w == 1.0 {
        String::new()
    } else {
        format!(":{w}")
    }
}

/// Canonical notation: pairs, then suited and offsuit hands by high card, with runs
/// collapsed to `X+` / `X-Y`; classes whose combos have different weights are listed
/// combo by combo at the end.
impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let w0 = self.weights[0];
        if w0 > 0.0 && self.weights.iter().all(|&w| w == w0) {
            return write!(f, "random{}", fmt_weight(w0));
        }
        let rc = |r: u8| RANK_CHARS[r as usize];
        let mut parts: Vec<String> = Vec::new();

        // A run is a list of (rank, weight) from the top kicker down.
        let mut emit_runs =
            |cells: Vec<(u8, Option<f32>)>, name: &dyn Fn(u8) -> String, top: u8| {
                let mut i = 0;
                while i < cells.len() {
                    let Some(w) = cells[i].1.filter(|&w| w > 0.0) else {
                        i += 1;
                        continue;
                    };
                    let mut j = i;
                    while j + 1 < cells.len() && cells[j + 1].1 == Some(w) {
                        j += 1;
                    }
                    let (first, last) = (cells[i].0, cells[j].0);
                    let text = if i == j {
                        name(first)
                    } else if first == top {
                        format!("{}+", name(last))
                    } else {
                        format!("{}-{}", name(first), name(last))
                    };
                    parts.push(format!("{text}{}", fmt_weight(w)));
                    i = j + 1;
                }
            };

        let pairs: Vec<(u8, Option<f32>)> = (0..13u8)
            .rev()
            .map(|r| (r, self.class_weight(HandClass::from_ranks(r, r, false))))
            .collect();
        emit_runs(pairs, &|r| format!("{}{}", rc(r), rc(r)), 12);
        for suited in [true, false] {
            let tag = if suited { 's' } else { 'o' };
            for hi in (1..13u8).rev() {
                let cells: Vec<(u8, Option<f32>)> = (0..hi)
                    .rev()
                    .map(|lo| (lo, self.class_weight(HandClass::from_ranks(hi, lo, suited))))
                    .collect();
                emit_runs(cells, &|lo| format!("{}{}{tag}", rc(hi), rc(lo)), hi - 1);
            }
        }
        let mut singles: Vec<(Combo, f32)> = self
            .combos()
            .filter(|(c, _)| self.class_weight(c.class()).is_none())
            .collect();
        singles.sort_by_key(|s| std::cmp::Reverse(s.0));
        parts.extend(
            singles
                .iter()
                .map(|(c, w)| format!("{c}{}", fmt_weight(*w))),
        );
        write!(f, "{}", parts.join(", "))
    }
}

impl Serialize for Range {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Range {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Range, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(s: &str) -> usize {
        Range::parse(s).unwrap().combo_count()
    }

    #[test]
    fn combo_counts() {
        assert_eq!(count("22+"), 78);
        assert_eq!(count("AKs"), 4);
        assert_eq!(count("AKo"), 12);
        assert_eq!(count("AK"), 16);
        assert_eq!(count("random"), 1326);
        assert_eq!(count("100%"), 1326);
        assert_eq!(count("A2s+"), 48);
        assert_eq!(count("KTo+"), 36);
        assert_eq!(count("77-55"), 18);
        assert_eq!(count("55-77"), 18);
        assert_eq!(count("A5s-A2s"), 16);
        assert_eq!(count("AhKh"), 1);
        assert_eq!(count("AA, AKs, AhKh"), 10);
        assert_eq!(count("KA"), 16);
    }

    #[test]
    fn weights_and_errors() {
        let r = Range::parse("AKo:0.5, AA").unwrap();
        assert!((r.weighted_count() - 12.0).abs() < 1e-9);
        assert_eq!(r.weight("AhKd".parse().unwrap()), 0.5);
        for bad in [
            "AAs", "AKx", "A", "AK-QJ", "AKs-AQo", "AK:2", "AK:x", "22-AKs",
        ] {
            assert!(Range::parse(bad).is_err(), "{bad} should fail");
        }
    }

    #[test]
    fn classes() {
        assert_eq!(HandClass::all().count(), 169);
        let total: u32 = HandClass::all().map(|c| c.combo_count()).sum();
        assert_eq!(total, 1326);
        for c in HandClass::all() {
            assert_eq!(c.combos().len() as u32, c.combo_count());
            assert_eq!(c.to_string().parse::<HandClass>().unwrap(), c);
        }
        assert_eq!(HandClass::from_index(0).to_string(), "AA");
        assert_eq!(HandClass::from_index(1).to_string(), "AKs");
        assert_eq!(HandClass::from_index(13).to_string(), "AKo");
        assert_eq!(HandClass::from_index(168).to_string(), "22");
        let aa: HandClass = "AA".parse().unwrap();
        let kk: HandClass = "KK".parse().unwrap();
        let aks: HandClass = "AKs".parse().unwrap();
        assert_eq!(class_pair_count(aa, kk), 36);
        assert_eq!(class_pair_count(aa, aa), 6);
        assert_eq!(class_pair_count(aa, aks), 6 * 4 - 3 * 4);
    }

    #[test]
    fn dead_card_removal() {
        let r = Range::parse("AA").unwrap();
        let dead = CardSet::parse("Ah").unwrap();
        assert_eq!(r.live_combos(dead).len(), 3);
        assert_eq!(r.without(dead).combo_count(), 3);
    }

    #[test]
    fn canonical_round_trip() {
        let cases = [
            ("22+", "22+"),
            ("random", "random"),
            ("AA, KK, QQ", "QQ+"),
            ("KK-22", "KK-22"),
            ("77-55, A2s+, KTo+, AhKd", "77-55, A2s+, KTo+, AhKd"),
            ("A5s-A2s, AKs", "AKs, A5s-A2s"),
            ("AK", "AKs, AKo"),
            ("A2s+, AA", "AA, A2s+"),
            ("AKo:0.5, AQo:0.5, JJ", "JJ, AQo+:0.5"),
            ("AA, AhAd:0.5", "AsAh, AsAd, AsAc, AhAd:0.5, AhAc, AdAc"),
            ("", ""),
            ("random:0.25", "random:0.25"),
        ];
        for (input, canonical) in cases {
            let r = Range::parse(input).unwrap();
            assert_eq!(r.to_string(), canonical, "{input}");
            assert_eq!(Range::parse(canonical).unwrap(), r, "{input}");
        }
    }
}
