//! Results computed from the database, kept until it changes: a page refresh asks the top
//! bar, the page and its panels for the same list at once.

use std::sync::Mutex;

/// The last few results by key; the key holds [`ringside_core::db::Db::version`].
pub struct Memo<K, T> {
    last: Mutex<Vec<(K, T)>>,
}

const KEEP: usize = 4;

impl<K: PartialEq, T: Clone> Memo<K, T> {
    pub const fn new() -> Self {
        Memo { last: Mutex::new(Vec::new()) }
    }

    /// The stored result for `key`, else `compute`'s. Take the version before computing, so a
    /// write while it runs makes the next call compute again.
    pub fn get_or(&self, key: K, compute: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        if let Some((_, v)) = self.last.lock().unwrap().iter().find(|(k, _)| *k == key) {
            return Ok(v.clone());
        }
        let v = compute()?;
        let mut last = self.last.lock().unwrap();
        last.retain(|(k, _)| *k != key);
        if last.len() >= KEEP {
            last.remove(0);
        }
        last.push((key, v.clone()));
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_once_per_key_and_again_after_a_change() {
        let memo: Memo<(u64, u64), u32> = Memo::new();
        let mut runs = 0;
        let mut get = |key| memo.get_or(key, || {
            runs += 1;
            Ok(runs)
        });
        assert_eq!(get((0, 1)), Ok(1));
        assert_eq!(get((0, 1)), Ok(1), "same version: the stored result");
        assert_eq!(get((0, 2)), Ok(2), "written since: computed again");
        assert_eq!(get((1, 2)), Ok(3), "another database");
    }
}
