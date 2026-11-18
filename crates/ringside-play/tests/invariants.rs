//! What every hand keeps, over many random hands and random moves: it ends in a fold or a
//! showdown, the chips add up, every graded move has exactly one source, and a move
//! without one says why it is not graded.

use std::sync::{Arc, Mutex};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use ringside_engine::pushfold_table::Ante;
use ringside_play::assess::{Assess, Assessment, Situation};
use ringside_play::hand::{EndReason, Event, Need, Source, Street, Unrated};
use ringside_play::pool::{NoLookup, Pool, Ready};
use ringside_play::scenario::PotType;
use ringside_play::table::{Config, Mode, TableKey, deep};
use ringside_play::tree::TreeSource;
use ringside_play::{Hand, HandView};
use ringside_solver::{Settings, solve};

/// A rough solve, enough to walk the tree; the tests check the hand, not the strategy.
fn rough(need: &Need) -> Result<Ready, String> {
    let settings = Settings { target_pct: 50.0, max_iterations: 2, threads: 2 };
    let solution = solve(&need.spot, &settings, 16 << 30, |_| {}, || false).map_err(|e| e.to_string())?;
    let exploitability_pct = solution.summary.exploitability_pct;
    Ok(Ready { id: 0, source: TreeSource::Live, complete: true, exploitability_pct, tree: Arc::new(Mutex::new(Box::new(solution))) })
}

/// Plays the hand to its end with random moves. The first `solves` live spots are solved,
/// later ones are cancelled, as the hero may do.
fn play_to_end(hand: &mut Hand, rng: &mut ChaCha8Rng, solves: &mut usize) {
    for _ in 0..200 {
        if let Some(need) = hand.need().cloned() {
            let result = if *solves > 0 {
                *solves -= 1;
                rough(&need)
            } else {
                Err("Abgebrochen".into())
            };
            hand.resume(result).unwrap();
            continue;
        }
        let Some(lesson) = hand.lesson() else { break };
        let a = rng.random_range(0..lesson.choices.len());
        hand.act(a).unwrap();
    }
    assert!(hand.is_over(), "the hand ends: {:?}", hand.view().events);
}

fn check(view: &HandView) {
    let o = view.outcome.as_ref().expect("an outcome");
    let left = view.snapshots.last().unwrap().folded.iter().filter(|f| !**f).count();
    match o.reason {
        EndReason::Folded => assert_eq!(o.winners.len(), 1, "{o:?}"),
        EndReason::Showdown => {
            assert_eq!(view.board.len(), 5, "a showdown shows the whole board");
            assert!(left >= 2 || o.winners.len() == 1);
            assert!(!o.winners.is_empty());
        }
    }
    // The chips add up, and nobody loses more than the stack.
    let total: f64 = o.net.iter().sum();
    assert!(total.abs() < 1e-6, "net sums to {total}: {:?}", o.net);
    for (s, &x) in o.net.iter().enumerate() {
        assert!(-x <= view.stack_bb + 1e-6, "seat {s} loses {x} with {} BB", view.stack_bb);
    }
    // Every move of yours is your own decision (in the postflop mode the line to the spot
    // is played for you), graded by one source or marked as not graded.
    let auto = view.config.mode == Mode::Postflop && view.spot.is_some();
    for e in &view.events {
        if let Event::Action { seat, verdict, street, .. } = e
            && *seat == view.hero
            && !(auto && *street == Street::Preflop)
        {
            assert!(verdict.is_some(), "you are folded without a decision: {:?}", view.events);
        }
    }
    for v in &view.verdicts {
        let l = &v.lesson;
        assert_eq!(v.judgement.is_some(), l.strategy.is_some());
        assert_eq!(v.points.is_some(), v.judgement.is_some());
        if l.strategy.is_some() {
            assert!(l.source.is_some() && l.unrated.is_none(), "{l:?}");
        } else {
            assert!(l.unrated.is_some(), "a move without a grade says why: {l:?}");
        }
    }
}

#[test]
fn every_hand_ends_in_a_fold_or_a_showdown() {
    let configs = [
        Config::default(),
        Config { players: 9, ante: Ante::Classic, stack_bb: 25.0, ..Config::default() },
        Config { players: 8, ante: Ante::None, stack_bb: 75.0, ..Config::default() },
        Config { mode: Mode::PushFold, stack_bb: 10.0, ..Config::default() },
        Config { mixed_stacks: true, hero_seat: Some(5), ..Config::default() },
    ];
    let mut solves = 4;
    let mut multiway = 0;
    for config in configs {
        for seed in 0..120 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xa5a5);
            let mut hand = Hand::start(&config, seed, None).unwrap();
            play_to_end(&mut hand, &mut rng, &mut solves);
            let view = hand.view();
            check(&view);
            multiway += view.verdicts.iter().filter(|v| v.lesson.unrated == Some(Unrated::Multiway)).count();
        }
    }
    assert_eq!(solves, 0, "some flops were solved live");
    assert!(multiway > 0, "some hands were played multiway");
}

