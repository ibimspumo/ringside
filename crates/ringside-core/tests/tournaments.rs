use std::fs;

use ringside_core::db::{Db, Filter, HandQuery};
use ringside_core::import::{import_summary, summary_dirs};
use ringside_core::model::Hand;
use ringside_core::tournament::{BuyIn, Summary, TournamentRow, parse_summaries, totals};
use ringside_core::{parse_hand, split_hands};

const REGULAR: &str = include_str!("fixtures/summaries/regular.txt");
const PKO: &str = include_str!("fixtures/summaries/pko.txt");
const FREEROLL: &str = include_str!("fixtures/summaries/freeroll.txt");
const BUSTED: &str = include_str!("fixtures/summaries/busted.txt");
const WON: &str = include_str!("fixtures/summaries/won.txt");
const KNOCKOUT: &str = include_str!("fixtures/knockout.txt");
const CASH: &str = include_str!("fixtures/cash.txt");
const TOURNAMENT: &str = include_str!("fixtures/tournament.txt");

fn one(text: &str) -> Summary {
    let mut all = parse_summaries(text);
    assert_eq!(all.len(), 1);
    all.remove(0)
}

fn buy_in(prize: i64, bounty: i64, fee: i64, currency: Option<&str>) -> Option<BuyIn> {
    Some(BuyIn { prize, bounty, fee, currency: currency.map(String::from) })
}

#[test]
fn regular_summary_in_the_money() {
    let s = one(REGULAR);
    assert_eq!(s.id, 4100000001);
    assert_eq!(s.game, "NL Hold'em");
    assert_eq!(s.buy_in, buy_in(500, 0, 50, Some("USD")));
    assert_eq!((s.entrants, s.prize_pool), (Some(18), Some(9000)));
    assert_eq!(s.started_at.as_deref(), Some("2026/11/14 20:00:00"));
    assert_eq!(s.finished_at.as_deref(), Some("2026/11/14 21:12:40"));
    assert_eq!((s.place, s.prize, s.bounties, s.extra_cost), (Some(3), 1530, None, 0));
    assert_eq!(s.currency.as_deref(), Some("USD"));
}

#[test]
fn knockout_summary_with_bounties_and_reentry() {
    let s = one(PKO);
    assert_eq!(s.buy_in, buy_in(1000, 1000, 200, Some("USD")));
    assert_eq!((s.place, s.prize), (Some(37), 0));
    assert_eq!(s.bounties, Some(1750));
    assert_eq!(s.extra_cost, 2200);
}

#[test]
fn freeroll_takes_its_currency_from_the_prize() {
    let s = one(FREEROLL);
    assert_eq!(s.buy_in, buy_in(0, 0, 0, None));
    assert_eq!((s.place, s.prize, s.currency.as_deref()), (Some(2), 600, Some("USD")));
}

#[test]
fn busted_out_of_the_money_with_bom_crlf_and_short_hour() {
    let s = one(BUSTED);
    assert_eq!(s.id, 4100000004);
    assert_eq!(s.game, "PL Omaha");
    assert_eq!(s.buy_in, buy_in(460, 0, 40, Some("EUR")));
    assert_eq!(s.started_at.as_deref(), Some("2026/11/16 02:05:00"));
    assert_eq!(s.finished_at, None);
    assert_eq!((s.place, s.prize), (Some(40), 0));
}

#[test]
fn hero_won_and_odd_names_do_not_confuse_ranks() {
    let s = one(WON);
    assert_eq!((s.place, s.prize, s.currency.as_deref()), (Some(1), 702, Some("EUR")));
    let pko = one(PKO);
    assert_eq!(pko.place, Some(37), "\"(Ratte)\" and \"[2]\" in names are not places");
}

#[test]
fn still_playing_has_no_result_yet() {
    let text = REGULAR.replace("You finished in 3rd place.", "").replace("$15.30 (17%)", "still playing");
    let s = one(&text);
    assert_eq!((s.place, s.prize), (None, 0));
}

