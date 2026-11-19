//! Ringside Cloud in the app: shared push/fold solutions and reports of hands the
//! importer could not read.
//!
//! Nothing here talks to the network while the PokerStars client runs (the same lock as
//! the learning area), and no network failure ever blocks a caller: lookups give up
//! after a few seconds and then leave the cloud alone for a while; submissions run in
//! the background.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ringside_cloud::client::{DEFAULT_BASE_URL, MAX_STATUS_KEYS, RetryPolicy};
use ringside_cloud::kinds::pushfold_mw::{evaluated, reference, to_caller_units};
use ringside_cloud::kinds::spot::MAX_BYTES as SPOT_MAX_BYTES;
use ringside_cloud::kinds::{Kind, PushFoldMw, PushFoldMwInput, PushFoldMwSolution, SpotInput, SpotKind, SpotSolution};
use ringside_cloud::{Client, Error, KeyStatus, Report, SubmitStatus};
use ringside_engine::multiway::{self, Params, Solution as PushFoldSolution};
use ringside_solver::library::format::{Stored, encode};
use ringside_solver::saved::spot_key;
use ringside_solver::Solution;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::library::{ShareState, Shared};
use crate::settings::CloudSettings;
use crate::{AppState, CmdResult, err, store_settings, unlocked};

/// How long the cloud is left alone after it could not be reached.
const OFFLINE_FOR: Duration = Duration::from_secs(10 * 60);
/// Solutions kept on disk; the oldest go first.
const KEEP_CACHED: usize = 200;
/// Unreadable hands kept for a report since the start.
const KEEP_MALFORMED: usize = 20;
/// The server takes at most 20 000 characters per hand.
const MAX_HAND_CHARS: usize = 19_000;

#[derive(Default)]
pub struct CloudState {
    malformed: Mutex<VecDeque<Malformed>>,
    next_id: AtomicU64,
    offline_until: Mutex<Option<Instant>>,
}

struct Malformed {
    id: u64,
    reason: String,
    text: String,
}

impl CloudState {
    /// Called by the importer with (hand text, reason) of hands it could not read.
    pub fn add_malformed(&self, hands: Vec<(String, String)>) {
        let mut list = self.malformed.lock().unwrap();
        for (text, reason) in hands {
            if list.len() >= KEEP_MALFORMED {
                list.pop_front();
            }
            list.push_back(Malformed { id: self.next_id.fetch_add(1, Ordering::Relaxed), reason, text });
        }
    }

    fn reachable(&self) -> bool {
        self.offline_until.lock().unwrap().is_none_or(|t| Instant::now() >= t)
    }

    pub(crate) fn note(&self, e: &Error) {
        if matches!(e, Error::Network(_) | Error::RateLimited { .. }) {
            *self.offline_until.lock().unwrap() = Some(Instant::now() + OFFLINE_FOR);
        }
    }
}

fn cloud_settings_of(app: &AppHandle) -> CloudSettings {
    app.state::<AppState>().settings.lock().unwrap().cloud
}

/// What the cloud may be used for now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Allowed {
    /// A request may go out at all.
    pub talk: bool,
    /// Solutions may be fetched (`cloud.fetch`).
    pub fetch: bool,
    /// Own solutions may be offered (`cloud.share`).
    pub share: bool,
}

impl Allowed {
    /// Nothing before the user has seen what the cloud shares (the first-start notice),
    /// nothing while PokerStars runs, nothing while the cloud is down; else what the
    /// user switched on.
    fn of(settings: CloudSettings, pokerstars_running: bool, reachable: bool) -> Allowed {
        let talk = settings.notice_seen && !pokerstars_running && reachable;
        Allowed { talk, fetch: talk && settings.fetch, share: talk && settings.share }
    }
}

pub(crate) fn allowed(app: &AppHandle) -> Allowed {
    Allowed::of(cloud_settings_of(app), unlocked(&app.state::<AppState>()).is_err(), app.state::<CloudState>().reachable())
}

pub(crate) fn may_talk(app: &AppHandle) -> bool {
    allowed(app).talk
}

// Solutions.

/// A fresh solve to offer the cloud.
type Offer = (PushFoldMwInput, PushFoldMwSolution);

/// Push/fold Nash for one table of 2–9 players off the bundled tables: what
/// `ringside_learn::lookup_or_solve` (practice, the coach, the charts) solves through.
///
/// Blocking (seconds when it has to solve). Order:
/// 1. the solutions cached on this computer;
/// 2. with `cloud.fetch`, the cloud's solution, checked here again (its exploitability
///    is measured with the engine) and dropped if it fails;
/// 3. otherwise solved here and, with `cloud.share`, offered to the cloud in the
///    background.
pub(crate) fn pushfold(app: &AppHandle, params: &Params) -> Result<PushFoldSolution, String> {
    let Some(cache) = cache_dir(app) else {
        return multiway::solve(params).map_err(|e| e.to_string());
    };
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient());
    let (solution, offer) = pushfold_with(params, &cache, &client, allowed(app), |e| app.state::<CloudState>().note(e))?;
    if let Some(offer) = offer {
        let app = app.clone();
        std::thread::spawn(move || {
            // Asked again when it goes out: the user may have switched sharing off or
            // opened PokerStars meanwhile.
            let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(10), ..RetryPolicy::default() };
            if let Err(e) = offer_pushfold(&Client::with_policy(DEFAULT_BASE_URL, policy), allowed(&app), &offer) {
                eprintln!("cloud submit: {e}");
                app.state::<CloudState>().note(&e);
            }
        });
    }
    Ok(solution)
}

