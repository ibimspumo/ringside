//! "Deine Ranges": the hero's real preflop frequencies per hand class, with the model's
//! range of the same situation next to them. The hero's own numbers are always there; the
//! model comparison is advice and stays locked while the PokerStars client runs.

use ringside_analysis::ranges::{Comparison, ModelRange, Situation, compare, model_range};
use ringside_core::db::{Filter, RangeCounts, RangeFilter, RangeGroup};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{AppState, CmdResult, err, locked};

#[derive(Serialize)]
pub struct RangeChart {
    counts: RangeCounts,
    model: Option<ModelRange>,
    comparison: Option<Comparison>,
    /// Why there is no model range: locked, no position chosen, not covered …
    model_missing: Option<String>,
    locked: bool,
}

/// Every filterable combination of the hero's preflop decisions with its count.
#[tauri::command]
pub async fn range_groups(filter: Filter, app: AppHandle) -> CmdResult<Vec<RangeGroup>> {
    tauri::async_runtime::spawn_blocking(move || app.state::<AppState>().db.lock().unwrap().range_groups(&filter).map_err(err))
        .await
        .map_err(err)?
}

#[tauri::command]
pub async fn range_chart(filter: Filter, range: RangeFilter, app: AppHandle) -> CmdResult<RangeChart> {
    tauri::async_runtime::spawn_blocking(move || chart(&app.state::<AppState>(), &filter, &range)).await.map_err(err)?
}

fn chart(state: &AppState, filter: &Filter, range: &RangeFilter) -> CmdResult<RangeChart> {
    let counts = state.db.lock().unwrap().range_counts(filter, range).map_err(err)?;
    let locked = locked(state);
    let model = match (&range.position, locked) {
        (_, true) => Err("Der Vergleich ist gesperrt, solange PokerStars läuft.".to_string()),
        (None, _) => Err("Wähle eine Position, dann steht das Modell daneben.".to_string()),
        (Some(position), _) => model_range(&Situation { position, history: range.history, counts: &counts }),
    };
    let comparison = model.as_ref().ok().map(|m| compare(&counts, m));
    let (model, model_missing) = match model {
        Ok(m) => (Some(m), None),
        Err(e) => (None, Some(e)),
    };
    Ok(RangeChart { counts, model, comparison, model_missing, locked })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::solutions::analysed_state;

    #[test]
    fn the_model_comparison_is_locked_while_pokerstars_runs() {
        let state = analysed_state();
        let groups = state.db.lock().unwrap().range_groups(&Filter::default()).unwrap();
        let (range, open) = groups
            .iter()
            .map(|g| RangeFilter { position: Some(g.position.clone()), history: Some(g.history), ..Default::default() })
            .map(|range| (chart(&state, &Filter::default(), &range).unwrap(), range))
            .find_map(|(c, range)| c.model.is_some().then_some((range, c)))
            .expect("a fixture situation with a model range");
        assert!(open.comparison.is_some());
        state.client_running.store(true, Ordering::Relaxed);
        let shut = chart(&state, &Filter::default(), &range).unwrap();
        assert!(shut.locked && shut.model.is_none() && shut.comparison.is_none());
        assert_eq!(shut.counts, open.counts, "own frequencies stay");
    }
}
