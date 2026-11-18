//! Frees what the training pages keep once nobody uses it: the solver's tree, the practice
//! stock and the chart tables. The overlay needs none of it, only the database.

use std::time::Duration;

use tauri::AppHandle;

const EVERY: Duration = Duration::from_secs(10);

pub fn spawn(app: AppHandle) {
    let _ = std::thread::Builder::new().name("housekeeping".into()).spawn(move || {
        loop {
            std::thread::sleep(EVERY);
            crate::solver::tick(&app);
            crate::play::tick();
            crate::charts::tick();
            ringside_engine::release_idle_tables(crate::charts::CACHE_IDLE);
        }
    });
}
