//! The user's own hand histories, kept outside the repository:
//! `RINGSIDE_REAL_HH=/path/to/HandHistory/<name> cargo test --release -p ringside-analysis --test real -- --ignored --nocapture`.

use std::time::Instant;

use ringside_analysis::{Status, analyze_hand};
use ringside_core::{parse_hand, split_hands};

#[test]
#[ignore = "needs RINGSIDE_REAL_HH"]
fn real_hands() {
    let dir = std::env::var("RINGSIDE_REAL_HH").expect("RINGSIDE_REAL_HH");
    let mut hands = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        hands.extend(split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok()));
    }
    let started = Instant::now();
    let results: Vec<_> = hands.iter().map(analyze_hand).collect();
    let elapsed = started.elapsed();
    let decisions: Vec<_> = results.iter().flat_map(|r| r.decisions.iter()).collect();
    let count = |s: Status| decisions.iter().filter(|d| d.status == s).count();
    println!(
        "{} Hände, {} Entscheidungen: {} bewertet, {} warten auf den Solver, {} nicht bewertet; {} Postflop-Jobs; {:.2} s ({:.0} µs je Hand)",
        hands.len(),
        decisions.len(),
        count(Status::Rated),
        count(Status::Pending),
        count(Status::Unrated),
        results.iter().filter(|r| r.job.is_some()).count(),
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1e6 / hands.len() as f64
    );
    let mut reasons: std::collections::BTreeMap<&str, usize> = Default::default();
    for d in decisions.iter().filter(|d| d.status == Status::Unrated) {
        *reasons.entry(d.reason.as_deref().unwrap_or("?")).or_default() += 1;
    }
    for (r, n) in &reasons {
        println!("  {n:5} × {r}");
    }
    let mut rated: Vec<_> = decisions.iter().filter(|d| d.status == Status::Rated).collect();
    let mut by_source: std::collections::BTreeMap<String, usize> = Default::default();
    for d in &rated {
        *by_source.entry(format!("{:?}/{:?}", d.source.unwrap(), d.rating.unwrap())).or_default() += 1;
    }
    println!("{by_source:#?}");
    rated.sort_by(|a, b| b.ev_loss_bb.unwrap_or(0.0).total_cmp(&a.ev_loss_bb.unwrap_or(0.0)));
    for d in rated.iter().take(5) {
        let hand = hands.iter().find(|h| h.id == d.hand_id).unwrap();
        println!(
            "\n#{} {} {:?} {:?}: gespielt {} – empfohlen {:?}, Verlust {:.2} BB, {:?}\n  {:?}\n  {}",
            d.hand_id,
            hand.hero_cards.join(""),
            d.spot,
            d.rating.unwrap(),
            d.played,
            d.best,
            d.ev_loss_bb.unwrap_or(0.0),
            d.source.unwrap(),
            d.strategy.iter().map(|c| format!("{} {:.0}% EV {:.2}", c.label, c.freq * 100.0, c.ev_bb.unwrap_or(f64::NAN))).collect::<Vec<_>>(),
            d.note
        );
    }
}

