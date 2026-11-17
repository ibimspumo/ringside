//! Exercises from the user's own hands: all-ins with every hand known, small-blind
//! push/fold spots and big-blind calls against a small-blind shove.
//!
//! All-in EV is exact for any number of players: every side pot gets its own exact
//! equity among the players who can win it, with the other known hands removed from
//! the deck. Rake is taken from each pot in proportion to its size, and bounties are
//! left out (chips only).

use ringside_core::model::{ActionKind, GameKind, Hand, Street};
use ringside_engine::Card;
use serde::{Deserialize, Serialize};

use crate::equity::all_in_shares;
use crate::fmt::{self, pct};
use crate::pushfold::{self, Spot};
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
    /// What the hero did (`shove`/`call`/`fold`); `None` for all-ins.
    pub played: Option<Answer>,
    /// What that decision cost in the model, in big blinds.
    pub played_ev_loss_bb: Option<f64>,
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
    if only.is_none_or(|k| k == Kind::PushFoldSb || k == Kind::PushFoldBb)
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

fn all_in(hand: &Hand) -> Option<OwnSpot> {
    let hero = hand.hero.as_deref()?;
    let hero_cards = cards(&hand.hero_cards)?;
    let bb = hand.big_blind as f64;
    if bb <= 0.0 {
        return None;
    }
    let folded = |name: &str| hand.actions.iter().any(|a| a.player == name && a.kind == ActionKind::Fold);
    let remaining: Vec<&str> =
        hand.active_players().map(|s| s.name.as_str()).filter(|n| !folded(n)).collect();
    if remaining.len() < 2 || !remaining.contains(&hero) {
        return None;
    }
    let stack = |name: &str| hand.seats.iter().find(|s| s.name == name).map(|s| s.stack).unwrap_or(0);
    let all_in_player = remaining
        .iter()
        .any(|&p| hand.actions.iter().any(|a| a.player == p && a.all_in) || hand.invested(p) >= stack(p));
    if !all_in_player {
        return None;
    }
    let last = hand.actions.iter().rposition(|a| !a.kind.is_post())?;
    let board_len = match hand.actions[last].street {
        Street::Preflop => 0,
        Street::Flop => 3,
        Street::Turn => 4,
        Street::River => return None,
    };
    if hand.board.len() < board_len {
        return None;
    }
    let board: Vec<Card> = hand.board[..board_len].iter().map(|c| c.parse().ok()).collect::<Option<_>>()?;
    let opponents: Vec<&str> = remaining.iter().copied().filter(|&p| p != hero).collect();
    let opp_cards: Vec<Vec<Card>> = opponents
        .iter()
        .map(|p| hand.shown.iter().find(|(n, _)| n == p).and_then(|(_, c)| cards(c)))
        .collect::<Option<_>>()?;
    let stop = hand.actions.iter().rposition(|a| a.player == hero && !a.kind.is_post())?;

    // Side pots by contribution level of the players still in.
    let order: Vec<&str> = std::iter::once(hero).chain(opponents.iter().copied()).collect();
    let contrib = |name: &str| hand.invested(name);
    let mut levels: Vec<i64> = order.iter().map(|p| contrib(p)).collect();
    levels.sort_unstable();
    levels.dedup();
    let mut pots = Vec::new();
    let mut prev = 0;
    for &level in &levels {
        let amount: i64 = hand.seats.iter().map(|s| contrib(&s.name).clamp(prev, level) - prev).sum();
        let players: Vec<usize> = (0..order.len()).filter(|&i| contrib(order[i]) >= level).collect();
        if amount > 0 {
            pots.push(PotView { amount_bb: amount as f64 / bb, players });
        }
        prev = level;
    }
    let total: i64 = hand.seats.iter().map(|s| contrib(&s.name)).sum();
    if prev < hand.seats.iter().map(|s| contrib(&s.name)).max().unwrap_or(0) || total <= 0 {
        return None;
    }
    let (share, _) = all_in_shares(&hero_cards, &opp_cards, &board, &pots).ok()?;
    let pot_bb = total as f64 / bb;
    let rake_bb = hand.rake as f64 / bb;
    let ev_bb = share / 100.0 * (pot_bb - rake_bb) - contrib(hero) as f64 / bb;
    let net_bb = hand.net(hero) as f64 / bb;
    let when = match board_len {
        0 => "vor dem Flop",
        3 => "am Flop",
        _ => "am Turn",
    };
    let task = crate::Task {
        question: format!("Du warst in dieser Hand {when} all-in, alle Karten liegen offen. Wie viel Equity hattest du?"),
        prompt: Prompt::AllIn { hero: hero_cards, opponents: opp_cards, board, pots },
        answer: crate::AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 },
        grading: Grading::Exakt,
        model_note: (hand.rake > 0).then(|| "Rake ist anteilig von jedem Pot abgezogen.".to_string()),
    };
    Some(OwnSpot {
        id: spot_id(Kind::AllIn, hand.id),
        kind: Kind::AllIn,
        hand_id: hand.id,
        played_at: hand.played_at.clone(),
        stop_before_action: stop,
        exercise: exercise(Kind::AllIn, 1, hand, task),
        played: None,
        played_ev_loss_bb: None,
        all_in: Some(AllInResult { equity_pct: share, pot_bb, ev_bb, net_bb, luck_bb: net_bb - ev_bb, rake_bb }),
    })
}

