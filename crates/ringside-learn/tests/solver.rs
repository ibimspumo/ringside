use std::sync::Mutex;

use ringside_engine::multiway::{self, Params};
use ringside_engine::pushfold_table::{self, Ante};
use ringside_learn::{lookup_or_solve, set_solver};

static ASKED: Mutex<Vec<Vec<f64>>> = Mutex::new(Vec::new());

#[test]
fn tables_off_the_bundled_grid_go_through_the_installed_solver() {
    set_solver(|p| {
        ASKED.lock().unwrap().push(p.stacks_bb.clone());
        multiway::solve(p).map_err(|e| e.to_string())
    });

    let bundled = pushfold_table::params(Ante::BigBlind, 6, 10.0);
    assert!(pushfold_table::lookup(&bundled).is_some());
    lookup_or_solve(&bundled).unwrap();
    assert!(ASKED.lock().unwrap().is_empty(), "bundled tables need no solver");

    let off_grid = Params::new(vec![10.0, 12.0, 8.0]);
    lookup_or_solve(&off_grid).unwrap();
    lookup_or_solve(&off_grid).unwrap();
    assert_eq!(*ASKED.lock().unwrap(), [vec![10.0, 12.0, 8.0]], "solved once, then from the cache");
}
