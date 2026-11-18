//! "Deine Ranges": the hero's real preflop frequencies per hand class, with the model's
//! range of the same situation next to them. The hero's own numbers are always there; the
//! model comparison is advice and stays locked while the PokerStars client runs.

use std::sync::atomic::Ordering;

use ringside_analysis::ranges::{Comparison, ModelRange, Situation, compare, model_range};
use ringside_core::db::{Filter, RangeCounts, RangeFilter, RangeGroup};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{AppState, CmdResult, err};

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
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let counts = state.db.lock().unwrap().range_counts(&filter, &range).map_err(err)?;
        let locked = state.client_running.load(Ordering::Relaxed);
        let model = match (&range.position, locked) {
            (_, true) => Err("Der Vergleich mit dem Modell ist gesperrt, solange PokerStars läuft.".to_string()),
            (None, _) => Err("Wähle eine Position, dann steht das Modell daneben.".to_string()),
            (Some(position), _) => model_range(&Situation { position, history: range.history, counts: &counts }),
        };
        let comparison = model.as_ref().ok().map(|m| compare(&counts, m));
        let (model, model_missing) = match model {
            Ok(m) => (Some(m), None),
            Err(e) => (None, Some(e)),
        };
        Ok(RangeChart { counts, model, comparison, model_missing, locked })
    })
    .await
    .map_err(err)?
}
