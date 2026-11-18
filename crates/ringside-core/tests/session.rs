//! The session review read from the database, over copies of the anonymised knockout fixture.

use ringside_core::db::{Choice, Db, Decision, Filter, HandAnalysis, Rating, Source, SpotType, Status};
use ringside_core::model::{Hand, Street};
use ringside_core::{parse_hand, split_hands};

const KNOCKOUT: &str = include_str!("fixtures/knockout.txt");
const ENGINE: &str = "test-engine";

/// The fixture's first hand as hand `id` of tournament `tournament`, at `hh:mm` local time.
fn copy(id: u64, tournament: u64, at: &str) -> String {
    split_hands(KNOCKOUT)[0]
        .replace("#260000000001", &format!("#{id}"))
        .replace("Tournament #4046789300", &format!("Tournament #{tournament}"))
        .replace("2026/11/16 20:10:00 CET [2026/11/16 14:10:00", &format!("2026/11/16 {at}:00 CET [2026/11/16 {at}:00"))
}

fn call(hand_id: u64, rating: Rating, loss: f64) -> Decision {
    Decision {
        hand_id,
        action_index: 8,
        street: Street::Preflop,
        spot: SpotType::CallShove,
        status: Status::Rated,
        played: "Call 10,9 BB".into(),
        best: Some("Fold".into()),
        strategy: vec![
            Choice { label: "Fold".into(), freq: 1.0, ev_bb: Some(0.0), played: false, kind: "fold".into() },
            Choice { label: "Call".into(), freq: 0.0, ev_bb: Some(-loss), played: true, kind: "call".into() },
        ],
        played_freq: Some(0.0),
        ev_loss_bb: Some(loss),
        rating: Some(rating),
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
fn sessions_from_the_database() {
    let mut db = Db::open_in_memory().unwrap();
    // Two tournaments side by side, then a pause of two hours and a third one.
    let texts = [copy(301, 11, "20:00"), copy(302, 12, "20:05"), copy(303, 11, "20:20"), copy(304, 13, "22:30")];
    let parsed: Vec<(Hand, String)> = texts.iter().map(|t| (parse_hand(t).unwrap(), t.clone())).collect();
    db.insert_hands(&parsed).unwrap();

    let spans = db.session_spans(10).unwrap();
    assert_eq!(spans.len(), 2);
    assert_eq!((spans[0].hands, spans[0].tournaments.clone()), (1, vec![13]), "newest first");
    assert_eq!((spans[1].hands, spans[1].tournaments.clone()), (3, vec![11, 12]));
    assert_eq!((spans[1].start.as_str(), spans[1].end.as_str()), ("2026/11/16 20:00:00", "2026/11/16 20:20:00"));
    assert_eq!(db.sessions(&Filter::default()).unwrap().iter().map(|s| s.hands).collect::<Vec<_>>(), [1, 3], "one rule");

    // A filter of the earlier session sees its hands only.
    let window = Filter { since: Some(spans[1].start.clone()), until: Some(spans[1].end.clone()), ..Default::default() };
    assert_eq!(db.hero_results(&window, 100).unwrap().hands, 3);
    assert_eq!(db.tournaments(&window).unwrap().len(), 2);

    // Calling the shove was a mistake by the equilibrium, and the aces held: good luck.
    let id = parsed[0].0.id;
    db.save_analysis(&[HandAnalysis { hand_id: id, engine: ENGINE.into(), decisions: vec![call(id, Rating::Mistake, 2.0)], job: None }])
        .unwrap();
    let r = db.session_review(&spans[1], ENGINE).unwrap();
    assert_eq!((r.report.hands, r.report.unanalyzed), (3, 2));
    assert!(r.bad_luck.is_empty());
    assert_eq!(r.good_luck.len(), 1);
    let g = &r.good_luck[0];
    assert_eq!((g.hand_id, g.tournament_id, g.loss_bb), (id, Some(11), 2.0));
    let equity = g.equity_pct.unwrap();
    assert!((75.0..90.0).contains(&equity), "aces against kings: {equity}");
    assert_eq!(g.opponents.len(), 1);
    assert!(r.report.luck.luck_bb > 0.0);

    let locked = r.locked();
    assert!(locked.good_luck.is_empty() && locked.report.worst.is_empty());
    assert_eq!(locked.report.hands, 3);
}
