//! The tournament report read from the database, over the anonymised knockout fixture.

use ringside_core::db::{Choice, Db, Decision, HandAnalysis, Rating, Source, SpotType, Status};
use ringside_core::model::{Hand, Street};
use ringside_core::{parse_hand, split_hands};

const KNOCKOUT: &str = include_str!("fixtures/knockout.txt");
const TOURNAMENT: u64 = 4046789300;
const ENGINE: &str = "test-engine";

fn decision(hand_id: u64, action_index: usize, status: Status, rating: Option<Rating>, loss: Option<f64>) -> Decision {
    Decision {
        hand_id,
        action_index,
        street: Street::Preflop,
        spot: SpotType::CallShove,
        status,
        played: "Call 10,9 BB".into(),
        best: Some("Call".into()),
        strategy: vec![
            Choice { label: "Fold".into(), freq: 0.0, ev_bb: Some(0.0), played: false, kind: "fold".into() },
            Choice { label: "Call".into(), freq: 1.0, ev_bb: Some(6.5), played: true, kind: "call".into() },
        ],
        played_freq: Some(1.0),
        ev_loss_bb: loss,
        rating,
        source: Some(Source::Equilibrium),
        why: String::new(),
        note: String::new(),
        reason: None,
        solver: None,
        engine: ENGINE.into(),
        pot_bb: Some(24.1),
        reliability: None,
    }
}

#[test]
fn the_report_of_a_stored_tournament() {
    let mut db = Db::open_in_memory().unwrap();
    // The fixture's first hand, and the same deal a few minutes later with the chips won.
    let first = split_hands(KNOCKOUT)[0].to_string();
    let later = first
        .replace("#260000000001", "#260000000003")
        .replace("20:10:00 CET [2026/11/16 14:10:00", "20:14:00 CET [2026/11/16 14:14:00")
        .replace("Hero (8725 in chips", "Hero (10025 in chips");
    let parsed: Vec<(Hand, String)> = [first, later].iter().map(|t| (parse_hand(t).unwrap(), t.clone())).collect();
    db.insert_hands(&parsed).unwrap();
    let ids: Vec<u64> = parsed.iter().map(|(h, _)| h.id).collect();

    // Only the first hand is analysed so far; its flop decision still waits for the solver.
    let mut pending = decision(ids[0], 9, Status::Pending, None, None);
    pending.street = Street::Flop;
    db.save_analysis(&[HandAnalysis {
        hand_id: ids[0],
        engine: ENGINE.into(),
        decisions: vec![decision(ids[0], 8, Status::Rated, Some(Rating::Best), Some(0.0)), pending],
        job: None,
    }])
    .unwrap();

    let r = db.tournament_report(TOURNAMENT, "2026/11/16 20:00:00", ENGINE).unwrap();
    assert_eq!(r.hands, 2);
    assert_eq!(r.unanalyzed, 1);
    assert_eq!((r.score.decisions, r.pending), (1, 1));
    assert_eq!(r.best.len(), 1);
    assert_eq!(r.best[0].alternative.as_deref(), Some("Fold"));
    // The stacks come from the seat lines, in hundredths like every amount.
    assert_eq!(r.timeline.iter().map(|p| p.stack).collect::<Vec<_>>(), vec![Some(872_500), Some(1_002_500)]);
    // The all-in with aces against kings is in the expectation; luck is what the cards added.
    assert_eq!(r.luck.all_ins, 2);
    assert_eq!(r.luck.luck, r.luck.net - r.luck.ev);
    assert!(r.luck.luck > 0, "aces held: {:?}", r.luck);
    assert_eq!(r.starting.iter().find(|s| s.class == "AA").map(|s| s.hands), Some(2));
}
