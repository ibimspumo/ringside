//! The Spielwertung for the app: computed from the stored analysis on every request (see
//! [`ringside_core::skill`]); a rating of decisions, so locked while PokerStars runs.

use ringside_core::db::Db;
use ringside_core::session::SessionSpan;
use ringside_core::skill::{Change, SkillProfile, change, profile};
use tauri::AppHandle;

use crate::{CmdResult, blocking, err};

#[tauri::command]
pub async fn skill_profile(app: AppHandle) -> CmdResult<SkillProfile> {
    blocking(app, |state| {
        let moves = state.db.lock().unwrap().skill_moves().map_err(err)?;
        Ok(profile(&moves))
    })
    .await
}

/// The change by the hands after `after` (0: every hand, no rating before).
pub fn change_after(db: &Db, after: u64) -> Option<Change> {
    let moves = db.skill_moves().ok()?;
    change(&moves, |m| m.hand_id <= after && after > 0)
}

/// The change over a session: what its hands added to the rating before it.
pub fn change_in(db: &Db, span: &SessionSpan) -> Option<Change> {
    let mut moves = db.skill_moves().ok()?;
    moves.retain(|m| m.played_at <= span.end);
    change(&moves, |m| m.played_at < span.start)
}
