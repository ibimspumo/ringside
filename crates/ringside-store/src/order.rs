//! The one order in which every area finds a solved spot: on this computer, in the
//! library, in Ringside Cloud, and only then by solving it.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::flight::InFlight;

/// Where a stored spot came from, as "Meine Lösungen" names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Solved on this computer.
    Own,
    /// From a signed library pack.
    Library,
    /// Solved by another Ringside user, checked by the verifier.
    Cloud,
}

/// A place to look, in the order of [`steps`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The store on this computer.
    Local,
    /// The library's catalog spots.
    Library,
    /// Ringside Cloud.
    Cloud,
    /// Solve it here.
    Solve,
}

/// What a lookup may do now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    /// PokerStars runs: nothing at all happens.
    pub locked: bool,
    /// The user lets Ringside fetch solutions (`cloud.fetch`).
    pub fetch: bool,
    /// The cloud may be asked now (notice seen, reachable).
    pub may_talk: bool,
    /// Spots others solved count (library packs, the cloud). Off for practice with "Üben
    /// mit fertigen Spots anderer" off: only what this computer solved.
    pub others: bool,
    /// The caller solves when nothing is found.
    pub solve: bool,
}

/// The places to look, in order.
pub fn steps(p: &Policy) -> Vec<Step> {
    if p.locked {
        return Vec::new();
    }
    let mut out = vec![Step::Local];
    if p.others {
        out.push(Step::Library);
        if p.fetch && p.may_talk {
            out.push(Step::Cloud);
        }
    }
    if p.solve {
        out.push(Step::Solve);
    }
    out
}

/// How long a caller waits at a time for another area's solve of the same spot.
const WAIT: Duration = Duration::from_millis(500);

/// Tries `step` for each of [`steps`] until one gives a result. `Step::Solve` runs only
/// with the claim on `key`: if another area solves the same spot, this one waits for it
/// and then looks on this computer again, as long as `keep_waiting` says so.
pub fn resolve<T>(
    policy: &Policy,
    flight: &InFlight,
    key: &str,
    keep_waiting: impl Fn() -> bool,
    mut step: impl FnMut(Step) -> Option<T>,
) -> Option<(Step, T)> {
    for s in steps(policy) {
        if s != Step::Solve {
            if let Some(t) = step(s) {
                return Some((s, t));
            }
            continue;
        }
        loop {
            if let Some(claim) = flight.claim(key) {
                let solved = step(Step::Solve);
                drop(claim);
                return solved.map(|t| (Step::Solve, t));
            }
            flight.wait(key, WAIT);
            if !flight.busy(key)
                && let Some(t) = step(Step::Local)
            {
                return Some((Step::Local, t));
            }
            if !keep_waiting() {
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    fn policies() -> impl Iterator<Item = Policy> {
        (0..32u8).map(|b| Policy {
            locked: b & 1 != 0,
            fetch: b & 2 != 0,
            may_talk: b & 4 != 0,
            others: b & 8 != 0,
            solve: b & 16 != 0,
        })
    }

    const ORDER: [Step; 4] = [Step::Local, Step::Library, Step::Cloud, Step::Solve];

    #[test]
    fn the_order_is_local_library_cloud_solve() {
        for p in policies() {
            let s = steps(&p);
            let at: Vec<usize> = s.iter().map(|x| ORDER.iter().position(|o| o == x).unwrap()).collect();
            assert!(at.windows(2).all(|w| w[0] < w[1]), "{p:?}: {s:?}");
            assert_eq!(s.contains(&Step::Cloud), !p.locked && p.fetch && p.may_talk && p.others, "{p:?}");
            assert_eq!(s.contains(&Step::Library), !p.locked && p.others, "{p:?}");
            assert_eq!(s.contains(&Step::Solve), !p.locked && p.solve, "{p:?}");
            assert_eq!(s.contains(&Step::Local), !p.locked, "{p:?}");
        }
    }

    #[test]
    fn with_pokerstars_running_nothing_is_tried() {
        for p in policies().filter(|p| p.locked) {
            let mut tried = Vec::new();
            let found = resolve(&p, &InFlight::new(), "k", || true, |s| {
                tried.push(s);
                Some(())
            });
            assert!(found.is_none() && tried.is_empty(), "{p:?}");
        }
    }

    #[test]
    fn the_first_place_that_has_it_wins_and_nothing_after_it_runs() {
        let all = Policy { locked: false, fetch: true, may_talk: true, others: true, solve: true };
        for (i, has) in ORDER.iter().enumerate() {
            let mut tried = Vec::new();
            let found = resolve(&all, &InFlight::new(), "k", || true, |s| {
                tried.push(s);
                (s == *has).then_some(s)
            });
            assert_eq!(found, Some((*has, *has)));
            assert_eq!(tried, ORDER[..=i].to_vec(), "never solved when stored, never past the hit");
        }
    }

    /// Two areas need the same spot at once: it is solved once, the other takes the stored
    /// copy.
    #[test]
    fn a_spot_is_solved_once_when_two_areas_ask_together() {
        let stored: Arc<Mutex<Option<u32>>> = Arc::default();
        let solves = Arc::new(AtomicUsize::new(0));
        let flight = Arc::new(InFlight::new());
        let policy = Policy { solve: true, ..Policy::default() };
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let (stored, solves, flight) = (stored.clone(), solves.clone(), flight.clone());
                std::thread::spawn(move || {
                    resolve(&policy, &flight, "spot:x", || true, |s| match s {
                        Step::Local => *stored.lock().unwrap(),
                        Step::Solve => {
                            solves.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(Duration::from_millis(50));
                            *stored.lock().unwrap() = Some(7);
                            Some(7)
                        }
                        _ => None,
                    })
                })
            })
            .collect();
        let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(solves.load(Ordering::SeqCst), 1, "{results:?}");
        assert!(results.iter().all(|r| r.map(|(_, v)| v) == Some(7)));
    }
}
