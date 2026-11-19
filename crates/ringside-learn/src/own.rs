//! Exercises from the user's own hands: all-ins with every hand known, and push/fold
//! decisions from any seat (open shoves and calls against a shove) at the real table.
//!
//! All-in EV comes from [`hero_all_in`], the same calculation the results graph uses.

use ringside_core::allin::hero_all_in;
use ringside_core::model::{Action, ActionKind, GameKind, Hand, Seat, Street};
use ringside_engine::Card;
use serde::{Deserialize, Serialize};

use crate::fmt::{self, pct};
use crate::multiway::{self, TableSpot};
use crate::{Answer, Exercise, Fact, Feedback, Grading, Kind, PotView, Prompt, grade};

/// A decision from one of the user's hands, ready to replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwnSpot {
    /// "<kind>:<hand_id>", the key for `learn_check_own`.
    pub id: String,
    pub kind: Kind,
    pub hand_id: u64,
    /// As written in the hand history, "YYYY/MM/DD HH:MM:SS".
    pub played_at: String,
    /// Index into `Hand.actions` of the hero's decision: the replayer shows every
    /// action before it and stops there.
    pub stop_before_action: usize,
    pub exercise: Exercise,
    /// What the hero did (`shove`/`call`/`fold`); `None` for all-ins. What it cost in
    /// the model comes with the feedback of [`check_spot`], so listing spots solves nothing.
    pub played: Option<Answer>,
    /// Result of an all-in spot.
    pub all_in: Option<AllInResult>,
}

/// All-in expectation against what happened, in big blinds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AllInResult {
    /// Hero's expected share of all pots.
    pub equity_pct: f64,
    pub pot_bb: f64,
    /// Expected result: equity × pot after rake − own chips in the pot.
    pub ev_bb: f64,
    /// Actual result.
    pub net_bb: f64,
    /// `net_bb − ev_bb`.
    pub luck_bb: f64,
    pub rake_bb: f64,
}

fn cards(list: &[String]) -> Option<Vec<Card>> {
    let v: Vec<Card> = list.iter().map(|s| s.parse().ok()).collect::<Option<_>>()?;
    (v.len() == 2).then_some(v)
}

fn spot_id(kind: Kind, hand_id: u64) -> String {
    format!("{}:{hand_id}", kind.id())
}

fn exercise(kind: Kind, level: u8, hand: &Hand, task: crate::Task) -> Exercise {
    Exercise {
        kind,
        level,
        seed: 0,
        hand_id: Some(hand.id),
        question: task.question,
        prompt: task.prompt,
        answer: task.answer,
        grading: task.grading,
        model_note: task.model_note,
    }
}

/// The spots the hand offers, at most one per kind; with `only`, just that kind.
pub fn detect_spots(hand: &Hand, only: Option<Kind>) -> Vec<OwnSpot> {
    let mut spots = Vec::new();
    if only.is_none_or(|k| k == Kind::AllIn)
        && let Some(s) = all_in(hand)
    {
        spots.push(s);
    }
    if only.is_none_or(|k| k == Kind::PushFoldOpen || k == Kind::PushFoldCall)
        && let Some(s) = push_fold(hand)
        && only.is_none_or(|k| k == s.kind)
    {
        spots.push(s);
    }
    spots
}

/// Re-detects the spot `id` in `hand` and grades `answer`.
pub fn check_spot(hand: &Hand, id: &str, answer: &Answer) -> Result<(OwnSpot, Feedback), String> {
    let (kind, _) = parse_spot_id(id).ok_or("Ungültige Spot-ID")?;
    let spot = detect_spots(hand, Some(kind))
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("Diesen Spot gibt es in der Hand nicht")?;
    let mut fb = grade(&spot.exercise, answer)?;
    if let Some(played @ Answer::Choice(did)) = &spot.played {
        let loss = if played == answer { fb.ev_loss_bb } else { grade(&spot.exercise, played)?.ev_loss_bb };
        let did = match did.as_str() {
            "shove" => "All-in",
            "call" => "Call",
            _ => "Fold",
        };
        let verdict = match loss {
            Some(l) if l > 0.005 => format!("kostete {}", fmt::bb(l)),
            _ => "im Modell richtig".into(),
        };
        fb.facts.push(Fact::new("Damals gespielt", format!("{did}, {verdict}")));
    }
    if let Some(r) = &spot.all_in {
        fb.facts.push(Fact::new("Pot", fmt::bb(r.pot_bb)));
        fb.facts.push(
            Fact::new("Erwartet (EV)", fmt::signed_bb(r.ev_bb))
                .with(format!("{} × {} − Einsatz", pct(r.equity_pct), fmt::bb(r.pot_bb - r.rake_bb))),
        );
        fb.facts.push(Fact::new("Tatsächlich", fmt::signed_bb(r.net_bb)));
        fb.facts.push(Fact::new("Glück", fmt::signed_bb(r.luck_bb)).with("Tatsächlich − EV"));
    }
    Ok((spot, fb))
}

