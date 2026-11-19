use ringside_analysis::{PostflopJob, Rating, Source, SpotType, Status, analyze_hand, postflop::JobError, solve_job};
use ringside_core::db::{Db, DecisionFilter, HandQuery};
use ringside_core::model::{Hand, Street};
use ringside_core::parse_hand;

/// A tournament hand at `bb` with antes of `ante`; `seats` as (name, chips), the
/// button on seat `button`, small and big blind on the next two seats.
fn hand(id: u64, seats: &[(&str, i64)], button: usize, bb: i64, ante: i64, cards: &str, body: &str) -> String {
    let n = seats.len();
    let sb = &seats[button % n].0;
    let big = &seats[(button + 1) % n].0;
    let mut out = format!(
        "PokerStars Hand #{id}: Tournament #3500000001, $10+$1 USD Hold'em No Limit - Level IV ({}/{bb}) - 2024/01/15 21:00:00 CET [2024/01/15 15:00:00 ET]\nTable '3500000001 12' 9-max Seat #{button} is the button\n",
        bb / 2
    );
    for (i, (name, chips)) in seats.iter().enumerate() {
        out.push_str(&format!("Seat {}: {name} ({chips} in chips)\n", i + 1));
    }
    if ante > 0 {
        for (name, _) in seats {
            out.push_str(&format!("{name}: posts the ante {ante}\n"));
        }
    }
    out.push_str(&format!("{sb}: posts small blind {}\n{big}: posts big blind {bb}\n*** HOLE CARDS ***\nDealt to Hero [{cards}]\n", bb / 2));
    out.push_str(body);
    out
}

fn parsed(raw: &str) -> Hand {
    parse_hand(raw).unwrap_or_else(|e| panic!("{e:?}\n{raw}"))
}

const SIX: [(&str, i64); 6] = [("P1", 1600), ("P2", 1600), ("P3", 1600), ("P4", 1600), ("Hero", 1600), ("P6", 1600)];

fn k6o_fold() -> String {
    hand(
        1,
        &SIX,
        4,
        200,
        25,
        "Kh 6c",
        "P1: folds\nP2: folds\nP3: folds\nP4: folds\nHero: folds\nUncalled bet (100) returned to P6\nP6 collected 350 from pot\n*** SUMMARY ***\nTotal pot 350 | Rake 0\n",
    )
}

#[test]
fn push_fold_mistake_is_found() {
    let a = analyze_hand(&parsed(&k6o_fold()));
    let [d] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!(d.source, Some(Source::Equilibrium));
    assert_eq!(d.spot, SpotType::Shove);
    assert_eq!(d.best.as_deref(), Some("All-in"));
    assert!(d.rating >= Some(Rating::Inaccuracy), "{d:?}");
    assert!(d.ev_loss_bb.unwrap() > 0.2, "{d:?}");
    assert!(d.why.contains("K6o gehört zu den besten"), "{}", d.why);
    assert!(a.job.is_none());
}

#[test]
fn model_comparison_names_the_nearest_configuration() {
    // 37 BB, eight players with an ante: the model's nearest depth is 40 BB.
    let seats: Vec<(&str, i64)> = vec![("Hero", 7400), ("P2", 7400), ("P3", 7400), ("P4", 7400), ("P5", 7400), ("P6", 7400), ("P7", 7400), ("P8", 7400)];
    let raw = hand(
        2,
        &seats,
        5,
        200,
        25,
        "7c 2d",
        "Hero: raises 300 to 500\nP2: folds\nP3: folds\nP4: folds\nP5: folds\nP6: folds\nP7: folds\nP8: folds\nUncalled bet (300) returned to Hero\nHero collected 700 from pot\n*** SUMMARY ***\nTotal pot 700 | Rake 0\n",
    );
    let a = analyze_hand(&parsed(&raw));
    let [d] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!(d.source, Some(Source::Model));
    assert_eq!(d.spot, SpotType::Open);
    assert_eq!(d.best.as_deref(), Some("Fold"));
    assert!(d.rating >= Some(Rating::Inaccuracy), "{d:?}");
    assert!(d.note.contains("nächste vorgerechnete Tiefe: 40 BB statt 37 BB"), "{}", d.note);
    assert!(d.played_freq.unwrap() < 0.05);
}