/// Size and time of the postflop jobs; with `RINGSIDE_REAL_SOLVE=n` the n largest pots are solved.
#[test]
#[ignore = "needs RINGSIDE_REAL_HH"]
fn real_postflop_jobs() {
    use ringside_analysis::{PostflopJob, solve_job};
    let dir = std::env::var("RINGSIDE_REAL_HH").expect("RINGSIDE_REAL_HH");
    let mut jobs: Vec<PostflopJob> = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        for hand in split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok()) {
            if let Some((_, json)) = analyze_hand(&hand).job {
                jobs.push(serde_json::from_str(&json).unwrap());
            }
        }
    }
    jobs.sort_by(|a, b| b.pot_bb.total_cmp(&a.pot_bb));
    let threads = 4;
    let limit = 4u64 << 30;
    let mut total = 0.0;
    for j in &jobs {
        let e = match ringside_solver::estimate(&j.spot, limit, threads) {
            Ok(e) => e,
            Err(e) => {
                println!("#{} pot {:.1} → {e}", j.hand_id, j.pot_bb);
                continue;
            }
        };
        total += e.seconds;
        println!("#{} pot {:.1} → {:.0} MB, {:.0} s, fits {}", j.hand_id, j.pot_bb, e.memory_bytes as f64 / 1e6, e.seconds, e.fits);
    }
    println!("{} Jobs, geschätzt {:.0} s mit {threads} Threads", jobs.len(), total);
    let n: usize = std::env::var("RINGSIDE_REAL_SOLVE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    for j in jobs.iter().take(n) {
        let t = Instant::now();
        let result = solve_job(j, threads, limit, |_| {}, || false);
        match result {
            Ok(ds) => {
                println!("#{} gelöst in {:.1} s", j.hand_id, t.elapsed().as_secs_f64());
                for d in ds {
                    println!("  {:?} {:?} {} → {:?} Verlust {:?} {:?} {:?}", d.street, d.spot, d.played, d.best, d.ev_loss_bb, d.rating, d.reason);
                }
            }
            Err(_) => println!("#{} fehlgeschlagen", j.hand_id),
        }
    }
}

/// Everything at once: preflop, then every postflop job, the five costliest decisions.
/// `RINGSIDE_REAL_THREADS` sets the solver threads (default 4).
#[test]
#[ignore = "needs RINGSIDE_REAL_HH; solves every postflop spot"]
fn real_full() {
    use ringside_analysis::{PostflopJob, solve_job};
    let dir = std::env::var("RINGSIDE_REAL_HH").expect("RINGSIDE_REAL_HH");
    let threads: usize = std::env::var("RINGSIDE_REAL_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let mut hands = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        hands.extend(split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok()));
    }
    let t = Instant::now();
    let results: Vec<_> = hands.iter().map(analyze_hand).collect();
    println!("Preflop: {} Hände in {:.2} s", hands.len(), t.elapsed().as_secs_f64());
    let mut decisions: Vec<_> = results.iter().flat_map(|r| r.decisions.clone()).collect();
    let jobs: Vec<PostflopJob> = results.iter().filter_map(|r| r.job.as_ref()).map(|(_, j)| serde_json::from_str(j).unwrap()).collect();
    let t = Instant::now();
    let mut failed = 0;
    for (k, j) in jobs.iter().enumerate() {
        let one = Instant::now();
        match solve_job(j, threads, 8 << 30, |_| {}, || false) {
            Ok(ds) => {
                for d in ds {
                    if let Some(slot) = decisions.iter_mut().find(|x| x.hand_id == d.hand_id && x.action_index == d.action_index) {
                        *slot = d;
                    }
                }
            }
            Err(_) => failed += 1,
        }
        println!("  Job {}/{} #{}: {:.1} s", k + 1, jobs.len(), j.hand_id, one.elapsed().as_secs_f64());
    }
    println!("Postflop: {} Jobs in {:.0} s mit {threads} Threads, {failed} nicht lösbar", jobs.len(), t.elapsed().as_secs_f64());
    let count = |s: Status| decisions.iter().filter(|d| d.status == s).count();
    println!("{} Entscheidungen: {} bewertet, {} offen, {} nicht bewertet", decisions.len(), count(Status::Rated), count(Status::Pending), count(Status::Unrated));
    let mut by: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
    for d in decisions.iter().filter(|d| d.status == Status::Rated) {
        let e = by.entry(format!("{:?}/{:?}", d.source.unwrap(), d.rating.unwrap())).or_default();
        e.0 += 1;
        e.1 += d.ev_loss_bb.unwrap_or(0.0);
    }
    println!("{by:#?}");
    let mut rated: Vec<_> = decisions.iter().filter(|d| d.status == Status::Rated).collect();
    rated.sort_by(|a, b| b.ev_loss_bb.unwrap_or(0.0).total_cmp(&a.ev_loss_bb.unwrap_or(0.0)));
    for d in rated.iter().take(5) {
        let hand = hands.iter().find(|h| h.id == d.hand_id).unwrap();
        println!("\n#{} {} {:?} {:?} {:?}\n  {}\n  {:?}\n  {}", d.hand_id, hand.hero_cards.join(""), d.street, d.spot, d.rating.unwrap(), d.why,
            d.strategy.iter().map(|c| format!("{} {:.0}% EV {:.2}", c.label, c.freq * 100.0, c.ev_bb.unwrap_or(f64::NAN))).collect::<Vec<_>>(), d.note);
    }
}

