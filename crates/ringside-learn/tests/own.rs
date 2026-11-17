use ringside_core::model::{ActionKind, Hand};
use ringside_core::parse_hand;
use ringside_engine::{CardSet, Method, Range, equity};
use ringside_learn::own::check_spot;
use ringside_learn::{Answer, Kind, OwnSpot, Prompt, detect_spots};

/// A 9-handed tournament hand at 50/100 with a 10 ante; seats 1 and 2 are SB and BB.
fn tournament(sb: (&str, i64), bb: (&str, i64), body: &str) -> Hand {
    let mut text = String::from(
        "PokerStars Hand #300000000001: Tournament #3500000009, $10+$1 USD Hold'em No Limit - Level IV (50/100) - 2026/11/10 21:00:00 CET [2026/11/10 15:00:00 ET]\n\
         Table '3500000009 4' 9-max Seat #9 is the button\n",
    );
    text.push_str(&format!("Seat 1: {} ({} in chips)\nSeat 2: {} ({} in chips)\n", sb.0, sb.1, bb.0, bb.1));
    for s in 3..=9 {
        text.push_str(&format!("Seat {s}: P{s} (5000 in chips)\n"));
    }
    for name in [sb.0, bb.0].into_iter().chain(["P3", "P4", "P5", "P6", "P7", "P8", "P9"]) {
        text.push_str(&format!("{name}: posts the ante 10\n"));
    }
    text.push_str(&format!("{}: posts small blind 50\n{}: posts big blind 100\n*** HOLE CARDS ***\n", sb.0, bb.0));
    text.push_str("Dealt to Hero [Ah 5h]\n");
    for s in 3..=9 {
        text.push_str(&format!("P{s}: folds\n"));
    }
    text.push_str(body);
    parse_hand(&text).unwrap()
}

fn spot(hand: &Hand, kind: Kind) -> Option<OwnSpot> {
    detect_spots(hand, Some(kind)).into_iter().next()
}

fn hero_action(hand: &Hand, spot: &OwnSpot) -> ActionKind {
    let a = &hand.actions[spot.stop_before_action];
    assert_eq!(a.player, "Hero");
    a.kind
}

#[test]
fn small_blind_shove_with_dead_antes() {
    let hand = tournament(
        ("Hero", 1000),
        ("Villain", 3000),
        "Hero: raises 890 to 990 and is all-in\nVillain: folds\nUncalled bet (890) returned to Hero\nHero collected 270 from pot\n*** SUMMARY ***\nTotal pot 270 | Rake 0\n",
    );
    let s = spot(&hand, Kind::PushFoldSb).expect("spot");
    assert_eq!(hero_action(&hand, &s), ActionKind::Raise);
    assert_eq!(s.played, Some(Answer::Choice("shove".into())));
    assert_eq!(s.id, format!("push_fold_sb:{}", hand.id));
    let Prompt::PushFold { stack_bb, ante_bb, dead_bb, small_blind_bb, players, ref class, .. } = s.exercise.prompt
    else {
        panic!()
    };
    assert_eq!((stack_bb, small_blind_bb, players), (10.0, 0.5, 9));
    assert!((ante_bb - 0.1).abs() < 1e-12 && (dead_bb - 0.7).abs() < 1e-12, "{ante_bb} {dead_bb}");
    assert_eq!(class, "A5s");
    // A5s is a clear shove at 10 bb with antes.
    assert_eq!(s.played_ev_loss_bb, Some(0.0));
    let (_, fb) = check_spot(&hand, &s.id, &Answer::Choice("fold".into())).unwrap();
    assert!(!fb.correct && fb.ev_loss_bb.unwrap() > 0.5);
    assert!(spot(&hand, Kind::AllIn).is_none());
}

