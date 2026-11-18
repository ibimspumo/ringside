//! Spots being solved right now, by key: a second area that needs the same spot waits for
//! the first solve instead of solving it again.

use std::collections::HashSet;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct InFlight {
    keys: Mutex<HashSet<String>>,
    done: Condvar,
}

/// The right to solve one key; dropping it lets the others look again.
pub struct Claim<'a> {
    flight: &'a InFlight,
    key: String,
}

impl InFlight {
    pub fn new() -> InFlight {
        InFlight::default()
    }

    /// The claim on `key`, unless another solve holds it.
    pub fn claim(&self, key: &str) -> Option<Claim<'_>> {
        self.keys.lock().unwrap().insert(key.to_string()).then(|| Claim { flight: self, key: key.to_string() })
    }

    pub fn busy(&self, key: &str) -> bool {
        self.keys.lock().unwrap().contains(key)
    }

    /// Waits up to `timeout` until nobody solves `key`; true when it is free.
    pub fn wait(&self, key: &str, timeout: Duration) -> bool {
        let keys = self.keys.lock().unwrap();
        let (keys, _) = self.done.wait_timeout_while(keys, timeout, |k| k.contains(key)).unwrap();
        !keys.contains(key)
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.flight.keys.lock().unwrap().remove(&self.key);
        self.flight.done.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_claim_per_key() {
        let f = InFlight::new();
        let a = f.claim("spot:a").expect("free");
        assert!(f.claim("spot:a").is_none() && f.busy("spot:a"));
        let b = f.claim("spot:b").expect("another key is free");
        assert!(!f.wait("spot:a", Duration::from_millis(5)));
        drop(a);
        assert!(f.wait("spot:a", Duration::ZERO) && f.claim("spot:a").is_some());
        drop(b);
    }
}
