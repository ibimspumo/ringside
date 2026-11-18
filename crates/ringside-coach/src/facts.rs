//! The facts a question to the coach is about, as anonymous German text: a tournament
//! report or the rated decisions of one hand. Every figure in it is computed by Ringside;
//! the model only explains them.

use std::fmt::Write;

use ringside_core::db::{Decision, Rating, Source, Status};
use ringside_core::model::{Hand, Street};
use ringside_core::report::{Breakdown, Moment, Report};
use ringside_core::tournament::TournamentRow;

use crate::render::{Names, num, render_hand};

fn rating(r: Rating) -> &'static str {
    match r {
        Rating::Best => "beste Wahl",
        Rating::Mix => "gemischt, gleich gut",
        Rating::Good => "gut",
        Rating::Inaccuracy => "Ungenauigkeit",
        Rating::Mistake => "Fehler",
        Rating::Blunder => "grober Fehler",
    }
}

fn source(s: Source) -> &'static str {
    match s {
        Source::Equilibrium => "Push/Fold-Gleichgewicht",
        Source::Model => "Preflop-Modell",
        Source::Solver => "Solver",
        Source::Estimate => "Einschätzung aus Equity und Pot Odds",
        Source::Forced => "keine Entscheidung",
    }
}

fn street(s: Street) -> &'static str {
    match s {
        Street::Preflop => "vor dem Flop",
        Street::Flop => "am Flop",
        Street::Turn => "am Turn",
        Street::River => "am River",
    }
}

fn signed(v: f64, digits: usize) -> String {
    let s = num(v, digits);
    if v > 0.0 && s != "0" { format!("+{s}") } else { s }
}

fn chips(hundredths: i64) -> String {
    signed(hundredths as f64 / 100.0, 0)
}

fn money(hundredths: i64, currency: Option<&str>) -> String {
    let unit = match currency {
        Some("USD") => " $",
        Some("EUR") => " €",
        Some("GBP") => " £",
        Some(_) => " Punkte",
        None => "",
    };
    format!("{}{unit}", num(hundredths as f64 / 100.0, 2))
}

fn breakdown(out: &mut String, b: &Breakdown) {
    let groups: Vec<String> = b
        .groups
        .iter()
        .take(4)
        .map(|g| {
            let unsure = if g.unsure { ", überwiegend aus Einschätzungen" } else { "" };
            format!("{} {} BB ({} Entscheidungen, {} ungenau oder schlechter{unsure})", g.title, num(g.loss_bb, 1), g.decisions, g.flawed)
        })
        .collect();
    if !groups.is_empty() {
        let _ = writeln!(out, "Verlust nach {}: {}.", b.title, groups.join("; "));
    }
}

fn moment(out: &mut String, i: usize, m: &Moment, mistake: bool) {
    let cards = if m.hero_cards.is_empty() { "unbekannte Karten".to_string() } else { m.hero_cards.clone() };
    let seat = m.position.as_deref().map(|p| format!(" auf {p}")).unwrap_or_default();
    let stack = m.stack_bb.map(|s| format!(", {} BB effektiv", num(s, 0))).unwrap_or_default();
    let _ = write!(out, "{}. Hand {} des Turniers, {cards}{seat}{stack}, {} ({}): gespielt {}", i + 1, m.n, m.situation, street(m.street), m.played);
    if mistake {
        if let Some(best) = &m.best {
            let _ = write!(out, ", besser {best}");
        }
        let _ = write!(out, ", Verlust {} BB", num(m.loss_bb, 2));
    } else if let (Some(margin), Some(alt)) = (m.margin_bb, &m.alternative) {
        let _ = write!(out, ", {alt} wäre {} BB schlechter gewesen", num(margin, 2));
    }
    if let Some(s) = m.source {
        let _ = write!(out, ", Quelle {}", source(s));
    }
    if !m.why.is_empty() {
        let _ = write!(out, ". Begründung von Ringside: {}", m.why);
    }
    out.push('\n');
}

