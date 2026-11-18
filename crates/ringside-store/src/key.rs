//! One key per solved spot, whichever area solved, downloaded or asks for it.
//!
//! The key is the one Ringside Cloud files the spot under, so a spot solved here, the
//! same spot from a pack and the same spot another player shared meet in one entry:
//! - a spot of the library catalog (its ranges, pot, stack and sizes, on one of its flops)
//!   has the key of the cloud kind `postflop`;
//! - otherwise a flop whose ranges are the preflop model's along a [`ModelLine`] has the
//!   key of the cloud kind `spot` (the same for every suit renaming of the flop);
//! - every other spot has a local key from board, ranges, pot, stack and sizes
//!   ([`spot_key`]), and never leaves the computer; when both ranges weigh every suit
//!   alike it is taken on the smallest suit renaming of the board ([`iso_key`]), so one
//!   solve serves every renaming.
//!
//! A catalog spot or a model flop has the same key on every suit renaming of its flop:
//! the store renames the stored solution to the board asked for. The local key of the
//! dealt board is kept beside it, so a caller that does not know the model line still
//! finds a spot that was filed under it.

use std::sync::LazyLock;

use ringside_cloud::kinds::spot::canonical_flop;
use ringside_cloud::kinds::{Kind, Postflop, PostflopInput, SpotInput, SpotKind};
use ringside_engine::preflop_table::ModelLine;
use ringside_engine::{Card, CardSet, Range};
use ringside_solver::Spot;
use ringside_solver::library::flops::classes;
use ringside_solver::library::{SpotId, catalog};
use ringside_solver::saved::spot_key;
use ringside_solver::suits;

