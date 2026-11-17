//! Picks the hands worth a coach's time and computes what Ringside already knows about
//! them, so the model gets facts instead of having to ask for every number.
//!
//! A hand qualifies when the hero had a real decision: a push/fold or all-in spot, money
//! put in voluntarily, any action after the flop, or an open fold with a short stack.
//! Hands where the hero just folded a weak hand before the flop are left out. When more
//! hands qualify than fit, the most important ones stay: all-ins and push/fold spots
//! first, then by pot size.
//!
//! With payouts ([`crate::prizes`]) each hand gets its tournament phase, and push/fold
//! spots near the money and at the final table are solved with ICM as well.

use ringside_core::model::{ActionKind, Hand, Street};
use ringside_engine::{HandClass, icm};
use ringside_learn::own::check_spot;
use ringside_learn::{Answer, Kind, OwnSpot, Prompt, TableSpot, detect_spots, lookup_or_solve};
use serde::Serialize;

use crate::prizes::{Phase, Prizes, Situation, Source, players_left, situation};
use crate::render::num;

/// Up to this stack an open fold before the flop counts as a decision.
const SHORT_STACK_BB: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    AllIn,
    PushFold,
    FacedAllIn,
    Invested,
    Postflop,
    ShortStackFold,
}

impl Reason {
    pub fn text(self) -> &'static str {
        match self {
            Reason::AllIn => "All-in",
            Reason::PushFold => "Push/Fold-Spot",
            Reason::FacedAllIn => "gegen ein All-in",
            Reason::Invested => "freiwillig Geld investiert",
            Reason::Postflop => "Entscheidung nach dem Flop",
            Reason::ShortStackFold => "kurzer Stack offen gefoldet",
        }
    }
}

pub struct Selected {
    pub hand: Hand,
    pub reasons: Vec<Reason>,
    /// Facts from Ringside's engine, one German sentence each.
    pub facts: Vec<String>,
    pub spots: usize,
    pub score: f64,
    /// Players left in the tournament when the hand started, if the hands tell.
    pub left: Option<usize>,
}

pub struct Selection {
    /// In the order played.
    pub hands: Vec<Selected>,
    pub total: usize,
    /// Hero only folded before the flop without a real decision.
    pub without_decision: usize,
    /// Qualified but left out to keep the request small.
    pub dropped: usize,
}

fn stack_bb(hand: &Hand, name: &str) -> f64 {
    let stack = hand.seats.iter().find(|s| s.name == name).map(|s| s.stack).unwrap_or(0);
    stack as f64 / hand.big_blind as f64
}

fn effective_bb(hand: &Hand, hero: &str) -> f64 {
    let own = stack_bb(hand, hero);
    let biggest_other = hand
        .active_players()
        .filter(|s| s.name != hero)
        .map(|s| s.stack as f64 / hand.big_blind as f64)
        .fold(0.0, f64::max);
    own.min(biggest_other)
}

fn reasons(hand: &Hand, hero: &str, spots: &[OwnSpot]) -> Vec<Reason> {
    let mut r = Vec::new();
    if spots.iter().any(|s| s.kind == Kind::AllIn) {
        r.push(Reason::AllIn);
    }
    if spots.iter().any(|s| matches!(s.kind, Kind::PushFoldOpen | Kind::PushFoldCall)) {
        r.push(Reason::PushFold);
    }
    let own: Vec<_> = hand.actions.iter().enumerate().filter(|(_, a)| a.player == hero && !a.kind.is_post()).collect();
    let faced_all_in = own.iter().any(|(i, a)| {
        hand.actions[..*i]
            .iter()
            .any(|b| b.street == a.street && b.player != hero && b.all_in && b.kind.is_aggressive())
    });
    if faced_all_in {
        r.push(Reason::FacedAllIn);
    }
    if own.iter().any(|(_, a)| a.amount > 0) {
        r.push(Reason::Invested);
    }
    if own.iter().any(|(_, a)| a.street != Street::Preflop) {
        r.push(Reason::Postflop);
    }
    if r.is_empty() {
        let first = hand.actions.iter().position(|a| a.player == hero && !a.kind.is_post());
        let unopened = first.is_some_and(|i| {
            hand.actions[..i].iter().filter(|a| !a.kind.is_post()).all(|a| a.kind == ActionKind::Fold)
        });
        let folded = first.is_some_and(|i| hand.actions[i].kind == ActionKind::Fold);
        if unopened && folded && effective_bb(hand, hero) <= SHORT_STACK_BB {
            r.push(Reason::ShortStackFold);
        }
    }
    r
}

