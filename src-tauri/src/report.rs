//! The tournament report, computed locally from the stored analysis. Locked while
//! PokerStars runs, like every other piece of advice.

use ringside_analysis::ENGINE;
use ringside_core::db::Filter;
use ringside_core::report::Report;
use ringside_core::tournament::TournamentRow;
use serde::Serialize;
use tauri::AppHandle;

use crate::{CmdResult, blocking, err};

#[derive(Serialize)]
pub struct TournamentReport {
    tournament: TournamentRow,
    #[serde(flatten)]
    report: Report,
    /// A language-model review of the tournament is stored and can be opened.
    coach_review: bool,
}

#[tauri::command]
pub async fn tournament_report(tournament_id: u64, app: AppHandle) -> CmdResult<TournamentReport> {
    blocking(app, move |state| {
        let (tournament, mut report, coach_review) = {
            let db = state.db.lock().unwrap();
            let tournament = db
                .tournaments(&Filter::default())
                .map_err(err)?
                .into_iter()
                .find(|t| t.id == tournament_id)
                .ok_or("Turnier nicht gefunden.")?;
            let report = db.tournament_report(tournament_id, &tournament.started_at, ENGINE).map_err(err)?;
            let coach_review = db.coach_report(tournament_id).map_err(err)?.is_some();
            (tournament, report, coach_review)
        };
        crate::next::fill_report(state, &mut report)?;
        Ok(TournamentReport { tournament, report, coach_review })
    })
    .await
}
