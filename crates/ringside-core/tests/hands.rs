use std::fs;
use std::io::Write;

use ringside_core::db::Db;
use ringside_core::import::import_file;
use ringside_core::model::{ActionKind, GameKind, Hand};
use ringside_core::stats::{Counters, StatId, analyze};
use ringside_core::{parse_hand, split_hands};

const CASH: &str = include_str!("fixtures/cash.txt");
const TOURNAMENT: &str = include_str!("fixtures/tournament.txt");

fn hands(text: &str) -> Vec<Hand> {
    split_hands(text).into_iter().map(|h| parse_hand(h).expect("parses")).collect()
}

fn counters(hand: &Hand, player: &str) -> Counters {
    analyze(hand).into_iter().find(|(n, _)| n == player).map(|(_, c)| c).expect("player dealt in")
}

/// Money invariants: what goes in comes out (minus rake), for every fixture hand.
#[test]
fn chips_are_conserved() {
    for hand in hands(CASH).iter().chain(hands(TOURNAMENT).iter()) {
        let invested: i64 = hand.seats.iter().map(|s| hand.invested(&s.name)).sum();
        let collected: i64 = hand.collected.iter().map(|(_, a)| a).sum();
        assert_eq!(invested, hand.total_pot, "hand {} invested", hand.id);
        assert_eq!(collected + hand.rake, hand.total_pot, "hand {} collected", hand.id);
        let net: i64 = hand.seats.iter().map(|s| hand.net(&s.name)).sum();
        assert_eq!(net, -hand.rake, "hand {} net", hand.id);
        for seat in &hand.seats {
            assert!(hand.invested(&seat.name) <= seat.stack, "hand {} {} over stack", hand.id, seat.name);
        }
    }
}

#[test]
fn parses_cash_header_and_names_with_colons() {
    let h = &hands(CASH)[0];
    assert_eq!(h.id, 250000000001);
    assert_eq!(h.game, GameKind::Cash);
    assert_eq!(h.variant, "Hold'em No Limit");
    assert_eq!((h.small_blind, h.big_blind), (5, 10));
    assert_eq!(h.currency.as_deref(), Some("USD"));
    assert_eq!(h.table_name, "Aquila II");
    assert_eq!((h.max_seats, h.button_seat), (6, 3));
    assert_eq!(h.hero.as_deref(), Some("Hero"));
    assert_eq!(h.hero_cards, ["Ah", "Kc"]);
    assert_eq!(h.board, ["2c", "7d", "Jh"]);
    assert_eq!(h.seats[5].name, "Eve: The Great");
    assert_eq!(h.actions.iter().filter(|a| a.player == "Eve: The Great").count(), 1);
    assert_eq!(h.net("Hero"), 107);
    assert_eq!(h.table_key(), "c:Aquila II");
}

#[test]
fn parses_tournament_with_antes_commas_and_sitting_out() {
    let h = &hands(TOURNAMENT)[0];
    assert_eq!(h.game, GameKind::Tournament);
    assert_eq!(h.tournament_id, Some(3500000001));
    assert_eq!((h.small_blind, h.big_blind), (5000, 10000));
    assert_eq!(h.currency, None);
    assert_eq!(h.seats[1].stack, 150000);
    assert!(h.seats[3].sitting_out);
    assert_eq!(h.table_key(), "t:3500000001:12");
    let shove = h.actions.iter().find(|a| a.kind == ActionKind::Raise).unwrap();
    assert_eq!((shove.amount, shove.to, shove.all_in), (149000, Some(149000), true));
    assert!(h.showdown);
    assert_eq!(h.board.len(), 5);
    assert_eq!(h.shown.len(), 2);
    assert_eq!(h.net("Hero"), -150000);
}

#[test]
fn preflop_stats_steal_three_bet_and_folds() {
    let h = &hands(CASH)[0];
    let bob = counters(h, "Bob");
    assert_eq!((bob.vpip, bob.pfr, bob.steal_opp, bob.steal), (1, 1, 1, 1));
    assert_eq!((bob.fold_to_three_bet_opp, bob.fold_to_three_bet), (1, 0));
    assert_eq!((bob.fold_to_cbet_opp, bob.fold_to_cbet), (1, 1));

    let hero = counters(h, "Hero");
    assert_eq!((hero.three_bet_opp, hero.three_bet), (1, 1));
    assert_eq!((hero.fold_to_steal_opp, hero.fold_to_steal), (1, 0));
    assert_eq!((hero.cbet_opp, hero.cbet), (1, 1));

    let dave = counters(h, "Dave");
    assert_eq!((dave.fold_to_steal_opp, dave.fold_to_steal), (1, 1));
    assert_eq!(dave.vpip, 0);

    let carol = counters(h, "Carol");
    assert_eq!((carol.steal_opp, carol.three_bet_opp), (0, 1));
}

#[test]
fn limped_pot_has_no_steal_or_cbet_and_counts_showdown() {
    let h = &hands(CASH)[1];
    let hero = counters(h, "Hero");
    assert_eq!((hero.vpip, hero.pfr, hero.steal_opp, hero.cbet_opp), (1, 0, 0, 0));
    assert_eq!((hero.postflop_aggr, hero.postflop_calls), (3, 1));
    assert_eq!((hero.wtsd, hero.wsd), (1, 0));

    let alice = counters(h, "Alice");
    assert_eq!((alice.wtsd, alice.wsd, alice.postflop_calls, alice.postflop_aggr), (1, 1, 3, 1));

    let eve = counters(h, "Eve: The Great");
    assert_eq!((eve.vpip_opp, eve.vpip, eve.saw_flop, eve.wtsd), (1, 0, 1, 0));
}