#[test]
fn an_isolation_raise_behind_a_limp_is_rated_by_the_model() {
    let deep: Vec<(&str, i64)> = SIX.iter().map(|&(name, _)| (name, 8000)).collect();
    let limp = hand(
        3,
        &deep,
        4,
        200,
        25,
        "9s 9d",
        "P1: calls 200\nP2: folds\nP3: folds\nP4: folds\nHero: raises 700 to 900\nP6: folds\nP1: folds\nUncalled bet (700) returned to Hero\nHero collected 850 from pot\n*** SUMMARY ***\nTotal pot 850 | Rake 0\n",
    );
    let a = analyze_hand(&parsed(&limp));
    let [d] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!((d.status, d.source, d.spot), (Status::Rated, Some(Source::Model), SpotType::VsLimp), "{d:?}");
}

/// Nine players at 30 BB with antes; the hero under the gun with aces.
fn aces_first_in(id: u64, body: &str) -> Hand {
    let names = ["P1", "Hero", "P3", "P4", "P5", "P6", "P7", "P8", "P9"];
    let seats: Vec<(&str, i64)> = names.iter().map(|&n| (n, 6000)).collect();
    parsed(&hand(id, &seats, 8, 200, 25, "As Ah", body))
}

/// The other players' limps first in are no move of the model: an open raise is rated
/// against fold and raise only, and a limp under the gun is left to the estimate.
#[test]
fn open_limps_of_the_other_players_are_no_model_move() {
    let folds = "P3: folds\nP4: folds\nP5: folds\nP6: folds\nP7: folds\nP8: folds\nP9: folds\n";
    let raise = aces_first_in(8, &format!("Hero: raises 240 to 440\n{folds}P1: folds\nUncalled bet (240) returned to Hero\nHero collected 825 from pot\n*** SUMMARY ***\nTotal pot 825 | Rake 0\n"));
    let a = analyze_hand(&raise);
    let [open] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!((open.source, open.rating, open.best.as_deref()), (Some(Source::Model), Some(Rating::Best), Some("Raise auf 2,2 BB")), "{open:?}");
    assert!(open.strategy.iter().all(|c| c.kind != "limp"), "{:?}", open.strategy);

    let limp = aces_first_in(9, &format!("Hero: calls 200\n{folds}P1: raises 600 to 800\nHero: folds\nUncalled bet (600) returned to P1\nP1 collected 725 from pot\n*** SUMMARY ***\nTotal pot 725 | Rake 0\n"));
    let a = analyze_hand(&limp);
    let [limped, _] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!(limped.source, Some(Source::Estimate), "{limped:?}");
    assert!(limped.note.contains("limpt von hier nicht selbst"), "{}", limped.note);
}

#[test]
fn folds_between_the_three_bettor_and_the_opener_keep_the_model_line() {
    // 40 BB, six players: the hero opens from the cutoff, the button 3-bets, both
    // blinds fold, the hero decides.
    let seats: Vec<(&str, i64)> = vec![("P1", 8000), ("P2", 8000), ("Hero", 8000), ("P4", 8000), ("P5", 8000), ("P6", 8000)];
    let raw = hand(
        7,
        &seats,
        4,
        200,
        25,
        "Ks Qs",
        "P1: folds\nP2: folds\nHero: raises 240 to 440\nP4: raises 770 to 1210\nP5: folds\nP6: folds\nHero: folds\nUncalled bet (770) returned to P4\nP4 collected 1330 from pot\n*** SUMMARY ***\nTotal pot 1330 | Rake 0\n",
    );
    let a = analyze_hand(&parsed(&raw));
    let [open, vs_three_bet] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!((open.source, open.spot), (Some(Source::Model), SpotType::Steal), "{open:?}");
    assert_eq!((vs_three_bet.status, vs_three_bet.source, vs_three_bet.spot), (Status::Rated, Some(Source::Model), SpotType::VsThreeBet), "{vs_three_bet:?}");

    // The hero in the small blind between them: the model only folds there.
    let raw = raw.replace("Hero", "P3").replace("P5", "Hero").replace("Dealt to P3 [Ks Qs]", "Dealt to Hero [Ks Qs]");
    let a = analyze_hand(&parsed(&raw));
    let [fold] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!((fold.status, fold.source, fold.spot), (Status::Rated, Some(Source::Estimate), SpotType::VsThreeBet), "{fold:?}");
}

