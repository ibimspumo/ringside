//! Tables built on first use and dropped after a while without use, so a long-running app
//! does not keep every chart it ever showed. Building them again takes milliseconds.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(crate) struct Idle<T> {
    slot: Mutex<Option<(Arc<T>, Instant)>>,
    build: fn() -> T,
}

impl<T> Idle<T> {
    pub(crate) const fn new(build: fn() -> T) -> Idle<T> {
        Idle { slot: Mutex::new(None), build }
    }

    pub(crate) fn get(&self) -> Arc<T> {
        let mut slot = self.slot.lock().expect("table lock");
        let (table, used) = slot.get_or_insert_with(|| (Arc::new((self.build)()), Instant::now()));
        *used = Instant::now();
        table.clone()
    }

    /// Drops the table if unused for `idle`; callers still holding it keep their copy.
    pub(crate) fn release(&self, idle: Duration) -> bool {
        let mut slot = self.slot.lock().expect("table lock");
        let old = slot.as_ref().is_some_and(|(_, used)| used.elapsed() >= idle);
        if old {
            *slot = None;
        }
        old
    }
}

/// Drops the push/fold and preflop tables and the three-way showdown tables of the
/// multiway solver that were unused for `idle`; returns how many went.
pub fn release_idle_tables(idle: Duration) -> usize {
    crate::pushfold_table::release(idle) + crate::preflop_table::release(idle) + crate::multiway::release_triples(idle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_released_table_is_built_again() {
        static BUILT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        static T: Idle<u32> = Idle::new(|| BUILT.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32);
        assert_eq!(*T.get(), 0);
        assert_eq!(*T.get(), 0);
        assert!(!T.release(Duration::from_secs(60)));
        assert!(T.release(Duration::ZERO));
        assert_eq!(*T.get(), 1);
    }
}
