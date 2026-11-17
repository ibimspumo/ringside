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

/// A title segment holding the blinds: "50/100", "50/100 Ante 12", "Blinds 1,000/2,000 Ante 250".
static BLINDS_SEGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\p{L}+\s+)*[\d.,]+/[\d.,]+(?:\s+\p{L}+\s+[\d.,]+)?$").unwrap());

/// Tournament id and name from a tournament table's window title. The title is
/// "<name> - <blinds> - <Tournament id Table n> - <Logged in as …>", and the name itself may
/// contain " - ", so the name is everything before the blinds segment closest to the id. When
/// no blinds segment is found, everything before the id segment is the name; it may be empty.
pub fn tournament_from_title(title: &str) -> Option<(u64, String)> {
    if title.starts_with("PokerStars") {
        return None;
    }
    let c = TOURNAMENT_TITLE.captures(title)?;
    let id: u64 = c[1].parse().ok()?;
    let start = c.get(0)?.start();
    let before: Vec<&str> = title[..start].split(" - ").collect();
    // The id segment itself: whatever precedes the match inside it is not part of the name.
    let named = &before[..before.len() - 1];
    let end = named.iter().rposition(|s| BLINDS_SEGMENT.is_match(s.trim())).unwrap_or(named.len());
    let name = named[..end].join(" - ").trim().to_string();
    Some((id, name))
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
    /// Where this seat's cards appear in the recap of the last hand, once moved.
    #[serde(default)]
    pub recap: Option<Point>,
}

impl SeatPos {
    pub const fn at(x: f64, y: f64) -> Self {
        SeatPos { x, y, card: None, plate: None, recap: None }
    }

    pub fn clamped(self) -> Self {
        let ring = Point { x: self.x, y: self.y }.clamped();
        SeatPos {
            x: ring.x,
            y: ring.y,
            card: self.card.map(Point::clamped),
            plate: self.plate.map(Point::clamped),
            recap: self.recap.map(Point::clamped),
        }
    }

