//! Card math: outs, pot odds, hit probabilities and preflop combinatorics.

use rand::Rng;
use ringside_engine::cards::binomial;
use ringside_engine::outs::{category, rank_of};
use ringside_engine::{Card, CardSet, HandCategory, HandClass, Range, outs_to};

use crate::deal::{self, Rng8};
use crate::fmt::{self, pct};
use crate::score::{band, partial};
use crate::{
    Answer, AnswerSpec, ChoiceOption, Exercise, Fact, Feedback, Grading, Highlight, HighlightTag, Prompt,
    ProbabilityMethod, Task, choice, feedback, number,
};

const TRIES: usize = 200_000;

fn hole(v: &[Card]) -> [Card; 2] {
    [v[0], v[1]]
}

/// Cards that improve the hero to a better category with a hole card playing but are
/// neither outs nor half outs.
fn dirty_outs(hero: [Card; 2], villain: [Card; 2], board: &[Card]) -> Vec<Card> {
    let r = ringside_engine::outs(hero, Some(villain), board).expect("valid spot");
    let mut now = hero.to_vec();
    now.extend(board);
    let Some(next) = HandCategory::ALL.into_iter().find(|&c| c > category(&now)) else {
        return Vec::new();
    };
    let improving = outs_to(hero, board, next).expect("valid spot").outs;
    improving
        .into_iter()
        .filter(|c| !villain.contains(c) && !r.outs.contains(c) && !r.half_outs.contains(c))
        .collect()
}

pub fn outs(level: u8, rng: &mut Rng8) -> Result<Task, String> {
    for _ in 0..TRIES {
        let mut used = CardSet::EMPTY;
        let hero = hole(&deal::cards(rng, &mut used, 2));
        let villain = hole(&deal::cards(rng, &mut used, 2));
        let board_len = match level {
            1 => 3,
            2 if rng.random_bool(0.5) => 3,
            _ => 4,
        };
        let board = deal::cards(rng, &mut used, board_len);
        if rank_of(&hero, &board) >= rank_of(&villain, &board) {
            continue;
        }
        let r = ringside_engine::outs(hero, Some(villain), &board).map_err(|e| e.to_string())?;
        let n = r.outs.len();
        let ok = match level {
            1 => (4..=15).contains(&n) && r.half_outs.is_empty(),
            2 => (2..=20).contains(&n),
            _ => (2..=20).contains(&n) && !dirty_outs(hero, villain, &board).is_empty(),
        };
        if !ok {
            continue;
        }
        let street = if board_len == 3 { "Turn" } else { "River" };
        let mut question = format!(
            "Du liegst hinten. Welche Karten bringen dich am {street} nach vorn? Tippe alle Outs an."
        );
        if level >= 2 {
            question.push_str(" Karten, die zu einem Split führen, sind halbe Outs.");
        }
        if level == 3 {
            question.push_str(" Achtung: Nicht jede Karte, die deine Hand verbessert, gewinnt.");
        }
        let from = CardSet::from_cards(&[&hero[..], &villain[..], &board[..]].concat()).complement().to_vec();
        return Ok(Task {
            question,
            prompt: Prompt::Outs { hero: hero.to_vec(), villain: Some(villain.to_vec()), board },
            answer: AnswerSpec::Cards { from },
            grading: Grading::Exakt,
            model_note: None,
        });
    }
    Err("Keine passende Outs-Aufgabe gefunden".into())
}

fn parse_cards(list: &[String]) -> Result<Vec<Card>, String> {
    let mut out: Vec<Card> = Vec::new();
    for s in list {
        let c: Card = s.parse().map_err(|_| format!("'{s}' ist keine Karte"))?;
        if !out.contains(&c) {
            out.push(c);
        }
    }
    Ok(out)
}