/// The figures of a tournament report. Names of tables, players and the tournament stay out.
pub fn report_facts(t: &TournamentRow, r: &Report) -> String {
    let mut out = String::new();
    let cur = t.currency.as_deref();
    match (t.place, t.prize) {
        (Some(p), Some(prize)) => {
            let field = t.entrants.map(|e| format!(" von {e}")).unwrap_or_default();
            let _ = write!(out, "Ergebnis: Platz {p}{field}, Preisgeld {}", money(prize, cur));
        }
        (None, Some(_)) => out.push_str("Ergebnis: ausgeschieden ohne Preisgeld"),
        _ => out.push_str("Ergebnis: noch offen"),
    }
    if t.bounties > 0 {
        let _ = write!(out, ", Bounties {}", money(t.bounties, cur));
    }
    if let Some(profit) = t.profit {
        let _ = write!(out, ", Gewinn nach Buy-in {}", money(profit, cur));
    }
    out.push_str(".\n");
    let l = &r.luck;
    let _ = writeln!(
        out,
        "Hände: {}. Chips gewonnen oder verloren: {} ({} BB). Mit normalem Glück, also jedes All-in mit allen Karten offen zum Erwartungswert: {} ({} BB). Glück: {} Chips ({} BB) aus {} All-ins.",
        r.hands,
        chips(l.net),
        signed(l.net_bb, 1),
        chips(l.ev),
        signed(l.ev_bb, 1),
        chips(l.luck),
        signed(l.luck_bb, 1),
        l.all_ins
    );
    let s = &r.score;
    let _ = write!(
        out,
        "Bewertete Entscheidungen: {}, davon gut {} ({} %), Ungenauigkeiten {}, Fehler {}. EV-Verlust zusammen {} BB",
        s.decisions,
        s.good,
        num(s.good_pct.unwrap_or(0.0), 1),
        s.inaccuracies,
        s.mistakes,
        num(s.loss_bb, 1)
    );
    if let Some(p) = s.per100 {
        let _ = write!(out, ", das sind {} BB je 100 Entscheidungen", num(p, 1));
    }
    if s.estimate_loss_pct > 0.0 {
        let _ = write!(out, "; {} % des Verlusts aus Einschätzungen", num(s.estimate_loss_pct, 0));
    }
    out.push_str(".\n");
    if let Some(a) = &r.average {
        let _ = writeln!(
            out,
            "Dein Schnitt aus {} früheren Turnieren: {} BB je 100 Entscheidungen, {} % gut.",
            a.tournaments,
            a.per100.map_or("–".into(), |p| num(p, 1)),
            a.good_pct.map_or("–".into(), |p| num(p, 1))
        );
    }
    for b in &r.breakdowns {
        breakdown(&mut out, b);
    }
    if !r.worst.is_empty() {
        out.push_str("Die teuersten Fehler:\n");
        for (i, m) in r.worst.iter().enumerate() {
            moment(&mut out, i, m, true);
        }
    }
    if !r.best.is_empty() {
        out.push_str("Die besten Entscheidungen:\n");
        for (i, m) in r.best.iter().enumerate() {
            moment(&mut out, i, m, false);
        }
    }
    if let Some(p) = &r.practice {
        let _ = writeln!(out, "Als Nächstes üben: {} ({}), {} BB Verlust in {} Entscheidungen.", p.title, p.detail, num(p.loss_bb, 1), p.decisions);
    }
    if r.pending > 0 || r.unanalyzed > 0 {
        let _ = writeln!(out, "Vorläufig: {} Entscheidungen nach dem Flop warten noch auf den Solver, {} Hände sind noch nicht bewertet.", r.pending, r.unanalyzed);
    }
    out
}

