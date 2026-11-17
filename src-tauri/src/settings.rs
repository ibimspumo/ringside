use std::collections::BTreeMap;
use std::sync::LazyLock;

use ringside_core::stats::StatId;
use ringside_core::table::{REFERENCE_ASPECT, SeatPos, normalize_layouts, resolve_layout};
use serde::{Deserialize, Serialize};

/// A stat shows the palette's "low" colour below `low`, "high" above `high` and "mid" in
/// between. Two thresholds means at most three colours per stat, which is the PokerStars
/// limit for HUD colour coding.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Threshold {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayMode {
    /// One movable text box per player.
    Classic,
    /// Name plate over the client's own plate plus a segmented ring around the avatar.
    Graphic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassicConfig {
    /// "beginner", "standard", "tournament", "detailed" or "custom".
    pub preset: String,
    /// Stats of the "custom" preset, in display order.
    pub custom: Vec<StatId>,
}

impl Default for ClassicConfig {
    fn default() -> Self {
        ClassicConfig {
            preset: "beginner".into(),
            custom: vec![StatId::Vpip, StatId::Pfr, StatId::ThreeBet, StatId::Af, StatId::Hands],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphicConfig {
    /// Ring segments clockwise from the top left.
    pub segments: Vec<StatId>,
    /// Pages of three big numbers on the plate; a click on a plate shows the next page.
    pub plate_pages: Vec<Vec<StatId>>,
    /// Lay the plate over the client's name plate (hides its action text and timer).
    pub cover_plate: bool,
    /// Plate width as a multiple of the default, for client themes with wider name plates.
    pub plate_width: f64,
}

impl Default for GraphicConfig {
    fn default() -> Self {
        GraphicConfig {
            segments: vec![StatId::Vpip, StatId::Pfr, StatId::Af, StatId::ThreeBet],
            plate_pages: vec![
                vec![StatId::Vpip, StatId::Pfr, StatId::ThreeBet],
                vec![StatId::FoldToThreeBet, StatId::Steal, StatId::FoldToSteal],
                vec![StatId::Cbet, StatId::FoldToCbet, StatId::Af],
            ],
            cover_plate: true,
            plate_width: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HudConfig {
    pub mode: OverlayMode,
    /// "traffic" (red / amber / green) or "icefire" (blue / white / red).
    pub palette: String,
    pub classic: ClassicConfig,
    pub graphic: GraphicConfig,
    pub thresholds: BTreeMap<String, Threshold>,
    pub scale: f64,
    pub opacity: f64,
    /// Count tournament and cash hands separately.
    pub split_by_game: bool,
    pub show_stack: bool,
    pub show_bounty: bool,
    /// How long the previous hand stays on screen after it ended; 0 turns it off.
    pub recap_seconds: f64,
    /// Your own numbers from this tournament (cash: this table today) instead of all hands.
    pub hero_session_only: bool,
    /// Top-left of the recap's board panel, as a fraction of the table window; `None` = default.
    pub recap_board: Option<ringside_core::table::Point>,
}

impl Default for HudConfig {
    fn default() -> Self {
        let t = |id: StatId, low: f64, high: f64| (stat_key(id), Threshold { low, high });
        HudConfig {
            mode: OverlayMode::Graphic,
            palette: "traffic".into(),
            classic: ClassicConfig::default(),
            graphic: GraphicConfig::default(),
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
            opacity: 1.0,
            split_by_game: true,
            show_stack: true,
            show_bounty: true,
            recap_seconds: 4.0,
            hero_session_only: true,
            recap_board: None,
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
    /// Seat positions per table size ("8"), indexed by visual seat, in the coordinates of a
    /// window with the reference shape (see `ringside_core::table::layout_key`).
    pub layouts: BTreeMap<String, Vec<SeatPos>>,
    /// Saved looks and layouts the user can switch between.
    pub presets: Vec<Preset>,
}

/// A named snapshot of the HUD look and all layouts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub preferred_seating: bool,
    pub hud: HudConfig,
    pub layouts: BTreeMap<String, Vec<SeatPos>>,
}

impl Default for Settings {
    fn default() -> Self {
        let builtin = &*BUILTIN;
        Settings {
            hand_history_dirs: Vec::new(),
            preferred_seating: builtin.preferred_seating,
            hud: builtin.hud.clone(),
            layouts: BTreeMap::new(),
            presets: Vec::new(),
        }
    }
}

impl Settings {
    pub fn layout(&self, max_seats: u8, aspect: f64) -> Vec<SeatPos> {
        resolve_layout(&self.layouts, &BUILTIN.layouts, max_seats, aspect)
    }
}

/// Everything about how the HUD looks and where it sits, as exchanged in profile files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub format: String,
    pub preferred_seating: bool,
    pub hud: HudConfig,
    pub layouts: BTreeMap<String, Vec<SeatPos>>,
}

pub const PROFILE_FORMAT: &str = "ringside-profile/1";

impl Default for Profile {
    fn default() -> Self {
        Profile {
            format: PROFILE_FORMAT.into(),
            preferred_seating: true,
            hud: HudConfig::default(),
            layouts: BTreeMap::new(),
        }
    }
}

impl Profile {
    pub fn from_settings(s: &Settings) -> Self {
        Profile {
            format: PROFILE_FORMAT.into(),
            preferred_seating: s.preferred_seating,
            hud: s.hud.clone(),
            layouts: s.layouts.clone(),
        }
    }

    /// Reads a profile file. Rejects anything that is not explicitly a Ringside profile, and
    /// brings every value into a usable range, so a wrong or edited file cannot break the HUD.
    pub fn parse(text: &str) -> Result<Profile, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("Keine JSON-Datei: {e}"))?;
        match value.get("format").and_then(|f| f.as_str()) {
            Some(PROFILE_FORMAT) => {}
            Some(other) => return Err(format!("Unbekanntes Profilformat „{other}“")),
            None => return Err("Das ist keine Ringside-Profildatei.".into()),
        }
        let mut p: Profile = serde_json::from_value(value).map_err(|e| format!("Profil unvollständig: {e}"))?;
        for (key, layout) in &p.layouts {
            // "8", or "8@1.45" as written by v0.2.0.
            let (max, aspect) = match key.split_once('@') {
                Some((m, a)) => (m.parse::<usize>().ok(), a.parse::<f64>().ok()),
                None => (key.parse::<usize>().ok(), Some(REFERENCE_ASPECT)),
            };
            let (Some(max), Some(aspect)) = (max, aspect) else {
                return Err(format!("Ungültiger Layout-Name „{key}“"));
            };
            if !(2..=10).contains(&max) || !(0.5..=4.0).contains(&aspect) || layout.len() != max {
                return Err(format!("Layout „{key}“ passt nicht zu seiner Tischgröße."));
            }
            if layout.iter().any(|s| !s.x.is_finite() || !s.y.is_finite()) {
                return Err(format!("Layout „{key}“ enthält ungültige Koordinaten."));
            }
        }
        normalize_layouts(&mut p.layouts);
        let defaults = HudConfig::default();
        let hud = &mut p.hud;
        hud.scale = hud.scale.clamp(0.5, 2.0);
        hud.opacity = hud.opacity.clamp(0.3, 1.0);
        hud.recap_seconds = hud.recap_seconds.clamp(0.0, 15.0);
        hud.graphic.plate_width = hud.graphic.plate_width.clamp(0.8, 1.5);
        hud.graphic.segments.resize(4, defaults.graphic.segments[0]);
        hud.graphic.plate_pages.truncate(5);
        if hud.graphic.plate_pages.is_empty() {
            hud.graphic.plate_pages = defaults.graphic.plate_pages.clone();
        }
        for page in &mut hud.graphic.plate_pages {
            page.resize(3, StatId::Vpip);
        }
        hud.thresholds.retain(|_, t| t.low.is_finite() && t.high.is_finite() && t.low <= t.high);
        Ok(p)
    }

    pub fn apply_to(self, s: &mut Settings) {
        s.preferred_seating = self.preferred_seating;
        s.hud = self.hud;
        s.layouts = self.layouts;
    }
}

/// The profile shipped with the app: defaults for the HUD and calibrated layouts. To ship a
/// layout a user made, export their profile and replace `defaults/profile.json` with it.
pub static BUILTIN: LazyLock<Profile> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../defaults/profile.json")).expect("defaults/profile.json is valid")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_profile_parses_and_old_settings_still_load() {
        assert_eq!(BUILTIN.format, PROFILE_FORMAT);
        let shipped = Profile::parse(include_str!("../defaults/profile.json")).expect("shipped profile is valid");
        assert_eq!(shipped.layouts.get("8").map(Vec::len), Some(8), "calibrated 8-seat layout ships");
        // Settings saved by v0.1.x: different HUD fields and layouts without a window shape.
        let old = r#"{"hand_history_dirs":["C:/HH"],"preferred_seating":true,
            "hud":{"ring_outer":"vpip","rows":["af"],"scale":1.2},
            "layouts":{"8":[{"x":0.4,"y":0.7}]}}"#;
        let s: Settings = serde_json::from_str(old).unwrap();
        assert_eq!(s.hand_history_dirs, ["C:/HH"]);
        assert_eq!(s.hud.scale, 1.2);
        assert_eq!(s.hud.mode, OverlayMode::Graphic);
    }

    #[test]
    fn profile_import_rejects_foreign_or_broken_files() {
        assert!(Profile::parse("{}").is_err(), "plain JSON is not a profile");
        assert!(Profile::parse(r#"{"format":"other/1"}"#).is_err());
        assert!(Profile::parse(r#"{"format":"ringside-profile/1","layouts":{"8@1.45":[{"x":0.5,"y":0.5}]}}"#).is_err());
        let p = Profile::parse(
            r#"{"format":"ringside-profile/1","hud":{"scale":9,"opacity":0,"graphic":{"segments":["vpip"]}},
                "layouts":{"2@1.45":[{"x":2,"y":0.5},{"x":0.5,"y":-1}]}}"#,
        )
        .unwrap();
        assert_eq!((p.hud.scale, p.hud.opacity, p.hud.graphic.segments.len()), (2.0, 0.3, 4));
        assert!(p.layouts.contains_key("2"), "v0.2.0 shape keys are folded per table size");
    }

    #[test]
    fn profile_round_trip() {
        let mut s = Settings::default();
        s.hud.mode = OverlayMode::Classic;
        s.layouts.insert("6".into(), vec![SeatPos::at(0.5, 0.5); 6]);
        let json = serde_json::to_string(&Profile::from_settings(&s)).unwrap();
        let mut other = Settings { hand_history_dirs: vec!["keep".into()], ..Settings::default() };
        Profile::parse(&json).unwrap().apply_to(&mut other);
        assert_eq!((other.hud.mode, other.layouts.len()), (OverlayMode::Classic, 1));
        assert_eq!(other.hand_history_dirs, ["keep"]);
    }
}