/// [`pushfold`] with the cache in `cache` and the cloud behind `client`; returns the
/// solution and, for a fresh solve the user shares, what to offer.
///
/// Amounts are rounded to milli-BB and ICM payouts are normalized for the cloud; values
/// come back in `params`' units. Params whose solver settings differ from the cloud's
/// ([`PushFoldMwInput::from_params`]) or that are outside its range (more than three
/// players all in, stacks above 100 BB, …) are solved here without cache or cloud.
fn pushfold_with(
    params: &Params,
    cache: &Path,
    client: &Client,
    allowed: Allowed,
    note: impl Fn(&Error),
) -> Result<(PushFoldSolution, Option<Offer>), String> {
    let input = PushFoldMwInput::from_params(params).and_then(|i| PushFoldMw::check_input(&i).map(|()| i));
    let Ok(input) = input else {
        return Ok((multiway::solve(params).map_err(|e| e.to_string())?, None));
    };
    let key = PushFoldMw::key(&input).map_err(|e| e.to_string())?;
    let path = cache.join(format!("{key}.json"));
    if let Some(cached) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<PushFoldSolution>(&b).ok()) {
        return Ok((to_caller_units(cached, params), None));
    }
    if allowed.fetch {
        match client.get_solution::<PushFoldMw>(&input) {
            // `get_solution` has already measured the download; measuring it once more
            // gives the full solution with EVs, still far cheaper than solving.
            Ok(Some(download)) => match evaluated(&input, &download) {
                Ok(solution) => {
                    store(cache, &path, &solution);
                    return Ok((to_caller_units(solution, params), None));
                }
                Err(e) => eprintln!("cloud lookup {key}: {e}"),
            },
            Ok(None) => {}
            Err(e) => {
                eprintln!("cloud lookup {key}: {e}");
                note(&e);
            }
        }
    }
    let solution = reference(&input)?;
    store(cache, &path, &solution);
    let offer = (allowed.share && solution.converged).then(|| (input, PushFoldMwSolution::from(&*solution)));
    Ok((to_caller_units((*solution).clone(), params), offer))
}

/// Sends `offer` if sharing is allowed now: only the calculation input and its result.
fn offer_pushfold(client: &Client, allowed: Allowed, (input, solution): &Offer) -> Result<(), Error> {
    if allowed.share {
        client.submit_solution::<PushFoldMw>(input, solution)?;
    }
    Ok(())
}

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = crate::data_dir(app)
        .ok()?
        .join("cloud")
        .join(format!("{}-{}", PushFoldMw::NAME, PushFoldMw::ENGINE_VERSION));
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn store(dir: &Path, path: &Path, solution: &PushFoldSolution) {
    let Ok(json) = serde_json::to_vec(solution) else { return };
    if std::fs::write(path, json).is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() > KEEP_CACHED {
        files.sort();
        for (_, old) in &files[..files.len() - KEEP_CACHED] {
            let _ = std::fs::remove_file(old);
        }
    }
}

// Shared spots: heads-up flops of the hand analysis and the practice table whose ranges
// are the preflop model's. Only the abstract spot leaves the computer (see
// `ringside_cloud::kinds::spot`); `spots.json` remembers what became of every upload, by
// the local spot key, for "Meine Lösungen".

const SPOTS_FILE: &str = "spots.json";
/// Read-modify-write of `spots.json` from several threads.
static SPOT_LOG: Mutex<()> = Mutex::new(());

/// An uploaded spot: its cloud key and where it stands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpotShared {
    pub key: String,
    #[serde(flatten)]
    pub shared: Shared,
}

/// By local spot key (`ringside_solver::saved::spot_key`).
pub type SpotLog = BTreeMap<String, SpotShared>;

fn spot_log_path(app: &AppHandle) -> Option<PathBuf> {
    let dir = crate::data_dir(app).ok()?.join("cloud");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(SPOTS_FILE))
}

