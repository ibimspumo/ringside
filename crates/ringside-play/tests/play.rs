use ringside_engine::preflop::Kind;
use ringside_engine::pushfold_table::Ante;
use ringside_engine::{Card, Combo};
use ringside_play::grade::Grade;
use ringside_play::hand::{EndReason, Event, Street, deal};
use ringside_play::pool::{NoLookup, Pool};
use ringside_play::scenario::PotType;
use ringside_play::table::{Config, Mode, TableKey, deep};
use ringside_play::{Hand, HandView};
use ringside_solver::{Settings, solve};

fn json(v: &HandView) -> String {
    serde_json::to_string(v).unwrap()
}

/// Plays the hero's first choice at every decision.
fn play_out(hand: &mut Hand, choose: impl Fn(&ringside_play::Lesson) -> usize) {
    while let Some(lesson) = hand.lesson() {
        let a = choose(&lesson);
        hand.act(a).unwrap();
    }
}

#[test]
fn the_seed_makes_the_hand() {
    let config = Config::default();
    let a = Hand::start(&config, 42, None).unwrap();
    let b = Hand::start(&config, 42, None).unwrap();
    assert_eq!(json(&a.view()), json(&b.view()));
    let c = Hand::start(&config, 43, None).unwrap();
    assert_ne!(a.view().hero_cards, c.view().hero_cards);
    // The hero always has something to decide.
    assert!(a.decision().is_some());
}

#[test]
fn repeating_a_hand_deals_and_plays_the_same() {
    let config = Config { players: 9, ante: Ante::Classic, stack_bb: 25.0, ..Config::default() };
    for seed in 0..20 {
        let first = Hand::start(&config, seed, None).unwrap();
        let mut a = Hand::play(first.deal.clone()).unwrap();
        let mut b = Hand::play(first.deal.clone()).unwrap();
        play_out(&mut a, |l| l.choices.len() - 1);
        play_out(&mut b, |l| l.choices.len() - 1);
        assert_eq!(json(&a.view()), json(&b.view()), "seed {seed}");
        assert!(a.is_over());
    }
}

#[test]
fn opponents_open_as_often_as_the_model() {
    // Hero in the big blind; the first player to act opens with the model's frequency.
    let config = Config { hero_seat: Some(5), ..Config::default() };
    let table = deep(TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 }).unwrap();
    let raise = table.solution.nodes[0].actions.iter().position(|a| a.kind == Kind::Raise).unwrap();
    let expected = table.solution.action_pct(0, raise) / 100.0;
    let n = 3000;
    let mut opens = 0;
    for seed in 0..n {
        let hand = Hand::play(deal(&config, seed, None).unwrap()).unwrap();
        let Some(Event::Action { seat: 0, choice, .. }) = hand.view().events.first().cloned() else { panic!("UTG acts first") };
        opens += (choice.kind == "raise") as u32;
    }
    let p = opens as f64 / n as f64;
    let sigma = (expected * (1.0 - expected) / n as f64).sqrt();
    assert!((p - expected).abs() < 4.0 * sigma, "opened {p:.4}, model {expected:.4}");
}

#[test]
fn push_fold_decisions_are_graded_by_the_equilibrium_ev() {
    let config = Config { mode: Mode::PushFold, stack_bb: 10.0, hero_seat: Some(4), ..Config::default() };
    let mut seen = 0;
    for seed in 0..40 {
        let mut hand = Hand::start(&config, seed, None).unwrap();
        let lesson = hand.lesson().unwrap();
        let (Some(strategy), Some(evs)) = (lesson.strategy.clone(), lesson.evs.clone()) else { continue };
        let best = if evs[1] > evs[0] { 1 } else { 0 };
        let worst = 1 - best;
        let mut again = Hand::play(hand.deal.clone()).unwrap();
        let good = hand.act(best).unwrap();
        let bad = again.act(worst).unwrap();
        let j = good.judgement.unwrap();
        assert_eq!(j.loss_bb, Some(0.0));
        if strategy[best] >= 0.1 {
            assert_eq!(j.grade, Grade::Best);
        }
        let b = bad.judgement.unwrap();
        assert!((b.loss_bb.unwrap() - (evs[best] - evs[worst])).abs() < 1e-9);
        assert!((b.loss_pct.unwrap() - b.loss_bb.unwrap() / lesson.pot * 100.0).abs() < 1e-9);
        assert!(b.grade >= j.grade);
        seen += 1;
    }
    assert!(seen > 10);
}

