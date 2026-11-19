//! Every reason the analysis writes reads as German: all forms of the sentence templates,
//! and the reasons of the fixture hands for every source.

use ringside_analysis::drill::answer;
use ringside_analysis::why::{Chart, Estimated, Move, Ranked, Solved, Third, estimate, ranked, solver};
use ringside_analysis::{Choice, Source, Status, analyze_hand, estimate_rest};
use ringside_core::{parse_hand, split_hands};

/// A comma between digits is the decimal mark; a dot only separates thousands.
fn decimal_dot(s: &str) -> bool {
    let b = s.as_bytes();
    (1..b.len().saturating_sub(1)).any(|i| b[i] == b'.' && b[i - 1].is_ascii_digit() && b[i + 1].is_ascii_digit())
}

fn check(what: &str, s: &str) {
    assert!(!s.trim().is_empty(), "{what}: empty");
    assert_eq!(s, s.trim(), "{what}: spaces at the ends: «{s}»");
    for bad in ["  ", " .", " ,", " :", "..", ",,", ":.", "{", "}", "Some(", "None", "NaN", "inf", "−0 ", "-0 ", " 0 %", "+−", " 0 BB mehr"] {
        assert!(!s.contains(bad), "{what}: «{bad}» in «{s}»");
    }
    assert!(!decimal_dot(s), "{what}: decimal point in «{s}»");
    // Reasons are read by casual players: plain words, not the solver's vocabulary.
    for word in ["Range", "Equity", "SPR", "Shove", "Value", "Tiefe"] {
        assert!(!s.contains(word), "{what}: «{word}» in «{s}»");
    }
    for unit in ["BB", "%"] {
        for (i, _) in s.match_indices(unit) {
            assert!(!s[..i].ends_with(|c: char| c.is_ascii_digit()), "{what}: no space before {unit} in «{s}»");
        }
    }
    let first = s.chars().next().unwrap();
    assert!(first.is_uppercase(), "{what}: starts in lower case: «{s}»");
    assert!(s.ends_with('.'), "{what}: ends without a full stop: «{s}»");
    for (i, _) in s.match_indices(". ").chain(s.match_indices(": ")) {
        let next = s[i + 2..].chars().next().unwrap();
        assert!(next.is_uppercase() || next.is_ascii_digit(), "{what}: lower case after «{}» in «{s}»", &s[i..i + 1]);
    }
    // Every percentage is a whole number between 1 and 100.
    for (i, _) in s.match_indices(" %") {
        let digits: String = s[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
        let n: u32 = digits.parse().unwrap_or_else(|_| panic!("{what}: % without a number in «{s}»"));
        assert!((1..=100).contains(&n), "{what}: {n} % in «{s}»");
        assert!(n != 100 || !s[..i].ends_with("über 100"), "{what}: «{s}»");
    }
}

const LABELS: [(&str, Move); 7] = [
    ("Fold", Move::Fold),
    ("Check", Move::Check),
    ("Call", Move::Call),
    ("Limp", Move::Limp),
    ("Bet 3 BB", Move::Bet),
    ("Raise auf 7,5 BB", Move::Raise),
    ("All-in", Move::AllIn),
];

fn key(m: Move) -> &'static str {
    match m {
        Move::Fold => "fold",
        Move::Check => "check",
        Move::Call => "call",
        Move::Limp => "limp",
        Move::Bet => "bet",
        Move::Raise => "raise",
        Move::AllIn => "all_in",
    }
}

/// Two-move strategies: each move first, against each other, with a clear gap and with none.
fn strategies() -> Vec<Vec<Choice>> {
    let mut out = Vec::new();
    for (a, ma) in LABELS {
        for (b, mb) in LABELS {
            if a == b {
                continue;
            }
            for second in [0.2, 1.48] {
                let c = |label: &str, m: Move, ev: f64, played: bool| Choice { label: label.into(), freq: 0.5, ev_bb: Some(ev), played, kind: key(m).into() };
                out.push(vec![c(a, ma, 1.5, false), c(b, mb, second, true)]);
            }
        }
    }
    out
}