pub(crate) fn spot_log(app: &AppHandle) -> SpotLog {
    spot_log_path(app)
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn update_spot_log(app: &AppHandle, f: impl FnOnce(&mut SpotLog)) {
    let _guard = SPOT_LOG.lock().unwrap();
    let Some(path) = spot_log_path(app) else { return };
    let mut log = spot_log(app);
    f(&mut log);
    if let Ok(bytes) = serde_json::to_vec(&log) {
        let tmp = path.with_extension("part");
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

fn compact(board: &str) -> String {
    board.chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// With `cloud.fetch`: the cloud's solution of `input` on its canonical flop, served as
/// accepted or verified, its form checked here (`Client::get_solution`).
pub(crate) fn download_spot(app: &AppHandle, input: &SpotInput) -> Option<Stored> {
    if !allowed(app).fetch {
        return None;
    }
    match Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient()).get_solution::<SpotKind>(input) {
        Ok(Some(found)) => found.stored(input).ok(),
        Ok(None) => None,
        Err(e) => {
            eprintln!("cloud spot lookup: {e}");
            app.state::<CloudState>().note(&e);
            None
        }
    }
}

/// Which of `keys` the cloud serves (accepted or verified); empty when it may not be asked.
pub(crate) fn served(app: &AppHandle, keys: &[String]) -> Vec<String> {
    if !allowed(app).fetch || keys.is_empty() {
        return Vec::new();
    }
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient());
    let mut out = Vec::new();
    for chunk in keys.chunks(MAX_STATUS_KEYS) {
        match client.statuses(chunk) {
            Ok(list) => out.extend(list.into_iter().filter(|(_, s)| matches!(s, KeyStatus::Accepted | KeyStatus::Verified)).map(|(k, _)| k)),
            Err(e) => {
                app.state::<CloudState>().note(&e);
                break;
            }
        }
    }
    out
}

/// The flops on which the line of `input` is served, in one request; `None` when the
/// cloud may not be asked, fails, or does not know the request yet (API before 1.5.0).
pub(crate) fn served_flops(app: &AppHandle, input: &SpotInput) -> Option<Vec<String>> {
    if !allowed(app).fetch {
        return None;
    }
    match Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient()).served_flops(input) {
        Ok(flops) => flops,
        Err(e) => {
            app.state::<CloudState>().note(&e);
            None
        }
    }
}

/// With `cloud.share`: asks the community queue to solve a spot this computer could not.
/// A refusal is only logged; it must not keep the cloud away for the other requests.
pub(crate) fn request_spot(app: &AppHandle, input: SpotInput) {
    if !allowed(app).share {
        return;
    }
    std::thread::spawn(move || {
        if let Err(e) = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient()).request_work::<SpotKind>(&input) {
            eprintln!("cloud request: {e}");
        }
    });
}

/// With `cloud.share`: offers a fresh local solve of `input`'s spot. The library format is
/// written here (it needs the solution), the upload runs in the background.
pub(crate) fn share_spot(app: &AppHandle, input: SpotInput, solution: &mut Solution) {
    if !allowed(app).share {
        return;
    }
    let Ok(local_key) = spot_key(&solution.spot) else { return };
    let board = compact(&solution.spot.board);
    let Ok(catalog) = input.catalog(Some(&board)) else { return };
    let stored = match encode(solution, &catalog, &catalog.scenarios[0], &board)
        .and_then(|s| s.renamed(&catalog.scenarios[0], &input.board))
    {
        Ok(s) => s,
        Err(e) => return eprintln!("cloud share: {e}"),
    };
    let app = app.clone();
    std::thread::spawn(move || {
        if !allowed(&app).share {
            return;
        }
        let key = SpotKind::key(&input).unwrap_or_default();
        let at = crate::now_secs().max(0) as u64;
        let bytes = stored.to_bytes();
        let shared = if bytes.len() > SPOT_MAX_BYTES {
            Shared { state: ShareState::Failed, at, note: Some("zu groß zum Teilen".into()) }
        } else {
            let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(20), ..RetryPolicy::default() };
            match Client::with_policy(DEFAULT_BASE_URL, policy).submit_solution::<SpotKind>(&input, &SpotSolution::from_stored(&stored)) {
                Ok(SubmitStatus::Submitted) => Shared { state: ShareState::Submitted, at, note: None },
                Ok(SubmitStatus::Accepted | SubmitStatus::AlreadyVerified) => Shared { state: ShareState::Served, at, note: None },
                Err(e) => {
                    app.state::<CloudState>().note(&e);
                    Shared { state: ShareState::Failed, at, note: Some(e.to_string()) }
                }
            }
        };
        update_spot_log(&app, |log| {
            log.insert(local_key, SpotShared { key, shared });
        });
    });
}