#[test]
fn big_blind_call_against_a_shove_is_also_an_all_in() {
    let hand = tournament(
        ("Villain", 2000),
        ("Hero", 900),
        "Villain: raises 1890 to 1990 and is all-in\nHero: calls 790 and is all-in\nUncalled bet (1100) returned to Villain\n\
         *** FLOP *** [Kd 8c 2s]\n*** TURN *** [Kd 8c 2s] [7d]\n*** RIVER *** [Kd 8c 2s 7d] [4c]\n*** SHOW DOWN ***\n\
         Villain: shows [Kh Qc] (a pair of Kings)\nHero: shows [Ah 5h] (a straight, Four to Eight)\nHero collected 1870 from pot\n\
         *** SUMMARY ***\nTotal pot 1870 | Rake 0\nBoard [Kd 8c 2s 7d 4c]\n",
    );
    let pf = spot(&hand, Kind::PushFoldBb).expect("push/fold spot");
    assert_eq!(hero_action(&hand, &pf), ActionKind::Call);
    assert_eq!(pf.played, Some(Answer::Choice("call".into())));
    let Prompt::PushFold { stack_bb, .. } = pf.exercise.prompt else { panic!() };
    assert_eq!(stack_bb, 9.0);

    let s = spot(&hand, Kind::AllIn).expect("all-in spot");
    assert_eq!(s.stop_before_action, pf.stop_before_action);
    let r = s.all_in.clone().unwrap();
    let hero_vs = [Range::parse("Ah5h").unwrap(), Range::parse("KhQc").unwrap()];
    let eq = equity(&hero_vs, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).unwrap().players[0].equity_pct;
    assert!((r.equity_pct - eq).abs() < 1e-9);
    assert!((r.pot_bb - 18.7).abs() < 1e-9);
    assert!((r.ev_bb - (eq / 100.0 * 18.7 - 9.0)).abs() < 1e-9);
    assert!((r.net_bb - 9.7).abs() < 1e-9);
    assert!((r.luck_bb - (r.net_bb - r.ev_bb)).abs() < 1e-12);
    let (_, fb) = check_spot(&hand, &s.id, &Answer::Number(eq)).unwrap();
    assert!(fb.correct && fb.facts.iter().any(|f| f.label == "Glück"));
}

#[test]
fn limps_min_raises_and_opens_are_not_push_fold_spots() {
    let limp = tournament(("Hero", 1000), ("Villain", 3000), "Hero: calls 50\nVillain: checks\n");
    assert!(spot(&limp, Kind::PushFoldSb).is_none());
    let min_raise = tournament(("Hero", 1000), ("Villain", 3000), "Hero: raises 100 to 200\nVillain: folds\n");
    assert!(spot(&min_raise, Kind::PushFoldSb).is_none());
    let fold = tournament(("Hero", 1000), ("Villain", 3000), "Hero: folds\n");
    assert!(spot(&fold, Kind::PushFoldSb).is_some(), "a fold is a decision");
    let deep = tournament(("Hero", 2500), ("Villain", 3000), "Hero: folds\n");
    assert!(spot(&deep, Kind::PushFoldSb).is_none(), "25 bb is too deep");
    let bb_vs_raise = tournament(("Villain", 3000), ("Hero", 1000), "Villain: raises 100 to 200\nHero: calls 100\n");
    assert!(spot(&bb_vs_raise, Kind::PushFoldBb).is_none());
}

#[test]
fn open_before_the_blinds_rules_out_push_fold() {
    let text = "PokerStars Hand #300000000002: Tournament #3500000009, $10+$1 USD Hold'em No Limit - Level IV (50/100) - 2026/11/10 21:00:00 CET [2026/11/10 15:00:00 ET]\n\
Table '3500000009 4' 9-max Seat #3 is the button\n\
Seat 1: Hero (1000 in chips)\nSeat 2: Villain (3000 in chips)\nSeat 3: Opener (3000 in chips)\n\
Hero: posts small blind 50\nVillain: posts big blind 100\n*** HOLE CARDS ***\nDealt to Hero [Ah 5h]\n\
Opener: raises 100 to 200\nHero: raises 800 to 1000 and is all-in\nVillain: folds\nOpener: folds\n";
    let hand = parse_hand(text).unwrap();
    assert!(spot(&hand, Kind::PushFoldSb).is_none());
}

