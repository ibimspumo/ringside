use ringside_engine::{Combo, HandClass};
use ringside_learn::{Answer, AnswerSpec, Exercise, Kind, Prompt, TableSpot, check, generate, lookup_or_solve};

/// Seeds per (kind, level); the exact equity and Nash kinds cost a second each in debug,
/// a table solve (with ICM too) up to a few seconds.
fn seeds(kind: Kind) -> u64 {
    match kind {
        Kind::PushFoldOpen | Kind::PushFoldCall | Kind::IcmCall => 1,
        Kind::EquityHand | Kind::PushFoldSb | Kind::PushFoldBb => 2,
        Kind::RiverBet => 4,
        Kind::OpenFold | Kind::OpenFoldShort => 10,
        _ => 25,
    }
}

fn generated() -> impl Iterator<Item = Kind> {
    Kind::ALL.into_iter().filter(|k| k.generated())
}

/// Same seed, same exercise; the expected answer always grades as fully correct.
#[test]
fn every_kind_is_deterministic_and_its_truth_scores_full() {
    for kind in generated() {
        for level in 1..=3 {
            for seed in 0..seeds(kind) {
                let a = generate(kind, level, seed).unwrap();
                assert_eq!(a, generate(kind, level, seed).unwrap(), "{kind:?} {level} {seed}");
                assert_eq!((a.kind, a.level, a.seed, a.hand_id), (kind, level, seed, None));
                assert!(!a.question.is_empty());
                let any = match &a.answer {
                    AnswerSpec::Number { .. } => Answer::Number(-1.0),
                    AnswerSpec::Choice { options } => Answer::Choice(options[0].id.clone()),
                    AnswerSpec::Cards { .. } => Answer::Cards(Vec::new()),
                    AnswerSpec::Seat { seats } => Answer::Seat(seats[0]),
                };
                let (_, first) = check(kind, level, seed, &any).unwrap_or_else(|e| panic!("{kind:?} {level} {seed}: {e}"));
                let (_, fb) = check(kind, level, seed, &first.truth).unwrap();
                assert!(fb.correct && fb.score == 1.0, "{kind:?} {level} {seed}: {fb:?}");
                assert!(!fb.facts.is_empty());
                assert_eq!(fb.grading, a.grading);
                assert_eq!(
                    fb.model_note.is_some(),
                    matches!(a.grading, ringside_learn::Grading::Modell | ringside_learn::Grading::Gleichgewicht)
                );
            }
        }
    }
}

#[test]
fn different_seeds_give_different_exercises() {
    for kind in generated().filter(|k| seeds(*k) > 2) {
        let prompts: Vec<Prompt> = (0..10).map(|s| generate(kind, 2, s).unwrap().prompt).collect();
        let distinct = prompts.iter().enumerate().filter(|(i, p)| !prompts[..*i].contains(p)).count();
        assert!(distinct >= 5, "{kind:?}: {distinct} distinct of 10");
    }
}

#[test]
fn rejects_unknown_levels_and_own_only_kinds() {
    assert!(generate(Kind::Outs, 0, 1).is_err());
    assert!(generate(Kind::Outs, 4, 1).is_err());
    assert!(generate(Kind::AllIn, 1, 1).is_err());
}