#[test]
fn windows_1252_summary_is_decoded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TS.txt");
    // Single bytes, the way older clients wrote the file: € is 0x80, ö is 0xF6.
    let bytes: Vec<u8> = WON.chars().map(|c| if c == '€' { 0x80 } else { c as u32 as u8 }).collect();
    fs::write(&path, bytes).unwrap();
    let mut db = Db::open_in_memory().unwrap();
    assert_eq!(import_summary(&mut db, &path).unwrap(), 1);
    let row = &db.tournaments(&Filter::default()).unwrap()[0];
    assert_eq!((row.currency.as_deref(), row.prize), (Some("EUR"), Some(702)));
}

fn insert(db: &mut Db, text: &str) {
    let parsed: Vec<(Hand, String)> =
        split_hands(text).into_iter().map(|t| (parse_hand(t).unwrap(), t.to_string())).collect();
    db.insert_hands(&parsed).unwrap();
}

fn row(db: &Db, id: u64) -> TournamentRow {
    db.tournaments(&Filter::default()).unwrap().into_iter().find(|r| r.id == id).expect("tournament listed")
}

#[test]
fn importing_a_summary_twice_counts_it_once_and_rewrites_replace_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TS20261115 T4100000002.txt");
    let running = PKO.replace("You finished in 37th place.", "").replace("Hero [2] (Germany),", "Hero [2] (Germany), still playing");
    fs::write(&path, &running).unwrap();
    let mut db = Db::open_in_memory().unwrap();
    assert_eq!(import_summary(&mut db, &path).unwrap(), 1);
    assert_eq!(import_summary(&mut db, &path).unwrap(), 0, "unchanged file is skipped");
    assert_eq!(row(&db, 4100000002).place, None);

    fs::write(&path, PKO).unwrap();
    assert_eq!(import_summary(&mut db, &path).unwrap(), 1);
    let rows = db.tournaments(&Filter::default()).unwrap();
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!((r.place, r.prize, r.bounties, r.cost), (Some(37), Some(0), 1750, Some(4400)));
    assert_eq!(r.profit, Some(1750 - 4400));
    let t = totals(&rows);
    assert_eq!((t[0].tournaments, t[0].cost, t[0].bounties, t[0].profit), (1, 4400, 1750, -2650));
}

#[test]
fn tournament_without_summary_comes_from_its_hands() {
    let mut db = Db::open_in_memory().unwrap();
    insert(&mut db, KNOCKOUT);
    let r = row(&db, 4046789300);
    assert_eq!(r.game, "NL Hold'em");
    assert_eq!(r.buy_in, buy_in(500, 0, 50, Some("USD")));
    assert_eq!((r.hands, r.chips, r.bounties), (1, 121_200, 95));
    assert_eq!(r.started_at, "2026/11/16 20:10:00");
    assert_eq!((r.place, r.prize, r.profit, r.has_summary), (None, None, None, false));
    assert_eq!(row(&db, 4046789301).buy_in, buy_in(1000, 1000, 200, Some("USD")));

    // Inserting the same hands again changes nothing.
    insert(&mut db, KNOCKOUT);
    assert_eq!(row(&db, 4046789300), r);
}

#[test]
fn elimination_line_in_a_hand_gives_place_and_prize() {
    let bust = split_hands(KNOCKOUT)[0]
        .replace("Astrid finished the tournament in 87th place", "Hero finished the tournament in 3rd place and received $5.40.");
    let h = parse_hand(&bust).unwrap();
    assert_eq!((h.finishes[0].place, h.finishes[0].prize), (3, Some(540)));
    let mut db = Db::open_in_memory().unwrap();
    insert(&mut db, &bust);
    let r = row(&db, 4046789300);
    assert_eq!((r.place, r.prize, r.cost), (Some(3), Some(540), Some(550)));
    assert_eq!(r.profit, Some(540 + 95 - 550));

    let winner = bust.replace("Hero finished the tournament in 3rd place and received $5.40.", "Hero wins the tournament and receives $27.00 - congratulations!");
    assert_eq!(parse_hand(&winner).unwrap().finishes[0].place, 1);
    let ticket = bust.replace("received $5.40.", "received a Tournament Ticket.");
    assert_eq!(parse_hand(&ticket).unwrap().finishes[0].prize, None);
}