/// Solves the table of push/fold spot `id` ahead of the answer, so that grading it
/// does not wait for the solver.
pub fn prepare_spot(hand: &Hand, id: &str) -> Result<(), String> {
    let (kind, _) = parse_spot_id(id).ok_or("Ungültige Spot-ID")?;
    let spot = detect_spots(hand, Some(kind))
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("Diesen Spot gibt es in der Hand nicht")?;
    if matches!(spot.exercise.prompt, Prompt::PushFoldTable { .. }) {
        multiway::lookup_or_solve(&multiway::spot_of(&spot.exercise.prompt).params())?;
    }
    Ok(())
}

fn all_in(hand: &Hand) -> Option<OwnSpot> {
    let bb = hand.big_blind as f64;
    if bb <= 0.0 {
        return None;
    }
    let a = hero_all_in(hand)?;
    let pots: Vec<PotView> =
        a.pots.iter().map(|p| PotView { amount_bb: p.amount / bb, players: p.players.clone() }).collect();
    let pot_bb = a.pot as f64 / bb;
    let rake_bb = a.rake as f64 / bb;
    let ev_bb = a.ev / bb;
    let net_bb = a.net as f64 / bb;
    let when = match a.board.len() {
        0 => "vor dem Flop",
        3 => "am Flop",
        _ => "am Turn",
    };
    let task = crate::Task {
        question: format!("Du warst in dieser Hand {when} all-in, alle Karten liegen offen. Wie viel Equity hattest du?"),
        prompt: Prompt::AllIn { hero: a.hero_cards, opponents: a.opponents, board: a.board, pots },
        answer: crate::AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 },
        grading: Grading::Exakt,
        model_note: (hand.rake > 0).then(|| "Rake ist anteilig von jedem Pot abgezogen.".to_string()),
    };
    Some(OwnSpot {
        id: spot_id(Kind::AllIn, hand.id),
        kind: Kind::AllIn,
        hand_id: hand.id,
        played_at: hand.played_at.clone(),
        stop_before_action: a.last_hero_action,
        exercise: exercise(Kind::AllIn, 1, hand, task),
        played: None,
        all_in: Some(AllInResult { equity_pct: a.share_pct, pot_bb, ev_bb, net_bb, luck_bb: net_bb - ev_bb, rake_bb }),
    })
}

/// Largest effective stack treated as a push/fold spot.
const MAX_STACK_BB: f64 = 20.0;