#[test]
fn a_three_way_flop_is_estimated_with_its_reliability() {
    let raw = hand(
        6,
        &SIX,
        4,
        200,
        25,
        "Ah Kh",
        "P1: calls 200\nP2: folds\nP3: folds\nP4: folds\nHero: calls 100\nP6: checks\n*** FLOP *** [Kd 7s 2c]\nHero: bets 300\nP6: folds\nP1: folds\nUncalled bet (300) returned to Hero\nHero collected 750 from pot\n*** SUMMARY ***\nTotal pot 750 | Rake 0\nBoard [Kd 7s 2c]\n",
    );
    let a = analyze_hand(&parsed(&raw));
    let flop = a.decisions.iter().find(|d| d.street == Street::Flop).unwrap();
    assert_eq!((flop.status, flop.source), (Status::Rated, Some(Source::Estimate)), "{flop:?}");
    assert_eq!(flop.reliability, Some(measured(Street::Flop)), "{flop:?}");
    assert!(flop.note.contains("mehreren Gegnern ist sie ungeprüft"), "{flop:?}");
    assert!(a.job.is_none());
}

/// The lowest agreement within one step of the checks at `street` with enough
/// comparisons, read from the calibration file itself: plain, and balanced over the
/// reference's grades where enough of its mistakes were compared.
fn measured(street: Street) -> f64 {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/estimate_calibration.json");
    let file: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let checks: Vec<&serde_json::Value> = match street {
        Street::Preflop => vec![&file["preflop"]],
        s => {
            let k = s as usize - 1;
            vec![&file["postflop_streets"][k], &file["real_streets"][k], &file["real"]]
        }
    };
    let enough = |c: &serde_json::Value, key: &str| c[key].as_u64().unwrap() >= ringside_analysis::calibration::MIN_SAMPLES as u64;
    let large: Vec<f64> = checks
        .iter()
        .filter(|c| enough(c, "samples"))
        .map(|c| {
            let within = c["within_one"].as_f64().unwrap();
            if enough(c, "off_best") { within.min(c["balanced"].as_f64().unwrap()) } else { within }
        })
        .collect();
    assert!(!large.is_empty(), "no check of {street:?} has enough comparisons");
    large.into_iter().fold(1.0, f64::min)
}

#[test]
fn the_reliability_shown_is_the_measured_one() {
    for street in [Street::Preflop, Street::Flop, Street::Turn, Street::River] {
        assert_eq!(ringside_analysis::replay::reliability(street), measured(street), "{street:?}");
    }
}

#[test]
fn calling_the_last_chips_is_no_decision() {
    let seats: Vec<(&str, i64)> = vec![("P1", 6200), ("P2", 9800), ("P3", 7400), ("Hero", 650), ("P5", 11200), ("P6", 8300)];
    let raw = hand(
        7,
        &seats,
        3,
        600,
        75,
        "8d 3s",
        "P6: folds\nP1: folds\nP2: raises 900 to 1500\nP3: folds\nHero: calls 275 and is all-in\nP5: folds\nUncalled bet (900) returned to P2\n*** FLOP *** [Kd 9s 4c]\n*** TURN *** [Kd 9s 4c] [2h]\n*** RIVER *** [Kd 9s 4c 2h] [Jd]\n*** SHOW DOWN ***\nP2 collected 2225 from pot\n*** SUMMARY ***\nTotal pot 2225 | Rake 0\n",
    );
    let a = analyze_hand(&parsed(&raw));
    let [d] = a.decisions.as_slice() else { panic!("{:?}", a.decisions) };
    assert_eq!((d.status, d.source), (Status::Forced, Some(Source::Forced)), "{d:?}");
}