#[test]
fn summary_overrides_hand_data_and_keeps_hand_counts() {
    let mut db = Db::open_in_memory().unwrap();
    insert(&mut db, KNOCKOUT);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TS.txt");
    fs::write(&path, REGULAR.replace("4100000001", "4046789300")).unwrap();
    import_summary(&mut db, &path).unwrap();
    let r = row(&db, 4046789300);
    assert_eq!((r.place, r.prize, r.entrants, r.hands, r.has_summary), (Some(3), Some(1530), Some(18), 1, true));
    assert_eq!(r.started_at, "2026/11/14 20:00:00");
    assert_eq!(r.bounties, 95, "no bounty line in the summary: bounties from the hands");
    assert_eq!(r.profit, Some(1530 + 95 - 550));
}

#[test]
fn per_currency_totals_from_the_database_never_mix_currencies() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Db::open_in_memory().unwrap();
    for (name, text) in [("a", REGULAR), ("b", PKO), ("c", FREEROLL), ("d", BUSTED), ("e", WON)] {
        let path = dir.path().join(format!("{name}.txt"));
        fs::write(&path, text).unwrap();
        import_summary(&mut db, &path).unwrap();
    }
    let rows = db.tournaments(&Filter::default()).unwrap();
    assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), [4100000005, 4100000004, 4100000002, 4100000001, 4100000003]);
    let t = totals(&rows);
    let usd = t.iter().find(|t| t.currency == "USD").unwrap();
    let eur = t.iter().find(|t| t.currency == "EUR").unwrap();
    assert_eq!((usd.tournaments, usd.cashes, usd.cost, usd.prizes, usd.bounties), (3, 2, 550 + 4400, 1530 + 600, 1750));
    assert_eq!(usd.profit, 1530 + 600 + 1750 - 4950);
    assert_eq!(usd.itm, Some(2.0 / 3.0 * 100.0));
    assert_eq!((eur.tournaments, eur.cost, eur.prizes, eur.profit), (2, 500 + 200, 702, 2));
    assert_eq!(eur.roi, Some(2.0 / 700.0 * 100.0));

    let since = Filter { since: Some("2026/11/16".into()), ..Default::default() };
    assert_eq!(db.tournaments(&since).unwrap().len(), 2);
}

#[test]
fn hands_can_be_listed_per_tournament() {
    let mut db = Db::open_in_memory().unwrap();
    insert(&mut db, KNOCKOUT);
    let filter = Filter { tournament_id: Some(4046789301), ..Default::default() };
    let q = HandQuery { filter, limit: 10, ..Default::default() };
    let hands = db.hands(&q).unwrap();
    assert_eq!(hands.iter().map(|h| h.id).collect::<Vec<_>>(), [260000000002]);
}

