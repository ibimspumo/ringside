//! Matching client windows to hand histories and placing seats on screen.

use std::collections::BTreeMap;
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

/// Normalised (0..1) point inside the table's client area.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn clamped(self) -> Self {
        Point { x: self.x.clamp(0.0, 1.0), y: self.y.clamp(0.0, 1.0) }
    }
}

/// Where a seat's HUD sits. `x`, `y` is the avatar centre (the ring). Once moved by the user,
/// `card` is the top-left corner of the classic stat box and `plate` that of the graphic
/// name plate; without them both sit at their default place next to the avatar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SeatPos {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub card: Option<Point>,
    #[serde(default)]
    pub plate: Option<Point>,
}

impl SeatPos {
    pub const fn at(x: f64, y: f64) -> Self {
        SeatPos { x, y, card: None, plate: None }
    }

    pub fn clamped(self) -> Self {
        let ring = Point { x: self.x, y: self.y }.clamped();
        SeatPos { x: ring.x, y: ring.y, card: self.card.map(Point::clamped), plate: self.plate.map(Point::clamped) }
    }

    fn map(self, f: impl Fn(Point) -> Point) -> Self {
        let ring = f(Point { x: self.x, y: self.y });
        SeatPos { x: ring.x, y: ring.y, card: self.card.map(&f), plate: self.plate.map(&f) }
    }
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
        return STARS_8_SEATS.iter().map(|&(x, y)| SeatPos::at(x, y)).collect();
    }
    let n = max_seats.max(2) as f64;
    (0..max_seats.max(2))
        .map(|i| {
            let angle = std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::TAU / n;
            SeatPos::at(0.5 + 0.311 * angle.cos(), 0.426 + 0.297 * angle.sin())
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

/// Width / height of the client area the default layouts were measured in.
pub const REFERENCE_ASPECT: f64 = 953.0 / 657.0;

/// Layouts are stored per table size and window shape, e.g. "8@1.45" or "8@1.80".
pub fn layout_key(max_seats: u8, aspect: f64) -> String {
    format!("{max_seats}@{:.2}", (aspect * 20.0).round() / 20.0)
}

fn parse_key(key: &str) -> Option<(u8, f64)> {
    match key.split_once('@') {
        Some((max, aspect)) => Some((max.parse().ok()?, aspect.parse().ok()?)),
        // Saved by versions that did not know about window shapes.
        None => Some((key.parse().ok()?, REFERENCE_ASPECT)),
    }
}

/// Moves positions from one window shape to another, assuming the client draws the table
/// scene at a fixed shape, centred and scaled to fit (letterboxed) inside the window.
pub fn adapt(layout: &[SeatPos], from_aspect: f64, to_aspect: f64) -> Vec<SeatPos> {
    // Window of height 1 and width `aspect`; the scene is the largest REFERENCE_ASPECT box in it.
    let scene = |aspect: f64| {
        let w = aspect.min(REFERENCE_ASPECT);
        let h = w / REFERENCE_ASPECT;
        (w, h, (aspect - w) / 2.0, (1.0 - h) / 2.0)
    };
    let (fw, fh, fx, fy) = scene(from_aspect);
    let (tw, th, tx, ty) = scene(to_aspect);
    layout
        .iter()
        .map(|p| {
            p.map(|pt| {
                let u = (pt.x * from_aspect - fx) / fw;
                let v = (pt.y - fy) / fh;
                Point { x: (tx + u * tw) / to_aspect, y: ty + v * th }
            })
        })
        .collect()
}

/// The layout to use for a table: saved by the user for exactly this shape, else shipped with
/// the app for this shape, else the nearest shape of either converted, else the measured default.
pub fn resolve_layout(
    saved: &BTreeMap<String, Vec<SeatPos>>,
    builtin: &BTreeMap<String, Vec<SeatPos>>,
    max_seats: u8,
    aspect: f64,
) -> Vec<SeatPos> {
    let key = layout_key(max_seats, aspect);
    let fits = |l: &&Vec<SeatPos>| l.len() == max_seats as usize;
    if let Some(l) = saved.get(&key).filter(fits).or_else(|| builtin.get(&key).filter(fits)) {
        return l.clone();
    }
    let nearest = saved
        .iter()
        .chain(builtin.iter())
        .filter_map(|(k, l)| parse_key(k).filter(|(m, _)| *m == max_seats).map(|(_, a)| (a, l)))
        .filter(|(_, l)| fits(l))
        .min_by(|a, b| (a.0 - aspect).abs().total_cmp(&(b.0 - aspect).abs()));
    match nearest {
        Some((from, l)) => adapt(l, from, aspect),
        None => adapt(&default_layout(max_seats), REFERENCE_ASPECT, aspect),
    }
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
                "$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd - 30/60 Ante 8 - Turnier 4046789300 Tisch 10 - Eingeloggt als Hero"
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

    #[test]
    fn window_shapes() {
        assert_eq!(layout_key(8, 953.0 / 657.0), "8@1.45");
        assert_eq!(layout_key(9, 1920.0 / 1050.0), "9@1.85");

        let l = default_layout(8);
        let same = adapt(&l, REFERENCE_ASPECT, REFERENCE_ASPECT);
        assert!((same[2].x - l[2].x).abs() < 1e-9 && (same[2].y - l[2].y).abs() < 1e-9);
        // Wider window: the scene is centred, so seats move towards the middle horizontally.
        let wide = adapt(&l, REFERENCE_ASPECT, 16.0 / 9.0);
        assert!(wide[2].x > l[2].x && (wide[2].y - l[2].y).abs() < 1e-9);
        let back = adapt(&wide, 16.0 / 9.0, REFERENCE_ASPECT);
        assert!((back[2].x - l[2].x).abs() < 1e-9);

        let mut saved = BTreeMap::new();
        let mut moved = l.clone();
        moved[0].x = 0.4;
        saved.insert("8".to_string(), moved.clone());
        let none = BTreeMap::new();
        assert_eq!(resolve_layout(&saved, &none, 8, REFERENCE_ASPECT), moved, "legacy key is the reference shape");
        let builtin = BTreeMap::from([("8@1.80".to_string(), l.clone())]);
        assert_eq!(resolve_layout(&saved, &builtin, 8, 1.8), l, "exact shipped shape beats converting");
        assert_eq!(resolve_layout(&none, &none, 6, REFERENCE_ASPECT), default_layout(6));
    }

    #[test]
    fn layouts_saved_before_movable_cards_still_load() {
        let old: SeatPos = serde_json::from_str(r#"{"x":0.4,"y":0.7}"#).unwrap();
        assert_eq!(old, SeatPos::at(0.4, 0.7));
        let moved = SeatPos { card: Some(Point { x: -0.1, y: 0.5 }), ..SeatPos::at(1.4, 0.7) }.clamped();
        assert_eq!(moved, SeatPos { card: Some(Point { x: 0.0, y: 0.5 }), ..SeatPos::at(1.0, 0.7) });
    }
}