/// Asks the cloud about uploaded spots that wait for the verifier.
pub(crate) fn check_spots(app: &AppHandle) -> Result<(), Error> {
    let waiting: Vec<(String, String)> = spot_log(app)
        .into_iter()
        .filter(|(_, s)| s.shared.state == ShareState::Submitted)
        .map(|(local, s)| (local, s.key))
        .take(MAX_STATUS_KEYS)
        .collect();
    if waiting.is_empty() {
        return Ok(());
    }
    let keys: Vec<String> = waiting.iter().map(|(_, k)| k.clone()).collect();
    let client = Client::with_policy(DEFAULT_BASE_URL, RetryPolicy { attempts: 2, ..RetryPolicy::impatient() });
    let statuses = client.statuses(&keys)?;
    update_spot_log(app, |log| {
        for ((local, _), (_, status)) in waiting.iter().zip(statuses) {
            if let Some(entry) = log.get_mut(local) {
                entry.shared.state = match status {
                    KeyStatus::Accepted | KeyStatus::Verified => ShareState::Served,
                    KeyStatus::Submitted => ShareState::Submitted,
                    KeyStatus::Rejected(note) => {
                        entry.shared.note = note;
                        ShareState::Rejected
                    }
                    KeyStatus::Unknown => ShareState::Failed,
                };
            }
        }
    });
    Ok(())
}

// Commands: settings and the first-start notice.

#[tauri::command]
pub fn cloud_settings(state: State<'_, AppState>) -> CloudSettings {
    state.settings.lock().unwrap().cloud
}

/// Saves the switches; having seen them counts as having seen the notice.
#[tauri::command]
pub async fn cloud_save_settings(cloud: CloudSettings, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.cloud = CloudSettings { notice_seen: true, ..cloud };
    store_settings(&app, &state, settings)
}

#[tauri::command]
pub async fn cloud_ack_notice(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let mut settings = state.settings.lock().unwrap().clone();
    settings.cloud.notice_seen = true;
    store_settings(&app, &state, settings)
}

// Commands: reports of unreadable hands.

/// An unreadable hand as it would be sent: anonymized.
#[derive(Serialize)]
pub struct MalformedView {
    id: u64,
    reason: String,
    hand: String,
}

fn view(m: &Malformed) -> MalformedView {
    MalformedView { id: m.id, reason: m.reason.clone(), hand: anonymize(&m.text) }
}

#[tauri::command]
pub fn cloud_malformed(cloud: State<'_, CloudState>) -> Vec<MalformedView> {
    cloud.malformed.lock().unwrap().iter().rev().map(view).collect()
}

#[tauri::command]
pub fn cloud_dismiss_malformed(id: u64, cloud: State<'_, CloudState>) {
    cloud.malformed.lock().unwrap().retain(|m| m.id != id);
}

/// Sends one unreadable hand, anonymized exactly as `cloud_malformed` showed it; only
/// after the user confirmed it in the dialog.
#[tauri::command]
pub async fn cloud_send_report(id: u64, app: AppHandle) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        unlocked(&app.state::<AppState>())?;
        let cloud = app.state::<CloudState>();
        let shown = cloud.malformed.lock().unwrap().iter().find(|m| m.id == id).map(view);
        let shown = shown.ok_or("Diese Hand ist nicht mehr in der Liste.")?;
        let report = Report {
            app_version: env!("CARGO_PKG_VERSION").into(),
            error: shown.reason.chars().take(1_000).collect(),
            hand: shown.hand.chars().take(MAX_HAND_CHARS).collect(),
        };
        let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(10), ..RetryPolicy::default() };
        Client::with_policy(DEFAULT_BASE_URL, policy).report(&report).map_err(|e| match e {
            Error::RateLimited { .. } => "Gerade zu viele Meldungen – bitte später noch einmal.".to_string(),
            Error::Network(_) => "Ringside Cloud ist nicht erreichbar.".to_string(),
            e => err(e),
        })?;
        cloud.malformed.lock().unwrap().retain(|m| m.id != id);
        Ok(())
    })
    .await
    .map_err(err)?
}

// Anonymization.

/// Words after a seat's name in the summary and in the posts of the hand.
const AFTER_SEAT: [&str; 10] = [
    " (button)",
    " (small blind)",
    " (big blind)",
    " folded",
    " showed",
    " mucked",
    " collected",
    " is sitting out",
    " didn't",
    " (",
];
const ACTIONS: [&str; 12] = [
    "posts", "folds", "checks", "calls", "bets", "raises", "shows", "mucks", "doesn't", "is sitting out", "sits out",
    "has returned",
];
const EVENTS: [&str; 11] = [
    " collected ",
    " finished the tournament",
    " wins the tournament",
    " has timed out",
    " is disconnected",
    " is connected",
    " has returned",
    " leaves the table",
    " joins the table",
    " will be allowed to play",
    " said, \"",
];