/// Button opens, big blind calls, the button c-bets and the big blind folds.
fn c_bet_hand(id: u64) -> String {
    // Short stacks keep the tree small: few bets fit behind.
    let seats: Vec<(&str, i64)> = vec![("P1", 3000), ("P2", 3000), ("P3", 3000), ("Hero", 3000), ("P5", 3000), ("P6", 3000)];
    hand(
        id,
        &seats,
        4,
        200,
        25,
        "Ah Kh",
        "P1: folds\nP2: folds\nP3: folds\nHero: raises 200 to 400\nP5: folds\nP6: calls 200\n*** FLOP *** [Kd 7s 2c]\nP6: checks\nHero: bets 300\nP6: folds\nUncalled bet (300) returned to Hero\nHero collected 1150 from pot\n*** SUMMARY ***\nTotal pot 1150 | Rake 0\nBoard [Kd 7s 2c]\n",
    )
}

#[test]
fn postflop_spot_is_queued_and_rated() {
    let raw = c_bet_hand(4);
    let h = parsed(&raw);
    let mut db = Db::open_in_memory().unwrap();
    db.insert_hands(&[(h.clone(), raw)]).unwrap();
    db.save_analysis(&[analyze_hand(&h)]).unwrap();

    let counts = db.analysis_counts(ringside_analysis::ENGINE).unwrap();
    assert_eq!((counts.analyzed, counts.queued, counts.pending), (1, 1, 1));
    let pending = db.hand_decisions(4).unwrap();
    let flop = pending.iter().find(|d| d.street == Street::Flop).unwrap();
    assert_eq!((flop.status, flop.spot), (Status::Pending, SpotType::CBet));

    let (id, json) = db.next_postflop_job(0).unwrap().unwrap();
    assert_eq!(id, 4);
    let mut job: PostflopJob = serde_json::from_str(&json).unwrap();
    assert!(job.note.contains("Ranges aus dem Preflop-Modell"), "{}", job.note);
    // The model's ranges take minutes to solve; narrow ones keep the test quick and still
    // run the whole path from the queue to stored ratings.
    job.spot.oop_range = "QQ,JJ,77,AQs,KQs".into();
    job.spot.ip_range = "AA,AKs,KQs,99".into();
    let cancelled = solve_job(&job, 2, 1 << 30, |_| {}, || true);
    assert!(matches!(cancelled, Err(JobError::Cancelled)));
    let Ok(solved) = solve_job(&job, 2, 1 << 30, |_| {}, || false) else { panic!("solve failed") };
    let [d] = solved.as_slice() else { panic!("{solved:?}") };
    assert_eq!((d.status, d.source, d.spot), (Status::Rated, Some(Source::Solver), SpotType::CBet));
    assert!(d.solver.as_deref().unwrap().contains("\"path\""));
    assert!(d.why.contains("Drittel deiner Range") && d.why.contains("Equity"), "{}", d.why);
    if std::env::var_os("RINGSIDE_PRINT_WHY").is_some() {
        println!("Solver {}: {}", d.played, d.why);
    }
    db.finish_postflop(id, &solved).unwrap();
    let counts = db.analysis_counts(ringside_analysis::ENGINE).unwrap();
    assert_eq!((counts.queued, counts.pending, counts.rated), (0, 0, 2));
}