#[test]
fn all_in_preflop_still_counts_showdown_and_skips_sitting_out() {
    let h = &hands(TOURNAMENT)[0];
    let fish = counters(h, "Fish");
    assert_eq!((fish.steal_opp, fish.steal, fish.cbet_opp, fish.wtsd, fish.wsd), (1, 1, 0, 1, 1));
    let reg = counters(h, "Reg");
    assert_eq!((reg.fold_to_steal_opp, reg.fold_to_steal), (1, 1));
    assert!(analyze(h).iter().all(|(n, _)| n != "Away"));
}

#[test]
fn stat_values() {
    let c = Counters { hands: 4, vpip_opp: 4, vpip: 1, pfr: 1, postflop_aggr: 3, ..Default::default() };
    assert_eq!(StatId::Vpip.value(&c), Some(25.0));
    assert_eq!(StatId::ThreeBet.value(&c), None);
    assert_eq!(StatId::Af.value(&c), Some(3.0));
}

#[test]
fn database_aggregates_and_dedupes() {
    let mut db = Db::open_in_memory().unwrap();
    let parsed: Vec<(Hand, String)> =
        split_hands(CASH).into_iter().map(|t| (parse_hand(t).unwrap(), t.to_string())).collect();
    assert_eq!(db.insert_hands(&parsed).unwrap().len(), 2);
    assert_eq!(db.insert_hands(&parsed).unwrap().len(), 0);

    let rows = db.counters_for(&["Hero".into(), "Nobody".into()], Some(GameKind::Cash)).unwrap();
    assert_eq!(rows[0].counters.hands, 2);
    assert_eq!(rows[0].counters.vpip, 2);
    assert_eq!(rows[0].net, 107 - 1000);
    assert_eq!(rows[1].counters.hands, 0);
    assert_eq!(db.counters_for(&["Hero".into()], Some(GameKind::Tournament)).unwrap()[0].counters.hands, 0);

    let latest = db.latest_hand_for_table("c:Aquila II").unwrap().unwrap();
    assert_eq!(latest.id, 250000000002);
    assert_eq!(db.players(None, "eve: the", 10).unwrap().len(), 1);
    assert_eq!(db.players(None, "%", 10).unwrap().len(), 0);
    assert_eq!(db.hero_results(None).unwrap().len(), 2);
}

#[test]
fn import_reads_only_complete_appended_hands() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("HH20240115 Aquila II.txt");
    let [first, second]: [&str; 2] = split_hands(CASH).try_into().unwrap();

    // The client is in the middle of writing the second hand.
    let cut = second.find("*** FLOP ***").unwrap();
    fs::write(&path, format!("\u{feff}{first}\n\n\n{}", &second[..cut])).unwrap();

    let mut db = Db::open_in_memory().unwrap();
    let r = import_file(&mut db, &path).unwrap();
    assert_eq!(r.new_hands.len(), 1);
    assert_eq!(r.malformed, 0);

    let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
    write!(f, "{}\n\n\n", &second[cut..]).unwrap();
    let r = import_file(&mut db, &path).unwrap();
    assert_eq!(r.new_hands.iter().map(|h| h.id).collect::<Vec<_>>(), [250000000002]);
    assert_eq!(import_file(&mut db, &path).unwrap().new_hands.len(), 0);
    assert_eq!(db.hand_count().unwrap(), 2);
}

#[test]
fn header_variants() {
    let body = "Table 'Pool' 6-max Seat #1 is the button\nSeat 1: A (€2 in chips)\nSeat 2: B (€2 in chips)\n";
    let zoom = format!(
        "PokerStars Zoom Hand #1:  Hold'em No Limit (€0.02/€0.05) - 2024/01/15 20:30:00 CET\n{body}"
    );
    let h = parse_hand(&zoom).unwrap();
    assert_eq!((h.small_blind, h.big_blind, h.currency.as_deref()), (2, 5, Some("EUR")));

    let plo = format!("PokerStars Hand #2:  Omaha Pot Limit ($0.25/$0.50 USD) - 2024/01/15 20:30:00 ET\n{body}");
    assert_eq!(parse_hand(&plo).unwrap().variant, "Omaha Pot Limit");

    let stud = format!("PokerStars Hand #3:  7 Card Stud Limit ($0.04/$0.08 USD) - 2024/01/15 20:30:00 ET\n{body}");
    assert!(matches!(parse_hand(&stud), Err(ringside_core::parser::ParseError::Unsupported(_))));
}

#[test]
fn preferred_seating_puts_hero_at_bottom() {
    use ringside_core::table::visual_index;
    let h = &hands(CASH)[0];
    assert_eq!(visual_index(h, 5, true), 0);
    assert_eq!(visual_index(h, 6, true), 1);
    assert_eq!(visual_index(h, 4, true), 5);
    assert_eq!(visual_index(h, 1, false), 0);
}

/// An app restart (or update) reopens the same database file: nothing is lost or re-read.
#[test]
fn reopened_database_keeps_hands_and_file_offsets() {
    let dir = tempfile::tempdir().unwrap();
    let hh = dir.path().join("HH.txt");
    fs::write(&hh, format!("{CASH}\n\n\n")).unwrap();
    let db_path = dir.path().join("ringside.sqlite");

    {
        let mut db = Db::open(&db_path).unwrap();
        assert_eq!(import_file(&mut db, &hh).unwrap().new_hands.len(), 2);
    }
    let mut db = Db::open(&db_path).unwrap();
    assert_eq!(db.hand_count().unwrap(), 2);
    assert_eq!(import_file(&mut db, &hh).unwrap().new_hands.len(), 0);
}
