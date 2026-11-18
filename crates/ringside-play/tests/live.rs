//! The worker solves a spot a hand waits for before the stock, counts it against the
//! memory budget while the hand holds it, and a cancelled one lets the hand play on.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use ringside_engine::pushfold_table::Ante;
use ringside_play::hand::Need;
use ringside_play::pool::{NoLookup, Pool, Status, Urgency};
use ringside_play::scenario::PotType;
use ringside_play::table::{TableKey, deep};
use ringside_play::tree::TreeSource;

fn need() -> Need {
    let key = TableKey { players: 6, ante: Ante::BigBlind, stack_bb: 40.0 };
    let table = deep(key).unwrap();
    let sc = table.scenarios.iter().filter(|s| s.pot_type == PotType::ThreeBet).min_by(|a, b| a.reach.total_cmp(&b.reach)).unwrap();
    Need::fresh(sc.spot("Qs7h2d"), key, sc.seats())
}

fn step(pool: &Pool) -> Vec<Status> {
    let seen = std::sync::Mutex::new(Vec::new());
    pool.step(&mut ChaCha8Rng::seed_from_u64(1), &|| false, &|_| {}, &|s| seen.lock().unwrap().push(s.status));
    seen.into_inner().unwrap()
}

#[test]
fn a_live_spot_is_solved_held_and_freed() {
    let pool = Pool::new(Box::new(NoLookup), 2, 4 << 30);
    let need = need();
    pool.request(need.clone(), Urgency::Now);
    assert!(pool.take_live(&need).is_none(), "not solved yet");
    let seen = step(&pool);
    assert_eq!(seen.first(), Some(&Status::Running));
    assert_eq!(seen.last(), Some(&Status::Done));
    let ready = pool.take_live(&need).unwrap().unwrap();
    assert_eq!(ready.source, TreeSource::Live);
    assert!(ready.complete);
    assert!(pool.memory_bytes() > 0, "the held spot counts against the budget");
    drop(ready);
    assert_eq!(pool.memory_bytes(), 0, "freed with the hand");
}

#[test]
fn a_cancelled_spot_lets_the_hand_go_on_and_one_too_large_says_so() {
    let pool = Pool::new(Box::new(NoLookup), 2, 4 << 30);
    let need = need();
    pool.request(need.clone(), Urgency::Now);
    pool.cancel_live();
    assert!(pool.take_live(&need).unwrap().is_err());

    let tiny = Pool::new(Box::new(NoLookup), 2, 1 << 20);
    tiny.request(need.clone(), Urgency::Now);
    assert_eq!(step(&tiny).last(), Some(&Status::Error));
    let err = tiny.take_live(&need).unwrap().err().unwrap();
    assert!(err.contains("Arbeitsspeicher"), "{err}");
}

#[test]
fn a_spot_solved_ahead_serves_the_hand_and_a_new_move_drops_it() {
    let pool = Pool::new(Box::new(NoLookup), 2, 4 << 30);
    let need = need();
    pool.request(need.clone(), Urgency::Ahead);
    assert_eq!(pool.live_status(), None, "nothing the hand waits for");
    step(&pool);
    // The hand then needs it: it is there at once.
    pool.request(need.clone(), Urgency::Now);
    assert!(pool.take_live(&need).unwrap().is_ok());

    let mut other = need.clone();
    other.spot.board = "Qs7h3d".into();
    pool.request(need.clone(), Urgency::Ahead);
    pool.request(other.clone(), Urgency::Now);
    assert!(pool.take_live(&need).is_none(), "the hand went elsewhere");
}