#[test]
fn the_queue_and_the_counts_keep_to_the_new_hands() {
    let mut db = Db::open_in_memory().unwrap();
    let raws = [k6o_fold(), c_bet_hand(4), c_bet_hand(9)];
    let hands: Vec<(Hand, String)> = raws.iter().map(|r| (parsed(r), r.clone())).collect();
    db.insert_hands(&hands).unwrap();
    assert_eq!(db.max_hand_id().unwrap(), Some(9));
    let next = |db: &Db, after: u64| db.next_postflop_job(after).unwrap().map(|(id, _)| id);
    assert_eq!(next(&db, 0), None, "nothing analysed, nothing queued");
    db.save_analysis(&hands.iter().map(|(h, _)| analyze_hand(h)).collect::<Vec<_>>()).unwrap();

    assert_eq!(next(&db, u64::MAX), None, "only marked hands, and none is");
    assert_eq!(next(&db, 5), Some(9), "only the hands after the mark");
    assert!(db.mark_postflop(4).unwrap());
    assert_eq!(next(&db, 5), Some(4), "a hand asked for goes first, new or not");
    assert_eq!(next(&db, u64::MAX), Some(4));

    let since = db.analysis_since(ringside_analysis::ENGINE, 5).unwrap();
    assert_eq!((since.hands, since.last_hand), (1, Some(9)));
    let all = db.analysis_since(ringside_analysis::ENGINE, 0).unwrap();
    assert_eq!((all.hands, all.last_hand), (3, Some(9)));
    let mistakes = [1, 4, 9]
        .iter()
        .flat_map(|&id| db.hand_decisions(id).unwrap())
        .filter(|d| d.status == Status::Rated && d.rating >= Some(Rating::Mistake))
        .count();
    assert_eq!(all.mistakes, mistakes as i64);
    assert_eq!(db.analysis_since(ringside_analysis::ENGINE, 9).unwrap(), Default::default());
}

#[test]
fn hand_list_filters_and_sorts_by_loss() {
    let mut db = Db::open_in_memory().unwrap();
    let raws = [k6o_fold(), c_bet_hand(5)];
    let hands: Vec<(Hand, String)> = raws.iter().map(|r| (parsed(r), r.clone())).collect();
    db.insert_hands(&hands).unwrap();
    let analyses: Vec<_> = hands.iter().map(|(h, _)| analyze_hand(h)).collect();
    db.save_analysis(&analyses).unwrap();
    let rows = |decisions: Option<DecisionFilter>, sort_by_loss: bool| {
        db.hands(&HandQuery { decisions, sort_by_loss, limit: 10, ..Default::default() }).unwrap()
    };
    let query = |decisions: Option<DecisionFilter>, sort_by_loss: bool| -> Vec<u64> {
        rows(decisions, sort_by_loss).into_iter().map(|h| h.id).collect()
    };
    let mistakes = DecisionFilter { min_rating: Some(Rating::Inaccuracy), ..Default::default() };
    let found = rows(Some(mistakes), true);
    assert!(found.iter().any(|h| h.id == 1));
    assert!(found.iter().all(|h| h.worst.as_ref().unwrap().rating >= Some(Rating::Inaccuracy)));
    let losses: Vec<f64> = found.iter().map(|h| h.worst.as_ref().unwrap().ev_loss_bb.unwrap()).collect();
    assert!(losses.windows(2).all(|w| w[0] >= w[1]), "{losses:?}");
    let shoves = DecisionFilter { spots: vec![SpotType::Shove], ..Default::default() };
    assert_eq!(query(Some(shoves), false), [1]);
    let pending = DecisionFilter { unrated: true, ..Default::default() };
    assert_eq!(query(Some(pending), false), [5]);
    let solver = DecisionFilter { source: Some(Source::Solver), street: Some(Street::Flop), ..Default::default() };
    assert_eq!(query(Some(solver), false), [5]);
    let all = rows(None, true);
    let worst = |h: &ringside_core::db::HandSummary| h.worst.as_ref().and_then(|w| w.ev_loss_bb).unwrap_or(0.0);
    assert_eq!(all.len(), 2);
    assert!(worst(&all[0]) >= worst(&all[1]));
    let list = db.hands(&HandQuery { limit: 10, ..Default::default() }).unwrap();
    let worst = list.iter().find(|h| h.id == 1).unwrap().worst.as_ref().unwrap();
    assert_eq!(worst.spot, SpotType::Shove);

    let summary = db.analysis_summary(&Default::default(), ringside_analysis::ENGINE, 5).unwrap();
    assert_eq!((summary.hands, summary.analyzed), (2, 2));
    assert_eq!(summary.costliest[0].hand_id, all[0].id);
    let total: f64 = summary.by_spot.iter().map(|s| s.loss_bb).sum();
    assert!((total - summary.loss_bb).abs() < 1e-9);
    assert!(summary.by_spot.windows(2).all(|w| w[0].loss_bb >= w[1].loss_bb));
    assert!(summary.by_spot.iter().any(|s| s.spot == Some(SpotType::Shove)));
}

