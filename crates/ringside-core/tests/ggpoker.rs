//! GGPoker hand histories and summaries (anonymised, shortened fixtures in `fixtures/ggpoker`).

use std::io::Write;

use ringside_core::allin::hero_all_in;
use ringside_core::db::{Db, Filter, HandQuery};
use ringside_core::import::import_paths;
use ringside_core::model::{ActionKind, GameKind, Hand, Site, hand_label, tournament_label};
use ringside_core::parser::ParseError;
use ringside_core::tournament::parse_summaries;
use ringside_core::{parse_hand, split_hands};

const MTT: &str = include_str!("fixtures/ggpoker/mtt.txt");
const CASH: &str = include_str!("fixtures/ggpoker/cash.txt");
const SPECIAL: &str = include_str!("fixtures/ggpoker/special.txt");
const SPIN: &str = include_str!("fixtures/ggpoker/spin.txt");
const SUMMARIES: &str = include_str!("fixtures/ggpoker/summaries.txt");
const PS_TOURNAMENT: &str = include_str!("fixtures/tournament.txt");

fn hands(text: &str) -> Vec<Hand> {
    split_hands(text).into_iter().filter_map(|h| parse_hand(h).ok()).collect()
}

fn hand(text: &str, label: &str) -> Hand {
    hands(text).into_iter().find(|h| hand_label(h.id) == label).unwrap_or_else(|| panic!("{label} parses"))
}

/// What goes into the pot comes out, minus every fee; a cash drop adds to the pot from outside.
#[test]
fn chips_are_conserved() {
    for h in [MTT, CASH, SPECIAL, SPIN].iter().flat_map(|t| hands(t)) {
        let invested: i64 = h.seats.iter().map(|s| h.invested(&s.name)).sum();
        let collected: i64 = h.collected.iter().map(|(_, a)| a).sum();
        assert_eq!(collected + h.rake, h.total_pot, "{} collected", hand_label(h.id));
        let drop = h.total_pot - invested;
        assert!(drop == 0 || hand_label(h.id) == "HD3100200455", "{} invested {invested} of {}", hand_label(h.id), h.total_pot);
        assert_eq!(h.site, Site::GgPoker);
        assert_eq!(h.hero.as_deref(), Some("Hero"));
    }
}

#[test]
fn tournament_header_from_the_right() {
    let h = hand(MTT, "TM5100200301");
    assert_eq!(h.game, GameKind::Tournament);
    assert_eq!(tournament_label(h.tournament_id.unwrap()), "241300777");
    assert_eq!(h.tournament_name.as_deref(), Some("Bounty Hunters $10.80 [No Limit Hold'em], $5K GTD"));
    assert_eq!(h.variant, "Hold'em No Limit");
    assert_eq!((h.small_blind, h.big_blind), (15000, 30000));
    assert_eq!(h.played_at, "2026/11/14 20:41:12");
    assert_eq!(h.max_seats, 8);
    assert_eq!(h.hero_cards, ["Ah", "Qh"]);
    assert_eq!(h.actions.iter().filter(|a| a.kind == ActionKind::PostAnte).count(), 7);
    assert!(h.showdown);
    assert_eq!(h.net("Hero"), -360000);
    // Several spaces before "- Level" and a level without a space.
    let earlier = hand(MTT, "TM5100200288");
    assert_eq!((earlier.small_blind, earlier.big_blind), (10000, 20000));
    assert!(!earlier.showdown, "the marker alone is no showdown");
    assert_eq!(earlier.net("Hero"), 116500);
}

#[test]
fn ids_never_meet_pokerstars_ids() {
    let gg = hand(MTT, "TM5100200301");
    let ps = hands(PS_TOURNAMENT).remove(0);
    assert_eq!(Site::of_id(ps.id), Site::PokerStars);
    assert_eq!(Site::of_id(gg.id), Site::GgPoker);
    assert!(gg.id < 1 << 53, "exact as a JavaScript number");
    assert_ne!(gg.tournament_id, ps.tournament_id);
    assert!(gg.table_key().starts_with("gg:"));
    for label in ["TM5100200301", "HD1", "RC942726385", "SG3100200805", "OM3100200700"] {
        assert_eq!(hand_label(ringside_core::model::gg_hand_id(label).unwrap()), label);
    }
}