/// Where a spot is known in Ringside Cloud.
#[derive(Clone, Debug, PartialEq)]
pub enum Cloud {
    /// A spot of the library catalog.
    Catalog { input: PostflopInput, id: SpotId },
    /// A flop of the preflop model.
    Spot(SpotInput),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyed {
    /// "postflop:…", "spot:…" or "local:…".
    pub key: String,
    /// "local:…" of the same spot: what a caller without the model line computes.
    pub local: String,
    pub cloud: Option<Cloud>,
}

impl Keyed {
    /// The spot's ranges come from the preflop model or the catalog, so it may be shared.
    pub fn shareable(&self) -> bool {
        self.cloud.is_some()
    }
}

fn compact(board: &str) -> String {
    board.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// The catalog scenario with exactly the ranges, pot, stack and sizes of `spot`.
pub fn catalog_scenario(spot: &Spot) -> Option<usize> {
    catalog().scenarios.iter().position(|s| {
        s.oop_range == spot.oop_range
            && s.ip_range == spot.ip_range
            && (s.pot - spot.pot).abs() < 0.011
            && (s.stack - spot.stack).abs() < 0.011
            && s.sizes == spot.sizes
            && s.allin_threshold == spot.allin_threshold
    })
}

/// The catalog spot that `spot` is, up to a suit renaming of its flop: a catalog
/// scenario on a flop whose canonical flop is one of the catalog's.
pub fn catalog_id(spot: &Spot) -> Option<SpotId> {
    let flop = catalog().flop(&canonical_flop(&spot.board).ok()?)?;
    Some(SpotId { scenario: catalog_scenario(spot)?, flop })
}

/// The cloud input of the catalog spot `id`.
pub fn catalog_input(id: SpotId) -> PostflopInput {
    let c = catalog();
    PostflopInput::new(&c.scenarios[id.scenario].id, &c.flops[id.flop])
}

/// The key of a catalog spot, as [`key_of`] gives it.
pub fn catalog_key(id: SpotId) -> String {
    format!("{}:{}", Postflop::NAME, Postflop::key(&catalog_input(id)).unwrap_or_default())
}

/// The key of a model flop, as [`key_of`] gives it.
pub fn spot_kind_key(input: &SpotInput) -> Result<String, String> {
    Ok(format!("{}:{}", SpotKind::NAME, SpotKind::key(input).map_err(|e| e.to_string())?))
}

/// The local key, as [`key_of`] gives it for a spot without a cloud key.
pub fn local_key(spot: &Spot) -> Result<String, String> {
    Ok(format!("local:{}", spot_key(spot).map_err(|e| e.to_string())?))
}

fn cards(board: &str) -> Option<Vec<Card>> {
    let b = compact(board);
    b.len().is_multiple_of(2).then(|| (0..b.len() / 2).map(|i| b[2 * i..2 * i + 2].parse().ok()).collect()).flatten()
}

/// The local key of `spot` on the smallest suit renaming of its board under which both
/// ranges stay the same; `None` when a range sets one suit apart from the others in
/// a way no renaming of the board keeps.
pub fn iso_key(spot: &Spot) -> Option<String> {
    let board = cards(&spot.board)?;
    let dead = CardSet::from_cards(&board);
    let ranges = [Range::parse(&spot.oop_range).ok()?, Range::parse(&spot.ip_range).ok()?];
    let order = |b: &[Card]| {
        let mut flop: Vec<u8> = b[..3].iter().map(|c| c.index()).collect();
        flop.sort_unstable();
        flop.extend(b[3..].iter().map(|c| c.index()));
        flop
    };
    let kept: Vec<_> = suits::all().into_iter().filter(|r| ranges.iter().all(|range| suits::keeps(range, dead, r))).collect();
    // Without every renaming the ranges are not class ranges; spots like that are rare and
    // keep the plain local key.
    if kept.len() != 24 {
        return None;
    }
    let smallest = kept.iter().map(|r| board.iter().map(|&c| suits::rename(c, r)).collect::<Vec<_>>()).min_by_key(|b| order(b))?;
    let text = smallest.iter().map(|c| c.to_string()).collect::<String>();
    Some(format!("iso:{}", spot_key(&Spot { board: text, ..spot.clone() }).ok()?))
}

/// The key of `spot`; `model` is the preflop line its ranges come from, if they do.
pub fn key_of(spot: &Spot, model: Option<&ModelLine>) -> Result<Keyed, String> {
    let local = local_key(spot)?;
    if let Some(id) = catalog_id(spot) {
        let input = catalog_input(id);
        return Ok(Keyed { key: catalog_key(id), local, cloud: Some(Cloud::Catalog { input, id }) });
    }
    if let Some(input) = model.and_then(|m| SpotInput::from_model(m, spot).ok()) {
        return Ok(Keyed { key: spot_kind_key(&input)?, local, cloud: Some(Cloud::Spot(input)) });
    }
    Ok(Keyed { key: iso_key(spot).unwrap_or_else(|| local.clone()), local, cloud: None })
}

/// The canonical flop of every suit class.
pub fn canonical_flops() -> &'static [String] {
    static FLOPS: LazyLock<Vec<String>> = LazyLock::new(|| classes().iter().map(|c| c.text()).collect());
    &FLOPS
}

/// Every flop a line may be solved on, as (key, canonical flop): `spot` is the line's
/// spot on any flop and `model` its preflop line. A catalog scenario has its catalog key
/// on the catalog's flops; a model line has the key of the model flop everywhere else.
pub fn line_flops(spot: &Spot, model: Option<&ModelLine>) -> Vec<(String, String)> {
    let c = catalog();
    let scenario = catalog_scenario(spot);
    let base = model.and_then(|m| SpotInput::from_model(m, spot).ok());
    canonical_flops()
        .iter()
        .filter_map(|flop| {
            if let (Some(scenario), Some(flop_index)) = (scenario, c.flop(flop)) {
                return Some((catalog_key(SpotId { scenario, flop: flop_index }), flop.clone()));
            }
            Some((spot_kind_key(&on_flop(base.as_ref()?, flop).ok()?).ok()?, flop.clone()))
        })
        .collect()
}

/// `input` on another flop, without measuring the tree again: for asking the cloud which
/// flops of a line are solved.
pub fn on_flop(input: &SpotInput, flop: &str) -> Result<SpotInput, String> {
    Ok(SpotInput { board: ringside_cloud::kinds::spot::canonical_flop(flop)?, ..input.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringside_engine::preflop_table::Family;
    use ringside_engine::pushfold_table::Ante;

    /// The button opens, the big blind calls: 9 players, big-blind ante, 40 BB, as practice
    /// plays it.
    fn practice_spot(flop: &str) -> (ModelLine, Spot) {
        let table = ringside_play::TableKey { players: 9, ante: Ante::BigBlind, stack_bb: 40.0 };
        let deep = ringside_play::table::deep(table).expect("plain table");
        let sc = deep.scenarios.iter().find(|s| s.line.len() == 2 && s.line[0].seat == 6 && s.line[1].seat == 8).expect("BTN vs BB");
        let model = ModelLine { family: Family::Plain, ante: table.ante, players: 9, stack_bb: 40.0, line: sc.line.clone() };
        (model, sc.spot(flop))
    }

    /// The analysis builds its spot from the hand: the same table, line, pot and sizes, its
    /// own `Spot` value.
    fn analysis_spot(flop: &str) -> (ModelLine, Spot) {
        let (model, spot) = practice_spot(flop);
        let rebuilt = Spot {
            oop_range: spot.oop_range.clone(),
            ip_range: spot.ip_range.clone(),
            board: flop.chars().collect::<Vec<_>>().chunks(2).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join(" "),
            pot: spot.pot,
            stack: spot.stack,
            sizes: spot.sizes.clone(),
            allin_threshold: spot.allin_threshold,
        };
        (model, rebuilt)
    }

    #[test]
    fn the_same_spot_has_the_same_key_in_every_area() {
        let (model, practice) = practice_spot("Kh8c2h");
        assert_eq!(catalog_id(&practice), None, "a flop off the catalog");
        let (analysed_model, analysed) = analysis_spot("Kh8c2h");
        let a = key_of(&practice, Some(&model)).unwrap();
        let b = key_of(&analysed, Some(&analysed_model)).unwrap();
        assert_eq!(a, b, "practice and analysis");
        assert!(a.key.starts_with("spot:"), "{a:?}");
        // The solver page with the model's ranges, the cloud download and a suit renaming.
        let renamed = key_of(&Spot { board: "Ks8d2s".into(), ..practice.clone() }, Some(&model)).unwrap();
        assert_eq!(renamed.key, a.key, "one key for every suit renaming");
        assert_ne!(renamed.local, a.local, "the local key names the dealt board");
        let Some(Cloud::Spot(input)) = &a.cloud else { panic!("a model flop") };
        assert_eq!(spot_kind_key(input).unwrap(), a.key, "the cloud files it under the same key");
        assert_eq!(spot_kind_key(&on_flop(input, "2sKs8d").unwrap()).unwrap(), a.key);
        // Without the line only the local key is known; the store keeps both.
        let unknown = key_of(&practice, None).unwrap();
        assert_eq!((unknown.local.as_str(), unknown.cloud.is_none()), (a.local.as_str(), true));
        assert!(unknown.key.starts_with("iso:"), "{unknown:?}");
    }

    #[test]
    fn a_local_spot_has_one_key_on_every_suit_renaming_when_its_ranges_allow_it() {
        let (_, spot) = practice_spot("Kh8c2h");
        let on = |board: &str, oop: &str| key_of(&Spot { board: board.into(), oop_range: oop.into(), ..spot.clone() }, None).unwrap();
        let classes = "AA,KK,AKs,T9s,76o";
        let a = on("Kh 7c 2h", classes);
        assert!(a.key.starts_with("iso:"));
        assert_eq!(on("2s Ks 7d", classes).key, a.key, "a suit renaming");
        assert_ne!(on("Ks 7s 2h", classes).key, a.key, "another suit pattern");
        assert_ne!(on("Kh 7c 2h 3d", classes).key, on("Kh 7c 3d 2h", classes).key, "turn and flop are not the same");
        assert_eq!(on("Kh 7c 2h 3d", classes).key, on("Ks 7d 2s 3c", classes).key);
        // A range that sets one suit apart keeps the key of its dealt board.
        let lopsided = on("Kh 7c 2h", "AA,KK,AhQh");
        assert_eq!(lopsided.key, lopsided.local);
        assert_ne!(on("Ks 7d 2s", "AA,KK,AhQh").key, lopsided.key);
    }

    #[test]
    fn a_catalog_spot_has_its_catalog_key_on_every_suit_renaming() {
        let c = catalog();
        let id = SpotId { scenario: 0, flop: 7 };
        let flop = &c.flops[id.flop];
        let renamed: String = flop
            .chars()
            .map(|ch| match ch {
                's' => 'c',
                'h' => 'd',
                'd' => 'h',
                'c' => 's',
                other => other,
            })
            .collect();
        assert_ne!(&renamed, flop);
        let spot = c.scenarios[0].spot(&renamed);
        assert_eq!(catalog_id(&spot), Some(id));
        assert_eq!(key_of(&spot, None).unwrap().key, catalog_key(id));
    }

    #[test]
    fn a_line_lists_every_flop_class_once_with_the_key_it_is_filed_under() {
        let (model, spot) = practice_spot("Kh8c2h");
        let flops = line_flops(&spot, Some(&model));
        assert_eq!(flops.len(), canonical_flops().len());
        assert_eq!(canonical_flops().len(), 1755);
        let (key, flop) = flops.iter().find(|(_, f)| f == &canonical_flop("Kh8c2h").unwrap()).unwrap();
        assert_eq!(key, &key_of(&spot, Some(&model)).unwrap().key, "{flop}");
        // A catalog scenario: its catalog key on the catalog's flops, the model key elsewhere.
        let c = catalog();
        let catalog_spot = c.scenarios[0].spot(&c.flops[0]);
        let mixed = line_flops(&catalog_spot, Some(&model));
        assert_eq!(mixed.iter().filter(|(k, _)| k.starts_with("postflop:")).count(), c.flops.len());
        assert!(line_flops(&catalog_spot, None).len() == c.flops.len(), "without a model line only the catalog's flops");
    }

    #[test]
    fn edited_ranges_or_another_flop_are_another_spot() {
        let (model, spot) = practice_spot("Kh8c2h");
        let base = key_of(&spot, Some(&model)).unwrap();
        let edited = Spot { oop_range: format!("{},72o", spot.oop_range), ..spot.clone() };
        let k = key_of(&edited, Some(&model)).unwrap();
        assert!(!k.key.starts_with("spot:") && !k.shareable(), "edited ranges are never shared");
        assert_ne!(k.key, base.key);
        assert_ne!(key_of(&Spot { board: "Kh8h2h".into(), ..spot.clone() }, Some(&model)).unwrap().key, base.key);
        assert_ne!(key_of(&Spot { pot: spot.pot + 1.0, ..spot }, Some(&model)).unwrap().key, base.key);
    }

    #[test]
    fn a_catalog_spot_has_its_catalog_key_from_every_area() {
        let c = catalog();
        let id = SpotId { scenario: 0, flop: 3 };
        let spot = c.spot(id);
        let k = key_of(&spot, None).unwrap();
        assert_eq!(k.key, catalog_key(id));
        assert!(k.key.starts_with("postflop:"));
        assert_eq!(k.cloud, Some(Cloud::Catalog { input: catalog_input(id), id }));
        // A caller with a model line gets the same key: the catalog goes first.
        let (model, _) = practice_spot("Kh8c2h");
        assert_eq!(key_of(&spot, Some(&model)).unwrap().key, k.key);
        assert_eq!(catalog_id(&Spot { board: c.flops[3].clone(), ..spot.clone() }), Some(id));
    }
}
