//! German number and card formatting for questions and facts.

use ringside_engine::{Card, HandCategory};

/// `1234.5` → "1234,50" with `decimals` places and a typographic minus.
pub fn num(x: f64, decimals: usize) -> String {
    let s = format!("{:.*}", decimals, x.abs()).replace('.', ",");
    if x < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        format!("−{s}")
    } else {
        s
    }
}

/// Percent with one decimal: "22,7 %".
pub fn pct(x: f64) -> String {
    format!("{} %", num(x, 1))
}

/// Big blinds, trailing zeros trimmed: "10 bb", "4,6 bb", "0,25 bb".
pub fn bb(x: f64) -> String {
    format!("{} bb", short(x))
}

/// Signed big blinds with two decimals: "+1,23 bb", "−0,50 bb".
pub fn signed_bb(x: f64) -> String {
    let s = num(x, 2);
    if s.starts_with('−') || s == "0,00" { format!("{s} bb") } else { format!("+{s} bb") }
}

/// Up to two decimals without trailing zeros: 15.7 → "15,7", 4.0 → "4".
pub fn short(x: f64) -> String {
    let s = num(x, 2);
    let s = s.trim_end_matches('0');
    s.trim_end_matches(',').to_string()
}

/// Two decimals, as amounts are written in the pot odds exercises: "15,70".
pub fn money(x: f64) -> String {
    num(x, 2)
}

pub fn cards(cards: &[Card]) -> String {
    cards.iter().map(Card::to_string).collect::<Vec<_>>().join(" ")
}

pub fn category(c: HandCategory) -> &'static str {
    match c {
        HandCategory::HighCard => "Höchste Karte",
        HandCategory::Pair => "Paar",
        HandCategory::TwoPair => "Zwei Paare",
        HandCategory::Trips => "Drilling",
        HandCategory::Straight => "Straße",
        HandCategory::Flush => "Flush",
        HandCategory::FullHouse => "Full House",
        HandCategory::Quads => "Vierling",
        HandCategory::StraightFlush => "Straight Flush",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_numbers() {
        assert_eq!(num(22.6666, 1), "22,7");
        assert_eq!(num(-0.5, 2), "−0,50");
        assert_eq!(num(-0.0001, 2), "0,00");
        assert_eq!(pct(22.66), "22,7 %");
        assert_eq!(bb(4.6), "4,6 bb");
        assert_eq!(bb(10.0), "10 bb");
        assert_eq!(bb(0.25), "0,25 bb");
        assert_eq!(signed_bb(1.234), "+1,23 bb");
        assert_eq!(signed_bb(-0.5), "−0,50 bb");
        assert_eq!(money(15.7), "15,70");
    }
}