#[test]
fn cash_fees_straddle_dead_blind_and_cash_drop() {
    let h = hand(CASH, "HD3100200401");
    assert_eq!(h.currency.as_deref(), Some("USD"));
    assert_eq!((h.total_pot, h.rake), (201, 11), "rake and jackpot both leave the pot");
    assert_eq!(h.net("Hero"), 98);
    assert!(h.showdown);
    assert_eq!(h.shown.len(), 2);

    let s = hand(CASH, "HD3100200455");
    let kind = |p: &str| s.actions.iter().find(|a| a.player == p).map(|a| a.kind);
    assert_eq!(kind("c0ffee42"), Some(ActionKind::PostStraddle));
    assert_eq!(kind("a1b2c3d4"), Some(ActionKind::PostDead));
    assert!(s.shown.is_empty(), "one card shown after folding is no hand");
    assert!(!s.showdown);
    assert_eq!(s.net("Hero"), -5);
    assert_eq!(s.net("c0ffee42"), 60 - 5, "straddle back minus what was returned");
}

#[test]
fn games_without_a_model_are_left_out() {
    let short_deck = split_hands(CASH).into_iter().find(|h| h.contains("ShortDeck")).unwrap();
    assert!(matches!(parse_hand(short_deck), Err(ParseError::Unsupported(_))));
    let all_in_or_fold = "Poker Hand #AF100: Hold'em No Limit ($0.25/$0.5) - 2026/11/12 21:20:00\nTable 'AoF1' 6-max Seat #1 is the button\nSeat 1: Hero ($5 in chips)\n";
    assert!(matches!(parse_hand(all_in_or_fold), Err(ParseError::Unsupported(_))));
    let plo5 = "Poker Hand #OM812126: PLO-5 ($5/$10/$20) - 2021/04/08 01:36:05\nTable 'P' 6-max Seat #1 is the button\nSeat 1: Hero ($500 in chips)\nSeat 2: ab12 ($500 in chips)\n";
    let h = parse_hand(plo5).unwrap();
    assert_eq!((h.variant.as_str(), h.small_blind, h.big_blind), ("5 Card Omaha Pot Limit", 500, 1000));
}

/// Run it twice and the EV cashout change what the hero got, never what the decision was worth.
#[test]
fn run_it_twice_and_cashout_change_the_result_not_the_expectation() {
    let twice = hand(SPECIAL, "HD3100200502");
    assert_eq!(twice.board, ["Td", "6s", "2h", "Ks", "3c"], "the first board");
    assert_eq!(twice.net("Hero"), 815 - 840);
    let a = hero_all_in(&twice).expect("all-in on the turn with both hands shown");
    assert_eq!(a.board.len(), 4);
    assert!(a.share_pct < 5.0, "one out on the turn: {}", a.share_pct);
    assert!(twice.net("Hero") as f64 - a.ev > 700.0, "half the pot back is luck");

    let cashout = hand(SPECIAL, "RC3100200610");
    assert_eq!(cashout.cashouts, [("Hero".to_string(), 1580)]);
    assert_eq!(cashout.won("Hero"), 0);
    assert_eq!(cashout.net("Hero"), 1580 - 1000);
    let a = hero_all_in(&cashout).expect("all-in preflop");
    assert!((75.0..90.0).contains(&a.share_pct), "aces against kings: {}", a.share_pct);
    assert!((cashout.net("Hero") as f64 - a.ev).abs() < 100.0, "a cashout pays about the expectation");

    let insured = hand(SPECIAL, "OM3100200700");
    assert_eq!(insured.variant, "Omaha Pot Limit");
    assert_eq!(insured.total_pot, 3750);
    assert_eq!(insured.cashouts, [("Hero".to_string(), -400)]);
    assert_eq!(insured.net("Hero"), 3750 - 400 - 1700);
}

#[test]
fn spin_and_gold_is_a_three_handed_tournament() {
    let h = hand(SPIN, "SG3100200805");
    assert_eq!(h.max_seats, 3);
    assert_eq!(h.tournament_name.as_deref(), Some("Spin&Gold #11"));
    assert_eq!((h.small_blind, h.big_blind), (1500, 3000));
    assert_eq!(h.net("Hero"), 97500 - 48000);
    assert_eq!(hand(SPIN, "SG3100200800").tournament_name.as_deref(), Some("Spin & Gold #6"));
}

#[test]
fn summaries() {
    let s = parse_summaries(SUMMARIES);
    assert_eq!(s.len(), 3);
    let ko = &s[0];
    assert_eq!(tournament_label(ko.id), "241300777");
    assert_eq!(ko.name.as_deref(), Some("Bounty Hunters $10.80 [No Limit Hold'em], $5K GTD"));
    assert_eq!(ko.game, "NL Hold'em");
    let b = ko.buy_in.as_ref().unwrap();
    assert_eq!((b.prize, b.fee, b.bounty, b.currency.as_deref()), (500, 80, 500, Some("USD")), "prize + fee + bounty");
    assert_eq!((ko.entrants, ko.place, ko.prize), (Some(412), Some(37), 270));
    assert_eq!(ko.started_at.as_deref(), Some("2026/11/14 20:00:00"));
    let spin = &s[1];
    assert_eq!((spin.place, spin.prize), (Some(1), 1000), "no total line: the place line names the prize");
    assert_eq!(spin.started_at, None, "UTC start: the first hand dates it in local time");
    let daily = &s[2];
    assert_eq!((daily.place, daily.prize, daily.buy_in.as_ref().map(|b| (b.prize, b.bounty, b.fee))), (Some(1322), 0, Some((2300, 0, 200))));
}