/// One hand with Ringside's verdict on each decision; `at` marks the decision asked about.
/// Player names in the verdicts are replaced like in the hand.
pub fn hand_facts(hand: &Hand, decisions: &[Decision], at: Option<usize>) -> Option<String> {
    let mut names = Names::default();
    let mut out = render_hand(hand, "Hand", &mut names)?;
    let mut labels: Vec<(String, String)> = hand.seats.iter().map(|s| (s.name.clone(), names.get(hand, &s.name))).collect();
    // Longer names first, so one name inside another is replaced whole.
    labels.sort_by_key(|(n, _)| std::cmp::Reverse(n.len()));
    let anon = |text: &str| labels.iter().fold(text.to_string(), |t, (name, label)| t.replace(name.as_str(), label));
    out.push_str("\nBewertung von Ringside:\n");
    for d in decisions.iter().filter(|d| d.status != Status::Forced) {
        let mark = if Some(d.action_index) == at { " ← danach fragt der Spieler" } else { "" };
        let _ = write!(out, "- {}: gespielt {}{mark}", street(d.street), anon(&d.played));
        match d.status {
            Status::Rated => {
                if let Some(best) = &d.best {
                    let _ = write!(out, "; Empfehlung {}", anon(best));
                }
                let mix: Vec<String> = d
                    .strategy
                    .iter()
                    .map(|c| {
                        let ev = c.ev_bb.map(|e| format!(", EV {} BB", num(e, 2))).unwrap_or_default();
                        format!("{} {} %{ev}", anon(&c.label), num(c.freq * 100.0, 0))
                    })
                    .collect();
                if !mix.is_empty() {
                    let _ = write!(out, "; Strategie: {}", mix.join(", "));
                }
                if let Some(r) = d.rating {
                    let _ = write!(out, "; Urteil {}", rating(r));
                }
                if let Some(l) = d.ev_loss_bb {
                    let _ = write!(out, "; EV-Verlust {} BB", num(l, 2));
                }
                if let Some(s) = d.source {
                    let _ = write!(out, "; Quelle {}", source(s));
                }
            }
            Status::Pending => out.push_str("; wird noch vom Solver gerechnet"),
            _ => {
                let _ = write!(out, "; nicht bewertet{}", d.reason.as_deref().map(|r| format!(" ({})", anon(r))).unwrap_or_default());
            }
        }
        if !d.why.is_empty() {
            let _ = write!(out, ". Begründung: {}", anon(&d.why));
        }
        if !d.note.is_empty() {
            let _ = write!(out, ". Annahmen: {}", anon(&d.note));
        }
        out.push('\n');
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use ringside_core::db::{Choice, SpotType};
    use ringside_core::report::{ReportHand, report};

    use super::*;

    const HAND: &str = "PokerStars Hand #260000000001: Tournament #4046789300, $5+$0.50 USD Hold'em No Limit - Level V (50/100) - 2026/11/16 20:10:00 CET [2026/11/16 14:10:00 ET]
Table '4046789300 10' 8-max Seat #2 is the button
Seat 1: Hero (8725 in chips)
Seat 2: Jorge (4618 in chips)
Seat 3: Astrid (1200 in chips)
Astrid: posts small blind 50
Hero: posts big blind 100
*** HOLE CARDS ***
Dealt to Hero [Ah Ad]
Jorge: folds
Astrid: raises 1100 to 1200 and is all-in
Hero: calls 1100
*** FLOP *** [2c 7d Jh]
*** TURN *** [2c 7d Jh] [Qs]
*** RIVER *** [2c 7d Jh Qs] [3d]
*** SHOW DOWN ***
Astrid: shows [Kc Kd] (a pair of Kings)
Hero: shows [Ah Ad] (a pair of Aces)
Hero collected 2450 from pot
*** SUMMARY ***
Total pot 2450 | Rake 0
";

    fn decision() -> Decision {
        Decision {
            hand_id: 260000000001,
            action_index: 4,
            street: Street::Preflop,
            spot: SpotType::CallShove,
            status: Status::Rated,
            played: "Call 11 BB".into(),
            best: Some("Call".into()),
            strategy: vec![
                Choice { label: "Fold".into(), freq: 0.0, ev_bb: Some(0.0), played: false, kind: "fold".into() },
                Choice { label: "Call".into(), freq: 1.0, ev_bb: Some(6.4), played: true, kind: "call".into() },
            ],
            played_freq: Some(1.0),
            ev_loss_bb: Some(0.0),
            rating: Some(Rating::Best),
            source: Some(Source::Equilibrium),
            why: "Gegen den Shove von Astrid ist Call klar besser.".into(),
            note: "Jorge hat gefoldet.".into(),
            reason: None,
            solver: None,
            engine: "e".into(),
            pot_bb: Some(24.5),
            reliability: None,
        }
    }

    #[test]
    fn the_facts_of_a_hand_name_no_player() {
        let hand = ringside_core::parse_hand(HAND).unwrap();
        let text = hand_facts(&hand, &[decision()], Some(4)).unwrap();
        for name in ["Hero", "Jorge", "Astrid", "4046789300", "260000000001"] {
            assert!(!text.contains(name), "{name} in:\n{text}");
        }
        assert!(text.contains("Shove von Spieler"), "{text}");
        assert!(text.contains("← danach fragt der Spieler"));
        assert!(text.contains("EV 6,4 BB"));
    }

    #[test]
    fn the_facts_of_a_report_carry_its_figures_and_no_names() {
        let hands = vec![ReportHand {
            hand_id: 7,
            played_at: "2026/11/16 20:10:00".into(),
            hero_cards: "Ah Ad".into(),
            class: Some("AA".into()),
            position: Some("BB".into()),
            net: 125_000,
            big_blind: 10_000,
            ev: Some(40_000),
            stack: Some(872_500),
        }];
        let r = report(&hands, &[], 0, "2026/11/16", &[]);
        let t = TournamentRow {
            id: 4046789300,
            game: "NL Hold'em".into(),
            currency: Some("USD".into()),
            buy_in: None,
            cost: Some(550),
            entrants: Some(18),
            place: Some(3),
            prize: Some(1530),
            bounties: 0,
            profit: Some(980),
            started_at: "2026/11/16 20:00:00".into(),
            hands: 1,
            chips: 125_000,
            has_summary: true,
            payouts: vec![],
            name: Some("Mini Battle".into()),
        };
        let text = report_facts(&t, &r);
        assert!(text.contains("Platz 3 von 18, Preisgeld 15,3 $"), "{text}");
        assert!(text.contains("Glück: +850 Chips (+8,5 BB) aus 1 All-ins"), "{text}");
        assert!(!text.contains("4046789300") && !text.contains("Mini Battle"));
    }
}
