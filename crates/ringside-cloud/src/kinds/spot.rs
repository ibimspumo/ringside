//! Heads-up flop spots outside the library catalog: a flop Ringside's preflop model
//! reaches along a line, with the pot, stack and bet sizes of the hand analysis or the
//! practice table that led there. Stored in the library format (`ringside_solver::library`).
//!
//! # What is shared
//! The input names the model configuration (table family, ante, table size, stack
//! depth, model version) and the preflop line in model seats; both ranges follow from that, so a free
//! range never leaves the computer and the verifier rebuilds the ranges itself. Board,
//! pot, stack and sizes are abstract numbers in big blinds and percent of the pot. No
//! names, hand or tournament numbers, times or money amounts: [`SpotInput`] has no field
//! for them, and the server takes no other fields.
//!
//! # Key
//! The board is the canonical flop of its suit class (`library::flops::canonical`); the
//! model ranges weigh every suit alike, so spots on A♠K♠2♦ and A♥K♥2♣ share one key and
//! one solution, renamed with `Stored::renamed` on the way in and out.
//!
//! # Check
//! As for catalog spots ([`super::postflop`]): three solves with flop strategies locked.
//! Only trees that fit the verifier's memory are inputs.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ringside_engine::preflop::{Action, Kind as PreflopKind, Step};
pub use ringside_engine::preflop_table::ModelLine;
use ringside_engine::preflop_table::{self, Family, STACKS};
use ringside_engine::pushfold_table::Ante;
use ringside_engine::Card;
use ringside_play::scenario::{PotType, Scenario as ModelScenario, heads_up, steer};
use ringside_solver::library::catalog::{Catalog, PotType as LibraryPot, Scenario};
use ringside_solver::library::flops::canonical;
use ringside_solver::library::format::Stored;
use ringside_solver::library::check;
use ringside_solver::{Spot, StreetSizes, estimate};
use serde::{Deserialize, Serialize};

use super::Kind;

pub struct SpotKind;

/// Version of the preflop model the ranges come from; bump it whenever the embedded
/// preflop tables of either family change (the test `model_tables_are_pinned` fails then).
pub const MODEL_VERSION: u32 = 2;
/// Catalog version and scenario id in the header of a stored spot of this kind.
pub const CATALOG_VERSION: u32 = 0;
pub const SCENARIO: &str = "spot";
/// Largest stored spot the cloud takes, as for catalog spots.
pub const MAX_BYTES: usize = super::postflop::MAX_BYTES;
const MAX_LINE: usize = 8;
const MAX_CBB: u32 = 50_000;
const MAX_TO_MBB: u32 = 1_000_000;
/// Per street and kind; a size more multiplies the tree.
const MAX_SIZES: usize = ringside_solver::MAX_SIZES;
/// The tree must fit in this much memory without compression, so each of the verifier's
/// three solves stays inside `CHECK_MEMORY` on a 16 GB runner.
const MAX_TREE: u64 = 4 << 30;
const CHECK_MEMORY: u64 = 8 << 30;
const CHECK_ITERATIONS: u32 = 300;
/// Exploitability the verifier's locked solves aim for, percent of the pot.
const CHECK_TARGET_PCT: f64 = 0.5;

/// One non-fold preflop action: model seat, action and the chips in front afterwards in
/// thousandths of a BB.
pub type LineStep = (u32, String, u32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpotInput {
    pub model: u32,
    /// Table family: "plain" (practice, library) or "limps" (hand analysis).
    pub family: String,
    /// "none", "classic" or "big_blind".
    pub ante: String,
    pub players: u32,
    pub stack_bb: u32,
    pub line: Vec<LineStep>,
    /// Canonical flop, e.g. "AsKs2h".
    pub board: String,
    /// Pot at the flop and the smaller stack behind, hundredths of a BB.
    pub pot_cbb: u32,
    pub stack_cbb: u32,
    /// Percent of the pot, ascending: flop bet, flop raise, turn bet, turn raise, river
    /// bet, river raise.
    pub sizes: Vec<Vec<u32>>,
    /// All-in is offered once it is at most this percentage of the pot.
    pub allin_pct: u32,
}

/// The stored spot, base64.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpotSolution {
    pub data: String,
}

