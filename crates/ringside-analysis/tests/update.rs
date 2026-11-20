//! An update of the analysis: every hand rated before is rated again, solver results of an
//! unchanged spot stay, and the end is the same as a fresh start with the same hands.

use std::collections::HashMap;

use ringside_analysis::postflop::{PostflopJob, solver_link};
use ringside_analysis::{ENGINE, Rating, Source, Status, analyze_again, analyze_hand, keep_rated};
use ringside_core::db::{Db, HandAnalysis};
use ringside_core::model::Hand;
use ringside_core::skill::profile;
use ringside_core::{parse_hand, split_hands};

const FILES: [&str; 3] = [
    include_str!("fixtures/hands.txt"),
    include_str!("../../ringside-core/tests/fixtures/tournament.txt"),
    include_str!("../../ringside-core/tests/fixtures/knockout.txt"),
];

fn hands() -> Vec<(Hand, String)> {
    FILES.iter().flat_map(|t| split_hands(t)).filter_map(|raw| parse_hand(raw).ok().map(|h| (h, raw.to_string()))).collect()
}

fn db() -> Db {
    let mut db = Db::open_in_memory().unwrap();
    db.insert_hands(&hands()).unwrap();
    db
}

/// The app's batches: `next` gives the rows, until none is left.
fn run(db: &mut Db, next: impl Fn(&Db) -> Vec<(u64, String)>) {
    loop {
        let rows = next(db);
        if rows.is_empty() {
            break;
        }
        let before: HashMap<_, _> = rows.iter().map(|(id, _)| (*id, db.hand_decisions(*id).unwrap())).collect();
        db.save_analysis(&analyze_again(&rows, &before)).unwrap();
    }
}

fn rating(db: &Db) -> serde_json::Value {
    serde_json::to_value(profile(&db.skill_moves().unwrap())).unwrap()
}

#[test]
fn an_update_rates_every_hand_again_and_ends_where_a_fresh_start_does() {
    let mut fresh = db();
    run(&mut fresh, |db| db.analysis_candidates(ENGINE, 0, 7).unwrap());
    assert_eq!(fresh.outdated_count(ENGINE).unwrap(), 0);
    let want = rating(&fresh);
    assert!(want["rating"].is_number(), "the fixtures make a rating");

    // The same hands, rated by an earlier analysis that judged every loss twice as hard.
    let mut old = db();
    let earlier: Vec<HandAnalysis> = hands()
        .iter()
        .map(|(h, _)| {
            let mut a = analyze_hand(h);
            a.engine = "0.1.0+analysis.1".into();
            for d in &mut a.decisions {
                d.engine = a.engine.clone();
                d.ev_loss_bb = d.ev_loss_bb.map(|l| l * 2.0 + 0.5);
            }
            a
        })
        .collect();
    old.save_analysis(&earlier).unwrap();
    let analysed = hands().len() as i64;
    assert_eq!(old.outdated_count(ENGINE).unwrap(), analysed);
    assert_ne!(rating(&old), want, "the earlier analysis rates differently");
    assert!(old.analysis_candidates(ENGINE, 0, 1000).unwrap().len() as i64 == analysed, "the bulk run sees them too");

    run(&mut old, |db| db.outdated_candidates(ENGINE, 0, 7).unwrap());
    assert_eq!(old.outdated_count(ENGINE).unwrap(), 0);
    assert_eq!(rating(&old), want);
}

/// A hand of the fixtures whose postflop decisions wait for the solver.
fn queued() -> (Hand, HandAnalysis, PostflopJob) {
    hands()
        .into_iter()
        .find_map(|(h, _)| {
            let a = analyze_hand(&h);
            let job: PostflopJob = serde_json::from_str(&a.job.as_ref()?.1).ok()?;
            (a.decisions.iter().filter(|d| d.status == Status::Pending).count() == 1).then_some((h, a, job))
        })
        .expect("a fixture hand with one decision for the solver")
}

#[test]
fn a_solved_spot_keeps_its_rating_while_the_spot_stays() {
    let (hand, fresh, job) = queued();
    let pending = fresh.decisions.iter().find(|d| d.status == Status::Pending).unwrap().clone();
    let solved = |link: String| ringside_core::db::Decision {
        status: Status::Rated,
        source: Some(Source::Solver),
        rating: Some(Rating::Mistake),
        ev_loss_bb: Some(1.5),
        solver: Some(link),
        engine: "0.1.0+analysis.1".into(),
        ..pending.clone()
    };

    let same = solved(solver_link(&job, &[], Some(4)));
    let mut again = fresh.clone();
    keep_rated(&hand, &mut again, std::slice::from_ref(&same));
    let kept = again.decisions.iter().find(|d| d.action_index == pending.action_index).unwrap();
    assert_eq!((kept.status, kept.source, kept.rating, kept.ev_loss_bb), (Status::Rated, Some(Source::Solver), Some(Rating::Mistake), Some(1.5)));
    assert_eq!(kept.engine, ENGINE);
    assert!(again.job.is_none(), "nothing left for the solver");

    let mut other = job.clone();
    other.spot.pot += 1.0;
    let changed = solved(solver_link(&other, &[], Some(4)));
    let mut again = fresh.clone();
    keep_rated(&hand, &mut again, &[changed]);
    assert_eq!(again, fresh, "another spot waits for the solver again");

    let estimated = ringside_core::db::Decision { status: Status::Rated, source: Some(Source::Estimate), rating: Some(Rating::Good), ..pending.clone() };
    let mut again = fresh.clone();
    keep_rated(&hand, &mut again, &[estimated]);
    let d = again.decisions.iter().find(|d| d.action_index == pending.action_index).unwrap();
    assert_eq!((d.status, d.source), (Status::Rated, Some(Source::Estimate)), "a failed solve is estimated again, not queued");
    assert!(again.job.is_none());
}
