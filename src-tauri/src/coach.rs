//! The AI coach in the app: the OpenRouter key in the OS vault, the model choice, and
//! preparing, running and storing analyses. Analyses and stored reports stay locked
//! while the PokerStars client runs, like the learning area; a run in progress stops
//! as soon as the client starts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ringside_coach::openrouter::{BASE_URL, DEFAULT_MODEL};
use ringside_coach::session::MAX_HANDS;
use ringside_coach::{
    ApiError, Client, CoachError, ConceptCount, Effort, Estimate, KeyInfo, ModelInfo, Prepared, Report, ReportInfo,
    concept_counts, prepare, run,
};
use ringside_core::db::Filter;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult, blocking, err, now_secs};

const VAULT_SERVICE: &str = "Ringside";
const VAULT_ACCOUNT: &str = "openrouter";
const SETTINGS_KEY: &str = "coach";
const MODELS_FRESH: Duration = Duration::from_secs(600);

#[derive(Default)]
pub struct CoachState {
    /// The last estimate shown; a run needs its token.
    pending: Mutex<Option<Prepared>>,
    cancel: Arc<AtomicBool>,
    running: AtomicBool,
    models: Mutex<Option<(Instant, Vec<ModelInfo>)>>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct CoachSettings {
    model: String,
    effort: Effort,
}

impl Default for CoachSettings {
    fn default() -> Self {
        CoachSettings { model: DEFAULT_MODEL.into(), effort: Effort::default() }
    }
}

fn settings(state: &AppState) -> CoachSettings {
    let json = state.db.lock().unwrap().setting(SETTINGS_KEY).ok().flatten();
    json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

fn save_settings(state: &AppState, change: impl FnOnce(&mut CoachSettings)) -> CmdResult<()> {
    let mut s = settings(state);
    change(&mut s);
    let json = serde_json::to_string(&s).map_err(err)?;
    state.db.lock().unwrap().set_setting(SETTINGS_KEY, &json).map_err(err)
}

fn vault() -> CmdResult<keyring::Entry> {
    keyring::Entry::new(VAULT_SERVICE, VAULT_ACCOUNT).map_err(vault_error)
}

fn vault_error(e: keyring::Error) -> String {
    format!("Der Schlüsselspeicher des Systems ist nicht erreichbar ({e}).")
}

fn stored_key() -> CmdResult<Option<String>> {
    match vault()?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(vault_error(e)),
    }
}

fn required_key() -> CmdResult<String> {
    stored_key()?.ok_or_else(|| "Hinterleg zuerst in den Einstellungen einen OpenRouter-Key.".into())
}

/// The model list, at most ten minutes old.
fn models(app: &AppHandle) -> CmdResult<Vec<ModelInfo>> {
    let coach = app.state::<CoachState>();
    if let Some((at, list)) = coach.models.lock().unwrap().as_ref()
        && at.elapsed() < MODELS_FRESH
    {
        return Ok(list.clone());
    }
    let list = Client::new(BASE_URL, None).models().map_err(err)?;
    *coach.models.lock().unwrap() = Some((Instant::now(), list.clone()));
    Ok(list)
}

/// Runs `f` on a worker thread without the client lock: key and model settings are no
/// analysis material.
async fn off_thread<T: Send + 'static>(f: impl FnOnce() -> CmdResult<T> + Send + 'static) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

#[derive(Serialize)]
pub struct CoachStatus {
    has_key: bool,
    model: String,
    effort: Effort,
    running: bool,
}

#[tauri::command]
pub async fn coach_status(app: AppHandle) -> CmdResult<CoachStatus> {
    off_thread(move || {
        let CoachSettings { model, effort } = settings(&app.state::<AppState>());
        Ok(CoachStatus {
            has_key: stored_key()?.is_some(),
            model,
            effort,
            running: app.state::<CoachState>().running.load(Ordering::Relaxed),
        })
    })
    .await
}

/// Stores the key in the OS vault and checks it with OpenRouter.
#[tauri::command]
pub async fn coach_set_key(key: String) -> CmdResult<KeyInfo> {
    off_thread(move || {
        let key = key.trim();
        if key.is_empty() || key.len() > 400 || key.chars().any(char::is_whitespace) {
            return Err("Das sieht nicht nach einem OpenRouter-Key aus.".into());
        }
        vault()?.set_password(key).map_err(vault_error)?;
        Client::new(BASE_URL, Some(key.to_string())).key_info().map_err(err)
    })
    .await
}

#[tauri::command]
pub async fn coach_test_key() -> CmdResult<KeyInfo> {
    off_thread(|| Client::new(BASE_URL, Some(required_key()?)).key_info().map_err(err)).await
}

#[tauri::command]
pub async fn coach_delete_key() -> CmdResult<()> {
    off_thread(|| match vault()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(vault_error(e)),
    })
    .await
}

#[tauri::command]
pub async fn coach_models(app: AppHandle) -> CmdResult<Vec<ModelInfo>> {
    off_thread(move || models(&app)).await
}

#[tauri::command]
pub async fn coach_set_model(model: String, app: AppHandle) -> CmdResult<()> {
    let model = model.trim().to_string();
    if model.is_empty() {
        return Err("Kein Modell gewählt.".into());
    }
    save_settings(&app.state::<AppState>(), |s| s.model = model)
}

