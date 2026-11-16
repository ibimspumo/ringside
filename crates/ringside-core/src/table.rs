//! Matching client windows to hand histories and placing seats on screen.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::model::{GameKind, Hand, table_key};

/// "Tournament 123 Table 4", "Turnier 123 Tisch 4", "Tournoi 123 Table 4", ...: the client
/// language only changes the words, so match a long tournament id, a word, a table number.
static TOURNAMENT_TITLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{6,})\s+\p{L}+\s+(\d{1,4})\b").unwrap());

/// Table key for a table window title in any client language, e.g.
/// `"Aquila II - $0.05/$0.10 USD - No Limit Hold'em - Logged In as Hero"` or
/// `"... - 30/60 Ante 8 - Turnier 4046789300 Tisch 10 - Eingeloggt als Hero"`.
pub fn key_from_title(title: &str) -> Option<String> {
    // Lobby and other client windows.
    if title.starts_with("PokerStars") {
        return None;
    }
    if let Some(c) = TOURNAMENT_TITLE.captures(title) {
        let id: u64 = c[1].parse().ok()?;
        return Some(table_key(GameKind::Tournament, Some(id), &format!("{id} {}", &c[2])));
    }
    let name = title.split(" - ").next()?.trim();
    (!name.is_empty() && title.contains(" - ")).then(|| table_key(GameKind::Cash, None, name))
}

/// Normalised (0..1) position of a seat's avatar centre inside the table's client area.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SeatPos {
    pub x: f64,
    pub y: f64,
}

/// Avatar centres of the PokerStars desktop client at an 8-seat table, measured from
/// screenshots of the client area (identical across table themes). Hero first, then clockwise.
const STARS_8_SEATS: [(f64, f64); 8] = [
    (0.432, 0.723),
    (0.278, 0.629),
    (0.189, 0.406),
    (0.278, 0.198),
    (0.570, 0.129),
    (0.722, 0.198),
    (0.812, 0.406),
    (0.722, 0.629),
];

/// Default avatar positions, hero first and then clockwise like seat numbers. Sizes without a
/// measured layout use the oval the measured seats lie on.
pub fn default_layout(max_seats: u8) -> Vec<SeatPos> {
    if max_seats == 8 {
        return STARS_8_SEATS.iter().map(|&(x, y)| SeatPos { x, y }).collect();
    }
    let n = max_seats.max(2) as f64;
    (0..max_seats.max(2))
        .map(|i| {
            let angle = std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::TAU / n;
            SeatPos { x: 0.5 + 0.311 * angle.cos(), y: 0.426 + 0.297 * angle.sin() }
        })
        .collect()
}

/// Index into the layout for a seat. With preferred seating the client rotates the table so
/// the hero sits at the bottom.
pub fn visual_index(hand: &Hand, seat: u8, preferred_seating: bool) -> usize {
    let max = hand.max_seats.max(1) as i32;
    let anchor = if preferred_seating {
        hand.hero.as_deref().and_then(|h| hand.seat_of(h)).unwrap_or(1)
    } else {
        1
    } as i32;
    (seat as i32 - anchor).rem_euclid(max) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles() {
        assert_eq!(
            key_from_title("Aquila II - $0.05/$0.10 USD - No Limit Hold'em - Logged In as Hero").as_deref(),
            Some("c:Aquila II")
        );
        assert_eq!(
            key_from_title("$10+$1 NLHE 9-Max - Blinds 50/100 Ante 10 - Tournament 3500000001 Table 12 - Logged In as Hero")
                .as_deref(),
            Some("t:3500000001:12")
        );
        assert_eq!(
            key_from_title(
                "$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd - 30/60 Ante 8 - Turnier 4046789300 Tisch 10 - Eingeloggt als thoralf69"
            )
            .as_deref(),
            Some("t:4046789300:10")
        );
        assert_eq!(
            key_from_title("Bounty Builder $11, $2.5K Gtd - 45/90 Ante 14 - Turnier 4046791618 Tisch 12 - Eingeloggt als x")
                .as_deref(),
            Some("t:4046791618:12")
        );
        assert_eq!(key_from_title("PokerStars Lobby"), None);
        assert_eq!(key_from_title("PokerStars.EU Lobby - Eingeloggt als x"), None);
    }

    #[test]
    fn layout_starts_at_bottom_and_goes_clockwise() {
        let l = default_layout(6);
        assert!((l[0].x - 0.5).abs() < 1e-9 && l[0].y > 0.7);
        assert!(l[1].x < 0.5, "next seat clockwise is on the left");
        assert!(l[3].y < 0.2, "opposite seat is at the top");
        let eight = default_layout(8);
        assert_eq!(eight.len(), 8);
        assert!(eight[0].y > 0.7 && eight[2].x < 0.2 && eight[6].x > 0.8);
    }
}
