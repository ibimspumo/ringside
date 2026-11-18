//! Which flops of a line are solved already, so practice deals one of them instead of
//! solving: the places to look follow [`steps`], so the same rules hold as for a lookup.
//! - PokerStars runs: nothing is looked at.
//! - "Üben mit fertigen Spots anderer" off (`others` false): only spots solved on this
//!   computer; the library and the cloud are never asked.
//! - The cloud only with `cloud.fetch` while it may be asked.

use crate::order::{Policy, Step, steps};

/// The flops of a line that are ready, as canonical flops, without repeats. `local` gets
/// whether spots others solved count (library packs, kept downloads) and answers from the
/// index on this computer; `cloud` asks Ringside Cloud.
pub fn ready_flops(policy: &Policy, local: impl FnOnce(bool) -> Vec<String>, cloud: impl FnOnce() -> Vec<String>) -> Vec<String> {
    let places = steps(policy);
    let mut out = Vec::new();
    if places.contains(&Step::Local) {
        out = local(places.contains(&Step::Library));
    }
    if places.contains(&Step::Cloud) {
        for f in cloud() {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn policies() -> impl Iterator<Item = Policy> {
        (0..32u8).map(|b| Policy { locked: b & 1 != 0, fetch: b & 2 != 0, may_talk: b & 4 != 0, others: b & 8 != 0, solve: b & 16 != 0 })
    }

    /// What each policy asks: (local asked, with others, cloud asked) and the flops.
    fn ask(p: &Policy) -> (Option<bool>, bool, Vec<String>) {
        let (local, cloud) = (Cell::new(None), Cell::new(false));
        let flops = ready_flops(
            p,
            |others| {
                local.set(Some(others));
                if others { vec!["AsKs2h".into(), "7s6h2d".into()] } else { vec!["AsKs2h".into()] }
            },
            || {
                cloud.set(true);
                vec!["7s6h2d".into(), "QsJh4d".into()]
            },
        );
        (local.get(), cloud.get(), flops)
    }

    #[test]
    fn with_pokerstars_running_nothing_is_looked_at() {
        for p in policies().filter(|p| p.locked) {
            assert_eq!(ask(&p), (None, false, Vec::new()), "{p:?}");
        }
    }

    #[test]
    fn only_own_spots_never_asks_the_cloud_or_counts_others() {
        for p in policies().filter(|p| !p.locked && !p.others) {
            assert_eq!(ask(&p), (Some(false), false, vec!["AsKs2h".to_string()]), "{p:?}");
        }
    }

    #[test]
    fn with_others_the_cloud_adds_its_flops_when_it_may_be_asked() {
        for p in policies().filter(|p| !p.locked && p.others) {
            let (local, cloud, flops) = ask(&p);
            assert_eq!(local, Some(true), "{p:?}");
            assert_eq!(cloud, p.fetch && p.may_talk, "{p:?}");
            let mut want = vec!["AsKs2h", "7s6h2d"];
            if cloud {
                want.push("QsJh4d");
            }
            assert_eq!(flops, want, "{p:?}: no repeats");
        }
    }
}