#[tauri::command]
pub async fn coach_set_effort(effort: Effort, app: AppHandle) -> CmdResult<()> {
    save_settings(&app.state::<AppState>(), |s| s.effort = effort)
}

/// Picks the hands of a tournament and estimates the cost. Nothing is sent yet.
#[tauri::command]
pub async fn coach_prepare(tournament_id: u64, app: AppHandle) -> CmdResult<Estimate> {
    let handle = app.clone();
    blocking(app, move |state| {
        required_key()?;
        let CoachSettings { model, effort } = settings(state);
        let (row, raws) = {
            let db = state.db.lock().unwrap();
            let row = db.tournaments(&Filter::default()).map_err(err)?.into_iter().find(|t| t.id == tournament_id);
            (row.ok_or("Turnier nicht gefunden.")?, db.tournament_raw_hands(tournament_id).map_err(err)?)
        };
        let hands = raws.iter().filter_map(|r| ringside_core::parse_hand(r).ok()).collect();
        // Without the model list the estimate has no price; the run then has no cost limit.
        let list = models(&handle).unwrap_or_default();
        let info = list.iter().find(|m| m.id == model);
        let prepared = prepare(&row, hands, &model, info, MAX_HANDS, effort).map_err(err)?;
        let estimate = prepared.estimate().clone();
        *handle.state::<CoachState>().pending.lock().unwrap() = Some(prepared);
        Ok(estimate)
    })
    .await
}

/// Sends the prepared analysis whose estimate carried `token`, reports progress as
/// `coach-progress` events and stores the report.
#[tauri::command]
pub async fn coach_run(token: String, app: AppHandle) -> CmdResult<Report> {
    let handle = app.clone();
    blocking(app, move |state| {
        let coach = handle.state::<CoachState>();
        if coach.running.swap(true, Ordering::SeqCst) {
            return Err("Es läuft schon eine Analyse.".into());
        }
        let result = (|| {
            let prepared = coach.pending.lock().unwrap().take().ok_or(CoachError::NotConfirmed).map_err(err)?;
            let confirmed = prepared.confirm(&token).map_err(err)?;
            let client = Client::new(BASE_URL, Some(required_key()?));
            coach.cancel.store(false, Ordering::SeqCst);
            let cancel = coach.cancel.clone();
            let stop = || cancel.load(Ordering::Relaxed) || state.client_running.load(Ordering::Relaxed);
            let mut progress = |p: &ringside_coach::Progress| {
                let _ = handle.emit("coach-progress", p);
            };
            match run(confirmed, &client, now_secs(), &mut progress, &stop) {
                Ok(report) => {
                    let json = serde_json::to_string(&report).map_err(err)?;
                    let db = state.db.lock().unwrap();
                    db.save_coach_report(report.tournament_id, report.meta.created_at, &report.meta.model, &json)
                        .map_err(err)?;
                    Ok(report)
                }
                Err(CoachError::Api(ApiError::Cancelled)) if state.client_running.load(Ordering::Relaxed) => {
                    Err("Abgebrochen, weil PokerStars gestartet wurde.".to_string())
                }
                Err(e) => Err(e.to_string()),
            }
        })();
        coach.running.store(false, Ordering::SeqCst);
        result
    })
    .await
}

#[tauri::command]
pub fn coach_cancel(app: AppHandle) {
    app.state::<CoachState>().cancel.store(true, Ordering::SeqCst);
}

fn stored_reports(state: &AppState) -> CmdResult<Vec<Report>> {
    let rows = state.db.lock().unwrap().coach_reports().map_err(err)?;
    Ok(rows.iter().filter_map(|j| serde_json::from_str(j).ok()).collect())
}

#[derive(Serialize)]
pub struct ReportListItem {
    #[serde(flatten)]
    info: ReportInfo,
    name: Option<String>,
    game: String,
    started_at: String,
}

#[tauri::command]
pub async fn coach_reports(app: AppHandle) -> CmdResult<Vec<ReportListItem>> {
    blocking(app, |state| {
        let reports = stored_reports(state)?;
        let rows = state.db.lock().unwrap().tournaments(&Filter::default()).map_err(err)?;
        Ok(reports
            .iter()
            .map(|r| {
                let row = rows.iter().find(|t| t.id == r.tournament_id);
                ReportListItem {
                    info: r.info(),
                    name: row.and_then(|t| t.name.clone()),
                    game: row.map(|t| t.game.clone()).unwrap_or_default(),
                    started_at: row.map(|t| t.started_at.clone()).unwrap_or_default(),
                }
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn coach_report(tournament_id: u64, app: AppHandle) -> CmdResult<Option<Report>> {
    blocking(app, move |state| {
        let json = state.db.lock().unwrap().coach_report(tournament_id).map_err(err)?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    })
    .await
}

#[tauri::command]
pub async fn coach_delete_report(tournament_id: u64, app: AppHandle) -> CmdResult<bool> {
    blocking(app, move |state| state.db.lock().unwrap().delete_coach_report(tournament_id).map_err(err)).await
}

/// Concepts from all stored reports, weakest first, for "Deine Schwachstellen".
#[tauri::command]
pub async fn coach_concepts(app: AppHandle) -> CmdResult<Vec<ConceptCount>> {
    blocking(app, |state| Ok(concept_counts(&stored_reports(state)?))).await
}