/// The session review of the user's own hands, as the start page shows it:
/// `RINGSIDE_REAL_HH=… cargo test --release -p ringside-analysis --test real real_session -- --ignored --nocapture`.
#[test]
#[ignore = "needs RINGSIDE_REAL_HH"]
fn real_session() {
    let dir = std::env::var("RINGSIDE_REAL_HH").expect("RINGSIDE_REAL_HH");
    let mut db = ringside_core::db::Db::open_in_memory().unwrap();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let parsed: Vec<_> = split_hands(&text).into_iter().filter_map(|raw| parse_hand(raw).ok().map(|h| (h, raw.to_string()))).collect();
        db.insert_hands(&parsed).unwrap();
        let analysed: Vec<_> = parsed.iter().map(|(h, _)| analyze_hand(h)).collect();
        db.save_analysis(&analysed).unwrap();
    }
    for span in db.session_spans(10).unwrap() {
        let r = db.session_review(&span, ringside_analysis::ENGINE).unwrap();
        let p = &r.preflop;
        println!(
            "\nSession {} – {}: {} Hände, {} Turniere, {} Cash-Hände",
            span.start,
            span.end,
            span.hands,
            span.tournaments.len(),
            span.cash_hands
        );
        println!(
            "  Preflop: {} von {} Händen verglichen ({} Gleichgewicht), gespielt {:.1} %, Modell {:.1} %, Stichprobe {:?}",
            p.compared,
            p.hands,
            p.equilibrium,
            p.you_pct.unwrap_or(0.0),
            p.model_pct.unwrap_or(0.0),
            p.sample
        );
        println!("  Entscheidungen vor dem Flop: {} verglichen, {} gut, Verlust {:.1} BB", p.decisions, p.good, p.loss_bb);
        if let Some(v) = &p.verdict {
            println!("  Urteil: {} (Stufe {}) – {}", v.title, v.level, v.line);
        }
        let names = |ids: &[u8]| ids.iter().filter_map(|i| p.classes.iter().find(|c| c.class == *i)).map(|c| c.name.clone()).collect::<Vec<_>>();
        println!("  Hättest du auch spielen sollen: {:?}", names(&p.tight));
        println!("  Hättest du folden sollen: {:?}", names(&p.wide));
        for pat in &p.patterns {
            println!("  Muster: {}", pat.text);
        }
        println!(
            "  Glück {:.1} BB in {} All-ins; Pech gehabt {}, Glück gehabt {}; offen: {} Hände, {} Flop-Spots",
            r.report.luck.luck_bb,
            r.report.luck.all_ins,
            r.bad_luck.len(),
            r.good_luck.len(),
            r.report.unanalyzed,
            r.report.pending
        );
        if let Some(s) = r.bad_luck.first() {
            println!("  Pech-Beispiel: {} {} ({:?}), Equity {:?}, Ergebnis {:.1} BB", s.hero_cards, s.played, s.rating, s.equity_pct, s.net_bb);
        }
        if let Some(s) = r.good_luck.first() {
            println!("  Glück-Beispiel: {} {} ({:?}), kostet {:.1} BB, Ergebnis {:.1} BB", s.hero_cards, s.played, s.rating, s.loss_bb, s.net_bb);
        }
    }
}
