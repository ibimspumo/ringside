use ringside_core::model::{ActionKind, Hand};
use ringside_core::parse_hand;
use ringside_engine::{CardSet, HandClass, Method, Range, equity};
use ringside_learn::own::check_spot;
use ringside_learn::{Answer, Kind, OwnSpot, Prompt, TableSpot, detect_spots, lookup_or_solve};

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

/// A tournament hand at 50/100 with a 10 ante. Seat 1 is the small blind, seat 2 the
/// big blind and the last seat the button, so the action starts at seat 3. `body` holds
/// every action after the hole cards.
fn table(players: &[(&str, i64)], body: &str) -> Hand {
    let mut text = format!(
        "PokerStars Hand #300000000010: Tournament #3500000009, $10+$1 USD Hold'em No Limit - Level IV (50/100) - 2026/11/10 21:00:00 CET [2026/11/10 15:00:00 ET]\n\
         Table '3500000009 4' 9-max Seat #{} is the button\n",
        players.len()
    );
    for (i, (name, chips)) in players.iter().enumerate() {
        text.push_str(&format!("Seat {}: {name} ({chips} in chips)\n", i + 1));
    }
    for (name, _) in players {
        text.push_str(&format!("{name}: posts the ante 10\n"));
    }
    text.push_str(&format!(
        "{}: posts small blind 50\n{}: posts big blind 100\n*** HOLE CARDS ***\nDealt to Hero [Ah 5h]\n{body}",
        players[0].0, players[1].0
    ));
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

fn table_spot(s: &OwnSpot) -> TableSpot {
    let Prompt::PushFoldTable {
        ref stacks_bb, small_blind_bb, ante_bb, bb_ante_bb, dead_bb, hero, shover, ref callers, ..
    } = s.exercise.prompt
    else {
        panic!("{:?}", s.exercise.prompt)
    };
    TableSpot {
        stacks_bb: stacks_bb.clone(),
        small_blind_bb,
        ante_bb,
        bb_ante_bb,
        dead_bb,
        hero,
        shover,
        callers: callers.clone(),
    }
}

/// Both answers graded against the solver's frequency and EVs for A5s at this node.
fn grades_like_the_solver(hand: &Hand, s: &OwnSpot, act: &str) {
    let t = table_spot(s);
    let solution = lookup_or_solve(&t.params()).unwrap();
    let class = "A5s".parse::<HandClass>().unwrap();
    let truth = solution.spot(t.hero, class, t.shover, &t.callers).unwrap();
    for (answer, acted) in [(act, true), ("fold", false)] {
        let (_, fb) = check_spot(hand, &s.id, &Answer::Choice(answer.into())).unwrap();
        let best = truth.ev_act.max(truth.ev_fold);
        let chosen = if acted { truth.ev_act } else { truth.ev_fold };
        assert_eq!(fb.correct, if acted { truth.freq > 0.0 } else { truth.freq < 1.0 }, "{answer}");
        assert!((fb.ev_loss_bb.unwrap() - (best - chosen)).abs() < 1e-12, "{answer}");
        assert!(fb.facts.iter().any(|f| f.label == "Damals gespielt"));
    }
}

#[test]
fn small_blind_shove_after_folds() {
    let hand = tournament(
        ("Hero", 1000),
        ("Villain", 3000),
        "Hero: raises 890 to 990 and is all-in\nVillain: folds\nUncalled bet (890) returned to Hero\nHero collected 270 from pot\n*** SUMMARY ***\nTotal pot 270 | Rake 0\n",
    );
    let s = spot(&hand, Kind::PushFoldOpen).expect("spot");
    assert_eq!(hero_action(&hand, &s), ActionKind::Raise);
    assert_eq!(s.played, Some(Answer::Choice("shove".into())));
    assert_eq!(s.id, format!("push_fold_open:{}", hand.id));
    let Prompt::PushFoldTable { ref positions, ref class, effective_bb, .. } = s.exercise.prompt else { panic!() };
    assert_eq!(positions.len(), 9);
    assert_eq!(class, "A5s");
    let t = table_spot(&s);
    assert_eq!((t.hero, t.shover, t.small_blind_bb), (7, None, 0.5));
    assert_eq!(&positions[t.hero], "SB");
    assert_eq!(t.stacks_bb[7..], [10.0, 30.0]);
    assert_eq!(effective_bb, 10.0);
    assert!((t.ante_bb - 0.1).abs() < 1e-12 && t.bb_ante_bb == 0.0 && t.dead_bb == 0.0);
    grades_like_the_solver(&hand, &s, "shove");
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
    let pf = spot(&hand, Kind::PushFoldCall).expect("push/fold spot");
    assert_eq!(hero_action(&hand, &pf), ActionKind::Call);
    assert_eq!(pf.played, Some(Answer::Choice("call".into())));
    let t = table_spot(&pf);
    assert_eq!((t.hero, t.shover, t.effective_bb()), (8, Some(7), 9.0));
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
fn limps_min_raises_and_deep_stacks_are_not_push_fold_spots() {
    let limp = tournament(("Hero", 1000), ("Villain", 3000), "Hero: calls 50\nVillain: checks\n");
    assert!(spot(&limp, Kind::PushFoldOpen).is_none());
    let min_raise = tournament(("Hero", 1000), ("Villain", 3000), "Hero: raises 100 to 200\nVillain: folds\n");
    assert!(spot(&min_raise, Kind::PushFoldOpen).is_none());
    let fold = tournament(("Hero", 1000), ("Villain", 3000), "Hero: folds\n");
    assert!(spot(&fold, Kind::PushFoldOpen).is_some(), "a fold is a decision");
    let deep = tournament(("Hero", 2500), ("Villain", 3000), "Hero: folds\n");
    assert!(spot(&deep, Kind::PushFoldOpen).is_none(), "25 bb is too deep");
    let bb_vs_raise = tournament(("Villain", 3000), ("Hero", 1000), "Villain: raises 100 to 200\nHero: calls 100\n");
    assert!(spot(&bb_vs_raise, Kind::PushFoldCall).is_none());
}


/// Hero opens first to act: a UTG shove at seven players.
#[test]
fn utg_shove() {
    let hand = table(
        &[("Sb", 1500), ("Bb", 1800), ("Hero", 1000), ("Lj", 900), ("Hj", 3000), ("Co", 1200), ("Btn", 2500)],
        "Hero: raises 890 to 990 and is all-in\nLj: folds\nHj: folds\nCo: folds\nBtn: folds\nSb: folds\nBb: folds\n\
         Uncalled bet (890) returned to Hero\nHero collected 320 from pot\n",
    );
    let s = spot(&hand, Kind::PushFoldOpen).expect("spot");
    assert_eq!(s.played, Some(Answer::Choice("shove".into())));
    let t = table_spot(&s);
    assert_eq!((t.hero, t.shover), (0, None));
    assert_eq!(t.stacks_bb, [10.0, 9.0, 30.0, 12.0, 25.0, 15.0, 18.0]);
    let Prompt::PushFoldTable { ref positions, .. } = s.exercise.prompt else { panic!() };
    assert_eq!(positions, &["UTG", "LJ", "HJ", "CO", "BTN", "SB", "BB"]);
    assert!(s.exercise.question.contains("7 Spieler, du sitzt auf UTG"), "{}", s.exercise.question);
    grades_like_the_solver(&hand, &s, "shove");
}

/// The hijack shoves at six players, everyone folds to hero in the big blind.
#[test]
fn call_against_a_shove() {
    let hand = table(
        &[("Sb", 1500), ("Hero", 1800), ("Utg", 1000), ("Hj", 800), ("Co", 1200), ("Btn", 2500)],
        "Utg: folds\nHj: raises 690 to 790 and is all-in\nCo: folds\nBtn: folds\nSb: folds\nHero: folds\n",
    );
    let s = spot(&hand, Kind::PushFoldCall).expect("spot");
    assert_eq!(s.played, Some(Answer::Choice("fold".into())));
    let t = table_spot(&s);
    assert_eq!((t.hero, t.shover, t.callers.clone()), (5, Some(1), vec![]));
    assert_eq!(t.effective_bb(), 8.0);
    grades_like_the_solver(&hand, &s, "call");
}

/// The first player shoves and the cutoff calls; hero in the small blind decides about
/// an overcall.
/// A second caller would make it four players, more than the model allows.
#[test]
fn overcall_after_a_caller() {
    let body = |sb: &str| {
        format!("Utg: raises 590 to 690 and is all-in\nHj: folds\nCo: calls 690\nBtn: folds\nHero: {sb}\n")
    };
    let players = [("Hero", 1500), ("Bb", 1800), ("Utg", 700), ("Hj", 3000), ("Co", 1200), ("Btn", 2500)];
    let hand = table(&players, &body("calls 640"));
    let s = spot(&hand, Kind::PushFoldCall).expect("spot");
    assert_eq!(s.played, Some(Answer::Choice("call".into())));
    let t = table_spot(&s);
    assert_eq!((t.hero, t.shover, t.callers.clone()), (4, Some(0), vec![2]));
    assert!(s.exercise.question.contains("LJ geht mit 7 bb all-in, CO callt"), "{}", s.exercise.question);
    grades_like_the_solver(&hand, &s, "call");

    let crowded = table(
        &players,
        "Utg: raises 590 to 690 and is all-in\nHj: calls 690\nCo: calls 690\nBtn: folds\nHero: folds\n",
    );
    assert!(detect_spots(&crowded, None).is_empty());
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
    assert!(spot(&mucked, Kind::PushFoldCall).is_some());
}

#[test]
fn spots_from_the_fixture_tournament() {
    let text = include_str!("../../ringside-core/tests/fixtures/tournament.txt");
    let hand = parse_hand(text).unwrap();
    // The button shoves, hero calls in the big blind: an all-in and a call against a shove.
    let kinds: Vec<Kind> = detect_spots(&hand, None).iter().map(|s| s.kind).collect();
    assert_eq!(kinds, vec![Kind::AllIn, Kind::PushFoldCall]);
}
