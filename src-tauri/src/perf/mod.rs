//! Ringside at scale: a generated history of whole tournaments and timings of what the app
//! does with it, through the same functions the commands call. Before a release:
//!
//! `RINGSIDE_PERF_HANDS=10000,50000,200000 cargo test --release -p ringside --lib perf::scale -- --ignored --nocapture`
//!
//! `RINGSIDE_PERF_KEEP=<dir>` keeps the last database there, e.g. to start the app on it;
//! `RINGSIDE_PERF_DB=<file>` times the pages on a copy of a database kept before.

mod synthetic;

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use ringside_analysis::{ENGINE, analyze_raw};
use ringside_core::db::{Db, DecisionFilter, Filter, HandQuery, Rating, RangeFilter};
use ringside_core::import::{Scanner, import_file};

use crate::AppState;

/// Threads of the preflop analysis here; the app takes half the cores, at most 8.
const THREADS: usize = 6;

fn state_on(path: &Path) -> AppState {
    AppState {
        db: std::sync::Mutex::new(Db::open(path).unwrap()),
        settings: Default::default(),
        import: Default::default(),
        overlays: Default::default(),
        client_running: Default::default(),
        running_client: Default::default(),
        client_scanned: Default::default(),
        notes: Default::default(),
    }
}

/// Resident memory of this process in MB.
fn rss_mb() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output();
    out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).and_then(|s| s.trim().parse::<f64>().ok()).map_or(0.0, |kb| kb / 1024.0)
}

/// CPU time of this thread in ms: on a busy machine steadier than the wall clock.
#[cfg(unix)]
fn cpu_ms() -> f64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `t` is a valid timespec for the call to write.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 * 1000.0 + t.tv_nsec as f64 / 1e6
}

#[cfg(not(unix))]
fn cpu_ms() -> f64 {
    0.0
}

struct Timings {
    rows: Vec<(String, f64, f64)>,
}

impl Timings {
    /// The first call, then the best of three more: what a page costs when opened and when
    /// it is opened again; wall clock and, in brackets, CPU time of the first call.
    fn time<T>(&mut self, label: &str, mut f: impl FnMut() -> T) -> T {
        let rss = rss_mb();
        let (t, cpu) = (Instant::now(), cpu_ms());
        let out = f();
        let first = t.elapsed().as_secs_f64() * 1000.0;
        let cpu = cpu_ms() - cpu;
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = Instant::now();
            std::hint::black_box(f());
            best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        }
        println!("  {label:<46} {first:>9.1} ms ({cpu:>6.1}) {best:>9.1} ms {:>+7.0} MB", rss_mb() - rss);
        self.rows.push((label.to_string(), first, best));
        out
    }
}

/// Writes the history as the client lays it out and returns the hand history folder.
fn write_files(root: &Path, tournaments: &[synthetic::Tournament]) -> PathBuf {
    let dir = root.join("HandHistory").join(synthetic::HERO);
    std::fs::create_dir_all(&dir).unwrap();
    for t in tournaments {
        std::fs::write(dir.join(&t.file_name), t.text()).unwrap();
    }
    root.join("HandHistory")
}

/// What the importer thread does on its first pass: every file, each in its own transaction.
fn import_all(state: &AppState, dirs: &[PathBuf], scanner: &mut Scanner) -> usize {
    let mut n = 0;
    for path in scanner.changed(dirs) {
        let mut db = state.db.lock().unwrap();
        n += import_file(&mut db, &path).unwrap().new_hands.len();
    }
    n
}

/// The background analysis: batches of 2000 candidates, rated in pieces of 100.
fn analyse_all(state: &AppState) -> usize {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(THREADS).build().unwrap();
    let mut after = 0;
    let mut done = 0;
    loop {
        let rows = state.db.lock().unwrap().analysis_candidates(ENGINE, after, 2000).unwrap();
        let Some(last) = rows.last() else { break };
        after = last.0;
        for chunk in rows.chunks(100) {
            let results = pool.install(|| analyze_raw(chunk));
            state.db.lock().unwrap().save_analysis(&results).unwrap();
            done += chunk.len();
        }
    }
    done
}

