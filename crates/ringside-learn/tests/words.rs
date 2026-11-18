//! Every sentence the newer exercises build reads as German: questions, solutions and
//! the explaining lines for every answer, over many seeds and all levels.

use ringside_learn::{Answer, AnswerSpec, Kind, check, generate};

/// Seeds per level: the solved kinds are slower.
fn seeds(kind: Kind, level: u8) -> u64 {
    match (kind, level) {
        (Kind::IcmCall, 2 | 3) => 1,
        (Kind::RiverBet, _) => 6,
        (Kind::OpenFold | Kind::OpenFoldShort, _) => 15,
        _ => 60,
    }
}

const KINDS: [Kind; 7] = [Kind::BountyOdds, Kind::IcmCall, Kind::OpenFold, Kind::OpenFoldShort, Kind::RiverBet, Kind::StackDepth, Kind::HudRead];

/// A dot between digits is a thousands separator ("10.000"), never a decimal point.
fn decimal_dot(s: &str) -> bool {
    let b = s.as_bytes();
    (1..b.len().saturating_sub(1)).any(|i| {
        b[i] == b'.' && b[i - 1].is_ascii_digit() && b[i + 1].is_ascii_digit() && {
            let digits = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            digits != 3
        }
    })
}

fn check_text(what: &str, s: &str) {
    assert!(!s.trim().is_empty(), "{what}: empty");
    assert_eq!(s, s.trim(), "{what}: spaces at the ends: «{s}»");
    for bad in ["  ", " .", " ,", " ?", "..", ",,", "{", "}", "Some(", "None", "NaN", "inf ", "−0 ", "+−"] {
        assert!(!s.contains(bad), "{what}: «{bad}» in «{s}»");
    }
    assert!(!decimal_dot(s), "{what}: decimal point in «{s}»");
    // A number is followed by a space before its unit: "5 bb", "20 %", "500 $".
    for unit in ["bb", "%", "$"] {
        let at: Vec<usize> = s.match_indices(unit).map(|(i, _)| i).collect();
        for i in at {
            let before = s[..i].chars().last();
            assert!(!before.is_some_and(|c| c.is_ascii_digit()), "{what}: no space before {unit} in «{s}»");
        }
    }
    // A new sentence starts with a capital letter.
    for (i, _) in s.match_indices(". ") {
        let next = s[i + 2..].chars().next().unwrap();
        assert!(!next.is_lowercase() || s[..i].ends_with("z. B"), "{what}: lower case after a full stop in «{s}»");
    }
}

fn sentence(what: &str, s: &str, end: &[char]) {
    check_text(what, s);
    let first = s.chars().next().unwrap();
    assert!(first.is_uppercase() || first.is_ascii_digit(), "{what}: starts in lower case: «{s}»");
    assert!(end.iter().any(|&e| s.ends_with(e)), "{what}: ends without {end:?}: «{s}»");
}

#[test]
fn every_generated_sentence_reads_as_german() {
    for kind in KINDS {
        for level in 1..=3 {
            for seed in 0..seeds(kind, level) {
                let ex = generate(kind, level, seed).unwrap();
                let at = format!("{} {level} {seed}", kind.id());
                sentence(&format!("{at} Frage"), &ex.question, &['?']);
                assert_eq!(ex.question.matches('?').count(), 1, "{at}: one question: «{}»", ex.question);
                if let Some(n) = &ex.model_note {
                    sentence(&format!("{at} Hinweis"), n, &['.']);
                }
                let answers: Vec<Answer> = match &ex.answer {
                    AnswerSpec::Choice { options } => {
                        for o in options {
                            check_text(&format!("{at} Knopf"), &o.label);
                        }
                        options.iter().map(|o| Answer::Choice(o.id.clone())).collect()
                    }
                    AnswerSpec::Number { min, max, .. } => vec![Answer::Number(*min), Answer::Number(*max)],
                    other => panic!("{at}: {other:?}"),
                };
                for a in &answers {
                    let (_, fb) = check(kind, level, seed, a).unwrap();
                    check_text(&format!("{at} Lösung"), &fb.truth_text);
                    assert!(!fb.facts.is_empty(), "{at}");
                    for f in &fb.facts {
                        check_text(&format!("{at} Zeile"), &f.label);
                        assert!(f.label.chars().next().unwrap().is_uppercase() || f.label.starts_with("3-"), "{at}: «{}»", f.label);
                        check_text(&format!("{at} {}", f.label), &f.value);
                        // Words without figures are a sentence; a sentence after a figure
                        // ("35 %: Er spielt …") starts in upper case too.
                        let figures = f.value.chars().any(|c| c.is_ascii_digit());
                        if !figures && f.value.split(' ').count() > 3 {
                            sentence(&format!("{at} {}", f.label), &f.value, &['.']);
                        }
                        if let Some((_, rest)) = f.value.split_once(": ").filter(|(_, r)| r.ends_with('.')) {
                            sentence(&format!("{at} {}", f.label), rest, &['.']);
                        }
                        if let Some(x) = &f.formula {
                            check_text(&format!("{at} Formel"), x);
                        }
                    }
                }
            }
        }
    }
}