pub fn grade_outs(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Outs { hero, villain: Some(villain), board } = &ex.prompt else {
        return Err("Outs-Aufgabe ohne Gegnerhand".into());
    };
    let (hero, villain) = (hole(hero), hole(villain));
    let r = ringside_engine::outs(hero, Some(villain), board).map_err(|e| e.to_string())?;
    let (n, m) = (r.outs.len(), r.half_outs.len());
    let effective = r.effective();
    let dirty = if ex.level == 3 { dirty_outs(hero, villain, board) } else { Vec::new() };

    let mut highlight = Vec::new();
    let (correct, score) = match answer {
        Answer::Cards(list) => {
            let picked = parse_cards(list)?;
            let hits = picked.iter().filter(|c| r.outs.contains(c)).count();
            let wrong: Vec<Card> =
                picked.iter().filter(|c| !r.outs.contains(c) && !r.half_outs.contains(c)).copied().collect();
            for &c in &r.outs {
                let tag = if picked.contains(&c) { HighlightTag::Out } else { HighlightTag::Missed };
                highlight.push(Highlight { card: c, tag });
            }
            for &c in &wrong {
                highlight.push(Highlight { card: c, tag: HighlightTag::Wrong });
            }
            let score = (hits as f64 - wrong.len() as f64) / n as f64;
            (hits == n && wrong.is_empty(), score)
        }
        Answer::Number(x) => {
            for &c in &r.outs {
                highlight.push(Highlight { card: c, tag: HighlightTag::Out });
            }
            // Half outs may be counted as nothing, half or whole.
            let (lo, hi) = (n as f64, (n + m) as f64);
            let err = if *x < lo { lo - x } else if *x > hi { x - hi } else { 0.0 };
            (err == 0.0, if err == 0.0 { 1.0 } else { partial(err, n as f64) })
        }
        _ => return Err("Hier werden Karten oder eine Anzahl erwartet".into()),
    };
    for &c in &r.half_outs {
        highlight.push(Highlight { card: c, tag: HighlightTag::HalfOut });
    }
    for &c in &dirty {
        if !highlight.iter().any(|h| h.card == c) {
            highlight.push(Highlight { card: c, tag: HighlightTag::Dirty });
        }
    }

    let unseen = 52 - 4 - board.len();
    let mut facts = vec![
        Fact::new("Deine Hand", fmt::category(rank_of(&hero, board).category())),
        Fact::new("Gegner", fmt::category(rank_of(&villain, board).category())),
        Fact::new("Outs", n.to_string()),
    ];
    if m > 0 {
        facts.push(Fact::new("Halbe Outs", m.to_string()));
        facts.push(Fact::new("Gezählt", fmt::short(effective)).with(format!("{n} + {m} ÷ 2")));
    }
    if !dirty.is_empty() {
        facts.push(Fact::new("Schmutzige Outs", fmt::cards(&dirty)));
    }
    facts.push(
        Fact::new("Trefferchance nächste Karte", pct(100.0 * effective / unseen as f64))
            .with(format!("{} ÷ {unseen}", fmt::short(effective))),
    );
    let mut fb = feedback(correct, score, Answer::Cards(r.outs.iter().map(Card::to_string).collect()), format!("{} Outs", fmt::short(effective)), facts);
    fb.highlight = highlight;
    Ok(fb)
}

const BET_FRACTIONS: [f64; 8] = [0.25, 1.0 / 3.0, 0.5, 2.0 / 3.0, 0.75, 1.0, 1.5, 2.0];

