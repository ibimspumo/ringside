//! Reports survive the trip through the database, and their concepts feed the learning area.

use ringside_coach::report::{Figure, Meta, Source, Spot, Verdict};
use ringside_coach::{Report, concept_counts};
use ringside_core::Db;

fn spot(concept: &str, verdict: Verdict) -> Spot {
    Spot {
        hand_id: 260000000103,
        played_at: "2026/11/16 20:43:00".into(),
        hero_cards: vec!["Kh".into(), "6d".into()],
        board: vec![],
        position: Some("SB".into()),
        stack_bb: 8.0,
        action: "K6o gefoldet".into(),
        verdict,
        better: Some("All-in".into()),
        why: "Nash schiebt K6o hier immer.".into(),
        numbers: vec![Figure { label: "Nash-Shove".into(), value: "100 %".into(), source: Source::Berechnet }],
        concept: concept.into(),
    }
}

fn report(tournament_id: u64, created_at: i64, spots: Vec<Spot>) -> Report {
    Report {
        tournament_id,
        summary: "Gut gespielt, ein verpasster Shove.".into(),
        spots,
        focus: vec!["Push/Fold im Small Blind".into()],
        meta: Meta { model: "openai/gpt-6.1-sol".into(), created_at, cost_usd: Some(0.05), ..Meta::default() },
    }
}

#[test]
fn save_load_replace_and_delete() {
    let db = Db::open_in_memory().unwrap();
    let first = report(1, 100, vec![spot("push_fold_sb", Verdict::Fehler)]);
    let second = report(2, 200, vec![spot("pot_odds", Verdict::Ok)]);
    for r in [&first, &second] {
        db.save_coach_report(r.tournament_id, r.meta.created_at, &r.meta.model, &serde_json::to_string(r).unwrap())
            .unwrap();
    }
    let loaded: Report = serde_json::from_str(&db.coach_report(1).unwrap().unwrap()).unwrap();
    assert_eq!(loaded, first);

    let newest: Vec<Report> = db.coach_reports().unwrap().iter().map(|j| serde_json::from_str(j).unwrap()).collect();
    assert_eq!(newest.iter().map(|r| r.tournament_id).collect::<Vec<_>>(), vec![2, 1]);

    let again = report(1, 300, vec![spot("icm", Verdict::Stark)]);
    db.save_coach_report(1, 300, "anderes/modell", &serde_json::to_string(&again).unwrap()).unwrap();
    assert_eq!(db.coach_reports().unwrap().len(), 2, "neu analysieren ersetzt den alten Bericht");

    assert!(db.delete_coach_report(1).unwrap());
    assert!(!db.delete_coach_report(1).unwrap());
    assert!(db.coach_report(1).unwrap().is_none());
}

#[test]
fn concepts_weakest_first() {
    let reports = [
        report(1, 1, vec![spot("push_fold_sb", Verdict::Fehler), spot("pot_odds", Verdict::Ok)]),
        report(2, 2, vec![spot("push_fold_sb", Verdict::Fraglich), spot("pot_odds", Verdict::Stark)]),
        report(3, 3, vec![spot("pot_odds", Verdict::Ok)]),
    ];
    let counts = concept_counts(&reports);
    assert_eq!(counts[0].id, "push_fold_sb");
    assert_eq!((counts[0].spots, counts[0].mistakes, counts[0].questionable, counts[0].tournaments), (2, 1, 1, 2));
    assert_eq!(counts[0].exercise, Some("push_fold_sb"));
    assert_eq!(counts[1].id, "pot_odds");
    assert_eq!(counts[1].spots, 3);
    assert_eq!(counts.len(), 2);
}
