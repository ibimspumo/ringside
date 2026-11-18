//! The playing profile in the learning area: which kinds of spot the hero plays and what
//! they cost, real decisions of a kind to decide again and endless variations of them
//! (another hand for the hero, rated by the same analysis). Locked while the client runs,
//! like the rest of the learning area.

use std::collections::HashSet;

use ringside_analysis::drill::{answer, row, variant};
use ringside_core::db::{Attempt, Decision, DecisionBrief, Filter, Rating, Source, Status};
use ringside_core::model::Hand;
use ringside_core::profile::{Profile, ProfileRow, kind_of, profile};
use ringside_learn::Kind;
use serde::Serialize;
use tauri::AppHandle;

use crate::{AppState, CmdResult, blocking, err, now_secs};

#[tauri::command]
pub async fn learn_profile(filter: Filter, app: AppHandle) -> CmdResult<Profile> {
    blocking(app, move |state| {
        let db = state.db.lock().unwrap();
        let rows = db.profile_rows(&filter).map_err(err)?;
        let practiced = db.spot_practice().map_err(err)?;
        Ok(profile(&rows, &practiced))
    })
    .await
}

#[derive(Clone, Debug, Serialize)]
pub struct DrillChoice {
    pub label: String,
    pub kind: String,
}

/// A decision to make again: from a real hand (`seed` is `None`) or a variation of one.
#[derive(Clone, Debug, Serialize)]
pub struct DrillSpot {
    pub key: String,
    pub title: String,
    pub detail: String,
    /// The real hand the spot comes from.
    pub hand_id: u64,
    pub action_index: usize,
    pub seed: Option<u64>,
    pub played_at: String,
    /// The hand up to the decision; a variation with the hero's new cards.
    pub hand: Hand,
    pub hero_cards: Vec<String>,
    /// The hero's seat label, e.g. "BB".
    pub position: Option<String>,
    pub choices: Vec<DrillChoice>,
    pub source: Option<Source>,
    pub reliability: Option<f64>,
    pub pot_bb: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DrillResult {
    /// The decision with the answer as the played action, rated like the analysis.
    pub decision: Decision,
    pub score: f64,
    pub correct: bool,
    /// What was played back then, for a real hand.
    pub then: Option<DecisionBrief>,
}

fn rows_of(state: &AppState, key: &str) -> CmdResult<Vec<ProfileRow>> {
    let rows = state.db.lock().unwrap().profile_rows(&Filter::default()).map_err(err)?;
    Ok(rows.into_iter().filter(|r| r.status == Status::Rated && kind_of(r).is_some_and(|k| k.key() == key)).collect())
}

fn hand(state: &AppState, id: u64) -> CmdResult<Hand> {
    state.db.lock().unwrap().hand(id).map_err(err)?.ok_or_else(|| "Hand nicht gefunden".into())
}

fn stored(state: &AppState, hand_id: u64, action_index: usize) -> CmdResult<Decision> {
    state
        .db
        .lock()
        .unwrap()
        .hand_decisions(hand_id)
        .map_err(err)?
        .into_iter()
        .find(|d| d.action_index == action_index && d.status == Status::Rated)
        .ok_or_else(|| "Diese Entscheidung ist nicht bewertet".into())
}

fn spot(key: &str, base: &Hand, shown: Hand, d: &Decision, seed: Option<u64>) -> CmdResult<DrillSpot> {
    let r = row(&shown, d);
    let kind = kind_of(&r).ok_or("Keine Entscheidung")?;
    if kind.key() != key {
        return Err("Der Spot gehört zu einer anderen Lage".into());
    }
    let mut shown = shown;
    // Only what was known at the decision goes out; a real hand is cut there as well.
    shown.actions.truncate(d.action_index + 1);
    shown.shown.clear();
    Ok(DrillSpot {
        key: key.to_string(),
        title: kind.title(),
        detail: kind.detail(),
        hand_id: base.id,
        action_index: d.action_index,
        seed,
        played_at: base.played_at.clone(),
        hero_cards: shown.hero_cards.clone(),
        position: r.position,
        hand: shown,
        choices: d.strategy.iter().map(|c| DrillChoice { label: c.label.clone(), kind: c.kind.clone() }).collect(),
        source: d.source,
        reliability: d.reliability,
        pot_bb: d.pot_bb,
    })
}

/// Real decisions of the kind `key`, the costliest mistakes first, then the newest; those
/// answered here before come last.
#[tauri::command]
pub async fn learn_drill_real(key: String, limit: usize, app: AppHandle) -> CmdResult<Vec<DrillSpot>> {
    blocking(app, move |state| {
        let mut rows = rows_of(state, &key)?;
        let answered: HashSet<u64> = state
            .db
            .lock()
            .unwrap()
            .attempts()
            .map_err(err)?
            .into_iter()
            .filter(|a| a.kind == Kind::OwnDecision.id() && a.seed.is_none())
            .filter_map(|a| a.hand_id)
            .collect();
        let flawed = |r: &ProfileRow| r.rating.is_some_and(|x| x >= Rating::Inaccuracy);
        rows.sort_by(|a, b| {
            answered
                .contains(&a.hand_id)
                .cmp(&answered.contains(&b.hand_id))
                .then(flawed(b).cmp(&flawed(a)))
                .then(b.ev_loss_bb.unwrap_or(0.0).total_cmp(&a.ev_loss_bb.unwrap_or(0.0)))
                .then(b.hand_id.cmp(&a.hand_id))
        });
        let mut out = Vec::new();
        for r in rows {
            if out.len() >= limit {
                break;
            }
            let h = hand(state, r.hand_id)?;
            let d = stored(state, r.hand_id, r.action_index)?;
            if d.strategy.len() < 2 {
                continue;
            }
            out.push(spot(&key, &h, h.clone(), &d, None)?);
        }
        Ok(out)
    })
    .await
}

/// A variation of a decision of the kind `key`: one of its hands, picked by `seed`, with
/// another hand dealt to the hero.
#[tauri::command]
pub async fn learn_drill_variant(key: String, seed: u64, app: AppHandle) -> CmdResult<DrillSpot> {
    blocking(app, move |state| {
        let rows = rows_of(state, &key)?;
        if rows.is_empty() {
            return Err("Für diese Lage gibt es noch keine bewerteten Hände".into());
        }
        // A base hand the analysis cannot vary is skipped for the next one.
        for k in 0..rows.len().min(8) {
            let r = &rows[(seed as usize + k * 7919) % rows.len()];
            let base = hand(state, r.hand_id)?;
            if let Ok((h, d)) = variant(&base, r.action_index, seed) {
                return spot(&key, &base, h, &d, Some(seed));
            }
        }
        Err("Für diese Lage konnte keine neue Hand bewertet werden".into())
    })
    .await
}

/// Grades `choice` for a spot of `learn_drill_real` or `learn_drill_variant` and records it.
#[tauri::command]
pub async fn learn_drill_check(
    key: String,
    hand_id: u64,
    action_index: usize,
    seed: Option<u64>,
    choice: usize,
    app: AppHandle,
) -> CmdResult<DrillResult> {
    blocking(app, move |state| {
        let base = hand(state, hand_id)?;
        let (shown, d, then) = match seed {
            None => {
                let d = stored(state, hand_id, action_index)?;
                let then = state.db.lock().unwrap().decision_brief(hand_id, action_index).map_err(err)?;
                (base.clone(), d, then)
            }
            Some(seed) => {
                let (h, d) = variant(&base, action_index, seed)?;
                (h, d, None)
            }
        };
        // Checks that the spot is of the kind it is recorded for.
        spot(&key, &base, shown, &d, seed)?;
        let a = answer(&d, choice)?;
        let correct = a.decision.rating.is_some_and(|r| r <= Rating::Good);
        let at = now_secs();
        let db = state.db.lock().unwrap();
        db.add_attempt(&Attempt {
            kind: Kind::OwnDecision.id().into(),
            level: 1,
            seed,
            hand_id: Some(hand_id),
            answer: serde_json::json!({ "key": key, "action_index": action_index, "choice": a.decision.played }).to_string(),
            score: a.score,
            ev_loss_bb: a.decision.ev_loss_bb,
            at,
        })
        .map_err(err)?;
        db.mark_spot_practiced(&key, at).map_err(err)?;
        Ok(DrillResult { decision: a.decision, score: a.score, correct, then })
    })
    .await
}