fn pages(state: &AppState, path: &Path, t: &mut Timings) {
    let all = Filter::default();
    let tour = Filter { game: Some(ringside_core::model::GameKind::Tournament), ..Default::default() };
    let tournaments = state.db.lock().unwrap().tournaments(&all).unwrap();
    let last_tournament = tournaments.iter().max_by(|a, b| a.started_at.cmp(&b.started_at)).map(|r| r.id).unwrap();
    let spans = state.db.lock().unwrap().session_spans(ringside_core::session::LISTED_SESSIONS).unwrap();
    let since = Filter { since: spans.last().map(|s| s.start.clone()), ..Default::default() };

    println!("  {:<46} {:>21} {:>12} {:>10}", "", "erster Aufruf (CPU)", "wiederholt", "RSS +");
    // The top bar and every page on each refresh (a new hand, a finished analysis).
    t.time("Kopfleiste: Turnierliste", || crate::totals(&state.db.lock().unwrap().tournaments(&tour).unwrap()));
    t.time("Kopfleiste: Spieler (300)", || state.db.lock().unwrap().players(&all, "", 300).unwrap());
    t.time("Kopfleiste: Analyse-Zusammenfassung", || crate::analysis::summary(&state.db.lock().unwrap(), &all).unwrap());
    t.time("Kopfleiste: Lernfortschritt", || {
        ringside_learn::progress(&state.db.lock().unwrap().attempts().unwrap(), crate::now_secs())
    });
    t.time("Spielwertung (skill_profile)", || crate::skill::skill_of(state).unwrap());
    // Übersicht.
    t.time("Übersicht: dashboard", || crate::dashboard_view(state, &all).unwrap());
    t.time("Übersicht: positions", || crate::group_view(state.db.lock().unwrap().hero_groups(&all, false).unwrap()));
    t.time("Übersicht: leaks", || crate::leak_report(state, &all).unwrap());
    t.time("Nächste Schritte (next_steps)", || crate::next::steps(state, &all).unwrap());
    t.time("Übersicht ab Datum: dashboard", || crate::dashboard_view(state, &since).unwrap());
    // Hände.
    let q = HandQuery { limit: 100, ..Default::default() };
    t.time("Hände: neueste 100", || crate::hand_list(state, q.clone()).unwrap());
    t.time("Hände: Seite 50 (offset 5000)", || crate::hand_list(state, HandQuery { offset: 5000, ..q.clone() }).unwrap());
    t.time("Hände: gewonnen, Showdown", || {
        crate::hand_list(state, HandQuery { result: Some("won".into()), showdown_only: true, ..q.clone() }).unwrap()
    });
    t.time("Hände: ein Gegner", || crate::hand_list(state, HandQuery { player: Some("Kalimar0".into()), ..q.clone() }).unwrap());
    t.time("Hände: ein Turnier", || {
        crate::hand_list(state, HandQuery { filter: Filter { tournament_id: Some(last_tournament), ..Default::default() }, ..q.clone() })
            .unwrap()
    });
    t.time("Hände: Fehler, teuerste zuerst", || {
        let decisions = DecisionFilter { min_rating: Some(Rating::Mistake), ..Default::default() };
        crate::hand_list(state, HandQuery { decisions: Some(decisions), sort_by_loss: true, ..q.clone() }).unwrap()
    });
    t.time("Hände: Bereich der Spielwertung", || {
        crate::hand_list(state, HandQuery { area: Some(ringside_core::skill::Area::VsAllIn), ..q.clone() }).unwrap()
    });
    t.time("Hände: ganzes Turnier (5000) zur Analyse", || {
        crate::hand_list(state, HandQuery { filter: Filter { tournament_id: Some(last_tournament), ..Default::default() }, limit: 5000, ..Default::default() })
            .unwrap()
    });
    // Spieler.
    let players = t.time("Spieler: Liste (500)", || state.db.lock().unwrap().players(&all, "", 500).unwrap());
    t.time("Spieler: Suche", || state.db.lock().unwrap().players(&all, "Kali", 500).unwrap());
    t.time("Spieler: Liste ab Datum", || state.db.lock().unwrap().players(&since, "", 500).unwrap());
    let villain = players.iter().find(|p| p.name != synthetic::HERO).map(|p| p.name.clone()).unwrap();
    t.time("Spieler: gezeigte Hände", || state.db.lock().unwrap().shown_hands(&villain, 200).unwrap());
    // Berichte, Turniere, Session, Ranges, Lernen.
    t.time("Berichte: reports", || crate::reports_view(&state.db.lock().unwrap(), &all).unwrap());
    t.time("Turniere: Liste", || crate::totals(&state.db.lock().unwrap().tournaments(&tour).unwrap()));
    t.time("Turnierbericht", || crate::report::report_page(state, last_tournament).unwrap());
    t.time("Session-Rückblick", || crate::report::session_page(&state.db.lock().unwrap(), None, false).unwrap());
    t.time("Analyse-Zusammenfassung ab Datum", || crate::analysis::summary(&state.db.lock().unwrap(), &since).unwrap());
    t.time("Ranges: Situationen", || state.db.lock().unwrap().range_groups(&all).unwrap());
    t.time("Ranges: Tabelle", || state.db.lock().unwrap().range_counts(&all, &RangeFilter::default()).unwrap());
    t.time("Lernen: Spielprofil", || {
        let db = state.db.lock().unwrap();
        ringside_core::profile::profile(&db.profile_rows(&all).unwrap(), &db.spot_practice().unwrap())
    });
    // HUD of the table that played last.
    let latest = state.db.lock().unwrap().hands(&HandQuery { limit: 1, ..Default::default() }).unwrap();
    let hand = state.db.lock().unwrap().hand(latest[0].id).unwrap().unwrap();
    let key = hand.table_key();
    t.time("App-Start: Datenbank öffnen", || Db::open(path).unwrap());
    t.time("HUD: ein Tisch (overlay_view)", || {
        let db = state.db.lock().unwrap();
        let hand = db.latest_hand_for_table(&key).unwrap().unwrap();
        let settings = state.settings.lock().unwrap();
        crate::build_view(&db, &settings, &Default::default(), &hand, String::new(), 1.4, true).unwrap()
    });
}

