//! The Spielwertung for the app: computed from the stored analysis (see
//! [`ringside_core::skill`]) whenever it changed; a rating of decisions, so locked while
//! PokerStars runs.
//!
//! After an update of the analysis every hand is rated again (`analysis.rs`). Until that is
//! done the page gets no rating, so old and new ratings never mix; afterwards it names the
//! rating before, so the jump does not read as play.

use ringside_analysis::ENGINE;
use ringside_core::db::Db;
use ringside_core::session::SessionSpan;
use ringside_core::skill::{Change, SkillProfile, change, change_between, profile};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::memo::Memo;
use crate::{AppState, CmdResult, blocking, err};

static PROFILE: Memo<(u64, u64), SkillProfile> = Memo::new();

/// The last rating the page got, and with which analysis.
const SHOWN_KEY: &str = "skill_shown";

#[derive(Default, Serialize, Deserialize)]
struct Shown {
    engine: String,
    rating: Option<f64>,
    entries: usize,
    /// The rating before the last recalculation, until the next tournament counts.
    before: Option<f64>,
}

#[derive(Serialize)]
pub struct SkillView {
    #[serde(flatten)]
    pub profile: SkillProfile,
    /// Hands rated by an earlier analysis wait to be rated again.
    pub recalculating: bool,
    pub recalculated_from: Option<f64>,
}

#[tauri::command]
pub async fn skill_profile(app: AppHandle) -> CmdResult<SkillView> {
    blocking(app, skill_of).await
}

pub(crate) fn skill_of(state: &AppState) -> CmdResult<SkillView> {
    let recalculates = crate::analysis::recalculates(state);
    let db = state.db.lock().unwrap();
    let current = profile_of(&db)?;
    let mut shown: Shown = db.setting(SHOWN_KEY).map_err(err)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let remember = |shown: &Shown| db.set_setting(SHOWN_KEY, &serde_json::to_string(shown).unwrap_or_default()).map_err(err);
    if recalculates && db.outdated_count(ENGINE).map_err(err)? > 0 {
        if shown.rating.is_none() {
            // Older than this note: the rating before is what the stored analysis gives now.
            remember(&Shown { engine: String::new(), rating: current.rating, entries: current.entries.len(), before: None })?;
        }
        return Ok(SkillView { profile: profile(&[]), recalculating: true, recalculated_from: None });
    }
    if shown.engine != ENGINE {
        shown = Shown { engine: ENGINE.into(), rating: current.rating, entries: current.entries.len(), before: shown.rating };
        remember(&shown)?;
    } else if shown.entries != current.entries.len() || shown.rating != current.rating {
        let before = if shown.entries == current.entries.len() { shown.before } else { None };
        shown = Shown { engine: ENGINE.into(), rating: current.rating, entries: current.entries.len(), before };
        remember(&shown)?;
    }
    let recalculated_from = shown.before.filter(|b| current.rating.is_some_and(|r| (r - b).abs() >= 0.5));
    Ok(SkillView { profile: current, recalculating: false, recalculated_from })
}

fn profile_of(db: &Db) -> CmdResult<SkillProfile> {
    PROFILE.get_or(db.version(), || Ok(profile(&db.skill_moves().map_err(err)?)))
}

/// The change by the hands after `after` (0: every hand, no rating before); none while
/// hands of an earlier analysis are left, which would mix two scales.
pub fn change_after(db: &Db, after: u64) -> Option<Change> {
    if db.outdated_count(ENGINE).ok()? > 0 {
        return None;
    }
    let moves = db.skill_moves().ok()?;
    change(&moves, |m| m.hand_id <= after && after > 0)
}

/// The change over a session: what its hands added to the rating before it.
pub fn change_in(db: &Db, span: &SessionSpan) -> Option<Change> {
    if db.outdated_count(ENGINE).ok()? > 0 {
        return None;
    }
    // A tournament never reaches over a session's edge, so the history tells, unless a cash
    // day does.
    if !db.cash_on_days(&span.start, &span.end).ok()? {
        return change_between(&profile_of(db).ok()?, &span.start, &span.end);
    }
    let mut moves = db.skill_moves().ok()?;
    moves.retain(|m| m.played_at <= span.end);
    change(&moves, |m| m.played_at < span.start)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use ringside_analysis::analyze_again;

    use super::*;

    #[test]
    fn a_recalculation_shows_no_mixed_rating_and_names_the_rating_before() {
        let state = crate::solutions::analysed_state();
        let fresh = skill_of(&state).unwrap();
        assert!(!fresh.recalculating && fresh.recalculated_from.is_none());
        let want = fresh.profile.rating.expect("the fixtures make a rating");

        // The same hands as an earlier version rated them, harder on every loss.
        let state = crate::solutions::analysed_state();
        {
            let mut db = state.db.lock().unwrap();
            let ids: Vec<u64> = db.analysis_candidates("", 0, 1000).unwrap().into_iter().map(|(id, _)| id).collect();
            let earlier: Vec<_> = ids
                .iter()
                .map(|&id| {
                    let mut decisions = db.hand_decisions(id).unwrap();
                    for d in &mut decisions {
                        d.engine = "0.1.0+analysis.1".into();
                        d.ev_loss_bb = d.ev_loss_bb.map(|l| l * 2.0 + 0.5);
                    }
                    ringside_core::db::HandAnalysis { hand_id: id, engine: "0.1.0+analysis.1".into(), decisions, job: None }
                })
                .collect();
            db.save_analysis(&earlier).unwrap();
        }
        let before = profile(&state.db.lock().unwrap().skill_moves().unwrap()).rating.unwrap();
        assert!((before - want).abs() >= 0.5);

        let during = skill_of(&state).unwrap();
        assert!(during.recalculating);
        assert_eq!(during.profile.rating, None, "no rating of two analyses at once");

        {
            let mut db = state.db.lock().unwrap();
            loop {
                let rows = db.outdated_candidates(ENGINE, 0, 5).unwrap();
                if rows.is_empty() {
                    break;
                }
                let old: HashMap<_, _> = rows.iter().map(|(id, _)| (*id, db.hand_decisions(*id).unwrap())).collect();
                db.save_analysis(&analyze_again(&rows, &old)).unwrap();
            }
        }
        let after = skill_of(&state).unwrap();
        assert!(!after.recalculating);
        assert_eq!(after.profile.rating, Some(want), "the same as a fresh start");
        assert_eq!(after.recalculated_from, Some(before), "the jump is named, not shown as play");
        assert_eq!(skill_of(&state).unwrap().recalculated_from, Some(before), "until the next tournament");
    }
}