    fn map(self, f: impl Fn(Point) -> Point) -> Self {
        let ring = f(Point { x: self.x, y: self.y });
        SeatPos { x: ring.x, y: ring.y, card: self.card.map(&f), plate: self.plate.map(&f), recap: self.recap.map(&f) }
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

/// Layouts are stored once per table size ("8"), in coordinates of a window with
/// [`REFERENCE_ASPECT`]; every table window converts them to its own shape with [`adapt`].
/// The client scales its table scene to fit the window and centres it, so one layout fits
/// every window size and shape.
pub fn layout_key(max_seats: u8) -> String {
    max_seats.to_string()
}

fn parse_key(key: &str) -> Option<(u8, f64)> {
    match key.split_once('@') {
        // Saved per window shape by v0.2.0.
        Some((max, aspect)) => Some((max.parse().ok()?, aspect.parse().ok()?)),
        None => Some((key.parse().ok()?, REFERENCE_ASPECT)),
    }
}

/// Folds layouts saved per window shape ("8@1.80", v0.2.0) into one per table size. Where a
/// size has several, the one closest to the reference shape wins.
pub fn normalize_layouts(layouts: &mut BTreeMap<String, Vec<SeatPos>>) {
    let mut best: BTreeMap<u8, (f64, Vec<SeatPos>)> = BTreeMap::new();
    for (key, layout) in layouts.iter() {
        let Some((max, aspect)) = parse_key(key) else { continue };
        if layout.len() != max as usize {
            continue;
        }
        let distance = (aspect - REFERENCE_ASPECT).abs();
        if best.get(&max).is_none_or(|(d, _)| distance < *d) {
            best.insert(max, (distance, adapt(layout, aspect, REFERENCE_ASPECT)));
        }
    }
    *layouts = best.into_iter().map(|(max, (_, l))| (layout_key(max), l)).collect();
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

/// The layout for a table of `max_seats` in a window of the given shape: the user's own,
/// else the one shipped with the app, else the measured default.
pub fn resolve_layout(
    saved: &BTreeMap<String, Vec<SeatPos>>,
    builtin: &BTreeMap<String, Vec<SeatPos>>,
    max_seats: u8,
    aspect: f64,
) -> Vec<SeatPos> {
    let key = layout_key(max_seats);
    let fits = |l: &&Vec<SeatPos>| l.len() == max_seats as usize;
    let stored = saved.get(&key).filter(fits).or_else(|| builtin.get(&key).filter(fits));
    match stored {
        Some(l) => adapt(l, REFERENCE_ASPECT, aspect),
        None => adapt(&default_layout(max_seats), REFERENCE_ASPECT, aspect),
    }
}

/// A position dragged in a window of shape `aspect`, in stored (reference) coordinates.
pub fn to_reference(pos: SeatPos, aspect: f64) -> SeatPos {
    adapt(&[pos], aspect, REFERENCE_ASPECT)[0]
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
    fn tournament_names_from_titles() {
        let name = tournament_from_title;
        assert_eq!(
            name("$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd - 50/100 Ante 12 - Turnier 4046789300 Tisch 10 - Eingeloggt als X"),
            Some((4046789300, "$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd".into()))
        );
        assert_eq!(
            name("$10+$1 NLHE 9-Max - Blinds 50/100 Ante 10 - Tournament 3500000001 Table 12 - Logged In as Hero"),
            Some((3500000001, "$10+$1 NLHE 9-Max".into()))
        );
        assert_eq!(
            name("Sunday Million - Day 1 - 1,000/2,000 Ante 250 - Tournament 3500000002 Table 7 - Logged In as Hero"),
            Some((3500000002, "Sunday Million - Day 1".into())),
            "a name with \" - \" stays whole"
        );
        assert_eq!(
            name("Hot $1/$2 Turbo - 1/2 Price Special - 15/30 - Turnier 4046791618 Tisch 3 - Eingeloggt als x"),
            Some((4046791618, "Hot $1/$2 Turbo - 1/2 Price Special".into())),
            "slashes in the name are not blinds"
        );
        assert_eq!(
            name("Freeroll - Turnier 4046791619 Tisch 1 - Eingeloggt als x"),
            Some((4046791619, "Freeroll".into())),
            "no blinds segment"
        );
        assert_eq!(name("Turnier 4046791620 Tisch 1"), Some((4046791620, String::new())));
        assert_eq!(name("Aquila II - $0.05/$0.10 USD - No Limit Hold'em - Logged In as Hero"), None);
        assert_eq!(name("PokerStars Lobby"), None);
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
    fn one_layout_fits_every_window_shape() {
        let l = default_layout(8);
        let same = adapt(&l, REFERENCE_ASPECT, REFERENCE_ASPECT);
        assert!((same[2].x - l[2].x).abs() < 1e-9 && (same[2].y - l[2].y).abs() < 1e-9);
        // Wider window: the scene is centred, so seats move towards the middle horizontally.
        let wide = adapt(&l, REFERENCE_ASPECT, 16.0 / 9.0);
        assert!(wide[2].x > l[2].x && (wide[2].y - l[2].y).abs() < 1e-9);
        let back = adapt(&wide, 16.0 / 9.0, REFERENCE_ASPECT);
        assert!((back[2].x - l[2].x).abs() < 1e-9);

        // Dragged in a wide window, saved once, shown correctly in every shape.
        let mut moved = wide[0];
        moved.x += 0.05;
        let stored = to_reference(moved, 16.0 / 9.0);
        let mut saved = BTreeMap::from([(layout_key(8), l.clone())]);
        saved.get_mut("8").unwrap()[0] = stored;
        let none = BTreeMap::new();
        let in_wide = resolve_layout(&saved, &none, 8, 16.0 / 9.0);
        assert!((in_wide[0].x - moved.x).abs() < 1e-9);
        let in_standard = resolve_layout(&saved, &none, 8, REFERENCE_ASPECT);
        assert!((in_standard[0].x - stored.x).abs() < 1e-9);

        let close = |a: &[SeatPos], b: &[SeatPos]| a.iter().zip(b).all(|(p, q)| (p.x - q.x).abs() < 1e-9 && (p.y - q.y).abs() < 1e-9);
        assert!(close(&resolve_layout(&none, &none, 6, REFERENCE_ASPECT), &default_layout(6)));
        let builtin = BTreeMap::from([(layout_key(8), wide.clone())]);
        assert!(close(&resolve_layout(&none, &builtin, 8, REFERENCE_ASPECT), &wide), "shipped layout beats the default");
    }

    #[test]
    fn layouts_from_v0_2_0_are_folded_per_table_size() {
        let l = default_layout(8);
        let wide = adapt(&l, REFERENCE_ASPECT, 1.8);
        let mut layouts = BTreeMap::from([
            ("8@1.80".to_string(), wide),
            ("8@1.45".to_string(), l.clone()),
            ("6".to_string(), default_layout(6)),
            ("9@1.45".to_string(), vec![SeatPos::at(0.5, 0.5)]),
        ]);
        normalize_layouts(&mut layouts);
        assert_eq!(layouts.keys().collect::<Vec<_>>(), ["6", "8"], "broken 9-seat layout dropped");
        let eight = &layouts["8"];
        assert!((eight[2].x - adapt(&l, 1.45, REFERENCE_ASPECT)[2].x).abs() < 1e-9, "closest shape wins");
    }

    #[test]
    fn layouts_saved_before_movable_cards_still_load() {
        let old: SeatPos = serde_json::from_str(r#"{"x":0.4,"y":0.7}"#).unwrap();
        assert_eq!(old, SeatPos::at(0.4, 0.7));
        let moved = SeatPos { card: Some(Point { x: -0.1, y: 0.5 }), ..SeatPos::at(1.4, 0.7) }.clamped();
        assert_eq!(moved, SeatPos { card: Some(Point { x: 0.0, y: 0.5 }), ..SeatPos::at(1.0, 0.7) });
    }
}