/// A tournament the client writes into while it runs: one hand at a time, each import pass
/// followed by what the open window reloads (PokerStars runs, so nothing is rated).
fn live_import(state: &AppState, dirs: &[PathBuf], scanner: &mut Scanner, extra: &synthetic::Tournament) -> (f64, f64, f64) {
    let path = dirs[0].join(synthetic::HERO).join(format!("live {}", extra.file_name));
    let mut text = String::new();
    let (mut import_ms, mut worst, mut refresh_ms) = (0.0, 0.0f64, 0.0);
    let n = extra.hands.len().min(200);
    state.client_running.store(true, Ordering::Relaxed);
    let all = Filter::default();
    let tour = Filter { game: Some(ringside_core::model::GameKind::Tournament), ..Default::default() };
    for hand in &extra.hands[..n] {
        text.push_str(hand);
        text.push_str("\n\n\n");
        std::fs::write(&path, &text).unwrap();
        let t = Instant::now();
        assert_eq!(import_all(state, dirs, scanner), 1);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        import_ms += ms;
        worst = worst.max(ms);
        let t = Instant::now();
        crate::totals(&state.db.lock().unwrap().tournaments(&tour).unwrap());
        state.db.lock().unwrap().players(&all, "", 300).unwrap();
        crate::dashboard_view(state, &all).unwrap();
        crate::group_view(state.db.lock().unwrap().hero_groups(&all, false).unwrap());
        refresh_ms += t.elapsed().as_secs_f64() * 1000.0;
    }
    state.client_running.store(false, Ordering::Relaxed);
    (import_ms / n as f64, worst, refresh_ms / n as f64)
}

fn run(hands: usize, keep: Option<&Path>) {
    println!("\n== {hands} Hände ==");
    let tmp = tempfile::tempdir().unwrap();
    let root = keep.map_or_else(|| tmp.path().to_path_buf(), Path::to_path_buf);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let rss0 = rss_mb();

    let t = Instant::now();
    let pool = (hands / 8).clamp(2_000, 60_000);
    let mut history = synthetic::history(hands + 200, pool, hands as u64);
    let extra = history.pop().unwrap();
    let made: usize = history.iter().map(|t| t.hands.len()).sum();
    let dirs = vec![write_files(&root, &history)];
    println!("  erzeugt: {made} Hände in {} Turnieren, {pool} Gegnernamen, {:.1} s", history.len(), t.elapsed().as_secs_f64());
    drop(history);

    let db_path = root.join("ringside.sqlite");
    let state = state_on(&db_path);
    let mut scanner = Scanner::default();
    let t = Instant::now();
    let imported = import_all(&state, &dirs, &mut scanner);
    let import_s = t.elapsed().as_secs_f64();
    println!("  Erstimport: {imported} Hände in {import_s:.1} s = {:.0} Hände/s; RSS {:.0} MB", imported as f64 / import_s, rss_mb());

    let t = Instant::now();
    let analysed = analyse_all(&state);
    let analyse_s = t.elapsed().as_secs_f64();
    let counts = state.db.lock().unwrap().analysis_counts(ENGINE).unwrap();
    println!(
        "  Preflop-Analyse ({THREADS} Threads): {analysed} Hände in {analyse_s:.1} s = {:.0} Hände/s; {counts:?}; RSS {:.0} MB",
        analysed as f64 / analyse_s,
        rss_mb()
    );
    let size = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
    let wal = db_path.with_extension("sqlite-wal");
    println!("  Datenbank: {:.1} MB (+ WAL {:.1} MB)", size(&db_path) as f64 / 1e6, size(&wal) as f64 / 1e6);

    let mut t = Timings { rows: Vec::new() };
    pages(&state, &db_path, &mut t);
    let (avg, worst, refresh) = live_import(&state, &dirs, &mut scanner, &extra);
    println!("  Laufender Import je Hand: {avg:.1} ms (höchstens {worst:.1} ms); Auffrischen danach {refresh:.1} ms");
    let slow: Vec<_> = t.rows.iter().filter(|r| r.1 > 300.0).map(|r| r.0.as_str()).collect();
    println!("  über 300 ms beim ersten Aufruf: {slow:?}");
    println!("  RSS am Ende {:.0} MB (Start {rss0:.0} MB)", rss_mb());
}