fn kind_name(k: PreflopKind) -> Option<&'static str> {
    Some(match k {
        PreflopKind::Check => "check",
        PreflopKind::Call => "call",
        PreflopKind::Limp => "limp",
        PreflopKind::Raise => "raise",
        PreflopKind::AllIn => "all_in",
        PreflopKind::Fold => return None,
    })
}

fn kind_of(s: &str) -> Option<PreflopKind> {
    [PreflopKind::Check, PreflopKind::Call, PreflopKind::Limp, PreflopKind::Raise, PreflopKind::AllIn]
        .into_iter()
        .find(|k| kind_name(*k) == Some(s))
}

fn family_name(f: Family) -> &'static str {
    match f {
        Family::Plain => "plain",
        Family::Limps => "limps",
    }
}

fn family_of(s: &str) -> Option<Family> {
    Family::ALL.into_iter().find(|f| family_name(*f) == s)
}

fn ante_of(s: &str) -> Option<Ante> {
    Ante::ALL.into_iter().find(|a| a.key() == s)
}

fn cards(text: &str) -> Result<Vec<Card>, String> {
    if text.len() != 6 || !text.is_ascii() {
        return Err("board must be a flop of three cards".into());
    }
    let cards: Vec<Card> = (0..3).map(|i| text[2 * i..2 * i + 2].parse::<Card>()).collect::<Result<_, _>>().map_err(|_| "board has no valid cards".to_string())?;
    if cards[0] == cards[1] || cards[1] == cards[2] || cards[0] == cards[2] {
        return Err("board has a card twice".into());
    }
    Ok(cards)
}

/// The canonical flop of a board's suit class.
pub fn canonical_flop(board: &str) -> Result<String, String> {
    let compact: String = board.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let c = cards(&compact)?;
    Ok(canonical([c[0], c[1], c[2]]).iter().map(|c| c.to_string()).collect())
}

/// `x` as a whole number of `per` units, if it is one (to a millionth).
fn whole(x: f64, per: f64, max: u32) -> Option<u32> {
    let v = x * per;
    let r = v.round();
    ((v - r).abs() < 1e-6 && r >= 0.0 && r <= max as f64).then_some(r as u32)
}