/// The players' names in `hand`: the hero (who was dealt visible cards) and the others
/// in order of their first appearance.
fn names(hand: &str) -> (Option<String>, Vec<String>) {
    let mut hero = None;
    let mut names: Vec<String> = Vec::new();
    let mut add = |n: &str| {
        let n = n.trim();
        if !n.is_empty() && !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    };
    for line in hand.lines() {
        let line = line.trim_start_matches('\u{feff}').trim_end();
        if let Some(rest) = line.strip_prefix("Seat ").and_then(|r| r.split_once(": ").map(|(_, r)| r)) {
            // "Seat 1: Lee Ann: KO (9000 in chips, $10 bounty)": the name ends at the
            // last " (" before "in chips"; summary lines at the first marker.
            match rest.rfind(" (").filter(|&i| rest[i..].contains("in chips")) {
                Some(i) => add(&rest[..i]),
                None => {
                    if let Some(i) = AFTER_SEAT.iter().filter_map(|m| rest.find(m)).min() {
                        add(&rest[..i]);
                    }
                }
            }
        } else if let Some((name, _)) = line.strip_prefix("Dealt to ").and_then(|r| r.split_once(" [")) {
            hero = Some(name.trim().to_string());
            add(name);
        } else if let Some(name) = line.strip_prefix("Uncalled bet (").and_then(|r| r.split_once(") returned to ")) {
            add(name.1);
        } else if let Some(i) = line.rfind(": ").filter(|&i| ACTIONS.iter().any(|a| line[i + 2..].starts_with(a))) {
            add(&line[..i]);
        } else if let Some(i) = EVENTS.iter().filter_map(|e| line.find(e)).min() {
            add(&line[..i]);
        }
    }
    (hero, names)
}