#[test]
fn import_counts_duplicates_reads_zips_and_keeps_players_apart() {
    let dir = tempfile::tempdir().unwrap();
    let txt = dir.path().join("GG20261114-1941 - Bounty Hunters.txt");
    std::fs::write(&txt, MTT).unwrap();
    let zip_path = dir.path().join("pokercraft.zip");
    {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in [("cash.txt", CASH), ("special.txt", SPECIAL), ("spin.txt", SPIN), ("mtt.txt", MTT), ("summary.txt", SUMMARIES)] {
            zip.start_file(name, opts).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.start_file("readme.pdf", opts).unwrap();
        zip.write_all(b"%PDF").unwrap();
        zip.finish().unwrap();
    }
    let mut db = Db::open_in_memory().unwrap();
    // A PokerStars player who happens to be called like the GGPoker hero.
    db.insert_hands(&[(hands(PS_TOURNAMENT).remove(0), split_hands(PS_TOURNAMENT)[0].to_string())]).unwrap();

    let first = import_paths(&mut db, std::slice::from_ref(&txt)).unwrap();
    assert_eq!((first.new_hands, first.duplicates), (2, 0));
    let again = import_paths(&mut db, &[txt.clone(), zip_path.clone()]).unwrap();
    assert_eq!(again.new_hands, 2 + 3 + 2, "cash (Short Deck left out), special, spin");
    assert_eq!(again.duplicates, 2 + 2, "the file again, and once more inside the archive");
    assert_eq!(again.tournaments, 3);
    assert_eq!(again.skipped, 1, "the PDF");

    let sites: Vec<_> = db.hands_per_site().unwrap();
    assert!(sites.contains(&(Site::GgPoker, 9)), "{sites:?}");
    let gg_only = db.hands(&HandQuery { site: Some(Site::GgPoker), limit: 100, ..Default::default() }).unwrap();
    assert_eq!(gg_only.len(), 9);
    assert!(gg_only.iter().all(|h| h.site == Site::GgPoker));
    let ps_only = db.hands(&HandQuery { site: Some(Site::PokerStars), limit: 100, ..Default::default() }).unwrap();
    assert_eq!(ps_only.len(), 1);

    // GGPoker ids are not reliably one person: no player rows, no mixing with PokerStars names.
    let players = db.players(&Filter::default(), "", 100).unwrap();
    assert!(players.iter().all(|p| p.name.len() > 8 || !p.name.chars().all(|c| c.is_ascii_hexdigit())), "{players:?}");
    assert!(!players.iter().any(|p| p.name == "e19a7b33"));
    let dated = db.players(&Filter { since: Some("2000".into()), ..Default::default() }, "", 100).unwrap();
    assert_eq!(dated.len(), players.len());

    let t = db.tournaments(&Filter::default()).unwrap();
    let ko = t.iter().find(|r| tournament_label(r.id) == "241300777").unwrap();
    assert_eq!(ko.site, Site::GgPoker);
    assert_eq!(ko.name.as_deref(), Some("Bounty Hunters $10.80 [No Limit Hold'em], $5K GTD"));
    assert_eq!((ko.place, ko.prize, ko.hands), (Some(37), Some(270), 2));
    let spin = t.iter().find(|r| tournament_label(r.id) == "210000123").unwrap();
    assert_eq!(spin.started_at, "2026/11/15 19:00:05", "UTC summary start replaced by the first hand");

    let sessions = db.sessions(&Filter::default()).unwrap();
    assert!(sessions.iter().any(|s| s.sites == [Site::GgPoker]));
}

#[test]
fn a_watched_folder_reads_a_file_again_only_when_it_changed() {
    use ringside_core::import::import_watched;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("GG20261112-2010 - NLHGreen17.txt");
    let mut db = Db::open_in_memory().unwrap();
    std::fs::write(&path, MTT).unwrap();
    assert_eq!(import_watched(&mut db, &path).unwrap().new_hands, 2);
    assert_eq!(import_watched(&mut db, &path).unwrap().files, 0, "unchanged: not read again");
    // A newer download of the same table puts its new hand on top.
    std::fs::write(&path, format!("{}\n\n\n{MTT}", split_hands(SPIN)[0])).unwrap();
    let r = import_watched(&mut db, &path).unwrap();
    assert_eq!((r.new_hands, r.duplicates), (1, 2));
}