/// The hero's push/fold decision at the whole table: shove or fold once everyone
/// before folded, or call or fold against a shove (also after callers). Hands with a
/// limp, a raise below all-in or already `max_all_in` players in the pot are no
/// push/fold spots. Antes of players who are not dealt in or all in with it become dead
/// money, and so does any ante above the table's lowest, which also leaves that player's stack.
fn push_fold(hand: &Hand) -> Option<OwnSpot> {
    if hand.game != GameKind::Tournament || hand.big_blind <= 0 {
        return None;
    }
    let hero = hand.hero.as_deref()?;
    let hero_cards = cards(&hand.hero_cards)?;
    let pre: Vec<(usize, &Action)> =
        hand.actions.iter().enumerate().filter(|(_, a)| a.street == Street::Preflop).collect();
    if pre.iter().any(|(_, a)| matches!(a.kind, ActionKind::PostBoth | ActionKind::PostDead | ActionKind::PostStraddle)) {
        return None;
    }
    let poster = |kind: ActionKind| -> Option<(&str, i64)> {
        let mut it = pre.iter().filter(|(_, a)| a.kind == kind);
        let (_, a) = it.next()?;
        it.next().is_none().then_some((a.player.as_str(), a.amount))
    };
    let (sb_name, sb_amount) = poster(ActionKind::PostSmallBlind)?;
    let (bb_name, bb_amount) = poster(ActionKind::PostBigBlind)?;
    if sb_amount != hand.small_blind || bb_amount != hand.big_blind || sb_name == bb_name {
        return None;
    }

    // Seats in preflop action order: the player after the big blind first, the big blind last.
    // A player all in with the ante has no decision left; the ante is dead money.
    let ante_all_in = |name: &str| pre.iter().any(|(_, a)| a.player == name && a.kind == ActionKind::PostAnte && a.all_in);
    let active: Vec<&Seat> = hand.active_players().filter(|s| !ante_all_in(&s.name)).collect();
    let n = active.len();
    let bb_at = active.iter().position(|s| s.name == bb_name)?;
    let order: Vec<&Seat> = (1..=n).map(|k| active[(bb_at + k) % n]).collect();
    if n < 2 || order[n - 2].name != sb_name {
        return None;
    }
    let seat_of = |name: &str| order.iter().position(|s| s.name == name);
    let ante_of = |name: &str| -> i64 {
        pre.iter().filter(|(_, a)| a.player == name && a.kind == ActionKind::PostAnte).map(|(_, a)| a.amount).sum()
    };
    let antes: Vec<i64> = order.iter().map(|s| ante_of(&s.name)).collect();
    let ante = *antes[..n - 1].iter().min()?;
    if antes[n - 1] < ante {
        return None;
    }
    let extra = |i: usize| if i + 1 == n { 0 } else { antes[i] - ante };
    let outside: i64 = pre
        .iter()
        .filter(|(_, a)| a.kind == ActionKind::PostAnte && seat_of(&a.player).is_none())
        .map(|(_, a)| a.amount)
        .sum();
    let chips: Vec<i64> = order.iter().enumerate().map(|(i, s)| s.stack - extra(i)).collect();
    let big = hand.big_blind as f64;
    let mut spot = TableSpot {
        stacks_bb: chips.iter().map(|&c| c as f64 / big).collect(),
        small_blind_bb: hand.small_blind as f64 / big,
        ante_bb: ante as f64 / big,
        bb_ante_bb: (antes[n - 1] - ante) as f64 / big,
        dead_bb: (outside + (0..n).map(extra).sum::<i64>()) as f64 / big,
        hero: seat_of(hero)?,
        shover: None,
        callers: Vec::new(),
    };
    if !spot.valid() {
        return None;
    }

    // Chips a seat can still put in after its ante.
    let live = |i: usize| order[i].stack - antes[i];
    // An opening raise is a shove when it is all-in or covers everyone still to act.
    let shoves = |a: &Action, i: usize| {
        a.kind == ActionKind::Raise && (a.all_in || (i + 1..n).all(|j| a.to.unwrap_or(0) >= live(j)))
    };
    let voluntary: Vec<(usize, &Action)> = pre.iter().filter(|(_, a)| !a.kind.is_post()).copied().collect();
    let hero_at = voluntary.iter().position(|(_, a)| a.player == hero)?;
    for (_, a) in &voluntary[..hero_at] {
        let Some(i) = seat_of(&a.player) else {
            if a.kind == ActionKind::Fold {
                continue;
            }
            return None;
        };
        match (a.kind, spot.shover) {
            (ActionKind::Fold, _) => {}
            (ActionKind::Raise, None) if shoves(a, i) => spot.shover = Some(i),
            (ActionKind::Call, Some(_)) => spot.callers.push(i),
            (ActionKind::Raise, Some(_)) if a.all_in => spot.callers.push(i),
            _ => return None,
        }
    }
    if spot.callers.len() + 1 >= spot.params().max_all_in {
        return None;
    }
    let (stop, action) = voluntary[hero_at];
    let (kind, played) = match (spot.shover, action.kind) {
        (None, ActionKind::Fold) => (Kind::PushFoldOpen, "fold"),
        (None, ActionKind::Raise) if shoves(action, spot.hero) => (Kind::PushFoldOpen, "shove"),
        (Some(_), ActionKind::Fold) => (Kind::PushFoldCall, "fold"),
        (Some(_), ActionKind::Call) => (Kind::PushFoldCall, "call"),
        (Some(_), ActionKind::Raise) if action.all_in => (Kind::PushFoldCall, "call"),
        _ => return None,
    };
    if spot.effective_bb() > MAX_STACK_BB {
        return None;
    }
    let task = multiway::task(&hero_cards, &spot);
    Some(OwnSpot {
        id: spot_id(kind, hand.id),
        kind,
        hand_id: hand.id,
        played_at: hand.played_at.clone(),
        stop_before_action: stop,
        exercise: exercise(kind, 3, hand, task),
        played: Some(Answer::Choice(played.into())),
        all_in: None,
    })
}

/// Parses a spot id back into kind and hand id.
pub fn parse_spot_id(id: &str) -> Option<(Kind, u64)> {
    let (kind, hand) = id.split_once(':')?;
    Some((Kind::parse(kind)?, hand.parse().ok()?))
}