#[test]
fn every_template_form_reads_as_german() {
    let mut n = 0;
    for s in strategies() {
        for third in [None, Some(Third::Top), Some(Third::Middle), Some(Third::Bottom)] {
            for equity in [0.004, 0.2, 0.45, 0.7, 1.0] {
                for need in [None, Some(0.25), Some(0.6)] {
                    for river in [false, true] {
                        for fold in [None, Some(0.0), Some(0.41)] {
                            let text = solver(&Solved { equity, third, need, river, fold }, &s, 10.0);
                            check("Solver", &text);
                            n += 1;
                        }
                    }
                }
            }
        }
        for chart in [Chart::Model, Chart::Equilibrium] {
            for act in std::iter::once(None).chain(Move::ALL.map(Some)) {
                for all_hands in [false, true] {
                    for rank in [0.0, 0.1, 0.3, 0.32, 0.7, 1.0] {
                        let f = Ranked { chart, place: "Als BTN mit 25 BB".into(), act, share: 0.32, all_hands, class: "KJo".into(), rank };
                        check("Rangliste", &ranked(&f, &s, 10.0));
                        n += 1;
                    }
                }
            }
        }
        for equity in [0.004, 0.3, 0.62, 1.0] {
            for opponents in [1, 3] {
                for need in [None, Some(0.29), Some(0.6)] {
                    for fold in [None, Some(0.35)] {
                        for spr in [None, Some(0.4), Some(2.0), Some(4.5), Some(11.0)] {
                            check("Einschätzung", &estimate(&Estimated { equity, opponents, need, fold, spr }, &s, 10.0));
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(n > 10_000, "{n}");
}

#[test]
fn the_reason_names_what_the_source_measured() {
    let s = strategies().into_iter().find(|s| s[0].kind == "raise" && s[1].kind == "call" && s[1].ev_bb == Some(0.2)).unwrap();
    let f = Solved { equity: 0.21, third: Some(Third::Bottom), need: Some(0.25), river: false, fold: Some(0.47) };
    assert_eq!(
        solver(&f, &s, 10.0),
        "Deine Hand gehört zum schwächsten Drittel der Hände, die du hier haben kannst. Gegen die Hände, die der Gegner hier haben kann, gewinnt sie in 21 % der Fälle. Damit ist der Einsatz ein Bluff: Er gewinnt vor allem, wenn der Gegner aufgibt. Auf Raise auf 7,5 BB gibt der Gegner in 47 % der Fälle auf. Ein Call bräuchte 25 %, so oft gewinnst du nicht. Raise auf 7,5 BB bringt 1,3 BB mehr als Call."
    );
    let f = Ranked { chart: Chart::Equilibrium, place: "Als SB bei 10 BB effektiv".into(), act: Some(Move::AllIn), share: 0.38, all_hands: true, class: "A7o".into(), rank: 0.21 };
    let fold = strategies().into_iter().find(|s| s[0].kind == "all_in" && s[1].kind == "fold" && s[1].ev_bb == Some(0.2)).unwrap();
    assert_eq!(
        ranked(&f, &fold, 10.0),
        "Als SB bei 10 BB effektiv: Das Gleichgewicht geht mit 38 % aller Hände all-in. A7o gehört zu den besten 21 % aller Hände, also klar dazu. All-in bringt 1,3 BB mehr als Fold."
    );
    // A gap within the noise of a solve is no reason.
    let close = strategies().into_iter().find(|s| s[0].kind == "all_in" && s[1].kind == "fold" && s[1].ev_bb == Some(1.48)).unwrap();
    assert!(!ranked(&f, &close, 10.0).contains("mehr als"));
    let e = Estimated { equity: 0.31, opponents: 2, need: Some(0.33), fold: None, spr: None };
    assert_eq!(
        estimate(&e, &s, 10.0),
        "Gegen die Hände, die 2 Gegner wahrscheinlich haben, gewinnt deine Hand geschätzt in 31 % der Fälle. Ein Call braucht 33 %, das ist knapp. Erhöhen bringt geschätzt 1,3 BB mehr als Callen. Mit mehreren Gegnern ist die Schätzung unsicherer als heads-up."
    );
}

/// The reasons of real hands, for every source the fixtures reach: German, and the same
/// whichever move is chosen when the spot is decided again.
#[test]
fn every_fixture_reason_reads_as_german_and_holds_for_any_choice() {
    let text = include_str!("fixtures/hands.txt");
    let mut sources = std::collections::BTreeSet::new();
    for raw in split_hands(text) {
        let Ok(hand) = parse_hand(raw) else { continue };
        let decisions = analyze_hand(&hand).decisions;
        let pending: Vec<_> = decisions.iter().filter(|d| d.status == Status::Pending).cloned().collect();
        let estimated = estimate_rest(&hand, pending, "Für den Test geschätzt.");
        for d in decisions.iter().filter(|d| d.status == Status::Rated).chain(&estimated) {
            if d.status != Status::Rated {
                continue;
            }
            let source = d.source.unwrap();
            check(&format!("#{} @{} {source:?}", d.hand_id, d.action_index), &d.why);
            sources.insert(format!("{source:?}"));
            for c in 0..d.strategy.len() {
                assert_eq!(answer(d, c).unwrap().decision.why, d.why, "#{} @{}", d.hand_id, d.action_index);
            }
            if std::env::var_os("RINGSIDE_PRINT_WHY").is_some() {
                println!("{source:?} #{} {}: {}\n  {}\n", d.hand_id, hand.hero_cards.join(""), d.played, d.why);
            }
        }
    }
    for s in [Source::Model, Source::Equilibrium, Source::Estimate] {
        assert!(sources.contains(&format!("{s:?}")), "no {s:?} in {sources:?}");
    }
}
