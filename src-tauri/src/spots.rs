//! „Wichtige Spots“ in the learning area: the bundled learning spots
//! (`ringside_analysis::spots`), what the user answered and which to practise next. Locked
//! while the client runs, like the rest of the learning area.

use ringside_analysis::drill::answer;
use ringside_analysis::spots::{self, LernSpot, Seen, SpotGrid, Stored, TOPICS};
use ringside_core::db::{Attempt, Rating};
use ringside_learn::Kind;
use serde::Serialize;
use tauri::AppHandle;

use crate::drill::{DrillChoice, DrillResult, DrillSpot};
use crate::{AppState, CmdResult, blocking, err, now_secs, unlocked};

#[derive(Clone, Debug, Serialize)]
pub struct TopicView {
    pub id: &'static str,
    pub title: &'static str,
    pub lesson: &'static str,
}

/// One spot in the overview.
#[derive(Clone, Debug, Serialize)]
pub struct KeySpotRow {
    pub id: String,
    pub topic: String,
    pub position: String,
    pub stack_bb: f64,
    pub players: usize,
    pub cards: Vec<String>,
    pub seen: Seen,
}

#[derive(Clone, Debug, Serialize)]
pub struct KeySpots {
    pub topics: Vec<TopicView>,
    pub spots: Vec<KeySpotRow>,
    /// Ids to practise, most urgent first.
    pub queue: Vec<String>,
}

/// One spot to answer.
#[derive(Clone, Debug, Serialize)]
pub struct KeySpotView {
    pub spot: DrillSpot,
    pub topic: TopicView,
    /// 1-based place in the pack.
    pub number: usize,
    pub total: usize,
    pub grid: Option<SpotGrid>,
}

fn topic(id: &str) -> TopicView {
    let t = TOPICS.iter().find(|t| t.id == id).unwrap_or(&TOPICS[0]);
    TopicView { id: t.id, title: t.title, lesson: t.lesson }
}

fn seen(state: &AppState) -> CmdResult<std::collections::BTreeMap<String, Seen>> {
    let attempts = state.db.lock().unwrap().attempts().map_err(err)?;
    Ok(spots::seen(
        attempts
            .into_iter()
            .filter(|a| a.kind == Kind::KeySpot.id())
            .filter_map(|a| serde_json::from_str::<Stored>(&a.answer).ok().map(|s| (s, a.at))),
    ))
}

fn players(s: &LernSpot) -> usize {
    s.hand.seats.len()
}

pub(crate) fn key_spots(state: &AppState) -> CmdResult<KeySpots> {
    unlocked(state)?;
    let pack = spots::pack();
    let seen = seen(state)?;
    let queue = spots::queue(&pack.spots, &seen, now_secs()).into_iter().map(String::from).collect();
    Ok(KeySpots {
        topics: TOPICS.iter().map(|t| topic(t.id)).collect(),
        spots: pack
            .spots
            .iter()
            .map(|s| KeySpotRow {
                id: s.id.clone(),
                topic: s.topic.clone(),
                position: s.position.clone(),
                stack_bb: s.stack_bb,
                players: players(s),
                cards: s.hand.hero_cards.clone(),
                seen: seen.get(&s.id).cloned().unwrap_or_default(),
            })
            .collect(),
        queue,
    })
}

pub(crate) fn key_spot(state: &AppState, id: &str) -> CmdResult<KeySpotView> {
    unlocked(state)?;
    let pack = spots::pack();
    let number = pack.spots.iter().position(|s| s.id == id).ok_or("Diesen Lern-Spot gibt es nicht.")?;
    let s = &pack.spots[number];
    let t = topic(&s.topic);
    let d = &s.decision;
    let spot = DrillSpot {
        key: s.id.clone(),
        title: t.title.into(),
        detail: format!("{} Spieler · {} BB", players(s), fmt_bb(s.stack_bb)),
        hand_id: s.hand.id,
        action_index: s.action_index,
        seed: None,
        played_at: s.hand.played_at.clone(),
        hand: s.hand.clone(),
        hero_cards: s.hand.hero_cards.clone(),
        position: Some(s.position.clone()),
        choices: d.strategy.iter().map(|c| DrillChoice { label: c.label.clone(), kind: c.kind.clone() }).collect(),
        source: d.source,
        reliability: None,
        pot_bb: d.pot_bb,
    };
    Ok(KeySpotView { spot, topic: t, number: number + 1, total: pack.spots.len(), grid: s.grid.clone() })
}