fn score(hand: &Hand, reasons: &[Reason]) -> f64 {
    let pot_bb = hand.total_pot as f64 / hand.big_blind as f64;
    reasons
        .iter()
        .map(|r| match r {
            Reason::AllIn => 100.0,
            Reason::PushFold => 80.0,
            Reason::FacedAllIn => 40.0,
            Reason::ShortStackFold => 30.0,
            Reason::Invested => 10.0,
            Reason::Postflop => 10.0,
        })
        .sum::<f64>()
        + pot_bb.min(60.0)
}

/// Ringside's own numbers for the spots of a hand.
fn facts(hand: &Hand, spots: &[OwnSpot], prizes: Option<&Prizes>, left: Option<usize>) -> Vec<String> {
    let mut out = Vec::new();
    let seated = hand.active_players().count();
    let sit = prizes.zip(left).map(|(p, l)| situation(p, l, seated));
    if let Some(p) = prizes {
        out.push(phase_fact(p, sit.as_ref()));
    }
    for spot in spots {
        if let Some(r) = &spot.all_in {
            out.push(format!(
                "All-in mit offenen Karten: deine Equity {} %, Pot {} bb, erwartetes Ergebnis (EV) {} bb, tatsächlich {} bb, Glück {} bb.",
                num(r.equity_pct, 1),
                num(r.pot_bb, 2),
                num(r.ev_bb, 2),
                num(r.net_bb, 2),
                num(r.luck_bb, 2),
            ));
            continue;
        }
        let Some(played) = &spot.played else { continue };
        let Prompt::PushFoldTable { positions, hero, shover, callers, .. } = &spot.exercise.prompt else { continue };
        let Ok((_, fb)) = check_spot(hand, &spot.id, played) else { continue };
        let did = match played {
            Answer::Choice(c) if c == "shove" => "all-in",
            Answer::Choice(c) if c == "call" => "Call",
            _ => "Fold",
        };
        let at = format!("{} am Tisch mit {} Spielern", positions[*hero], positions.len());
        let side = match shover {
            None => format!("Push/Fold {at}, alle vor dir haben gefoldet"),
            Some(s) => {
                let calls: String = callers.iter().map(|&c| format!(", {} callt", positions[c])).collect();
                format!("{at} gegen den Shove von {}{calls}", positions[*s])
            }
        };
        let details: Vec<String> =
            fb.facts.iter().filter(|f| f.label != "Damals gespielt").map(|f| format!("{} {}", f.label, f.value)).collect();
        let loss = fb
            .ev_loss_bb
            .map(|l| format!(" Verlust deiner Wahl gegenüber der besten: {} bb.", num(l, 2)))
            .unwrap_or_default();
        out.push(format!(
            "{side} (Push/Fold-Gleichgewicht am ganzen Tisch, Chip-EV, höchstens 3 Spieler all-in, 3-Wege-Equity geschätzt): {}. Nash spielt hier: {}. Du hast {did} gespielt.{loss}",
            details.join(", "),
            fb.truth_text,
        ));
        if let (Some(p), Some(sit)) = (prizes, sit.as_ref().filter(|s| s.icm())) {
            out.extend(icm_fact(&spot.exercise.prompt, played, p, sit));
        }
    }
    out
}

fn source_text(p: &Prizes) -> String {
    match p.source {
        Source::Summary => "Auszahlungen aus der Turnier-Zusammenfassung".into(),
        Source::Standard => format!(
            "angenommene Standard-Struktur, nicht die echten Auszahlungen: {} von {}{} Spielern bezahlt",
            p.pct.len(),
            if p.entrants_known { "" } else { "mindestens " },
            p.entrants
        ),
    }
}

