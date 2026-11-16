use std::collections::BTreeMap;

use ringside_core::stats::StatId;
use ringside_core::table::{SeatPos, default_layout};
use serde::{Deserialize, Serialize};

/// A stat is shown in the "low" colour below `low`, in the "high" colour above `high`, and in
/// the neutral text colour otherwise. Two thresholds means at most three colours per stat,
/// which is the PokerStars limit for HUD colour coding.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Threshold {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HudConfig {
    pub ring_outer: StatId,
    pub ring_inner: StatId,
    pub rows: Vec<StatId>,
    pub thresholds: BTreeMap<String, Threshold>,
    pub scale: f64,
    pub opacity: f64,
    /// Count tournament and cash hands separately.
    pub split_by_game: bool,
    pub show_stack: bool,
}

impl Default for HudConfig {
    fn default() -> Self {
        let t = |id: StatId, low: f64, high: f64| (stat_key(id), Threshold { low, high });
        HudConfig {
            ring_outer: StatId::Vpip,
            ring_inner: StatId::Pfr,
            rows: vec![StatId::ThreeBet, StatId::Af, StatId::Cbet],
            thresholds: BTreeMap::from([
                t(StatId::Vpip, 20.0, 35.0),
                t(StatId::Pfr, 12.0, 25.0),
                t(StatId::ThreeBet, 4.0, 10.0),
                t(StatId::FoldToThreeBet, 45.0, 70.0),
                t(StatId::Steal, 25.0, 45.0),
                t(StatId::FoldToSteal, 50.0, 75.0),
                t(StatId::Cbet, 45.0, 70.0),
                t(StatId::FoldToCbet, 35.0, 60.0),
                t(StatId::Wtsd, 22.0, 32.0),
                t(StatId::Wsd, 45.0, 55.0),
                t(StatId::Af, 1.5, 3.5),
                t(StatId::Afq, 35.0, 55.0),
            ]),
            scale: 1.0,
            opacity: 0.92,
            split_by_game: true,
            show_stack: true,
        }
    }
}

pub fn stat_key(id: StatId) -> String {
    serde_json::to_value(id).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hand_history_dirs: Vec<String>,
    pub preferred_seating: bool,
    pub hud: HudConfig,
    /// Seat positions per table size ("6", "9", ...), indexed by visual seat.
    pub layouts: BTreeMap<String, Vec<SeatPos>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hand_history_dirs: Vec::new(),
            preferred_seating: true,
            hud: HudConfig::default(),
            layouts: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn layout(&self, max_seats: u8) -> Vec<SeatPos> {
        let saved = self.layouts.get(&max_seats.to_string());
        match saved {
            Some(l) if l.len() == max_seats as usize => l.clone(),
            _ => default_layout(max_seats),
        }
    }
}