#[test]
fn tournament_filter_restricts_every_report() {
    let mut db = Db::open_in_memory().unwrap();
    for text in [CASH, TOURNAMENT, KNOCKOUT] {
        insert(&mut db, text);
    }
    let all = Filter::default();
    let ko = Filter { tournament_id: Some(4046789300), ..Default::default() };
    let names = |f: &Filter| {
        let mut n: Vec<String> = db.players(f, "", 100).unwrap().into_iter().map(|p| p.name).collect();
        n.sort();
        n
    };

    let results = db.hero_results(&ko, 100).unwrap();
    assert_eq!((results.hands, results.net), (1, (2412 - 12 - 1188) * 100), "chips in hundredths");
    assert_eq!(results.bounties.get("USD"), Some(&95));
    assert_eq!(names(&ko), ["Astrid", "Hero", "Jorge"]);
    assert_eq!(names(&Filter { tournament_id: Some(4046789301), ..Default::default() }), ["Lee Ann: KO", "Mo", "Short"]);
    let hero = db.players(&ko, "Hero", 1).unwrap().remove(0);
    assert_eq!((hero.counters.hands, hero.net), (1, results.net));
    assert_eq!(db.sessions(&ko, 30.0).unwrap().iter().map(|s| s.hands).sum::<i64>(), 1);
    let by_pos = db.hero_groups(&ko, false).unwrap();
    assert_eq!(by_pos.iter().map(|g| (g.key.as_str(), g.counters.hands)).collect::<Vec<_>>(), [("BB", 1)]);
    assert_eq!(db.hero_groups(&ko, true).unwrap().iter().map(|g| g.key.as_str()).collect::<Vec<_>>(), ["AA"]);
    let ids = |f: Filter| db.hands(&HandQuery { filter: f, limit: 10, ..Default::default() }).unwrap().len();
    assert_eq!(ids(ko.clone()), 1);
    assert_eq!(ids(Filter { since: Some("2026/11/17".into()), ..ko.clone() }), 0, "filters combine");
    assert_eq!(db.players(&Filter { since: Some("2026/11/17".into()), ..ko.clone() }, "", 10).unwrap().len(), 0);

    // Without a tournament nothing changes: every hero hand and every player counts.
    assert_eq!(db.hero_results(&all, 100).unwrap().hands, 4);
    assert_eq!(ids(all.clone()), 5);
    assert_eq!(db.sessions(&all, 30.0).unwrap().iter().map(|s| s.hands).sum::<i64>(), 4);
    let everyone = names(&all);
    assert!(everyone.len() > 6 && ["Astrid", "Mo", "Hero"].iter().all(|n| everyone.iter().any(|e| e == n)));
    assert_eq!(everyone, names(&Filter { since: Some("0000".into()), ..all }), "running totals equal the per-hand rows");
}

#[test]
fn tournament_names_are_kept_and_never_blanked() {
    let mut db = Db::open_in_memory().unwrap();
    assert!(db.set_tournament_name(4046789300, "Mini Early Battle").unwrap());
    assert!(db.tournaments(&Filter::default()).unwrap().is_empty(), "no row before a hand or summary");
    insert(&mut db, KNOCKOUT);
    assert_eq!(row(&db, 4046789300).name.as_deref(), Some("Mini Early Battle"));
    assert_eq!(row(&db, 4046789301).name, None);
    assert!(!db.set_tournament_name(4046789300, "Mini Early Battle").unwrap(), "unchanged");
    assert!(!db.set_tournament_name(4046789300, "  ").unwrap());
    assert!(db.set_tournament_name(4046789300, "Mini Early Battle [Progressive KO]").unwrap());
    assert_eq!(row(&db, 4046789300).name.as_deref(), Some("Mini Early Battle [Progressive KO]"));
    db.rebuild_all().unwrap();
    assert_eq!(row(&db, 4046789300).game, "NL Hold'em", "hands still fill the rest of the row");
}

#[test]
fn summary_folders_sit_next_to_hand_history_folders() {
    let dir = tempfile::tempdir().unwrap();
    let stars = dir.path().join("PokerStars.EU");
    fs::create_dir_all(stars.join("HandHistory/Hero")).unwrap();
    fs::create_dir_all(stars.join("TournSummary/Hero")).unwrap();
    let other = dir.path().join("Elsewhere");
    fs::create_dir_all(&other).unwrap();
    let found = summary_dirs(&[stars.join("HandHistory"), stars.join("HandHistory/Hero"), other]);
    assert_eq!(found, [stars.join("TournSummary")]);
}