/// Largest effective stack treated as a push/fold spot.
const MAX_STACK_BB: f64 = 20.0;

fn push_fold(hand: &Hand) -> Option<OwnSpot> {
    if hand.game != GameKind::Tournament || hand.big_blind <= 0 {
        return None;
    }
    let hero = hand.hero.as_deref()?;
    let hero_cards = cards(&hand.hero_cards)?;
    let pre: Vec<(usize, &ringside_core::model::Action)> =
        hand.actions.iter().enumerate().filter(|(_, a)| a.street == Street::Preflop).collect();
    if pre.iter().any(|(_, a)| a.kind == ActionKind::PostBoth) {
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
    let ante_of = |name: &str| -> i64 {
        pre.iter().filter(|(_, a)| a.player == name && a.kind == ActionKind::PostAnte).map(|(_, a)| a.amount).sum()
    };
    let stack = |name: &str| hand.seats.iter().find(|s| s.name == name).map(|s| s.stack);
    let (a_sb, a_bb) = (ante_of(sb_name), ante_of(bb_name));
    let ante = a_sb.min(a_bb);
    let other_antes: i64 = pre
        .iter()
        .filter(|(_, a)| a.kind == ActionKind::PostAnte && a.player != sb_name && a.player != bb_name)
        .map(|(_, a)| a.amount)
        .sum();
    let (sb_stack, bb_stack) = (stack(sb_name)?, stack(bb_name)?);
    let big = hand.big_blind as f64;
    let spot = Spot {
        stack_bb: (sb_stack - (a_sb - ante)).min(bb_stack - (a_bb - ante)) as f64 / big,
        small_blind_bb: hand.small_blind as f64 / big,
        ante_bb: ante as f64 / big,
        dead_bb: (other_antes + (a_sb - ante) + (a_bb - ante)) as f64 / big,
    };
    if spot.stack_bb > MAX_STACK_BB || spot.stack_bb <= 1.0 + spot.ante_bb || spot.small_blind_bb > 1.0 {
        return None;
    }
    let voluntary: Vec<(usize, &ringside_core::model::Action)> =
        pre.iter().filter(|(_, a)| !a.kind.is_post()).copied().collect();
    // Everyone before the small blind folds.
    let first_sb = voluntary.iter().position(|(_, a)| a.player == sb_name)?;
    if voluntary[..first_sb].iter().any(|(_, a)| a.kind != ActionKind::Fold) {
        return None;
    }
    let (sb_index, sb_action) = voluntary[first_sb];
    let covers = |cap: i64| sb_action.kind == ActionKind::Raise && (sb_action.all_in || sb_action.to.unwrap_or(0) >= cap);
    let sb_shoves = covers(bb_stack - a_bb);
    let players = hand.active_players().count() as u8;
    let (kind, stop, played) = if hero == sb_name {
        let played = match sb_action.kind {
            ActionKind::Fold => "fold",
            _ if sb_shoves => "shove",
            _ => return None,
        };
        (Kind::PushFoldSb, sb_index, played)
    } else if hero == bb_name {
        if !sb_shoves {
            return None;
        }
        let (bb_index, bb_action) = *voluntary[first_sb + 1..].iter().find(|(_, a)| a.player == bb_name)?;
        let played = match bb_action.kind {
            ActionKind::Fold => "fold",
            ActionKind::Call => "call",
            _ => return None,
        };
        (Kind::PushFoldBb, bb_index, played)
    } else {
        return None;
    };
    let task = pushfold::task(kind == Kind::PushFoldSb, &hero_cards, spot, players);
    let exercise = exercise(kind, 3, hand, task);
    let played = Answer::Choice(played.into());
    let loss = grade(&exercise, &played).ok().and_then(|f| f.ev_loss_bb);
    Some(OwnSpot {
        id: spot_id(kind, hand.id),
        kind,
        hand_id: hand.id,
        played_at: hand.played_at.clone(),
        stop_before_action: stop,
        exercise,
        played: Some(played),
        played_ev_loss_bb: loss,
        all_in: None,
    })
}

/// Parses a spot id back into kind and hand id.
pub fn parse_spot_id(id: &str) -> Option<(Kind, u64)> {
    let (kind, hand) = id.split_once(':')?;
    Some((Kind::parse(kind)?, hand.parse().ok()?))
}