fn pcts(v: &[f64]) -> String {
    v.iter().map(|x| format!("{} %", num(*x, 2))).collect::<Vec<_>>().join(", ")
}

/// The tournament phase of a hand and what it means for ICM.
fn phase_fact(p: &Prizes, sit: Option<&Situation>) -> String {
    let Some(sit) = sit else {
        return format!("Turnierphase unbekannt: Die Hände verraten nicht, wie viele Spieler noch dabei sind. Rechne mit Chip-EV; ICM-Aussagen sind Einschätzungen ({}).", source_text(p));
    };
    let (left, paid) = (sit.left, sit.paid);
    let approx = format!(
        "ICM-Näherung, weil die Stacks an den anderen Tischen unbekannt sind: Jeder Platz an deinem Tisch steht für {} der {left} offenen Plätze und zahlt deren Durchschnitt (andere Tische wie deiner, ähnlich dem MTT-Modus von ICMIZER)",
        num(left as f64 / sit.table.len().max(1) as f64, 1),
    );
    let text = match sit.phase {
        Phase::Early => format!("Turnierphase früh: etwa {left} Spieler übrig, {paid} bezahlt. Hier gilt Chip-EV; ICM spielt noch kaum eine Rolle."),
        Phase::Bubble => format!("Turnierphase nahe der Bubble: etwa {left} Spieler übrig, {paid} bezahlt. {approx}."),
        Phase::InTheMoney => format!("Turnierphase im Geld: etwa {left} Spieler übrig, {paid} bezahlt. {approx}."),
        Phase::FinalTable => format!("Finaltisch: {left} Spieler übrig, alle an deinem Tisch. ICM nach Malmuth-Harville mit den verbleibenden Auszahlungen."),
    };
    let table = if sit.icm() { format!(" Auszahlungen für die Plätze an deinem Tisch in % des Preispools: {}.", pcts(&sit.table)) } else { String::new() };
    format!("{text}{table} Quelle: {}.", source_text(p))
}

/// The push/fold spot solved again with the table's ICM payouts, plus the bubble factor
/// against a lone shover.
fn icm_fact(prompt: &Prompt, played: &Answer, p: &Prizes, sit: &Situation) -> Option<String> {
    let Prompt::PushFoldTable { class, positions, stacks_bb, hero, shover, callers, small_blind_bb, ante_bb, bb_ante_bb, dead_bb, .. } =
        prompt
    else {
        return None;
    };
    let class: HandClass = class.parse().ok()?;
    let mut payouts = sit.table.clone();
    payouts.truncate(stacks_bb.len());
    let spot = TableSpot {
        stacks_bb: stacks_bb.clone(),
        small_blind_bb: *small_blind_bb,
        ante_bb: *ante_bb,
        bb_ante_bb: *bb_ante_bb,
        dead_bb: *dead_bb,
        hero: *hero,
        shover: *shover,
        callers: callers.clone(),
    };
    let sol = lookup_or_solve(&spot.params().with_payouts(payouts.clone())).ok()?;
    let s = sol.spot(*hero, class, *shover, callers)?;
    let act = if shover.is_some() { "Call" } else { "All-in" };
    let acted = matches!(played, Answer::Choice(c) if c == "shove" || c == "call");
    let chosen = if acted { s.ev_act } else { s.ev_fold };
    let loss = s.ev_act.max(s.ev_fold) - chosen;
    let money = |pct: f64| p.pool.map(|pool| format!(" (≈ {})", num(pct / 100.0 * pool as f64 / 100.0, 2))).unwrap_or_default();
    let mut text = format!(
        "Dasselbe mit ICM ({}): Nash spielt {class} hier zu {} % {act}. EV {act} {} %, EV Fold {} % des Preispools; Verlust deiner Wahl {} % des Preispools{}.",
        match sit.phase {
            Phase::FinalTable => "Finaltisch, exakt im Modell",
            _ => "Tisch-Näherung",
        },
        num(100.0 * s.freq, 1),
        num(s.ev_act, 3),
        num(s.ev_fold, 3),
        num(loss, 3),
        money(loss),
    );
    if let (Some(v), true) = (*shover, callers.is_empty())
        && let Some(bf) = bubble_factor(stacks_bb, &payouts, *hero, v)
    {
        text.push_str(&format!(
            " Bubble-Faktor gegen {}: {} (1 wäre Chip-EV; darüber kostet Verlieren mehr, als Gewinnen bringt).",
            positions[v],
            num(bf, 2)
        ));
    }
    Some(text)
}