#[test]
fn a_flop_without_a_prepared_spot_is_solved_live() {
    // Hero in the big blind calls whatever comes; a heads-up flop then needs a live solve.
    let config = Config { hero_seat: Some(5), ..Config::default() };
    for seed in 0..400 {
        let mut hand = Hand::start(&config, seed, None).unwrap();
        while hand.need().is_none() {
            let Some(l) = hand.lesson() else { break };
            let a = l.choices.iter().position(|c| c.kind == "call" || c.kind == "check").unwrap_or(0);
            hand.act(a).unwrap();
        }
        let Some(need) = hand.need().cloned() else { continue };
        let view = hand.view();
        assert_eq!(view.board.len(), 3, "the flop is on the table while it is solved");
        assert!(view.solving.is_some());
        assert!(need.seats.contains(&view.hero));
        // The hero's hand is part of the hero's range, so the solution has a strategy for it.
        let side = need.seats.iter().position(|&s| s == view.hero).unwrap();
        let range = ringside_engine::Range::parse(if side == 0 { &need.spot.oop_range } else { &need.spot.ip_range }).unwrap();
        let [a, b] = view.hero_cards;
        assert!(range.weight(ringside_engine::Combo::new(a, b)) > 0.0);

        hand.resume(rough(&need)).unwrap();
        let lesson = hand.lesson();
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut solves = 0;
        play_to_end(&mut hand, &mut rng, &mut solves);
        let view = hand.view();
        check(&view);
        assert_eq!(view.spot.as_ref().unwrap().source, Source::Live);
        if let Some(l) = lesson {
            assert!(l.street != Street::Preflop);
            assert_eq!(l.source, Some(Source::Live));
        }
        return;
    }
    panic!("no heads-up flop without a prepared spot");
}

#[test]
fn leaving_the_prepared_line_goes_on() {
    let key = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    let table = deep(key).unwrap();
    let scenario = table.scenarios.iter().filter(|s| s.pot_type == PotType::ThreeBet).min_by(|a, b| a.reach.total_cmp(&b.reach)).unwrap().clone();
    let flop = ["Kd", "8s", "3h"].map(|c| c.parse().unwrap());
    let settings = Settings { target_pct: 50.0, max_iterations: 2, threads: 2 };
    let pool = Pool::new(Box::new(NoLookup), 1, 4 << 30);
    let mut live = 0;
    for seed in 0..30 {
        if pool.status().ready == 0 {
            pool.insert(key, scenario.clone(), flop, Box::new(solve(&scenario.spot("Kd8s3h"), &settings, 16 << 30, |_| {}, || false).unwrap()));
        }
        let deal = ringside_play::hand::deal(&Config::default(), seed, Some(&pool)).unwrap();
        let first = Hand::play(deal.clone()).unwrap().lesson().expect("a preflop decision");
        // Every first move, the one on the line to the spot and all the others.
        for a in 0..first.choices.len() {
            let mut hand = Hand::play(deal.clone()).unwrap();
            hand.act(a).unwrap();
            let mut rng = ChaCha8Rng::seed_from_u64(a as u64);
            let mut solves = 1;
            play_to_end(&mut hand, &mut rng, &mut solves);
            check(&hand.view());
            live += (solves == 0) as usize;
        }
        if live >= 2 {
            return;
        }
    }
    panic!("leaving the line never led to a live solve");
}

#[test]
fn a_stored_spot_is_solved_again_where_it_ends() {
    let key = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    let table = deep(key).unwrap();
    let scenario = table.scenarios.iter().filter(|s| s.pot_type == PotType::ThreeBet).min_by(|a, b| a.reach.total_cmp(&b.reach)).unwrap().clone();
    let settings = Settings { target_pct: 5.0, max_iterations: 40, threads: 2 };
    let mut solution = solve(&scenario.spot("Jd9h6h"), &settings, 4 << 30, |_| {}, || false).unwrap();
    let flop = ["Jd", "9h", "6h"].map(|c| c.parse().unwrap());
    let mut deeper = 0;
    for seed in 0..40 {
        let snapshot = ringside_solver::saved::Snapshot::take(&mut solution, &settings).unwrap();
        let pool = Pool::new(Box::new(NoLookup), 1, 4 << 30);
        pool.insert(key, scenario.clone(), flop, Box::new(snapshot));
        let config = Config { mode: Mode::Postflop, ..Config::default() };
        let mut hand = Hand::start(&config, seed, Some(&pool)).unwrap();
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        // Checks and calls reach the turn and river most often.
        for _ in 0..40 {
            if let Some(need) = hand.need().cloned() {
                assert!(need.complete, "a stored spot is solved again as a whole");
                assert_eq!(need.spot, scenario.spot("Jd9h6h"));
                deeper += 1;
                hand.resume(rough(&need)).unwrap();
                continue;
            }
            let Some(l) = hand.lesson() else { break };
            let a = l.choices.iter().position(|c| c.kind == "check" || c.kind == "call").unwrap_or_else(|| rng.random_range(0..l.choices.len()));
            hand.act(a).unwrap();
        }
        check(&hand.view());
    }
    assert!(deeper > 0, "some hands went deeper than the stored spot");
}

/// Stands in for the calibrated estimate that will grade multiway decisions.
struct Fixed;

impl Assess for Fixed {
    fn assess(&self, s: &Situation) -> Option<Assessment> {
        let n = s.choices.len();
        Some(Assessment { strategy: (0..n).map(|k| if k == 0 { 1.0 } else { 0.0 }).collect(), evs: None, equity: None })
    }
}

#[test]
fn an_estimate_docks_where_no_solver_plays() {
    let config = Config::default();
    let mut graded = 0;
    for seed in 0..300 {
        let mut hand = Hand::start(&config, seed, None).unwrap();
        hand.set_assess(Arc::new(Fixed));
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut solves = 0;
        play_to_end(&mut hand, &mut rng, &mut solves);
        let view = hand.view();
        check(&view);
        for v in &view.verdicts {
            if v.lesson.source == Some(Source::Estimate) {
                assert!(v.judgement.is_some());
                graded += 1;
            }
        }
    }
    assert!(graded > 0, "the estimate graded some multiway moves");
}
