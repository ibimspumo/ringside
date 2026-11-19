//! The tournament report and the session review, computed locally from the stored analysis.
//! The report is locked while PokerStars runs, like every other piece of advice; the
//! session review then keeps its results and leaves out everything that rates or advises.

use ringside_analysis::ENGINE;
use ringside_core::db::{Db, Filter};
use ringside_core::report::Report;
use ringside_core::session::{LISTED_SESSIONS, SessionReview, SessionSpan};
use ringside_core::tournament::TournamentRow;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{AppState, CmdResult, blocking, err, unlocked};

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
    blocking(app, move |state| report_page(state, tournament_id)).await
}

pub(crate) fn report_page(state: &AppState, tournament_id: u64) -> CmdResult<TournamentReport> {
    {
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
    }
}

#[derive(Serialize)]
pub struct SessionPage {
    /// The latest sessions, newest first, for choosing an earlier one.
    sessions: Vec<SessionSpan>,
    /// The chosen session; `None` without any hands.
    review: Option<SessionReview>,
    /// The tournaments of the chosen session, in order of their first hand.
    tournaments: Vec<TournamentRow>,
    /// How far the session moved the Spielwertung; `None` while locked.
    skill: Option<ringside_core::skill::Change>,
}

/// The session starting at `start`, or the latest one; reduced to its results while `locked`.
pub(crate) fn session_page(db: &Db, start: Option<&str>, locked: bool) -> Result<SessionPage, String> {
    let sessions = db.session_spans(LISTED_SESSIONS).map_err(err)?;
    let span = match start {
        Some(s) => sessions.iter().find(|x| x.start == s).or(sessions.first()),
        None => sessions.first(),
    };
    let Some(span) = span else { return Ok(SessionPage { sessions, review: None, tournaments: Vec::new(), skill: None }) };
    let review = db.session_review(span, ENGINE).map_err(err)?;
    let review = if locked { review.locked() } else { review };
    let mut all = db.tournaments(&Filter::default()).map_err(err)?;
    all.retain(|t| span.tournaments.contains(&t.id));
    all.sort_by_key(|t| span.tournaments.iter().position(|x| *x == t.id));
    let skill = if locked { None } else { crate::skill::change_in(db, span) };
    Ok(SessionPage { review: Some(review), tournaments: all, sessions, skill })
}

#[tauri::command]
pub async fn session_review(start: Option<String>, app: AppHandle) -> CmdResult<SessionPage> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let locked = unlocked(&state).is_err();
        let db = state.db.lock().unwrap();
        session_page(&db, start.as_deref(), locked)
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringside_core::db::{Choice, Decision, HandAnalysis, Rating, Source, SpotType, Status};
    use ringside_core::model::{Hand, Street};
    use ringside_core::{parse_hand, split_hands};

    const KNOCKOUT: &str = include_str!("../../crates/ringside-core/tests/fixtures/knockout.txt");

    #[test]
    fn while_pokerstars_runs_the_review_keeps_results_only() {
        let mut db = Db::open_in_memory().unwrap();
        let text = split_hands(KNOCKOUT)[0].to_string();
        let hand: Hand = parse_hand(&text).unwrap();
        let id = hand.id;
        db.insert_hands(&[(hand, text)]).unwrap();
        let decision = Decision {
            hand_id: id,
            action_index: 8,
            street: Street::Preflop,
            spot: SpotType::CallShove,
            status: Status::Rated,
            played: "Call".into(),
            best: Some("Fold".into()),
            strategy: vec![
                Choice { label: "Fold".into(), freq: 1.0, ev_bb: Some(0.0), played: false, kind: "fold".into() },
                Choice { label: "Call".into(), freq: 0.0, ev_bb: Some(-3.0), played: true, kind: "call".into() },
            ],
            played_freq: Some(0.0),
            ev_loss_bb: Some(3.0),
            rating: Some(Rating::Blunder),
            source: Some(Source::Equilibrium),
            why: "Fold wäre besser gewesen".into(),
            note: String::new(),
            reason: None,
            solver: None,
            engine: ENGINE.into(),
            pot_bb: Some(24.0),
            reliability: None,
        };
        db.save_analysis(&[HandAnalysis { hand_id: id, engine: ENGINE.into(), decisions: vec![decision], job: None }]).unwrap();

        let open = session_page(&db, None, false).unwrap();
        let r = open.review.as_ref().unwrap();
        assert!(!r.report.worst.is_empty() && !r.good_luck.is_empty() && r.report.score.decisions == 1);
        assert!(open.skill.as_ref().is_some_and(|s| s.delta.is_none()), "the first session starts the rating");

        let page = session_page(&db, None, true).unwrap();
        let r = page.review.as_ref().unwrap();
        assert!(r.locked && page.skill.is_none(), "no rating while PokerStars runs");
        assert_eq!(r.report.hands, 1, "the hand counter stays");
        let json = serde_json::to_string(&page).unwrap();
        for advice in ["Blunder", "blunder", "Fold wäre besser", "\"best\":\"Fold\"", "model_pct\":1", "rating\":"] {
            assert!(!json.contains(advice), "{advice} leaks while locked: {json}");
        }
    }
}