/// What losing the effective stack to `villain` costs in ICM equity per unit gained by
/// winning it (Bill Chen's bubble factor), with blinds and antes left out.
fn bubble_factor(stacks: &[f64], payouts: &[f64], hero: usize, villain: usize) -> Option<f64> {
    let eff = stacks[hero].min(stacks[villain]);
    let ev = |d: f64| -> Option<f64> {
        let mut s = stacks.to_vec();
        s[hero] += d;
        s[villain] -= d;
        Some(icm(&s, payouts).ok()?.ev[hero])
    };
    let now = ev(0.0)?;
    let gain = ev(eff)? - now;
    (gain > 0.0).then(|| (now - ev(-eff).unwrap_or(now)) / gain)
}

pub fn select(hands: Vec<Hand>, max: usize, prizes: Option<&Prizes>) -> Selection {
    let total = hands.len();
    let left = players_left(&hands);
    let mut without_decision = 0;
    let mut picked: Vec<(usize, Vec<OwnSpot>, Selected)> = Vec::new();
    for (i, hand) in hands.into_iter().enumerate() {
        let Some(hero) = hand.hero.clone() else { continue };
        if hand.big_blind <= 0 || hand.hero_cards.is_empty() {
            continue;
        }
        let spots = detect_spots(&hand, None);
        let reasons = reasons(&hand, &hero, &spots);
        if reasons.is_empty() {
            without_decision += 1;
            continue;
        }
        let score = score(&hand, &reasons);
        picked.push((i, spots, Selected { spots: 0, hand, reasons, facts: Vec::new(), score, left: left[i] }));
    }
    let qualified = picked.len();
    picked.sort_by(|a, b| b.2.score.total_cmp(&a.2.score).then(a.0.cmp(&b.0)));
    picked.truncate(max);
    picked.sort_by_key(|(i, ..)| *i);
    // Push/fold facts solve the table, so only the hands that stay get them.
    let mut facts = all_facts(&picked, prizes);
    let hands: Vec<Selected> = picked
        .into_iter()
        .zip(&mut facts)
        .map(|((_, spots, mut s), f)| {
            s.facts = std::mem::take(f);
            s.spots = spots.len();
            s
        })
        .collect();
    Selection { dropped: qualified - hands.len(), hands, total, without_decision }
}

/// Solves run a few at a time: one table alone leaves most cores idle, and a solve
/// takes seconds at a full table.
const FACT_THREADS: usize = 4;