/// Every preflop decision rated by a bundled table has a chart target that opens a spot
/// existing in it, with the hero's seat and hand; estimated decisions offer none.
#[test]
fn chart_targets_hit_existing_tables() {
    use ringside_analysis::{ChartKind, chart_targets};
    use ringside_engine::range::HandClass;
    use ringside_engine::{preflop_table, pushfold_table};

    let eight: Vec<(&str, i64)> = vec![("Hero", 7400), ("P2", 7400), ("P3", 7400), ("P4", 7400), ("P5", 7400), ("P6", 7400), ("P7", 7400), ("P8", 7400)];
    let spin: [(&str, i64); 3] = [("P1", 500), ("Hero", 500), ("P3", 500)];
    let hands = [
        // Push/fold open, six players.
        k6o_fold(),
        // Preflop model, open from early position.
        hand(2, &eight, 5, 200, 25, "7c 2d", "Hero: raises 300 to 500\nP2: folds\nP3: folds\nP4: folds\nP5: folds\nP6: folds\nP7: folds\nP8: folds\nUncalled bet (300) returned to Hero\nHero collected 700 from pot\n*** SUMMARY ***\nTotal pot 700 | Rake 0\n"),
        // Preflop model, facing an open and then a 3-bet.
        hand(4, &eight, 1, 200, 25, "Ah Kd", "P5: raises 300 to 500\nP6: folds\nP7: folds\nP8: folds\nHero: raises 1100 to 1600\nP2: folds\nP3: folds\nP4: folds\nP5: folds\nUncalled bet (1100) returned to Hero\nHero collected 1475 from pot\n*** SUMMARY ***\nTotal pot 1475 | Rake 0\n"),
        // Three-handed at 10 BB on the button (a Spin & Go), push/fold.
        hand(5, &spin, 1, 50, 0, "6d 2c", "Hero: folds\nP3: folds\nUncalled bet (25) returned to P1\nP1 collected 50 from pot\n*** SUMMARY ***\nTotal pot 50 | Rake 0\n"),
        // Behind a limp with 8 BB: estimated, so no target.
        hand(3, &SIX, 4, 200, 25, "9s 9d", "P1: calls 200\nP2: folds\nP3: folds\nP4: folds\nHero: raises 800 to 1000\nP6: folds\nP1: folds\nUncalled bet (800) returned to Hero\nHero collected 850 from pot\n*** SUMMARY ***\nTotal pot 850 | Rake 0\n"),
    ];
    let mut seen = (0, 0);
    for raw in &hands {
        let h = parsed(raw);
        let rated: Vec<usize> = analyze_hand(&h)
            .decisions
            .iter()
            .filter(|d| d.street == Street::Preflop && matches!(d.source, Some(Source::Model | Source::Equilibrium)))
            .map(|d| d.action_index)
            .collect();
        let targets = chart_targets(&h);
        assert_eq!(targets.iter().map(|t| t.action_index).collect::<Vec<_>>(), rated, "{raw}");
        for t in &targets {
            let class: HandClass = t.hand.parse().unwrap_or_else(|_| panic!("{t:?}"));
            match t.kind {
                ChartKind::Pushfold => {
                    seen.0 += 1;
                    assert!(pushfold_table::stacks(t.ante).contains(&t.stack_bb), "{t:?}");
                    let sol = pushfold_table::chart(t.ante, t.players, t.stack_bb).unwrap_or_else(|| panic!("{t:?}"));
                    assert!(sol.spot(t.seat, class, t.shover, &t.callers).is_some(), "{t:?}");
                }
                ChartKind::Preflop => {
                    seen.1 += 1;
                    assert!(preflop_table::STACKS.contains(&t.stack_bb) && preflop_table::PLAYERS.contains(&t.players), "{t:?}");
                    let sol = preflop_table::chart(t.ante, t.players, t.stack_bb).unwrap_or_else(|| panic!("{t:?}"));
                    let node = t.node.and_then(|n| sol.nodes.get(n)).unwrap_or_else(|| panic!("{t:?}"));
                    assert_eq!(node.seat, t.seat, "{t:?}");
                }
            }
        }
    }
    assert!(seen.0 >= 2 && seen.1 >= 2, "{seen:?}");
}