/// Three players all-in preflop: hero's short stack only plays for the main pot.
#[test]
fn side_pots_are_exact() {
    let text = "PokerStars Hand #300000000003: Tournament #3500000009, $10+$1 USD Hold'em No Limit - Level IV (50/100) - 2026/11/10 21:00:00 CET [2026/11/10 15:00:00 ET]\n\
Table '3500000009 4' 9-max Seat #1 is the button\n\
Seat 1: Hero (500 in chips)\nSeat 2: Big (1500 in chips)\nSeat 3: Deep (3000 in chips)\nSeat 4: Folder (2000 in chips)\n\
Big: posts small blind 50\nDeep: posts big blind 100\n*** HOLE CARDS ***\nDealt to Hero [Qs Qd]\n\
Folder: folds\nHero: raises 400 to 500 and is all-in\nBig: raises 1000 to 1500 and is all-in\nDeep: calls 1400\n\
*** FLOP *** [2c 7d 9h]\n*** TURN *** [2c 7d 9h] [Jc]\n*** RIVER *** [2c 7d 9h Jc] [3s]\n*** SHOW DOWN ***\n\
Hero: shows [Qs Qd] (a pair of Queens)\nBig: shows [Ac Kc] (high card Ace)\nDeep: shows [Th Ts] (a pair of Tens)\n\
Deep collected 2000 from side pot\nHero collected 1500 from main pot\n*** SUMMARY ***\nTotal pot 3500 Main pot 1500. Side pot 2000. | Rake 0\n";
    let hand = parse_hand(text).unwrap();
    let s = spot(&hand, Kind::AllIn).expect("all-in spot");
    let Prompt::AllIn { pots, opponents, board, .. } = &s.exercise.prompt else { panic!() };
    assert!(board.is_empty());
    assert_eq!(opponents.len(), 2);
    assert_eq!(pots.len(), 2);
    assert_eq!((pots[0].amount_bb, pots[0].players.clone()), (15.0, vec![0, 1, 2]));
    assert_eq!((pots[1].amount_bb, pots[1].players.clone()), (20.0, vec![1, 2]));
    let three = ["QsQd", "AcKc", "ThTs"].map(|h| Range::parse(h).unwrap());
    let eq = equity(&three, CardSet::EMPTY, CardSet::EMPTY, Method::Exact).unwrap().players[0].equity_pct;
    let r = s.all_in.unwrap();
    assert!((r.ev_bb - (eq / 100.0 * 15.0 - 5.0)).abs() < 1e-9, "{} vs {}", r.ev_bb, eq);
    assert!((r.equity_pct - eq * 15.0 / 35.0).abs() < 1e-9);
    assert!((r.net_bb - 10.0).abs() < 1e-9);
}

#[test]
fn unknown_cards_or_a_complete_board_are_not_all_in_spots() {
    let mucked = tournament(
        ("Villain", 2000),
        ("Hero", 900),
        "Villain: raises 1890 to 1990 and is all-in\nHero: calls 790 and is all-in\nUncalled bet (1100) returned to Villain\n\
         *** FLOP *** [Kd 8c 2s]\n*** TURN *** [Kd 8c 2s] [7d]\n*** RIVER *** [Kd 8c 2s 7d] [4c]\n*** SHOW DOWN ***\n\
         Hero: shows [Ah 5h] (a straight, Four to Eight)\nHero collected 1870 from pot\n",
    );
    assert!(spot(&mucked, Kind::AllIn).is_none());
    assert!(spot(&mucked, Kind::PushFoldBb).is_some());
}

#[test]
fn spots_from_the_fixture_tournament() {
    let text = include_str!("../../ringside-core/tests/fixtures/tournament.txt");
    let hand = parse_hand(text).unwrap();
    // The button shoves, hero calls in the big blind: an all-in, but no SB-vs-BB spot.
    let kinds: Vec<Kind> = detect_spots(&hand, None).iter().map(|s| s.kind).collect();
    assert_eq!(kinds, vec![Kind::AllIn]);
}
