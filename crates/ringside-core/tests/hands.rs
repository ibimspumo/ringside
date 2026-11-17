use std::fs;
use std::io::Write;

use ringside_core::db::{Db, Filter, HandQuery};
use ringside_core::import::import_file;
use ringside_core::model::{ActionKind, GameKind, Hand};
use ringside_core::stats::{Counters, StatId, analyze};
use ringside_core::{parse_hand, split_hands};

const CASH: &str = include_str!("fixtures/cash.txt");
const TOURNAMENT: &str = include_str!("fixtures/tournament.txt");
const KNOCKOUT: &str = include_str!("fixtures/knockout.txt");

fn hands(text: &str) -> Vec<Hand> {
    split_hands(text).into_iter().map(|h| parse_hand(h).expect("parses")).collect()
}

fn counters(hand: &Hand, player: &str) -> Counters {
    analyze(hand).into_iter().find(|(n, _)| n == player).map(|(_, c)| c).expect("player dealt in")
}

/// Money invariants: what goes in comes out (minus rake), for every fixture hand.
#[test]
fn chips_are_conserved() {
    for hand in hands(CASH).iter().chain(hands(TOURNAMENT).iter()).chain(hands(KNOCKOUT).iter()) {
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
    assert_eq!(db.players(&Filter::default(), "eve: the", 10).unwrap().len(), 1);
    assert_eq!(db.players(&Filter::default(), "%", 10).unwrap().len(), 0);
    assert_eq!(db.hero_results(&Filter::default(), 100).unwrap().hands, 2);
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
fn single_digit_hours_are_padded() {
    let text = "PokerStars Hand #1:  Hold'em No Limit ($0.01/$0.02 USD) - 2026/11/16 11:00:45 CET [2026/11/16 5:00:45 ET]\n\
                Table 'T' 6-max Seat #1 is the button\nSeat 1: A ($2 in chips)\nSeat 2: B ($2 in chips)\n";
    let early = text.replace("11:00:45 CET", "5:00:45 CET");
    assert_eq!(parse_hand(&early).unwrap().played_at, "2026/11/16 05:00:45");
    assert_eq!(parse_hand(text).unwrap().played_at, "2026/11/16 11:00:45");
}

/// Courchevel shows one board card before preflop betting; "raises to" still counts from the
/// blinds already posted.
#[test]
fn courchevel_preflop_card_keeps_preflop_bets() {
    let text = "PokerStars Hand #2: Tournament #9, $1.44+$1.50+$0.36 USD Courchevel Hi/Lo Pot Limit - Level VII (60/120) - 2026/11/16 5:00:45 ET
Table '9 7' 6-max Seat #1 is the button
Seat 1: A (3000 in chips)
Seat 2: B (3000 in chips)
Seat 3: C (3000 in chips)
A: posts the ante 15
B: posts the ante 15
C: posts the ante 15
B: posts small blind 60
C: posts big blind 120
*** HOLE CARDS ***
Dealt to C [Kh Td Qs Ad 8d]
*** PRE-FLOP *** [4d]
A: calls 120
B: folds
C: raises 264 to 384
A: calls 264
*** FLOP *** [4d] [Ah Kd]
C: bets 900
A: folds
Uncalled bet (900) returned to C
C collected 873 from pot
*** SUMMARY ***
Total pot 873 | Rake 0
Board [4d Ah Kd]
";
    let h = parse_hand(text).unwrap();
    assert_eq!(h.board, ["4d", "Ah", "Kd"]);
    assert_eq!((h.invested("A"), h.invested("C")), (39_900, 39_900));
    let c = counters(&h, "C");
    assert_eq!((c.vpip, c.pfr), (1, 1), "the raise is a preflop action");
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

#[test]
fn knockout_bounties_on_seats_and_eliminations() {
    let [pko, split]: [Hand; 2] = hands(KNOCKOUT).try_into().unwrap();
    assert_eq!(pko.seats[0].bounty, Some(250));
    assert_eq!(pko.seats[2].bounty, Some(190));
    assert_eq!(pko.bounty_currency.as_deref(), Some("USD"));
    assert_eq!(pko.bounties_won, [("Hero".to_string(), 95)]);
    assert_eq!(pko.net("Hero"), 2412 * 100 - 1200 * 100);

    assert_eq!(split.seats[0].name, "Lee Ann: KO");
    assert_eq!(split.bounties_won, [("Lee Ann: KO".to_string(), 500), ("Mo".to_string(), 500)]);

    let cash = &hands(CASH)[0];
    assert!(cash.seats.iter().all(|s| s.bounty.is_none()) && cash.bounty_currency.is_none());
}

#[test]
fn positions_and_hand_classes() {
    use ringside_core::stats::{hand_class, position_labels};
    let h = &hands(CASH)[0]; // button seat 3 (Carol), six players
    let labels: Vec<(String, &str)> = position_labels(h);
    let of = |n: &str| labels.iter().find(|(p, _)| p == n).unwrap().1;
    assert_eq!(
        [of("Carol"), of("Dave"), of("Hero"), of("Eve: The Great"), of("Alice"), of("Bob")],
        ["BTN", "SB", "BB", "LJ", "HJ", "CO"]
    );
    let heads_up = &hands(KNOCKOUT)[0];
    assert_eq!(position_labels(heads_up).len(), 3);

    let c = |a: &str, b: &str| hand_class(&[a.to_string(), b.to_string()]);
    assert_eq!(c("Kc", "Ah").as_deref(), Some("AKo"));
    assert_eq!(c("9s", "Ts").as_deref(), Some("T9s"));
    assert_eq!(c("Qd", "Qh").as_deref(), Some("QQ"));
    assert_eq!(hand_class(&["Ah".into(), "Kh".into(), "Qh".into(), "Jh".into()]), None);
}

fn db_with(texts: &[&str]) -> Db {
    let mut db = Db::open_in_memory().unwrap();
    let parsed: Vec<(Hand, String)> = texts
        .iter()
        .flat_map(|t| split_hands(t))
        .map(|t| (parse_hand(t).unwrap(), t.to_string()))
        .collect();
    db.insert_hands(&parsed).unwrap();
    db
}

#[test]
fn reports_by_position_hand_and_session() {
    let db = db_with(&[CASH, KNOCKOUT]);
    let cash = Filter { game: Some(GameKind::Cash), ..Default::default() };

    let by_pos = db.hero_groups(&cash, false).unwrap();
    let pos = |k: &str| by_pos.iter().find(|r| r.key == k).unwrap();
    assert_eq!((pos("BB").counters.hands, pos("BB").net), (1, 107));
    assert_eq!((pos("SB").counters.hands, pos("SB").net), (1, -1000));

    let by_hand = db.hero_groups(&cash, true).unwrap();
    assert!(by_hand.iter().any(|r| r.key == "AKo" && (r.net_bb - 10.7).abs() < 1e-9));
    assert!(by_hand.iter().any(|r| r.key == "99"));

    let results = db.hero_results(&cash, 100).unwrap();
    assert_eq!((results.hands, results.net), (2, 107 - 1000));
    assert!((results.non_showdown_bb - 10.7).abs() < 1e-9 && (results.showdown_bb + 100.0).abs() < 1e-9);
    assert_eq!(results.points.len(), 2);
    assert_eq!(db.hero_results(&cash, 1).unwrap().points.len(), 1, "thinned, last point kept");

    let ko = db.hero_results(&Filter { game: Some(GameKind::Tournament), ..Default::default() }, 100).unwrap();
    assert_eq!(ko.bounties, std::collections::BTreeMap::from([("USD".to_string(), 95)]));

    let sessions = db.sessions(&Filter::default(), 30.0).unwrap();
    assert_eq!(sessions.len(), 2, "2024 cash and 2026 knockout are separate sessions");
    assert_eq!((sessions[1].hands, sessions[1].net), (2, 107 - 1000));

    let since = Filter { since: Some("2026/01/01".into()), ..Default::default() };
    assert!(db.hero_results(&since, 100).unwrap().points.iter().all(|r| r.played_at.starts_with("2026")));
    assert_eq!(db.players(&since, "", 10).unwrap().len(), 6, "date filter reads per-hand rows");
    assert_eq!(db.players(&Filter::default(), "", 100).unwrap().len(), 11, "totals cover all players (Hero plays both)");
}

#[test]
fn hand_filters_and_shown_cards() {
    let db = db_with(&[CASH]);
    let q = |showdown_only, result: Option<&str>, min_pot_bb| HandQuery {
        showdown_only,
        result: result.map(String::from),
        min_pot_bb,
        limit: 50,
        ..Default::default()
    };
    assert_eq!(db.hands(&q(false, None, None)).unwrap().len(), 2);
    assert_eq!(db.hands(&q(true, None, None)).unwrap()[0].id, 250000000002);
    assert_eq!(db.hands(&q(false, Some("won"), None)).unwrap()[0].id, 250000000001);
    assert_eq!(db.hands(&q(false, None, Some(100.0))).unwrap().len(), 1);

    let shown = db.shown_hands("Alice", 10).unwrap();
    assert_eq!((shown[0].cards.as_str(), shown[0].board.as_str()), ("Ah 5c", "9c 4h 2s Kd 3c"));
}

/// A database written by v0.1.x gains the new columns and gets them filled from raw hands.
#[test]
fn old_database_is_upgraded_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.sqlite");
    {
        let conn = rusqlite_conn(&path);
        conn.execute_batch(
            "CREATE TABLE hands (id INTEGER PRIMARY KEY, game TEXT NOT NULL, tournament_id INTEGER,
                table_key TEXT NOT NULL, table_name TEXT NOT NULL, played_at TEXT NOT NULL,
                small_blind INTEGER NOT NULL, big_blind INTEGER NOT NULL, currency TEXT, hero TEXT,
                hero_cards TEXT NOT NULL DEFAULT '', hero_net INTEGER, raw TEXT NOT NULL);",
        )
        .unwrap();
        for raw in split_hands(KNOCKOUT) {
            let h = parse_hand(raw).unwrap();
            conn.execute(
                "INSERT INTO hands (id, game, table_key, table_name, played_at, small_blind, big_blind, raw)
                 VALUES (?1, 'tournament', ?2, ?3, ?4, 0, ?5, ?6)",
                (h.id as i64, h.table_key(), &h.table_name, &h.played_at, h.big_blind, raw),
            )
            .unwrap();
        }
    }
    let mut db = Db::open(&path).unwrap();
    assert!(db.needs_rebuild().unwrap());
    let first = db.rebuild_step(1).unwrap();
    assert_eq!((first.done, first.total), (1, 2));
    drop(db);
    // Closing in the middle of the upgrade and reopening resumes where it stopped.
    let mut db = Db::open(&path).unwrap();
    db.rebuild_all().unwrap();
    assert!(!db.needs_rebuild().unwrap());
    assert_eq!(db.hand_count().unwrap(), 2);
    assert_eq!(db.counters_for(&["Mo".into()], None).unwrap()[0].counters.hands, 1);
    assert_eq!(db.shown_hands("Short", 5).unwrap()[0].cards, "Ac Kc");
    assert_eq!(db.tournaments(&Filter::default()).unwrap().len(), 2, "tournaments filled from old hands");
}

fn rusqlite_conn(path: &std::path::Path) -> rusqlite::Connection {
    rusqlite::Connection::open(path).unwrap()
}

#[test]
fn dead_button_positions_follow_the_blinds() {
    use ringside_core::stats::position_labels;
    let text = "PokerStars Hand #9: Tournament #1, $1+$0.10 USD Hold'em No Limit - Level I (10/20) - 2026/01/01 10:00:00 CET
Table '1 1' 9-max Seat #8 is the button
Seat 1: A (1500 in chips)
Seat 2: B (1500 in chips)
Seat 3: C (1500 in chips)
Seat 5: D (1500 in chips)
A: posts small blind 10
B: posts big blind 20
*** HOLE CARDS ***
C: folds
D: folds
A: folds
*** SUMMARY ***
Total pot 30 | Rake 0
";
    let labels = position_labels(&parse_hand(text).unwrap());
    let of = |n: &str| labels.iter().find(|(p, _)| p == n).unwrap().1;
    assert_eq!([of("A"), of("B"), of("C"), of("D")], ["SB", "BB", "CO", "BTN"]);
}

#[test]
fn split_bounty_with_commas_in_names() {
    let text = split_hands(KNOCKOUT)[1].replace("Lee Ann: KO", "Lee, Ann");
    let h = parse_hand(&text).unwrap();
    assert_eq!(h.bounties_won, [("Lee, Ann".to_string(), 500), ("Mo".to_string(), 500)]);
}

/// Overlays keep showing real numbers while an upgrade is still rebuilding the totals.
#[test]
fn counters_during_an_upgrade_come_from_hand_rows() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db.sqlite");
    {
        let mut db = Db::open(&path).unwrap();
        let parsed: Vec<(Hand, String)> =
            split_hands(CASH).into_iter().map(|t| (parse_hand(t).unwrap(), t.to_string())).collect();
        db.insert_hands(&parsed).unwrap();
    }
    {
        let conn = rusqlite_conn(&path);
        conn.execute_batch("PRAGMA user_version = 1; DELETE FROM player_totals;").unwrap();
    }
    let mut db = Db::open(&path).unwrap();
    assert!(db.needs_rebuild().unwrap());
    assert_eq!(db.counters_for(&["Hero".into()], None).unwrap()[0].counters.hands, 2);
    db.rebuild_all().unwrap();
    assert_eq!(db.counters_for(&["Hero".into()], None).unwrap()[0].counters.hands, 2);
}

/// The hero's HUD can show only the current tournament, or the current cash table today.
#[test]
fn counters_limited_to_the_session() {
    let db = db_with(&[CASH, KNOCKOUT]);
    let ko = &hands(KNOCKOUT)[0];
    let cash = &hands(CASH)[1];
    assert_eq!(db.counters_for(&["Hero".into()], None).unwrap()[0].counters.hands, 3);
    assert_eq!(db.counters_in_session("Hero", ko).unwrap().hands, 1);
    assert_eq!(db.counters_in_session("Hero", cash).unwrap().hands, 2);
    assert_eq!(db.counters_in_session("Mo", ko).unwrap().hands, 0, "other tournament");
}
