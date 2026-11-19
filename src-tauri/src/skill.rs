//! The Spielwertung for the app: computed from the stored analysis (see
//! [`ringside_core::skill`]) whenever it changed; a rating of decisions, so locked while
//! PokerStars runs.

use ringside_core::db::Db;
use ringside_core::session::SessionSpan;
use ringside_core::skill::{Change, SkillProfile, change, change_between, profile};
use tauri::AppHandle;

use crate::memo::Memo;
use crate::{AppState, CmdResult, blocking, err};

static PROFILE: Memo<(u64, u64), SkillProfile> = Memo::new();

#[tauri::command]
pub async fn skill_profile(app: AppHandle) -> CmdResult<SkillProfile> {
    blocking(app, skill_of).await
}

pub(crate) fn skill_of(state: &AppState) -> CmdResult<SkillProfile> {
    profile_of(&state.db.lock().unwrap())
}

fn profile_of(db: &Db) -> CmdResult<SkillProfile> {
    PROFILE.get_or(db.version(), || Ok(profile(&db.skill_moves().map_err(err)?)))
}

/// The change by the hands after `after` (0: every hand, no rating before).
pub fn change_after(db: &Db, after: u64) -> Option<Change> {
    let moves = db.skill_moves().ok()?;
    change(&moves, |m| m.hand_id <= after && after > 0)
}

/// The change over a session: what its hands added to the rating before it.
pub fn change_in(db: &Db, span: &SessionSpan) -> Option<Change> {
    // A tournament never reaches over a session's edge, so the history tells, unless a cash
    // day does.
    if !db.cash_on_days(&span.start, &span.end).ok()? {
        return change_between(&profile_of(db).ok()?, &span.start, &span.end);
    }
    let mut moves = db.skill_moves().ok()?;
    moves.retain(|m| m.played_at <= span.end);
    change(&moves, |m| m.played_at < span.start)
}