/// The profile reads from the database what the practice derives from the hand, so a
/// variation lands in the group of its hand; the hand filter of a group lists exactly the
/// hands with such a decision, and the first practice is kept.
#[test]
fn profile_rows_match_the_hands_and_filter_them() {
    use ringside_analysis::drill;
    use ringside_core::profile::{kind_of, profile};

    let text = include_str!("fixtures/hands.txt");
    let hands: Vec<(Hand, String)> = ringside_core::split_hands(text)
        .into_iter()
        .filter_map(|raw| parse_hand(raw).ok().map(|h| (h, raw.to_string())))
        .collect();
    let mut db = Db::open_in_memory().unwrap();
    db.insert_hands(&hands).unwrap();
    let analyses: Vec<_> = hands.iter().map(|(h, _)| analyze_hand(h)).collect();
    db.save_analysis(&analyses).unwrap();

    let stored = db.profile_rows(&Default::default()).unwrap();
    let mut derived: Vec<_> = hands
        .iter()
        .zip(&analyses)
        .flat_map(|((h, _), a)| a.decisions.iter().filter(|d| d.status != Status::Forced).map(|d| drill::row(h, d)))
        .collect();
    derived.sort_by_key(|r| (r.hand_id, r.action_index));
    assert_eq!(stored.len(), derived.len());
    for (s, d) in stored.iter().zip(&derived) {
        assert_eq!(kind_of(s), kind_of(d), "hand {} @{}", s.hand_id, s.action_index);
        assert_eq!((s.hand_id, s.action_index, s.status, s.rating), (d.hand_id, d.action_index, d.status, d.rating));
    }

    let p = profile(&stored, &[]);
    assert!(p.decisions > 0);
    for g in &p.groups {
        let want: std::collections::BTreeSet<u64> =
            stored.iter().filter(|r| kind_of(r).is_some_and(|k| k.key() == g.key)).map(|r| r.hand_id).collect();
        let got: std::collections::BTreeSet<u64> = db
            .hands(&HandQuery { profile: Some(g.key.clone()), limit: 100, ..Default::default() })
            .unwrap()
            .into_iter()
            .map(|h| h.id)
            .collect();
        assert_eq!(got, want, "{}", g.key);
    }
    let none = HandQuery { profile: Some("river_bet|-|0|2".into()), limit: 10, ..Default::default() };
    assert!(db.hands(&none).unwrap().is_empty() || p.groups.iter().any(|g| g.key == "river_bet|-|0|2"));

    let key = &p.groups[0].key;
    db.mark_spot_practiced(key, 100).unwrap();
    db.mark_spot_practiced(key, 200).unwrap();
    let newest = hands.iter().map(|(h, _)| h.id).max().unwrap();
    assert_eq!(db.spot_practice().unwrap(), vec![(key.clone(), newest)]);
}