fn fmt_bb(x: f64) -> String {
    if x.fract().abs() < 1e-9 { format!("{x:.0}") } else { format!("{x:.1}").replace('.', ",") }
}

pub(crate) fn key_spot_check(state: &AppState, id: &str, choice: usize) -> CmdResult<DrillResult> {
    unlocked(state)?;
    let s = spots::spot(id).ok_or("Diesen Lern-Spot gibt es nicht.")?;
    let a = answer(&s.decision, choice)?;
    let right = a.decision.rating.is_some_and(|r| r <= Rating::Good);
    let stored = Stored { id: id.into(), choice: a.decision.played.clone(), right };
    state
        .db
        .lock()
        .unwrap()
        .add_attempt(&Attempt {
            kind: Kind::KeySpot.id().into(),
            level: 1,
            seed: None,
            hand_id: Some(s.hand.id),
            answer: serde_json::to_string(&stored).map_err(err)?,
            score: a.score,
            ev_loss_bb: a.decision.ev_loss_bb,
            at: now_secs(),
        })
        .map_err(err)?;
    Ok(DrillResult { decision: a.decision, score: a.score, correct: right, then: None })
}

#[tauri::command]
pub async fn learn_key_spots(app: AppHandle) -> CmdResult<KeySpots> {
    blocking(app, key_spots).await
}

#[tauri::command]
pub async fn learn_key_spot(id: String, app: AppHandle) -> CmdResult<KeySpotView> {
    blocking(app, move |state| key_spot(state, &id)).await
}

#[tauri::command]
pub async fn learn_key_spot_check(id: String, choice: usize, app: AppHandle) -> CmdResult<DrillResult> {
    blocking(app, move |state| key_spot_check(state, &id, choice)).await
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::solutions::test_state;

    #[test]
    fn an_answer_is_stored_and_moves_the_spot_in_the_queue() {
        let state = test_state();
        let list = key_spots(&state).unwrap();
        assert_eq!(list.spots.len(), spots::pack().spots.len());
        let id = list.queue[0].clone();
        let view = key_spot(&state, &id).unwrap();
        assert_eq!(view.spot.hand.actions.len(), view.spot.action_index + 1);
        let wrong = (0..view.spot.choices.len())
            .find(|&c| spots::check(&id, c).unwrap().decision.rating.is_some_and(|r| r > Rating::Good))
            .expect("every spot has a wrong answer");
        assert!(!key_spot_check(&state, &id, wrong).unwrap().correct);
        let after = key_spots(&state).unwrap();
        let row = after.spots.iter().find(|s| s.id == id).unwrap();
        assert_eq!((row.seen.tries, row.seen.right), (1, Some(false)));
        assert_eq!(after.queue[0], id, "a wrong answer comes back first");
        let progress = ringside_learn::progress(&state.db.lock().unwrap().attempts().unwrap(), now_secs());
        assert!(progress.skills.iter().any(|s| s.id == Kind::KeySpot && s.attempts >= 1));
    }

    #[test]
    fn the_key_spots_are_locked_while_pokerstars_runs() {
        let state = test_state();
        let id = spots::pack().spots[0].id.clone();
        state.client_running.store(true, Ordering::Relaxed);
        assert_eq!(key_spots(&state).unwrap_err(), "gesperrt");
        assert_eq!(key_spot(&state, &id).unwrap_err(), "gesperrt");
        assert_eq!(key_spot_check(&state, &id, 0).unwrap_err(), "gesperrt");
    }
}