/// Replaces `marker` + digits with `marker` + `with`.
fn replace_number(line: &str, marker: &str, with: &str) -> String {
    let Some(i) = line.find(marker) else { return line.to_string() };
    let start = i + marker.len();
    let end = start + line[start..].bytes().take_while(u8::is_ascii_digit).count();
    if end == start {
        return line.to_string();
    }
    format!("{}{with}{}", &line[..start], &line[end..])
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// `text` with every name (as a whole word, longest first) replaced by its label.
fn replace_names(text: &str, labels: &[(String, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'scan: while i < text.len() {
        let before = text[..i].chars().next_back();
        if !is_word(before) {
            for (name, label) in labels {
                if text[i..].starts_with(name.as_str()) && !is_word(text[i + name.len()..].chars().next()) {
                    out.push_str(label);
                    i += name.len();
                    continue 'scan;
                }
            }
        }
        let c = text[i..].chars().next().expect("i is on a char boundary");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// The hand as it may leave this computer: players become "Hero" and "Spieler 1…",
/// table, tournament, club and hand numbers are dropped, chat text is removed. Dates,
/// stakes, amounts, cards and the shape of every line stay, that is what a parser
/// bug report needs.
pub fn anonymize(hand: &str) -> String {
    let (hero, names) = names(hand);
    let mut others = 0;
    let mut labels: Vec<(String, String)> = names
        .into_iter()
        .map(|n| {
            let label = if hero.as_deref() == Some(n.as_str()) {
                "Hero".to_string()
            } else {
                others += 1;
                format!("Spieler {others}")
            };
            (n, label)
        })
        .collect();
    labels.sort_by_key(|(n, _)| std::cmp::Reverse(n.len()));
    hand.lines()
        .map(|line| {
            let line = line.trim_start_matches('\u{feff}').trim_end();
            let mut line = replace_number(line, "Hand #", "1");
            line = replace_number(&line, "Tournament #", "1");
            line = replace_number(&line, "Home Game #", "1");
            if let (Some(open), Some(close)) = (line.find(" {"), line.find("} "))
                && open < close
            {
                line = format!("{} {{Club}}{}", &line[..open], &line[close + 1..]);
            }
            if line.starts_with("Table '")
                && let Some(end) = line.rfind("' ")
            {
                line = format!("Table 'Tisch'{}", &line[end + 1..]);
            }
            if let Some(i) = line.find(" said, \"") {
                line = format!("{} said, \"…\"", &line[..i]);
            }
            replace_names(&line, &labels)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    use ringside_engine::preflop_table::ModelLine;
    use ringside_solver::Spot;

    /// The cloud input of a local flop spot, as the store files it.
    fn spot_input(model: &ModelLine, spot: &Spot) -> Option<SpotInput> {
        SpotInput::from_model(model, spot).ok()
    }

    const HAND: &str = "PokerStars Hand #260000000002: Tournament #4046789301, $10+$10+$2 USD Hold'em No Limit - Level IX (300/600) - 2026/11/16 21:00:00 CET\r
Table '4046789301 3' 9-max Seat #1 is the button\r
Seat 1: Lee Ann: KO (9000 in chips, $10 bounty)\r
Seat 2: Mo (9000 in chips, $10 bounty)\r
Seat 3: Moritz_K (1500 in chips, $10 bounty)\r
Lee Ann: KO: posts small blind 300\r
Mo: posts big blind 600\r
*** HOLE CARDS ***\r
Dealt to Moritz_K [Ah Kd]\r
Moritz_K: raises 900 to 1500 and is all-in\r
Lee Ann: KO said, \"call Mo, you are Mo Smith from Berlin\"\r
Lee Ann: KO: calls 1200\r
Mo: folds\r
Uncalled bet (300) returned to Moritz_K\r
Moritz_K collected 3300 from pot\r
Moritz_K wins $5 for eliminating Lee Ann: KO\r
Lee Ann: KO finished the tournament in 12th place\r
*** SUMMARY ***\r
Seat 1: Lee Ann: KO (small blind) showed [Qc Qd] and lost\r
Seat 2: Mo (big blind) folded before Flop\r
Seat 3: Moritz_K showed [Ah Kd] and won (3300)\r
";

    #[test]
    fn names_tables_tournaments_and_chat_are_removed() {
        let a = anonymize(HAND);
        for secret in ["Lee Ann", "KO", "Mo ", "Mo:", "Moritz", "4046789301", "260000000002", "Berlin", "Smith"] {
            assert!(!a.contains(secret), "{secret:?} left in:\n{a}");
        }
        assert!(a.starts_with("PokerStars Hand #1: Tournament #1, $10+$10+$2 USD Hold'em"), "{a}");
        assert!(a.contains("Table 'Tisch' 9-max Seat #1 is the button"));
        assert!(a.contains("Seat 1: Spieler 1 (9000 in chips, $10 bounty)"));
        assert!(a.contains("Dealt to Hero [Ah Kd]"));
        assert!(a.contains("Spieler 1: calls 1200"));
        assert!(a.contains("Hero wins $5 for eliminating Spieler 1"));
        assert!(a.contains("Spieler 1 said, \"…\""));
        assert!(!a.contains('\r'), "line endings do not matter");
    }

    /// A real-looking tournament hand: the button opens, the big blind calls, a c-bet.
    const POSTFLOP_HAND: &str = "PokerStars Hand #260000000077: Tournament #4046789301, $10+$10+$2 USD Hold'em No Limit - Level IV (100/200) - 2026/11/16 21:00:00 CET [2026/11/16 15:00:00 ET]
Table '4046789301 3' 6-max Seat #4 is the button
Seat 1: Lee Ann: KO (8000 in chips, $10 bounty)
Seat 2: Mo (8000 in chips, $10 bounty)
Seat 3: Smith_Berlin (8000 in chips, $10 bounty)
Seat 4: Moritz_K (8000 in chips, $10 bounty)
Seat 5: anna.k (8000 in chips, $10 bounty)
Seat 6: xX_shark_Xx (8000 in chips, $10 bounty)
Lee Ann: KO: posts the ante 25
Mo: posts the ante 25
Smith_Berlin: posts the ante 25
Moritz_K: posts the ante 25
anna.k: posts the ante 25
xX_shark_Xx: posts the ante 25
anna.k: posts small blind 100
xX_shark_Xx: posts big blind 200
*** HOLE CARDS ***
Dealt to Moritz_K [Ah Kh]
Lee Ann: KO: folds
Mo: folds
Smith_Berlin: folds
Moritz_K: raises 240 to 440
anna.k: folds
xX_shark_Xx: calls 240
*** FLOP *** [Kd 7s 2c]
xX_shark_Xx: checks
Moritz_K: bets 330
xX_shark_Xx: folds
Uncalled bet (330) returned to Moritz_K
Moritz_K collected 1130 from pot
*** SUMMARY ***
Total pot 1130 | Rake 0
Board [Kd 7s 2c]
";

    #[test]
    fn a_shared_spot_carries_nothing_of_the_hand() {
        let hand = ringside_core::parse_hand(POSTFLOP_HAND).unwrap();
        let analysis = ringside_analysis::analyze_hand(&hand);
        let (_, json) = analysis.job.expect("a heads-up flop to solve");
        let job: ringside_analysis::PostflopJob = serde_json::from_str(&json).unwrap();
        let input = spot_input(job.model.as_ref().expect("model line"), &job.spot).expect("model ranges are shareable");
        assert_eq!(input.board, "Ks7h2d", "the canonical flop, not the dealt suits");
        let body = ringside_cloud::kinds::submission::<SpotKind>(&input, &SpotSolution { data: "AAAA".into() });
        ringside_cloud::privacy::assert_anonymous(&body);
        let text = body.to_string();
        for secret in [
            "Lee Ann", "Mo\"", "Smith", "Berlin", "Moritz", "anna", "shark", "260000000077", "4046789301", "2026", "21:00", "$",
            "USD", "bounty", "Ah", "Kh", "Kd7s2c",
        ] {
            assert!(!text.contains(secret), "{secret:?} in the upload: {text}");
        }
        // The hero's hand is no part of it, and neither is the hero's seat by name.
        assert!(!text.contains("hero") && !text.contains("Hero"), "{text}");
    }

    #[test]
    fn practice_and_analysis_spots_are_both_shared_under_their_own_keys() {
        use ringside_engine::preflop_table::Family;
        // The hand analysis rates with the limps tables.
        let hand = ringside_core::parse_hand(POSTFLOP_HAND).unwrap();
        let (_, json) = ringside_analysis::analyze_hand(&hand).job.expect("a heads-up flop");
        let job: ringside_analysis::PostflopJob = serde_json::from_str(&json).unwrap();
        let analysed = job.model.clone().unwrap();
        assert_eq!(analysed.family, Family::Limps);
        let limps = spot_input(&analysed, &job.spot).expect("an analysed spot is shareable");

        // Practice plays the plain tables: the same table and line, as the pool solves it.
        let table = ringside_play::TableKey { players: analysed.players, ante: analysed.ante, stack_bb: analysed.stack_bb };
        let deep = ringside_play::table::deep(table).expect("plain table");
        let sc = deep.scenarios.iter().find(|s| s.line == analysed.line).expect("the line in the plain table");
        let practice = ModelLine { family: Family::Plain, ante: table.ante, players: table.players, stack_bb: table.stack_bb, line: sc.line.clone() };
        let plain = spot_input(&practice, &sc.spot("Kd7s2c")).expect("a practice spot is shareable");

        assert_eq!((plain.family.as_str(), limps.family.as_str()), ("plain", "limps"));
        assert_ne!(SpotKind::key(&plain).unwrap(), SpotKind::key(&limps).unwrap());
        // The limps ranges are not the plain ones: a spot never passes for the other family.
        let swapped = ModelLine { family: Family::Plain, ..analysed };
        assert!(spot_input(&swapped, &job.spot).is_none());
    }

    /// Ringside Cloud on a local port: serves `served` for any solution asked for (404
    /// without), accepts every submission and records each request with its body.
    struct FakeCloud {
        client: Client,
        seen: std::sync::Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    }

    impl FakeCloud {
        fn start(served: Option<PushFoldMwSolution>) -> FakeCloud {
            use std::io::{BufRead, BufReader, Read, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
            let log = seen.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { return };
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let (mut request, mut length) = (String::new(), 0);
                    reader.read_line(&mut request).unwrap();
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                            break;
                        }
                        if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                            length = v.trim().parse().unwrap();
                        }
                    }
                    let mut body = vec![0; length];
                    reader.read_exact(&mut body).unwrap();
                    let body: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
                    let request: Vec<&str> = request.split_whitespace().take(2).collect();
                    let (status, reply) = match request[0] {
                        "GET" => match &served {
                            Some(solution) => {
                                let key = request[1].rsplit('/').next().unwrap();
                                let input = serde_json::from_slice::<serde_json::Value>(&std::fs::read(input_file(key)).unwrap()).unwrap();
                                let r = serde_json::json!({"kind": PushFoldMw::NAME, "engine_version": PushFoldMw::ENGINE_VERSION, "key": key, "input": input, "result": solution});
                                (200, r.to_string())
                            }
                            None => (404, "{}".to_string()),
                        },
                        _ => {
                            let input: PushFoldMwInput = serde_json::from_value(body["input"].clone()).unwrap();
                            (200, serde_json::json!({"key": PushFoldMw::key(&input).unwrap(), "status": "accepted"}).to_string())
                        }
                    };
                    log.lock().unwrap().push((format!("{} {}", request[0], request[1]), body));
                    let _ = write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len());
                }
            });
            FakeCloud { client: Client::with_policy(&url, RetryPolicy::impatient()), seen }
        }

        fn requests(&self) -> Vec<String> {
            self.seen.lock().unwrap().iter().map(|(r, _)| r.split('/').take(3).collect::<Vec<_>>().join("/")).collect()
        }
    }

    /// Where the fake cloud finds the input of a key it serves.
    fn input_file(key: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ringside-fake-cloud-{}-{key}.json", std::process::id()))
    }

    /// Three players off the bundled grid, solved in well under a second.
    fn table() -> (Params, PushFoldMwInput) {
        let params = Params::new(vec![10.0, 12.0, 8.0]);
        let input = PushFoldMwInput::from_params(&params).unwrap();
        std::fs::write(input_file(&PushFoldMw::key(&input).unwrap()), serde_json::to_vec(&input).unwrap()).unwrap();
        (params, input)
    }

    fn cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringside-pushfold-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn switched(fetch: bool, share: bool) -> CloudSettings {
        CloudSettings { fetch, share, notice_seen: true, practice_ready: true }
    }

    const ON: Allowed = Allowed { talk: true, fetch: true, share: true };

    fn aces() -> usize {
        "AA".parse::<ringside_engine::HandClass>().unwrap().index()
    }

    #[test]
    fn a_served_pushfold_solution_is_used_and_kept() {
        let (params, input) = table();
        let good = PushFoldMwSolution::from(&*reference(&input).unwrap());
        let cloud = FakeCloud::start(Some(good.clone()));
        let dir = cache("hit");
        let (solution, offer) = pushfold_with(&params, &dir, &cloud.client, ON, |_| {}).unwrap();
        assert_eq!(cloud.requests(), ["GET /v1/solutions"]);
        assert!(offer.is_none(), "a download is not offered back");
        assert_eq!(PushFoldMwSolution::from(&solution), good);
        assert!(dir.join(format!("{}.json", PushFoldMw::key(&input).unwrap())).exists());

        pushfold_with(&params, &dir, &cloud.client, ON, |_| {}).unwrap();
        assert_eq!(cloud.requests().len(), 1, "the second time it comes from this computer");
    }

    #[test]
    fn a_pushfold_miss_is_solved_here_and_only_the_calculation_is_shared() {
        let (params, input) = table();
        let cloud = FakeCloud::start(None);
        let (solution, offer) = pushfold_with(&params, &cache("miss"), &cloud.client, ON, |_| {}).unwrap();
        assert_eq!(cloud.requests(), ["GET /v1/solutions"]);
        assert!(solution.converged);
        let offer = offer.expect("a fresh solve is offered");
        assert_eq!(offer.0, input);

        offer_pushfold(&cloud.client, ON, &offer).unwrap();
        assert_eq!(cloud.requests(), ["GET /v1/solutions", "POST /v1/solutions"]);
        let body = cloud.seen.lock().unwrap()[1].1.clone();
        ringside_cloud::privacy::assert_anonymous(&body);
        let mut fields: Vec<&String> = body.as_object().unwrap().keys().collect();
        fields.sort();
        assert_eq!(fields, ["engine_version", "input", "kind", "result"]);
        assert_eq!(body["input"], serde_json::to_value(&input).unwrap());
    }

    #[test]
    fn without_consent_no_pushfold_request_goes_out() {
        let (params, _) = table();
        let off = [
            Allowed::of(switched(false, false), false, true),
            Allowed::of(CloudSettings { notice_seen: false, ..switched(true, true) }, false, true),
            Allowed::of(switched(true, true), false, false),
        ];
        for allowed in off {
            let cloud = FakeCloud::start(None);
            let (_, offer) = pushfold_with(&params, &cache("off"), &cloud.client, allowed, |_| {}).unwrap();
            assert!(offer.is_none(), "{allowed:?}");
            assert!(cloud.requests().is_empty(), "{allowed:?}");
        }
        let fetch_only = Allowed::of(switched(true, false), false, true);
        let cloud = FakeCloud::start(None);
        let (_, offer) = pushfold_with(&params, &cache("fetch-only"), &cloud.client, fetch_only, |_| {}).unwrap();
        assert!(offer.is_none());
        assert_eq!(cloud.requests(), ["GET /v1/solutions"]);
    }

    #[test]
    fn while_pokerstars_runs_no_pushfold_request_goes_out() {
        let (params, input) = table();
        let locked = Allowed::of(switched(true, true), true, true);
        assert_eq!(locked, Allowed { talk: false, fetch: false, share: false });
        let cloud = FakeCloud::start(None);
        let (_, offer) = pushfold_with(&params, &cache("locked"), &cloud.client, locked, |_| {}).unwrap();
        assert!(offer.is_none());
        // PokerStars started between the solve and the upload.
        let solved = (input.clone(), PushFoldMwSolution::from(&*reference(&input).unwrap()));
        offer_pushfold(&cloud.client, locked, &solved).unwrap();
        assert!(cloud.requests().is_empty());
    }

    #[test]
    fn a_manipulated_pushfold_solution_is_rejected_and_solved_here() {
        let (params, input) = table();
        let mut bad = PushFoldMwSolution::from(&*reference(&input).unwrap());
        bad.act[0][aces()] = 0.0;
        let cloud = FakeCloud::start(Some(bad));
        let e = cloud.client.get_solution::<PushFoldMw>(&input).unwrap_err();
        assert!(e.to_string().contains("served solution fails the local check"), "{e}");

        let dir = cache("manipulated");
        let (solution, offer) = pushfold_with(&params, &dir, &cloud.client, ON, |_| {}).unwrap();
        assert_eq!(solution.nodes[0].act[aces()], 1.0, "the first seat shoves aces in the own solve");
        assert!(offer.is_some(), "the correct solution is offered instead");
        let kept: PushFoldSolution = serde_json::from_slice(&std::fs::read(dir.join(format!("{}.json", PushFoldMw::key(&input).unwrap()))).unwrap()).unwrap();
        assert_eq!(kept.nodes[0].act[aces()], 1.0, "the download is not kept");
    }

    #[test]
    fn names_only_match_whole_words() {
        let hand = "Seat 1: Al (100 in chips)\nSeat 2: Alan (100 in chips)\nAlan: raises to 100 and is all-in\nAl: calls 50";
        let a = anonymize(hand);
        assert!(a.contains("Spieler 2: raises to 100 and is all-in"), "{a}");
        assert!(a.contains("Spieler 1: calls 50"), "{a}");
        assert!(a.contains("is all-in"), "\"all\" in words is not a name");
    }

    #[test]
    fn hands_with_broken_seat_lines_still_lose_their_names() {
        let hand = "PokerStars Hand #1: Hold'em No Limit ($0.01/$0.02 USD)\nSeat 1 Alice 200 in chips\nAlice: folds\nBob: checks\nBob collected $0.03 from pot";
        let a = anonymize(hand);
        assert!(!a.contains("Alice") && !a.contains("Bob"), "{a}");
    }
}