impl SpotInput {
    /// The input for a local flop spot whose ranges are exactly the model's along
    /// `model`'s line, or why it cannot be shared.
    pub fn from_model(model: &ModelLine, spot: &Spot) -> Result<SpotInput, String> {
        let line = model
            .line
            .iter()
            .map(|s| {
                let kind = kind_name(s.action.kind).ok_or("fold in the line")?;
                let to = whole(s.action.to_bb, 1000.0, MAX_TO_MBB).ok_or("amount out of range")?;
                Ok((s.seat as u32, kind.to_string(), to))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let int = |x: f64, what: &str| whole(x, 1.0, 1000).ok_or(format!("{what} is not a whole percentage"));
        let mut sizes = Vec::with_capacity(6);
        for s in &spot.sizes {
            for list in [&s.bet, &s.raise] {
                let mut v = list.iter().map(|&x| int(x, "a size")).collect::<Result<Vec<u32>, _>>()?;
                v.sort_unstable();
                v.dedup();
                sizes.push(v);
            }
        }
        let input = SpotInput {
            model: MODEL_VERSION,
            family: family_name(model.family).into(),
            ante: model.ante.key().into(),
            players: model.players as u32,
            stack_bb: whole(model.stack_bb, 1.0, 1000).ok_or("stack depth is not whole")?,
            line,
            board: canonical_flop(&spot.board)?,
            pot_cbb: whole(spot.pot, 100.0, MAX_CBB).ok_or("pot out of range")?,
            stack_cbb: whole(spot.stack, 100.0, MAX_CBB).ok_or("stack out of range")?,
            sizes,
            allin_pct: int(spot.allin_threshold, "the all-in threshold")?,
        };
        SpotKind::check_input(&input)?;
        let own = input.spot()?;
        if own.oop_range != spot.oop_range || own.ip_range != spot.ip_range {
            return Err("the ranges differ from the model's".into());
        }
        Ok(input)
    }

    fn model(&self) -> Result<(ModelScenario, Ante), String> {
        if self.model != MODEL_VERSION {
            return Err(format!("model version must be {MODEL_VERSION}"));
        }
        let family = family_of(&self.family).ok_or("unknown table family")?;
        let ante = ante_of(&self.ante).ok_or("unknown ante")?;
        let players = self.players as usize;
        if !family.players().contains(&players) || !STACKS.contains(&(self.stack_bb as f64)) {
            return Err("model configuration is not precomputed".into());
        }
        if self.line.is_empty() || self.line.len() > MAX_LINE {
            return Err(format!("line must have 1 to {MAX_LINE} actions"));
        }
        let line = self
            .line
            .iter()
            .map(|(seat, kind, to)| {
                let kind = kind_of(kind).ok_or("unknown action in the line")?;
                if *seat as usize >= players || *to > MAX_TO_MBB {
                    return Err("line out of range".to_string());
                }
                Ok(Step { seat: *seat as usize, action: Action { kind, to_bb: *to as f64 / 1000.0 } })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let params = family.params(ante, players, self.stack_bb as f64);
        let sol = preflop_table::lookup(&params).ok_or("model configuration is not precomputed")?;
        let (path, walk) = steer(&sol, &params, &line).ok_or("the line is not in the model")?;
        let sc = heads_up(&sol, &params, &path, &walk, None, 0.0).ok_or("the line ends in no heads-up flop with ranges")?;
        Ok((sc, ante))
    }

    /// The solver spot on the canonical board, ranges from the model.
    pub fn spot(&self) -> Result<Spot, String> {
        let (sc, _) = self.model()?;
        let list = |i: usize| self.sizes.get(i).map(|v| v.iter().map(|&x| x as f64).collect()).unwrap_or_default();
        Ok(Spot {
            oop_range: sc.range(0),
            ip_range: sc.range(1),
            board: self.board.clone(),
            pot: self.pot_cbb as f64 / 100.0,
            stack: self.stack_cbb as f64 / 100.0,
            sizes: [0, 1, 2].map(|s| StreetSizes { bet: list(2 * s), raise: list(2 * s + 1) }),
            allin_threshold: self.allin_pct as f64,
        })
    }

    /// A one-scenario catalog for the library format: the canonical board and, if given,
    /// the local board it is renamed from or to.
    pub fn catalog(&self, local_board: Option<&str>) -> Result<Catalog, String> {
        let (sc, _) = self.model()?;
        let spot = self.spot()?;
        let mut flops = vec![self.board.clone()];
        if let Some(b) = local_board {
            let b: String = b.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            if b != self.board {
                flops.push(b);
            }
        }
        let names = ringside_engine::multiway::position_names(self.players as usize);
        Ok(Catalog {
            version: CATALOG_VERSION,
            source: "Preflop-Modell von Ringside".into(),
            flop_weights: vec![1.0; flops.len()],
            flops,
            scenarios: vec![Scenario {
                id: SCENARIO.into(),
                pot_type: if sc.pot_type == PotType::ThreeBet { LibraryPot::ThreeBet } else { LibraryPot::Srp },
                oop: names.get(sc.oop).copied().unwrap_or("?").into(),
                ip: names.get(sc.ip).copied().unwrap_or("?").into(),
                stack_bb: self.stack_bb as f64,
                pot: spot.pot,
                stack: spot.stack,
                oop_range: spot.oop_range,
                ip_range: spot.ip_range,
                sizes: spot.sizes,
                allin_threshold: spot.allin_threshold,
                oop_note: String::new(),
                ip_note: String::new(),
            }],
            target_pct: CHECK_TARGET_PCT,
            max_iterations: CHECK_ITERATIONS,
        })
    }
}

impl SpotSolution {
    pub fn from_stored(stored: &Stored) -> SpotSolution {
        SpotSolution { data: STANDARD.encode(stored.to_bytes()) }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        if self.data.len() > MAX_BYTES.div_ceil(3) * 4 {
            return Err("stored spot too large".into());
        }
        STANDARD.decode(&self.data).map_err(|e| format!("data: {e}"))
    }

    /// The stored spot on the canonical board, read and checked against `input`.
    pub fn stored(&self, input: &SpotInput) -> Result<Stored, String> {
        let bytes = self.bytes()?;
        let catalog = input.catalog(None)?;
        let stored = Stored::from_bytes(&bytes, &catalog).map_err(|e| e.to_string())?;
        if stored.flop != input.board {
            return Err("stored spot is for another board".into());
        }
        Ok(stored)
    }
}

impl Kind for SpotKind {
    const NAME: &'static str = "spot";
    const ENGINE_VERSION: u32 = 1;
    /// Percent of the pot. The analysis solves to 1 %; the locked re-solves add their own
    /// error, an unsolved strategy measures tens of percent.
    const TOLERANCE: f64 = 2.0;
    const CHECK_SECONDS: u64 = 900;

    type Input = SpotInput;
    type Solution = SpotSolution;

    fn check_input(i: &SpotInput) -> Result<(), String> {
        i.model()?;
        if canonical_flop(&i.board)? != i.board {
            return Err("board must be the canonical flop".into());
        }
        if !(100..=MAX_CBB).contains(&i.pot_cbb) || !(1..=MAX_CBB).contains(&i.stack_cbb) {
            return Err("pot or stack out of range".into());
        }
        if i.sizes.len() != 6 {
            return Err("sizes must have six lists".into());
        }
        for list in &i.sizes {
            if list.len() > MAX_SIZES || list.iter().any(|&x| !(1..=1000).contains(&x)) || list.windows(2).any(|w| w[0] >= w[1]) {
                return Err(format!("each size list must hold up to {MAX_SIZES} ascending sizes of 1 to 1000 %"));
            }
        }
        if i.allin_pct > 1000 {
            return Err("all-in threshold out of range".into());
        }
        let e = estimate(&i.spot()?, CHECK_MEMORY, 4).map_err(|e| e.to_string())?;
        if e.compressed || e.memory_bytes > MAX_TREE {
            return Err("the tree is too large for the verifier".into());
        }
        Ok(())
    }

    fn exploitability(input: &SpotInput, solution: &SpotSolution) -> Result<f64, String> {
        Ok(Self::assess(input, solution)?.0)
    }

    fn assess(input: &SpotInput, solution: &SpotSolution) -> Result<(f64, Option<SpotSolution>), String> {
        let stored = solution.stored(input)?;
        let catalog = input.catalog(None)?;
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let checked = check(&stored, &catalog, threads, CHECK_MEMORY, CHECK_ITERATIONS, || false).map_err(|e| e.to_string())?;
        Ok((checked.exploitability_pct, Some(SpotSolution::from_stored(&checked.rebuilt))))
    }

    fn check_download(input: &SpotInput, solution: &SpotSolution) -> Result<(), String> {
        // The memory estimate in `check_input` builds the tree; the form check suffices.
        input.model()?;
        solution.stored(input).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical::canonicalize;

    fn shared() -> serde_json::Value {
        serde_json::from_str(include_str!("../../testdata/spot.json")).unwrap()
    }

    /// The button opens, the big blind calls: 9 players, big-blind ante, 40 BB.
    fn btn_vs_bb() -> (ModelLine, Spot) {
        let ante = Ante::BigBlind;
        let sol = preflop_table::chart(ante, 9, 40.0).unwrap();
        let params = preflop_table::params(ante, 9, 40.0);
        let sc = ringside_play::scenario::scenarios(&sol, &params)
            .into_iter()
            .find(|s| s.line.len() == 2 && s.line[0].seat == 6 && s.line[1].seat == 8)
            .expect("the model's BTN vs BB flop");
        let model = ModelLine { family: Family::Plain, ante, players: 9, stack_bb: 40.0, line: sc.line.clone() };
        (model, sc.spot("Kh7c2h"))
    }

    #[test]
    fn a_model_spot_becomes_an_input_on_the_canonical_flop() {
        let (model, spot) = btn_vs_bb();
        let input = SpotInput::from_model(&model, &spot).unwrap();
        assert_eq!(input.board, "Ks7h2s");
        let back = input.spot().unwrap();
        assert_eq!((back.oop_range, back.ip_range), (spot.oop_range.clone(), spot.ip_range.clone()));
        assert!(SpotKind::check_input(&input).is_ok());
    }

    #[test]
    fn the_key_is_the_same_for_every_suit_renaming() {
        let (model, spot) = btn_vs_bb();
        let keys: Vec<String> = ["Kh7c2h", "Ks7d2s", "Kd7h2d", "2cKc7s"]
            .iter()
            .map(|b| SpotKind::key(&SpotInput::from_model(&model, &Spot { board: b.to_string(), ..spot.clone() }).unwrap()).unwrap())
            .collect();
        assert!(keys.windows(2).all(|w| w[0] == w[1]), "{keys:?}");
        let other = SpotInput::from_model(&model, &Spot { board: "Kh7h2h".into(), ..spot.clone() }).unwrap();
        assert_ne!(SpotKind::key(&other).unwrap(), keys[0], "a monotone flop is another spot");
    }

    #[test]
    fn edited_ranges_and_odd_amounts_are_not_shared() {
        let (model, spot) = btn_vs_bb();
        let edited = Spot { oop_range: format!("{},72o", spot.oop_range), ..spot.clone() };
        assert!(SpotInput::from_model(&model, &edited).is_err(), "a range off the model");
        let odd = Spot { pot: spot.pot + 0.004, ..spot.clone() };
        assert!(SpotInput::from_model(&model, &odd).is_err(), "pot finer than a hundredth of a BB");
        let mut far = model.clone();
        far.stack_bb = 41.0;
        assert!(SpotInput::from_model(&far, &spot).is_err(), "a depth the model has no table for");
        let turn = Spot { board: "Kh7c2h5d".into(), ..spot.clone() };
        assert!(SpotInput::from_model(&model, &turn).is_err(), "only flops");
    }

    #[test]
    fn inputs_out_of_range_are_refused() {
        let (model, spot) = btn_vs_bb();
        let good = SpotInput::from_model(&model, &spot).unwrap();
        let bad = [
            SpotInput { board: "Kh7c2h".into(), ..good.clone() },
            SpotInput { model: MODEL_VERSION + 1, ..good.clone() },
            SpotInput { line: vec![(6, "raise".into(), 2_300)], ..good.clone() },
            SpotInput { sizes: vec![vec![75, 33], vec![], vec![], vec![], vec![], vec![]], ..good.clone() },
            SpotInput { sizes: vec![vec![10, 20, 33, 50, 75]; 6], ..good.clone() },
            SpotInput { pot_cbb: 0, ..good.clone() },
        ];
        for b in bad {
            assert!(SpotKind::check_input(&b).is_err(), "{b:?}");
        }
    }

    #[test]
    fn keys_match_the_shared_vectors() {
        let v = shared();
        for case in v["keys"].as_array().unwrap() {
            let input: SpotInput = serde_json::from_value(case["input"].clone()).unwrap();
            assert_eq!(canonicalize(&case["input"]).unwrap(), case["canonical"].as_str().unwrap());
            assert_eq!(SpotKind::key(&input).unwrap(), case["key"].as_str().unwrap());
        }
        for case in v["flops"].as_array().unwrap() {
            assert_eq!(canonical_flop(case[0].as_str().unwrap()).unwrap(), case[1].as_str().unwrap());
        }
    }

    #[test]
    fn model_tables_are_pinned() {
        let mut all = Vec::new();
        for family in Family::ALL {
            for ante in Ante::ALL {
                for &players in family.players() {
                    all.extend_from_slice(preflop_table::embedded_file(family, ante, players).unwrap());
                }
            }
        }
        let digest = ring::digest::digest(&ring::digest::SHA256, &all);
        let hex: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, shared()["model_tables_sha256"].as_str().unwrap(), "preflop tables changed: bump MODEL_VERSION and this hash");
    }

    #[test]
    fn uploads_name_no_person_hand_time_or_money() {
        let (model, spot) = btn_vs_bb();
        let input = SpotInput::from_model(&model, &spot).unwrap();
        let body = crate::kinds::submission::<SpotKind>(&input, &SpotSolution { data: "AAAA".into() });
        crate::privacy::assert_anonymous(&body);
    }
}