/// The pages on a copy of a database from an earlier run.
fn pages_of(db: &Path) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("ringside.sqlite");
    std::fs::copy(db, &path).unwrap();
    let state = state_on(&path);
    println!("\n== {} Hände aus {} ==", state.db.lock().unwrap().hand_count().unwrap(), db.display());
    println!("  RSS vor den Seiten {:.0} MB", rss_mb());
    pages(&state, &path, &mut Timings { rows: Vec::new() });
    println!("  RSS danach {:.0} MB", rss_mb());
}

#[test]
#[ignore = "slow; run before a release"]
fn scale() {
    if let Some(db) = std::env::var_os("RINGSIDE_PERF_DB") {
        return pages_of(Path::new(&db));
    }
    let sizes = std::env::var("RINGSIDE_PERF_HANDS").unwrap_or_else(|_| "10000,50000".into());
    let keep = std::env::var_os("RINGSIDE_PERF_KEEP").map(PathBuf::from);
    for n in sizes.split(',').filter_map(|s| s.trim().parse().ok()) {
        run(n, keep.as_deref());
    }
}




/// A history of `hands` hands stored straight away, without files and analysis.
fn stored(hands: usize) -> AppState {
    let state = state_on(Path::new(":memory:"));
    let rows: Vec<_> = synthetic::history(hands, 1_000, 3)
        .iter()
        .flat_map(|t| t.hands.iter())
        .map(|raw| (ringside_core::parse_hand(raw).unwrap(), raw.clone()))
        .collect();
    state.db.lock().unwrap().insert_hands(&rows).unwrap();
    state
}

/// What one tournament or session reads must not grow with the history around it: SQLite
/// steps of the same first tournament and session in a history and in one four times as long.
#[test]
fn one_tournament_costs_the_same_in_a_longer_history() {
    let work = |state: &AppState| {
        let db = state.db.lock().unwrap();
        let first = synthetic::FIRST_TOURNAMENT;
        let filter = Filter { tournament_id: Some(first), ..Default::default() };
        let span = db.session_spans(usize::MAX).unwrap().pop().unwrap();
        let hand = db.hand(db.report_hands(&filter, ENGINE).unwrap().0.last().unwrap().hand_id).unwrap().unwrap();
        [
            db.steps_of(|db| db.report_hands(&filter, ENGINE).unwrap()).1,
            db.steps_of(|db| db.report_decisions(&filter).unwrap()).1,
            db.steps_of(|db| db.counters_in_session(synthetic::HERO, &hand).unwrap()).1,
            db.steps_of(|db| db.session_review(&span, ENGINE).unwrap()).1,
        ]
    };
    let short = work(&stored(1_500));
    let long = work(&stored(6_000));
    for (what, (s, l)) in ["tournament hands", "tournament decisions", "HUD session counters", "session review"].iter().zip(short.iter().zip(&long)) {
        assert!(*l as f64 <= *s as f64 * 1.5 + 5.0, "{what}: {s} thousand steps in the short history, {l} in the long one");
    }
}

/// The overview over a mid-sized history, with a bound loose enough for a slow CI machine.
#[test]
fn the_overview_stays_quick() {
    let state = stored(6_000);
    let t = Instant::now();
    crate::dashboard_view(&state, &Filter::default()).unwrap();
    crate::group_view(state.db.lock().unwrap().hero_groups(&Filter::default(), false).unwrap());
    crate::totals(&state.db.lock().unwrap().tournaments(&Filter::default()).unwrap());
    let ms = t.elapsed().as_millis();
    assert!(ms < 3_000, "overview of 6000 hands took {ms} ms");
}