pub fn pot_odds(level: u8, rng: &mut Rng8) -> Task {
    let (pot_before, bet) = if level == 1 {
        let pot = rng.random_range(2..=30) as f64;
        let f = *deal::pick(rng, &BET_FRACTIONS);
        (pot, ((f * pot * 2.0).round() / 2.0).max(0.5))
    } else {
        let pot = rng.random_range(300..=6000) as f64 / 100.0;
        let f: f64 = rng.random_range(0.2..1.5);
        (pot, ((f * pot * 20.0).round() / 20.0).max(0.1))
    };
    let pot = pot_before + bet;
    let to_call = bet;
    let fmt_amount = |x: f64| if level == 1 { fmt::short(x) } else { fmt::money(x) };
    let mut question = format!(
        "Im Pot liegen {} bb, der Einsatz deines Gegners eingerechnet. Du musst {} bb callen.",
        fmt_amount(pot),
        fmt_amount(to_call)
    );
    if level < 3 {
        question.push_str(" Wie viel Equity brauchst du mindestens für den Call?");
        return Task {
            question,
            prompt: Prompt::PotOdds { pot, to_call, equity_pct: None },
            answer: AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 },
            grading: Grading::Formel,
            model_note: None,
        };
    }
    let need = 100.0 * to_call / (pot + to_call);
    let equity = loop {
        let delta: f64 = rng.random_range(2.0..15.0) * if rng.random_bool(0.5) { 1.0 } else { -1.0 };
        let e = (need + delta).round();
        if (1.0..=99.0).contains(&e) && (e - need).abs() >= 2.0 {
            break e;
        }
    };
    question.push_str(&format!(" Deine Equity beträgt {}. Callst du?", pct(equity)));
    Task {
        question,
        prompt: Prompt::PotOdds { pot, to_call, equity_pct: Some(equity) },
        answer: AnswerSpec::Choice {
            options: vec![ChoiceOption::new("call", "Call"), ChoiceOption::new("fold", "Fold")],
        },
        grading: Grading::Modell,
        model_note: Some(
            "Ohne implizite Odds: Nach dem Call wird nicht mehr gesetzt, und die Equity gilt bis zum Showdown."
                .into(),
        ),
    }
}

pub fn grade_pot_odds(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::PotOdds { pot, to_call, equity_pct } = ex.prompt else { unreachable!() };
    let amount = |x: f64| if ex.level == 1 { fmt::short(x) } else { fmt::money(x) };
    let need = 100.0 * to_call / (pot + to_call);
    let mut facts = vec![
        Fact::new("Benötigte Equity", pct(need))
            .with(format!("{} ÷ ({} + {})", amount(to_call), amount(pot), amount(to_call))),
        Fact::new("Pot Odds", format!("{} : 1", fmt::num(pot / to_call, 1)))
            .with(format!("{} : {}", amount(pot), amount(to_call))),
        Fact::new("Pot nach dem Call", fmt::bb(pot + to_call)),
    ];
    let Some(equity) = equity_pct else {
        let x = number(answer)?;
        let err = x - need;
        let score = band(err, 2.0, 10.0);
        return Ok(feedback(err.abs() <= 2.0, score, Answer::Number(need), pct(need), facts));
    };
    let call = choice(answer)? == "call";
    let ev_call = equity / 100.0 * (pot + to_call) - to_call;
    let best_call = ev_call > 0.0;
    facts.push(Fact::new("Deine Equity", pct(equity)));
    facts.push(
        Fact::new("EV Call", fmt::signed_bb(ev_call))
            .with(format!("{} × {} − {}", fmt::num(equity / 100.0, 2), amount(pot + to_call), amount(to_call))),
    );
    facts.push(Fact::new("EV Fold", fmt::signed_bb(0.0)));
    let correct = call == best_call;
    let loss = if correct { 0.0 } else { ev_call.abs() };
    let (truth, text) = if best_call { ("call", "Call") } else { ("fold", "Fold") };
    let mut fb = feedback(correct, if correct { 1.0 } else { 0.0 }, Answer::Choice(truth.into()), text.into(), facts);
    fb.ev_loss_bb = Some(loss);
    Ok(fb)
}

const COMMON_OUTS: [u8; 6] = [4, 6, 8, 9, 12, 15];

fn exact_hit(outs: u8, unseen: u8, cards: u8) -> f64 {
    let miss = binomial((unseen - outs) as u64, cards as u64) as f64 / binomial(unseen as u64, cards as u64) as f64;
    100.0 * (1.0 - miss)
}