fn all_facts(picked: &[(usize, Vec<OwnSpot>, Selected)], prizes: Option<&Prizes>) -> Vec<Vec<String>> {
    let mut out = vec![Vec::new(); picked.len()];
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..FACT_THREADS)
            .map(|k| {
                scope.spawn(move || {
                    let mine = picked.iter().enumerate().skip(k).step_by(FACT_THREADS);
                    mine.map(|(i, (_, spots, s))| (i, facts(&s.hand, spots, prizes, s.left))).collect::<Vec<_>>()
                })
            })
            .collect();
        for worker in workers {
            for (i, f) in worker.join().unwrap_or_default() {
                out[i] = f;
            }
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hand(body: &str, id: u64) -> Hand {
        let text = format!(
            "PokerStars Hand #{id}: Tournament #77, $5+$0.50 USD Hold'em No Limit - Level V (50/100) - 2026/11/16 20:{:02}:00 CET [2026/11/16 14:00:00 ET]
Table '77 1' 6-max Seat #1 is the button
Seat 1: Hero (3000 in chips)
Seat 2: Ann (800 in chips)
Seat 3: Bob (5000 in chips)
Seat 4: Cid (4000 in chips)
Ann: posts small blind 50
Bob: posts big blind 100
*** HOLE CARDS ***
Dealt to Hero [Ah 7c]
{body}
*** SUMMARY ***
Total pot 150 | Rake 0
",
            id % 60
        );
        ringside_core::parse_hand(&text).unwrap()
    }

    #[test]
    fn leaves_out_plain_folds_and_keeps_decisions() {
        let fold = hand("Cid: folds\nHero: folds\nAnn: folds\nBob collected 150 from pot", 1);
        let call = hand(
            "Cid: raises 200 to 300\nHero: calls 300\nAnn: folds\nBob: folds\n*** FLOP *** [2c 3d 9h]\nCid: bets 400\nHero: folds\nUncalled bet (400) returned to Cid\nCid collected 750 from pot",
            2,
        );
        let s = select(vec![fold, call], 10, None);
        assert_eq!(s.total, 2);
        assert_eq!(s.without_decision, 1);
        assert_eq!(s.hands.len(), 1);
        assert_eq!(s.hands[0].hand.id, 2);
        assert!(s.hands[0].reasons.contains(&Reason::Invested));
        assert!(s.hands[0].reasons.contains(&Reason::Postflop));
    }

    #[test]
    fn keeps_the_most_important_hands_in_played_order() {
        let small = |id| {
            hand(
                "Cid: folds\nHero: calls 100\nAnn: folds\nBob: checks\n*** FLOP *** [2c 3d 9h]\nBob: checks\nHero: checks\n*** TURN *** [2c 3d 9h] [Ks]\nBob: checks\nHero: checks\n*** RIVER *** [2c 3d 9h Ks] [4d]\nBob: checks\nHero: checks\nBob collected 250 from pot",
                id,
            )
        };
        let shove = hand(
            "Cid: folds\nHero: folds\nAnn: raises 700 to 800 and is all-in\nBob: calls 700\n*** FLOP *** [2c 3d 9h]\n*** TURN *** [2c 3d 9h] [Ks]\n*** RIVER *** [2c 3d 9h Ks] [4d]\n*** SHOW DOWN ***\nAnn: shows [Qs Qd]\nBob: shows [Kc Kd]\nBob collected 1600 from pot",
            12,
        );
        let faced = hand(
            "Cid: folds\nHero: raises 200 to 300\nAnn: raises 500 to 800 and is all-in\nBob: folds\nHero: calls 500\n*** FLOP *** [2c 3d 9h]\n*** TURN *** [2c 3d 9h] [Ks]\n*** RIVER *** [2c 3d 9h Ks] [4d]\n*** SHOW DOWN ***\nHero: shows [Ah 7c]\nAnn: shows [Qs Qd]\nAnn collected 1700 from pot",
            13,
        );
        let s = select(vec![small(10), small(11), shove, faced, small(14)], 2, None);
        assert_eq!(s.dropped, 2);
        assert_eq!(s.without_decision, 1, "Hero hat bei der 12 nur gefoldet");
        let ids: Vec<u64> = s.hands.iter().map(|h| h.hand.id).collect();
        assert_eq!(ids, vec![10, 13]);
        let faced = s.hands.iter().find(|h| h.hand.id == 13).unwrap();
        assert!(faced.reasons.contains(&Reason::FacedAllIn));
        assert!(faced.reasons.contains(&Reason::AllIn), "{:?}", faced.reasons);
        assert!(faced.facts.iter().any(|f| f.contains("Equity")), "{:?}", faced.facts);
    }

    /// A 12 bb open fold on the button is a push/fold decision at the whole table.
    #[test]
    fn short_stack_open_fold_is_a_push_fold_spot() {
        let mut h = hand("Cid: folds\nHero: folds\nAnn: folds\nBob collected 150 from pot", 20);
        for s in &mut h.seats {
            if s.name == "Hero" {
                s.stack = 120_000;
            }
        }
        let s = select(vec![h], 10, None);
        assert_eq!(s.hands.len(), 1);
        assert_eq!(s.hands[0].reasons, vec![Reason::PushFold]);
        let fact = &s.hands[0].facts[0];
        assert!(fact.starts_with("Push/Fold BTN am Tisch mit 4 Spielern"), "{fact}");
        assert!(fact.contains("Du hast Fold gespielt"), "{fact}");
    }

    /// Heads-up, the hero folds 12 bb in the small blind; a player busting in place
    /// `left` in this hand says that `left` players were in when it started.
    fn short_fold(left: i64) -> Hand {
        let text = "PokerStars Hand #21: Tournament #77, $5+$0.50 USD Hold'em No Limit - Level V (50/100) - 2026/11/16 20:21:00 CET [2026/11/16 14:00:00 ET]
Table '77 1' 9-max Seat #1 is the button
Seat 1: Hero (1200 in chips)
Seat 2: Bob (5000 in chips)
Hero: posts small blind 50
Bob: posts big blind 100
*** HOLE CARDS ***
Dealt to Hero [Kh 6d]
Hero: folds
Bob collected 100 from pot
*** SUMMARY ***
Total pot 100 | Rake 0
";
        let mut h = ringside_core::parse_hand(text).unwrap();
        h.finishes = vec![ringside_core::model::Finish { player: "Ann".into(), place: Some(left), prize: None, currency: None }];
        h
    }

    fn row(entrants: i64, payouts: Vec<i64>) -> ringside_core::tournament::TournamentRow {
        ringside_core::tournament::TournamentRow {
            id: 77,
            game: String::new(),
            currency: Some("USD".into()),
            buy_in: None,
            cost: None,
            entrants: Some(entrants),
            place: None,
            prize: None,
            bounties: 0,
            profit: None,
            started_at: String::new(),
            hands: 1,
            chips: 0,
            has_summary: !payouts.is_empty(),
            payouts,
            name: None,
        }
    }

    fn facts_with(row: &ringside_core::tournament::TournamentRow, h: Hand) -> Vec<String> {
        let p = crate::prizes::prizes(row, std::slice::from_ref(&h));
        select(vec![h], 10, p.as_ref()).hands.remove(0).facts
    }

    /// Two left, both at the hero's table: the summary's payouts, exact ICM.
    #[test]
    fn final_table_uses_the_summary_payouts() {
        let facts = facts_with(&row(9, vec![5000, 3000, 2000]), short_fold(2));
        let all = facts.join("\n");
        assert!(all.contains("Finaltisch: 2 Spieler übrig"), "{all}");
        assert!(all.contains("Quelle: Auszahlungen aus der Turnier-Zusammenfassung"), "{all}");
        assert!(all.contains("in % des Preispools: 50 %, 30 %."), "{all}");
        assert!(all.contains("Dasselbe mit ICM (Finaltisch, exakt im Modell)"), "{all}");
        assert!(all.contains("(≈ "), "money from the summary's prize pool: {all}");
    }

    /// Without a summary the standard structure is used and said to be an assumption.
    #[test]
    fn without_a_summary_the_standard_structure_is_labelled() {
        let facts = facts_with(&row(9, Vec::new()), short_fold(3));
        let all = facts.join("\n");
        assert!(all.contains("angenommene Standard-Struktur, nicht die echten Auszahlungen: 2 von 9 Spielern bezahlt"), "{all}");
        // Three left, two seated: the table's places stand for 1.5 field places each.
        assert!(all.contains("Turnierphase nahe der Bubble: etwa 3 Spieler übrig, 2 bezahlt"), "{all}");
        assert!(all.contains("in % des Preispools: 56,67 %, 10 %."), "{all}");
        assert!(all.contains("Dasselbe mit ICM"), "{all}");
        assert!(!all.contains("(≈ "), "no money without real payouts: {all}");
    }

    /// Far from the money the coach is told chip EV, and no ICM solve runs.
    #[test]
    fn early_phase_is_chip_ev() {
        let facts = facts_with(&row(1000, Vec::new()), short_fold(600));
        let all = facts.join("\n");
        assert!(all.contains("Turnierphase früh: etwa 600 Spieler übrig, 200 bezahlt. Hier gilt Chip-EV"), "{all}");
        assert!(!all.contains("Dasselbe mit ICM"), "{all}");
    }
}