#[test]
fn json_shapes_are_stable() {
    let json = |a: &Answer| serde_json::to_string(a).unwrap();
    assert_eq!(json(&Answer::Number(22.5)), r#"{"number":22.5}"#);
    assert_eq!(json(&Answer::Choice("fold".into())), r#"{"choice":"fold"}"#);
    assert_eq!(json(&Answer::Cards(vec!["Ah".into()])), r#"{"cards":["Ah"]}"#);
    assert_eq!(json(&Answer::Seat(3)), r#"{"seat":3}"#);
    let ex = generate(Kind::Position, 2, 4).unwrap();
    let text = serde_json::to_string(&ex).unwrap();
    assert!(text.contains(r#""kind":"position""#) && text.contains(r#""type":"position""#), "{text}");
    let back: Exercise = serde_json::from_str(&text).unwrap();
    assert_eq!(back, ex);
    let ex = generate(Kind::Outs, 1, 4).unwrap();
    let text = serde_json::to_string(&ex).unwrap();
    assert!(text.contains(r#""grading":"exakt""#) && text.contains(r#""type":"cards""#), "{text}");
}

#[test]
fn outs_answers_by_cards_and_by_count() {
    let ex = generate(Kind::Outs, 2, 7).unwrap();
    let AnswerSpec::Cards { from } = &ex.answer else { panic!() };
    let (_, fb) = check(Kind::Outs, 2, 7, &Answer::Cards(Vec::new())).unwrap();
    let Answer::Cards(outs) = &fb.truth else { panic!() };
    assert!(!fb.correct && fb.score == 0.0);
    assert!(outs.iter().all(|c| from.iter().any(|f| f.to_string() == *c)));
    // Every out but one plus one wrong card.
    let wrong = from.iter().map(|c| c.to_string()).find(|c| !outs.contains(c)).unwrap();
    let mut picked: Vec<String> = outs[1..].to_vec();
    picked.push(wrong);
    let (_, fb) = check(Kind::Outs, 2, 7, &Answer::Cards(picked)).unwrap();
    let expected = (outs.len() as f64 - 2.0) / outs.len() as f64;
    assert!(!fb.correct && (fb.score - expected.max(0.0)).abs() < 1e-12, "{}", fb.score);
    let (_, fb) = check(Kind::Outs, 2, 7, &Answer::Number(outs.len() as f64)).unwrap();
    assert!(fb.correct);
}

#[test]
fn estimates_are_scored_in_bands() {
    let (_, fb) = check(Kind::PotOdds, 2, 3, &Answer::Number(0.0)).unwrap();
    let Answer::Number(need) = fb.truth else { panic!() };
    let score = |x: f64| check(Kind::PotOdds, 2, 3, &Answer::Number(x)).unwrap().1;
    assert!(score(need + 1.9).correct && score(need - 1.9).score == 1.0);
    let half = score(need + 6.0);
    assert!(!half.correct && (half.score - 0.5).abs() < 1e-9);
    assert_eq!(score(need + 10.5).score, 0.0);
}

#[test]
fn push_fold_reports_ev_loss() {
    for seed in 0..2 {
        let (_, fb) = check(Kind::PushFoldSb, 1, seed, &Answer::Choice("fold".into())).unwrap();
        let loss = fb.ev_loss_bb.unwrap();
        // Level 1 asks clear spots: a wrong answer costs at least the closeness margin.
        if fb.correct {
            assert_eq!(loss, 0.0);
        } else {
            assert!(loss >= 0.3 && fb.score == 0.0, "{fb:?}");
        }
        let other = if fb.correct { "shove" } else { "fold" };
        let (_, fb2) = check(Kind::PushFoldSb, 1, seed, &Answer::Choice(other.into())).unwrap();
        assert_ne!(fb.correct, fb2.correct);
    }
}

/// Table exercises: the level decides the table, and both answers are graded with the
/// solver's frequency and EVs for the dealt class at that node.
#[test]
fn table_push_fold_follows_the_level_and_the_solver() {
    for kind in [Kind::PushFoldOpen, Kind::PushFoldCall] {
        for level in 1..=3 {
            let ex = generate(kind, level, 0).unwrap();
            let Prompt::PushFoldTable {
                ref hand, ref stacks_bb, small_blind_bb, ante_bb, bb_ante_bb, dead_bb, hero, shover, ref callers, ..
            } = ex.prompt
            else {
                panic!("{:?}", ex.prompt)
            };
            let t = TableSpot {
                stacks_bb: stacks_bb.clone(),
                small_blind_bb,
                ante_bb,
                bb_ante_bb,
                dead_bb,
                hero,
                shover,
                callers: callers.clone(),
            };
            let n = stacks_bb.len();
            let equal = stacks_bb.iter().all(|&x| x == stacks_bb[0]);
            match level {
                1 => assert!(n <= 6 && equal && ante_bb == 0.0 && bb_ante_bb == 0.0),
                2 => assert!(n >= 6 && equal && ante_bb + bb_ante_bb > 0.0),
                _ => assert!(n >= 3 && !equal),
            }
            assert_eq!(shover.is_none(), kind == Kind::PushFoldOpen);
            assert!(level == 3 || callers.is_empty());
            assert!(t.effective_bb() <= 20.0);

            let class: HandClass = Combo::new(hand[0], hand[1]).class();
            let truth = lookup_or_solve(&t.params()).unwrap().spot(hero, class, shover, callers).unwrap();
            let close = (truth.freq > 0.0 && truth.freq < 1.0) || (truth.ev_act - truth.ev_fold).abs() < 0.3;
            assert_eq!(close, level == 3, "{kind:?} {level}: {truth:?}");
            let act = if kind == Kind::PushFoldOpen { "shove" } else { "call" };
            for (answer, acted) in [(act, true), ("fold", false)] {
                let (_, fb) = check(kind, level, 0, &Answer::Choice(answer.into())).unwrap();
                let chosen = if acted { truth.ev_act } else { truth.ev_fold };
                assert_eq!(fb.correct, if acted { truth.freq > 0.0 } else { truth.freq < 1.0 });
                assert!((fb.ev_loss_bb.unwrap() - (truth.ev_act.max(truth.ev_fold) - chosen)).abs() < 1e-12);
            }
        }
    }
}

/// Equal stacks on the bundled grid come from the precomputed table, without solving.
#[test]
fn bundled_tables_answer_without_solving() {
    let p = ringside_engine::multiway::Params::equal(9, 10.0);
    let start = std::time::Instant::now();
    let s = ringside_learn::lookup_or_solve(&p).unwrap();
    assert_eq!(s.players, 9);
    assert!(start.elapsed() < std::time::Duration::from_secs(2), "{:?}: solved instead of read", start.elapsed());
}
