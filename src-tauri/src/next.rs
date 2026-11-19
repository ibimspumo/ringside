//! „Was als Nächstes?“ for every page: reads each source from the database and ranks them
//! with [`ringside_core::next::rank`]. Advice, so locked while the client runs.

use std::collections::HashSet;

use ringside_coach::session::concept_counts;
use ringside_core::db::Filter;
use ringside_core::leaks;
use ringside_core::next::{Concept, Inputs, NextStep, Review, kind_losses, rank, within};
use ringside_core::profile::profile;
use ringside_core::report::Report;
use ringside_learn::Kind;
use ringside_learn::mastery::{days_since, is_due, next_level};
use tauri::AppHandle;

use crate::memo::Memo;
use crate::{AppState, CmdResult, blocking, err, now_secs};

/// Single decisions the list looks at.
const COSTLIEST: usize = 5;

type StepsKey = ((u64, u64), String, i64);
static STEPS: Memo<StepsKey, Vec<NextStep>> = Memo::new();

/// The list for `filter`. Due reviews follow the clock, so a stored list counts for a minute.
pub fn steps(state: &AppState, filter: &Filter) -> CmdResult<Vec<NextStep>> {
    let version = state.db.lock().unwrap().version();
    STEPS.get_or((version, format!("{filter:?}"), now_secs() / 60), || compute(state, filter))
}

fn compute(state: &AppState, filter: &Filter) -> CmdResult<Vec<NextStep>> {
    let (rows, practiced, cells, costliest, attempts) = {
        let db = state.db.lock().unwrap();
        (
            db.profile_rows(filter).map_err(err)?,
            db.spot_practice().map_err(err)?,
            crate::hero_cells(&db, filter)?,
            db.costliest(filter, COSTLIEST).map_err(err)?,
            db.attempts().map_err(err)?,
        )
    };
    let now = now_secs();
    let progress = ringside_learn::progress(&attempts, now);
    let skill = |k: Kind| progress.skills.iter().find(|s| s.id == k);
    let concepts: Vec<Concept> = concept_counts(&crate::coach::stored_reports(state)?)
        .into_iter()
        .map(|c| Concept {
            id: c.id.into(),
            name: c.name.into(),
            exercise: c
                .exercise
                .and_then(Kind::parse)
                .filter(|k| k.generated())
                .map(|k| (k.id().to_string(), skill(k).map_or(1, next_level))),
            article: c.articles.first().copied(),
            mistakes: c.mistakes,
            questionable: c.questionable,
            tournaments: c.tournaments,
        })
        .collect();
    let reviews: Vec<Review> = progress
        .skills
        .iter()
        .filter(|s| is_due(s, now))
        .map(|s| Review {
            kind: s.id.id().into(),
            name: s.name_de.clone(),
            level: next_level(s),
            days: days_since(s, now).unwrap_or(0.0),
            mean_score: s.mean_score,
        })
        .collect();
    Ok(rank(&Inputs {
        profile: &profile(&rows, &practiced),
        leaks: &leaks::find(&cells),
        costliest: &costliest,
        concepts: &concepts,
        reviews: &reviews,
    }))
}

/// The report's „Was als Nächstes?“: the list of the whole history, filtered to the tournament.
pub fn fill_report(state: &AppState, report: &mut Report) -> CmdResult<()> {
    let list = steps(state, &Filter::default())?;
    let hands: HashSet<u64> = report.timeline.iter().map(|p| p.hand_id).collect();
    report.next = within(&list, &report.kinds, &hands, "in diesem Turnier");
    Ok(())
}

/// The list for `filter`; with `part` (a session: `since` and `until`) only what happened in it,
/// in the same order, as the session's review shows it.
#[tauri::command]
pub async fn next_steps(filter: Filter, part: Option<Filter>, app: AppHandle) -> CmdResult<Vec<NextStep>> {
    blocking(app, move |state| {
        let list = steps(state, &filter)?;
        let Some(part) = part else { return Ok(list) };
        let rows = state.db.lock().unwrap().profile_rows(&part).map_err(err)?;
        let hands: HashSet<u64> = rows.iter().map(|r| r.hand_id).collect();
        Ok(within(&list, &kind_losses(&profile(&rows, &[])), &hands, "in dieser Session"))
    })
    .await
}