#[test]
fn hands_land_in_the_prepared_spot() {
    let key = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    let table = deep(key).unwrap();
    // The smallest 3-bet pot solves in about a second at this precision.
    let scenario = table.scenarios.iter().filter(|s| s.pot_type == PotType::ThreeBet).min_by(|a, b| a.reach.total_cmp(&b.reach)).unwrap().clone();
    let flop: [Card; 3] = ["Kd".parse().unwrap(), "8s".parse().unwrap(), "3h".parse().unwrap()];
    let spot = scenario.spot("Kd8s3h");
    let solution = solve(&spot, &Settings { target_pct: 5.0, max_iterations: 40, threads: 2 }, 4 << 30, |_| {}, || false).unwrap();
    let pool = Pool::new(Box::new(NoLookup), 1, 4 << 30);
    pool.insert(key, scenario.clone(), flop, Box::new(solution));

    let config = Config { mode: Mode::Postflop, ..Config::default() };
    let mut hand = Hand::start(&config, 7, Some(&pool)).unwrap();
    let view = hand.view();
    assert!(view.spot.is_some(), "steered into the spot");
    assert_eq!(&view.board[..], &flop[..]);
    assert!(scenario.seats().contains(&view.hero));
    // The hero's hand comes from the hero's range at the flop.
    let side = scenario.seats().iter().position(|&s| s == view.hero).unwrap();
    let [a, b] = view.hero_cards;
    assert!(scenario.weights[side][Combo::new(a, b).class().index()] > 0.0);
    // Everyone else folded preflop; the hero decides on the flop.
    let lesson = hand.lesson().unwrap();
    assert_eq!(lesson.street, Street::Flop);
    assert!(lesson.node.is_some() && lesson.strategy.is_some());

    // Repeating it deals the same cards, board and opponent.
    let again = Hand::play(hand.deal.clone()).unwrap();
    assert_eq!(json(&again.view()), json(&view));

    // Grading against the solution: the chosen action's loss is the EV gap at this node.
    let evs = lesson.evs.clone().unwrap();
    let worst = (0..evs.len()).min_by(|&x, &y| evs[x].total_cmp(&evs[y])).unwrap();
    let top = evs.iter().copied().fold(f64::MIN, f64::max);
    let verdict = hand.act(worst).unwrap();
    assert!((verdict.judgement.unwrap().loss_bb.unwrap() - (top - evs[worst])).abs() < 1e-9);

    play_out(&mut hand, |l| l.strategy.as_ref().map_or(0, |s| (0..s.len()).max_by(|&x, &y| s[x].total_cmp(&s[y])).unwrap()));
    let outcome = hand.view().outcome.unwrap();
    assert!(matches!(outcome.reason, EndReason::Folded | EndReason::Showdown));
    // Chips are conserved.
    let total: f64 = outcome.net.iter().sum();
    assert!(total.abs() < 1e-6, "net sums to {total}");
}

#[test]
fn an_empty_stock_gives_a_preflop_hand() {
    let config = Config::default();
    let mut hand = Hand::start(&config, 3, None).unwrap();
    assert!(hand.view().waiting);
    play_out(&mut hand, |l| l.choices.iter().position(|c| c.kind == "call" || c.kind == "check").unwrap_or(0));
    let o = hand.view().outcome.unwrap();
    assert!(matches!(o.reason, EndReason::Folded | EndReason::Showdown | EndReason::FlopUnsolved));
}