pub fn probability(level: u8, rng: &mut Rng8) -> Task {
    let outs = if rng.random_bool(0.6) { *deal::pick(rng, &COMMON_OUTS) } else { rng.random_range(2..=17) };
    let (unseen, cards_to_come, method) = match level {
        1 => {
            if rng.random_bool(0.5) { (47, 2, ProbabilityMethod::Rule) } else { (47 - rng.random_range(0..=1), 1, ProbabilityMethod::Rule) }
        }
        2 => (47 - rng.random_range(0..=1), 1, ProbabilityMethod::Exact),
        _ => (47, 2, ProbabilityMethod::Exact),
    };
    let street = match (unseen, cards_to_come) {
        (47, 2) => "bis zum River (Turn und River)",
        (47, _) => "am Turn",
        _ => "am River",
    };
    let question = match method {
        ProbabilityMethod::Rule => format!(
            "Du hast {outs} Outs. Wie groß ist die Chance, {street} zu treffen – nach der Regel von 2 und 4?"
        ),
        ProbabilityMethod::Exact => format!(
            "Du hast {outs} Outs, {unseen} Karten sind unbekannt. Wie groß ist die Chance, {street} zu treffen – exakt?"
        ),
    };
    Task {
        question,
        prompt: Prompt::Probability { outs, unseen, cards_to_come, method },
        answer: AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 },
        grading: if method == ProbabilityMethod::Rule { Grading::Formel } else { Grading::Exakt },
        model_note: None,
    }
}

pub fn grade_probability(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Probability { outs, unseen, cards_to_come, method } = ex.prompt else { unreachable!() };
    let x = number(answer)?;
    let exact = exact_hit(outs, unseen, cards_to_come);
    let factor = if cards_to_come == 2 { 4.0 } else { 2.0 };
    let rule = outs as f64 * factor;
    let exact_formula = if cards_to_come == 1 {
        format!("{outs} ÷ {unseen}")
    } else {
        format!("1 − ({} ÷ {unseen}) × ({} ÷ {})", unseen - outs, unseen - outs - 1, unseen - 1)
    };
    let mut facts = vec![
        Fact::new("Regel von 2 und 4", pct(rule)).with(format!("{outs} × {factor}")),
        Fact::new("Exakt", pct(exact)).with(exact_formula),
        Fact::new("Abweichung der Regel", format!("{} Prozentpunkte", fmt::num(rule - exact, 1))),
    ];
    if cards_to_come == 2 && outs > 8 {
        let corrected = 4.0 * outs as f64 - (outs as f64 - 8.0);
        facts.push(Fact::new("Korrigierte Regel", pct(corrected)).with(format!("4 × {outs} − ({outs} − 8)")));
    }
    let (truth, full, zero) = match (method, ex.level) {
        (ProbabilityMethod::Rule, _) => (rule, 0.5, 10.0),
        (_, 2) => (exact, 1.0, 10.0),
        _ => (exact, 2.0, 12.0),
    };
    let err = x - truth;
    Ok(feedback(err.abs() <= full, band(err, full, zero), Answer::Number(truth), pct(truth), facts))
}

const COMBO_RANGES: [&str; 8] = [
    "JJ+, AKs, AKo",
    "TT+, AQs+, AQo+",
    "88+, ATs+, KQs, AJo+",
    "QQ+, AKs",
    "22+",
    "A2s+",
    "KTs+, QTs+, JTs",
    "99+, AJs+, KQs, AQo+",
];

pub fn combos(level: u8, rng: &mut Rng8) -> Task {
    let answer = AnswerSpec::Number { unit: "Kombos".into(), min: 0.0, max: 1326.0, step: 1.0 };
    match level {
        1 => {
            let class = HandClass::from_index(rng.random_range(0..169));
            Task {
                question: format!("Wie viele Kombos gibt es von {class}?"),
                prompt: Prompt::Combos { class: Some(class.to_string()), range: None, dead: Vec::new() },
                answer,
                grading: Grading::Exakt,
                model_note: None,
            }
        }
        2 => loop {
            let class = HandClass::from_index(rng.random_range(0..169));
            let (hi, lo) = class.ranks();
            let mut used = CardSet::EMPTY;
            let n = rng.random_range(1..=3);
            let dead = deal::cards(rng, &mut used, n);
            if !dead.iter().any(|c| c.rank() == hi || c.rank() == lo) {
                continue;
            }
            break Task {
                question: format!(
                    "Du kennst diese Karten: {}. Wie viele Kombos von {class} sind noch möglich?",
                    fmt::cards(&dead)
                ),
                prompt: Prompt::Combos { class: Some(class.to_string()), range: None, dead },
                answer,
                grading: Grading::Exakt,
                model_note: None,
            };
        },
        _ => {
            let range = deal::pick(rng, &COMBO_RANGES).to_string();
            let mut used = CardSet::EMPTY;
            let dead = deal::cards(rng, &mut used, 3);
            Task {
                question: format!(
                    "Auf dem Flop liegt {}. Wie viele Kombos hat die Range {range} noch?",
                    fmt::cards(&dead)
                ),
                prompt: Prompt::Combos { class: None, range: Some(range), dead },
                answer,
                grading: Grading::Exakt,
                model_note: None,
            }
        }
    }
}

