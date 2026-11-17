//! All-in EV from a hand history, checked against a calculation by hand, and the EV
//! line of the results graph.

use ringside_core::allin::hero_all_in;
use ringside_core::db::{Db, Filter};
use ringside_core::parse_hand;

/// Three-way all-in on the turn. Board As Ks 7d 2c; Hero Ah Ad (set), Bob Kh Kd (set),
/// Alice Qc Jc (gutshot), all-in for $5; Bob all-in for $15, Hero covers.
const SIDE_POT: &str = "PokerStars Hand #250000000101:  Hold'em No Limit ($0.05/$0.10 USD) - 2024/01/16 20:30:00 CET [2024/01/16 14:30:00 ET]
Table 'Aquila II' 6-max Seat #3 is the button
Seat 1: Alice ($5 in chips)
Seat 2: Bob ($15 in chips)
Seat 3: Hero ($20 in chips)
Alice: posts small blind $0.05
Bob: posts big blind $0.10
*** HOLE CARDS ***
Dealt to Hero [Ah Ad]
Hero: raises $0.20 to $0.30
Alice: calls $0.25
Bob: calls $0.20
*** FLOP *** [As Ks 7d]
Alice: bets $1
Bob: calls $1
Hero: calls $1
*** TURN *** [As Ks 7d] [2c]
Alice: bets $3.70 and is all-in
Bob: raises $10 to $13.70 and is all-in
Hero: calls $13.70
*** RIVER *** [As Ks 7d 2c] [7h]
*** SHOW DOWN ***
Alice: shows [Qc Jc] (a pair of Sevens)
Bob: shows [Kh Kd] (a full house, Kings full of Sevens)
Hero: shows [Ah Ad] (a full house, Aces full of Sevens)
Hero collected $19.40 from side pot
Hero collected $14.60 from main pot
*** SUMMARY ***
Total pot $35 Main pot $14.60. Side pot $19.40. | Rake $1
Board [As Ks 7d 2c 7h]
Seat 1: Alice (small blind) showed [Qc Jc] and lost with a pair of Sevens
Seat 2: Bob (big blind) showed [Kh Kd] and lost with a full house, Kings full of Sevens
Seat 3: Hero (button) showed [Ah Ad] and won ($34) with a full house, Aces full of Sevens";

/// By hand: 42 river cards. Kc gives Bob quads (both pots); the four tens give Alice
/// broadway (main pot), Hero keeps the side pot; the other 37 win Hero both pots.
/// Main pot 3 × $5 = $15, side pot 2 × $10 = $20, rake $1 taken pro rata.
#[test]
fn three_way_side_pot_with_rake_matches_hand_calculation() {
    let hand = parse_hand(SIDE_POT).unwrap();
    let a = hero_all_in(&hand).expect("all-in with every hand shown");
    let share = (15.0 * 37.0 / 42.0 + 20.0 * 41.0 / 42.0) / 35.0;
    assert!((a.share_pct - 100.0 * share).abs() < 1e-9, "{}", a.share_pct);
    let ev = share * 3400.0 - 1500.0;
    assert!((a.ev - ev).abs() < 1e-6, "{} vs {ev}", a.ev);
    assert_eq!((a.pot, a.rake, a.invested, a.net), (3500, 100, 1500, 1900));
    assert_eq!(a.pots.len(), 2);
    assert_eq!(a.board.len(), 4);

    let mut db = Db::open_in_memory().unwrap();
    db.insert_hands(&[(hand, SIDE_POT.to_string())]).unwrap();
    let r = db.hero_results(&Filter::default(), 10).unwrap();
    assert_eq!(r.all_ins, 1);
    // EV is stored rounded to whole cents: 1680.27 → 1680.
    assert!((r.ev_bb - 1680.0 / 10.0).abs() < 1e-9, "{}", r.ev_bb);
    assert!((r.luck_bb - (1900.0 - 1680.0) / 10.0).abs() < 1e-9, "{}", r.luck_bb);
    assert!((r.points.last().unwrap().ev_bb - r.ev_bb).abs() < 1e-9);
}

/// A river all-in has nothing left to deal, and a mucked hand leaves the EV unknown.
#[test]
fn river_all_ins_and_unknown_hands_are_left_out() {
    let mucked = SIDE_POT.replace("Alice: shows [Qc Jc] (a pair of Sevens)\n", "");
    let mucked = mucked.replace("showed [Qc Jc] and lost with a pair of Sevens", "mucked");
    assert!(hero_all_in(&parse_hand(&mucked).unwrap()).is_none());
    let cash = include_str!("fixtures/cash.txt");
    for raw in ringside_core::split_hands(cash) {
        assert!(hero_all_in(&parse_hand(raw).unwrap()).is_none());
    }
}
