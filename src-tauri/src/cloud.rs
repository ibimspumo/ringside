//! Ringside Cloud in the app: shared push/fold solutions and reports of hands the
//! importer could not read.
//!
//! Nothing here talks to the network while the PokerStars client runs (the same lock as
//! the learning area), and no network failure ever blocks a caller: lookups give up
//! after a few seconds and then leave the cloud alone for a while; submissions run in
//! the background.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use ringside_cloud::client::{DEFAULT_BASE_URL, RetryPolicy};
use ringside_cloud::kinds::pushfold_mw::{evaluated, reference, to_caller_units};
use ringside_cloud::kinds::{Kind, PushFoldMw, PushFoldMwInput, PushFoldMwSolution};
use ringside_cloud::{Client, Error, Report};
use ringside_engine::multiway::{self, Params, Solution};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

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

/// Whether a request may go out now: not before the user has seen what the cloud shares
/// (the first-start notice), not while PokerStars runs, not while the cloud is down.
pub(crate) fn may_talk(app: &AppHandle) -> bool {
    cloud_settings_of(app).notice_seen
        && unlocked(&app.state::<AppState>()).is_ok()
        && app.state::<CloudState>().reachable()
}

// ---------------------------------------------------------------------------------
// Solutions.

/// Push/fold Nash for one table of 2–9 players, for the learning area and the coach.
///
/// Blocking (seconds when it has to solve), so call it from a blocking task. Order:
/// 1. the solutions cached on this computer;
/// 2. with `cloud.fetch`, the cloud's verified solution, checked here again (its
///    exploitability is measured with the engine) and dropped if it fails;
/// 3. otherwise solved here and, with `cloud.share`, offered to the cloud in the
///    background.
///
/// Amounts are rounded to milli-BB and ICM payouts are normalized for the cloud; values
/// come back in `params`' units. Params whose solver settings differ from the
/// cloud's ([`PushFoldMwInput::from_params`]) or that are outside its range (more
/// than three players all in, stacks above 100 BB, …) are solved here without cache or
/// cloud. `None` only for params the engine rejects.
pub fn cloud_lookup_or_submit(app: &AppHandle, params: &Params) -> Option<Solution> {
    let input = PushFoldMwInput::from_params(params).and_then(|i| PushFoldMw::check_input(&i).map(|()| i));
    let (Ok(input), Some(cache)) = (input, cache_dir(app)) else {
        return multiway::solve(params).ok();
    };
    let key = PushFoldMw::key(&input).ok()?;
    let path = cache.join(format!("{key}.json"));
    if let Some(cached) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Solution>(&b).ok()) {
        return Some(to_caller_units(cached, params));
    }
    let settings = cloud_settings_of(app);
    let state = app.state::<CloudState>();
    if settings.fetch && may_talk(app) {
        match Client::with_policy(DEFAULT_BASE_URL, RetryPolicy::impatient()).get_solution::<PushFoldMw>(&input) {
            // `get_solution` has already measured the download; measuring it once more
            // gives the full solution with EVs, still far cheaper than solving.
            Ok(Some(download)) => {
                if let Ok(solution) = evaluated(&input, &download) {
                    store(&cache, &path, &solution);
                    return Some(to_caller_units(solution, params));
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("cloud lookup {key}: {e}");
                state.note(&e);
            }
        }
    }
    let solution = reference(&input).ok()?;
    store(&cache, &path, &solution);
    if settings.share && solution.converged {
        submit_later(app.clone(), input, PushFoldMwSolution::from(&*solution));
    }
    Some(to_caller_units((*solution).clone(), params))
}

fn submit_later(app: AppHandle, input: PushFoldMwInput, solution: PushFoldMwSolution) {
    std::thread::spawn(move || {
        // Checked again when it goes out: the user may have switched sharing off or
        // opened PokerStars meanwhile.
        if !cloud_settings_of(&app).share || !may_talk(&app) {
            return;
        }
        let policy = RetryPolicy { attempts: 2, timeout: Duration::from_secs(10), ..RetryPolicy::default() };
        if let Err(e) = Client::with_policy(DEFAULT_BASE_URL, policy).submit_solution::<PushFoldMw>(&input, &solution) {
            eprintln!("cloud submit: {e}");
            app.state::<CloudState>().note(&e);
        }
    });
}

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    let dir = crate::data_dir(app)
        .ok()?
        .join("cloud")
        .join(format!("{}-{}", PushFoldMw::NAME, PushFoldMw::ENGINE_VERSION));
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn store(dir: &Path, path: &Path, solution: &Solution) {
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

// ---------------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------------
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