/// Live combos of a class and how to count them by hand.
fn class_count(class: HandClass, dead: CardSet) -> (usize, String) {
    let (hi, lo) = class.ranks();
    let left = |rank: u8| (0..4).filter(|&s| !dead.contains(Card::new(rank, s))).count();
    let suits_free = (0..4).filter(|&s| !dead.contains(Card::new(hi, s)) && !dead.contains(Card::new(lo, s))).count();
    if class.is_pair() {
        let n = left(hi);
        (n * n.saturating_sub(1) / 2, format!("{n} × {} ÷ 2", n.saturating_sub(1)))
    } else if class.is_suited() {
        (suits_free, format!("{suits_free} freie Farben"))
    } else {
        let (a, b) = (left(hi), left(lo));
        (a * b - suits_free, format!("{a} × {b} − {suits_free}"))
    }
}

pub fn grade_combos(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::Combos { class, range, dead } = &ex.prompt else { unreachable!() };
    let dead = CardSet::from_cards(dead);
    let x = number(answer)?;
    let classes: Vec<HandClass> = match (class, range) {
        (Some(c), _) => vec![c.parse().map_err(|_| "ungültige Hand")?],
        (_, Some(r)) => {
            let range = Range::parse(r).map_err(|e| e.to_string())?;
            HandClass::all().filter(|c| range.weight(c.combos()[0]) > 0.0).collect()
        }
        _ => return Err("Kombo-Aufgabe ohne Hand".into()),
    };
    let mut facts = Vec::new();
    let (mut total, mut full) = (0, 0);
    for c in &classes {
        let (n, how) = class_count(*c, dead);
        total += n;
        full += c.combo_count() as usize;
        if dead.is_empty() {
            let base = match c.combo_count() {
                6 => "4 × 3 ÷ 2",
                4 => "4 Farben",
                _ => "4 × 4 − 4",
            };
            facts.push(Fact::new(&c.to_string(), n.to_string()).with(base));
        } else if n != c.combo_count() as usize || classes.len() == 1 {
            facts.push(Fact::new(&c.to_string(), n.to_string()).with(how));
        }
    }
    if classes.len() > 1 {
        facts.push(Fact::new("Ohne bekannte Karten", full.to_string()));
        facts.push(Fact::new("Blockiert", (full - total).to_string()));
    }
    facts.push(Fact::new("Übrig", total.to_string()));
    let err = x - total as f64;
    let correct = err.abs() < 0.5;
    let score = if correct { 1.0 } else { partial(err, (total as f64).max(4.0)) };
    Ok(feedback(correct, score, Answer::Number(total as f64), format!("{total} Kombos"), facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_counted_combos_match_the_range_engine() {
        let mut rng = deal::rng(crate::Kind::Combos, 9, 1);
        for _ in 0..2000 {
            let mut used = CardSet::EMPTY;
            let n = rng.random_range(0..=5);
            let dead = CardSet::from_cards(&deal::cards(&mut rng, &mut used, n));
            let class = HandClass::from_index(rng.random_range(0..169));
            let engine = Range::parse(&class.to_string()).unwrap().live_combos(dead).len();
            assert_eq!(class_count(class, dead).0, engine, "{class} with {dead}");
        }
    }

    #[test]
    fn exact_hit_chances() {
        assert!((exact_hit(9, 47, 1) - 19.149).abs() < 0.01);
        assert!((exact_hit(9, 47, 2) - 34.97).abs() < 0.01);
        assert!((exact_hit(15, 47, 2) - 54.12).abs() < 0.01);
    }
}
