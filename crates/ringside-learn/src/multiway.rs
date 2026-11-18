//! Push/fold drills at a whole table of 2–9 players against the equilibrium of
//! [`ringside_engine::multiway`]: open shoves once everyone before folded, and calls or
//! overcalls against a shove.
//!
//! A solve takes from a fraction of a second to a few seconds, so generated tables
//! come from a fixed grid of parameter sets and solutions stay in a process-wide LRU
//! cache that holds the whole grid of one level; practising one level stops solving
//! once every table of it has come up.

use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use rand::Rng;
pub use ringside_engine::multiway::position_names;
use ringside_engine::multiway::{self, MAX_PLAYERS, Params, Solution};
use ringside_engine::{Card, CardSet, ClassEv, Combo};

use crate::deal::{self, Rng8};
use crate::fmt::{self, pct};
use crate::score::decision;
use crate::{Answer, AnswerSpec, ChoiceOption, Exercise, Fact, Feedback, Grading, Kind, Prompt, Task, choice, feedback};

pub const MODEL_NOTE: &str = "Gleichgewicht für reines Push/Fold in Chips (ohne ICM): schieben oder folden, nach einem Shove nur callen oder folden. Höchstens 3 Spieler all-in, 3-Wege-Equity geschätzt.";

/// Solutions kept in the process: at least the largest level grid (level 2, 48 tables).
/// A 9-player solution holds about 0.5 MB.
const CACHE_LIMIT: usize = 48;
/// Decisions closer than this (in bb per hand) are close calls, asked on level 3 only.
const CLOSE_BB: f64 = 0.3;
/// Nodes reached less often than this are left out: their EVs rest on almost no hands.
const MIN_REACH: f64 = 1e-3;
/// Deepest effective stack asked about.
const MAX_EFFECTIVE_BB: f64 = 20.0;

type Slot = Arc<Mutex<Option<Arc<Solution>>>>;

/// Most recently used last.
static CACHE: LazyLock<Mutex<Vec<(String, Slot)>>> = LazyLock::new(Default::default);

/// The push/fold equilibrium for `params`: from the cache, the bundled tables or solved
/// now. Callers asking
/// for the same parameters at once wait for a single solve.
pub fn lookup_or_solve(params: &Params) -> Result<Arc<Solution>, String> {
    // Debug prints every f64 so that it parses back exactly, so equal keys mean equal parameters.
    let key = format!("{params:?}");
    let slot = {
        let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
        let slot = match cache.iter().position(|(k, _)| *k == key) {
            Some(i) => cache.remove(i).1,
            None => Slot::default(),
        };
        cache.push((key, slot.clone()));
        if cache.len() > CACHE_LIMIT {
            cache.remove(0);
        }
        slot
    };
    let mut solved = slot.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(s) = &*solved {
        return Ok(s.clone());
    }
    // Equal stacks on the bundled grid come precomputed; everything else is solved here.
    let s = match ringside_engine::pushfold_table::lookup(params) {
        Some(table) => Arc::new(table),
        None => Arc::new(multiway::solve(params).map_err(|e| format!("Push/Fold nicht lösbar: {e}"))?),
    };
    *solved = Some(s.clone());
    Ok(s)
}

/// One decision at a table. Only stacks, blinds, antes and dead money of `params` are
/// used; the solver settings are always the defaults of [`Params::new`].
#[derive(Clone, Debug, PartialEq)]
pub struct TableSpot {
    pub stacks_bb: Vec<f64>,
    pub small_blind_bb: f64,
    pub ante_bb: f64,
    pub bb_ante_bb: f64,
    pub dead_bb: f64,
    /// Seats in preflop action order, the big blind last.
    pub hero: usize,
    /// `None`: everyone before the hero folded.
    pub shover: Option<usize>,
    pub callers: Vec<usize>,
}

impl TableSpot {
    pub fn params(&self) -> Params {
        let mut p = Params::new(self.stacks_bb.clone());
        p.small_blind_bb = self.small_blind_bb;
        p.ante_bb = self.ante_bb;
        p.bb_ante_bb = self.bb_ante_bb;
        p.dead_bb = self.dead_bb;
        p
    }

    pub fn players(&self) -> usize {
        self.stacks_bb.len()
    }

    /// The hero's stack against the deepest player it can lose to: everyone behind
    /// when opening, the shover and callers when facing a shove.
    pub fn effective_bb(&self) -> f64 {
        let n = self.players();
        let others = match self.shover {
            None => (self.hero + 1..n).map(|i| self.stacks_bb[i]).fold(0.0, f64::max),
            Some(s) => self.callers.iter().map(|&c| self.stacks_bb[c]).fold(self.stacks_bb[s], f64::max),
        };
        self.stacks_bb[self.hero].min(others)
    }

    /// Whether the engine accepts the table: 2–9 players and every stack covers its
    /// antes and blind.
    pub fn valid(&self) -> bool {
        let n = self.players();
        (2..=MAX_PLAYERS).contains(&n)
            && self.small_blind_bb > 0.0
            && self.small_blind_bb <= 1.0
            && (0..n).all(|i| {
                let blind = if i + 1 == n { 1.0 } else if i + 2 == n { self.small_blind_bb } else { 0.0 };
                let live = self.stacks_bb[i] - self.ante(i);
                live.is_finite() && live > 0.0 && live >= blind
            })
    }

    fn ante(&self, seat: usize) -> f64 {
        self.ante_bb + if seat + 1 == self.players() { self.bb_ante_bb } else { 0.0 }
    }

    fn blind(&self, seat: usize) -> f64 {
        let n = self.players();
        if seat + 1 == n {
            1.0
        } else if seat + 2 == n {
            self.small_blind_bb
        } else {
            0.0
        }
    }

    /// Hero against the shover alone: what calling costs and the pot it wins if the
    /// players still to act fold.
    fn call_odds(&self) -> Option<(f64, f64)> {
        let s = self.shover?;
        if !self.callers.is_empty() {
            return None;
        }
        let h = self.hero;
        let live = |i: usize| self.stacks_bb[i] - self.ante(i);
        let matched = live(h).min(live(s));
        let antes: f64 = (0..self.players()).map(|i| self.ante(i)).sum();
        let blinds: f64 = (0..self.players()).filter(|&i| i != h && i != s).map(|i| self.blind(i)).sum();
        Some((matched - self.blind(h), self.dead_bb + antes + blinds + 2.0 * matched))
    }
}

pub(crate) fn spot_of(prompt: &Prompt) -> TableSpot {
    let Prompt::PushFoldTable {
        stacks_bb, small_blind_bb, ante_bb, bb_ante_bb, dead_bb, hero, shover, callers, ..
    } = prompt
    else {
        unreachable!()
    };
    TableSpot {
        stacks_bb: stacks_bb.clone(),
        small_blind_bb: *small_blind_bb,
        ante_bb: *ante_bb,
        bb_ante_bb: *bb_ante_bb,
        dead_bb: *dead_bb,
        hero: *hero,
        shover: *shover,
        callers: callers.clone(),
    }
}

/// Equal stacks for levels 1 and 2, so that precomputed equal-stack tables can serve them.
const STACKS_1: [f64; 4] = [8.0, 10.0, 12.0, 15.0];
const STACKS_2: [f64; 6] = [6.0, 8.0, 10.0, 12.0, 15.0, 20.0];
/// Level 3 stacks in action order of a full table; smaller tables take the last seats.
const PROFILES: [[f64; 9]; 2] = [
    [12.0, 25.0, 7.0, 18.0, 9.0, 30.0, 14.0, 5.0, 11.0],
    [20.0, 8.0, 15.0, 6.0, 22.0, 10.0, 16.0, 13.0, 9.0],
];
/// (ante per player, big-blind ante) on levels 2 and 3.
const ANTES: [(f64, f64); 2] = [(0.125, 0.0), (0.0, 1.0)];

/// The table of an exercise: level 1 has 2–6 players, equal stacks and no ante (20
/// tables); level 2 has 6–9 players, equal stacks and an ante or big-blind ante (48);
/// level 3 has 3–9 players with uneven stacks (28).
fn table(level: u8, rng: &mut Rng8) -> TableSpot {
    let (stacks, (ante, bb_ante)) = match level {
        1 => (vec![*deal::pick(rng, &STACKS_1); rng.random_range(2..=6)], (0.0, 0.0)),
        2 => {
            let n = rng.random_range(6..=9);
            (vec![*deal::pick(rng, &STACKS_2); n], *deal::pick(rng, &ANTES))
        }
        _ => {
            let n = rng.random_range(3..=9);
            (deal::pick(rng, &PROFILES)[MAX_PLAYERS - n..].to_vec(), *deal::pick(rng, &ANTES))
        }
    };
    TableSpot {
        stacks_bb: stacks,
        small_blind_bb: 0.5,
        ante_bb: ante,
        bb_ante_bb: bb_ante,
        dead_bb: 0.0,
        hero: 0,
        shover: None,
        callers: Vec::new(),
    }
}

pub fn generate(kind: Kind, level: u8, rng: &mut Rng8) -> Result<Task, String> {
    let open = kind == Kind::PushFoldOpen;
    let mut spot = table(level, rng);
    let solution = lookup_or_solve(&spot.params())?;
    let n = spot.players();
    for _ in 0..10_000 {
        if open {
            spot.hero = rng.random_range(0..n - 1);
        } else {
            let s = rng.random_range(0..n - 1);
            spot.shover = Some(s);
            spot.hero = rng.random_range(s + 1..n);
            spot.callers.clear();
            if level == 3 && spot.hero > s + 1 && rng.random_bool(0.4) {
                spot.callers.push(rng.random_range(s + 1..spot.hero));
            }
        }
        let mut used = CardSet::EMPTY;
        let hand = deal::cards(rng, &mut used, 2);
        let class = Combo::new(hand[0], hand[1]).class();
        let Some(s) = solution.spot(spot.hero, class, spot.shover, &spot.callers) else { continue };
        if s.reach < MIN_REACH || spot.effective_bb() > MAX_EFFECTIVE_BB {
            continue;
        }
        let close = (s.freq > 0.0 && s.freq < 1.0) || (s.ev_act - s.ev_fold).abs() < CLOSE_BB;
        if close != (level == 3) {
            continue;
        }
        return Ok(task(&hand, &spot));
    }
    Err("Kein passender Push/Fold-Spot gefunden".into())
}

/// "im Big Blind", "am Button", "auf UTG+1".
/// "der Big Blind", "der Button", "UTG+1": a position as the subject of a sentence.
pub(crate) fn subject(position: &str) -> String {
    match position {
        "BB" => "der Big Blind".into(),
        "SB" => "der Small Blind".into(),
        "BTN" => "der Button".into(),
        "CO" => "der Cutoff".into(),
        "HJ" => "der Hijack".into(),
        "LJ" => "der Lojack".into(),
        other => other.into(),
    }
}

pub(crate) fn at(position: &str) -> String {
    match position {
        "BB" => "im Big Blind".into(),
        "SB" => "im Small Blind".into(),
        "BTN" => "am Button".into(),
        "CO" => "im Cutoff".into(),
        "HJ" => "im Hijack".into(),
        "LJ" => "im Lojack".into(),
        other => format!("auf {other}"),
    }
}

pub fn task(hand: &[Card], spot: &TableSpot) -> Task {
    let class = Combo::new(hand[0], hand[1]).class();
    let n = spot.players();
    let positions: Vec<String> = position_names(n).into_iter().map(String::from).collect();
    let mut setup = format!("Dein Stack {}", fmt::bb(spot.stacks_bb[spot.hero]));
    let effective = spot.effective_bb();
    if effective < spot.stacks_bb[spot.hero] {
        setup.push_str(&format!(", effektiv {}", fmt::bb(effective)));
    }
    if spot.ante_bb > 0.0 {
        setup.push_str(&format!(", Ante {}", fmt::bb(spot.ante_bb)));
    }
    if spot.bb_ante_bb > 0.0 {
        setup.push_str(&format!(", Big-Blind-Ante {}", fmt::bb(spot.bb_ante_bb)));
    }
    if spot.dead_bb > 0.0 {
        setup.push_str(&format!(", totes Geld {}", fmt::bb(spot.dead_bb)));
    }
    let table = format!("{n} Spieler, du sitzt {}", at(&positions[spot.hero]));
    let (question, options) = match spot.shover {
        None => (
            format!("{table}. Alle vor dir haben gefoldet. {setup}. Schiebst du mit {class} all-in?"),
            vec![ChoiceOption::new("shove", "All-in"), ChoiceOption::new("fold", "Fold")],
        ),
        Some(s) => {
            let mut action =
                format!("{} geht mit {} all-in", positions[s], fmt::bb(spot.stacks_bb[s]));
            for &c in &spot.callers {
                action.push_str(&format!(", {} callt", positions[c]));
            }
            (
                format!("{table}. {action}. {setup}. Callst du mit {class}?"),
                vec![ChoiceOption::new("call", "Call"), ChoiceOption::new("fold", "Fold")],
            )
        }
    };
    Task {
        question,
        prompt: Prompt::PushFoldTable {
            hand: hand.to_vec(),
            class: class.to_string(),
            positions,
            stacks_bb: spot.stacks_bb.clone(),
            hero: spot.hero,
            shover: spot.shover,
            callers: spot.callers.clone(),
            effective_bb: effective,
            small_blind_bb: spot.small_blind_bb,
            ante_bb: spot.ante_bb,
            bb_ante_bb: spot.bb_ante_bb,
            dead_bb: spot.dead_bb,
        },
        answer: AnswerSpec::Choice { options },
        grading: Grading::Modell,
        model_note: Some(MODEL_NOTE.into()),
    }
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::PushFoldTable { hand, positions, .. } = &ex.prompt else { unreachable!() };
    let spot = spot_of(&ex.prompt);
    let open = spot.shover.is_none();
    let (act_id, act_name) = if open { ("shove", "All-in") } else { ("call", "Call") };
    let picked = choice(answer)?;
    if picked != act_id && picked != "fold" {
        return Err(format!("Unbekannte Aktion '{picked}'"));
    }
    let solution = lookup_or_solve(&spot.params())?;
    let class = Combo::new(hand[0], hand[1]).class();
    let s = solution
        .spot(spot.hero, class, spot.shover, &spot.callers)
        .ok_or("Diesen Spot gibt es im Push/Fold-Modell nicht")?;
    let f = s.freq;
    let act = picked == act_id;
    let correct = if act { f > 0.0 } else { f < 1.0 };
    let loss = ClassEv { act: s.ev_act, fold: s.ev_fold }.loss(act);
    let strategy = if f >= 1.0 {
        format!("{act_name} (immer)")
    } else if f <= 0.0 {
        "Fold (immer)".to_string()
    } else {
        format!("Gemischt: {} {act_name}", pct(100.0 * f))
    };
    let position = &positions[spot.hero];
    let mut facts = vec![
        Fact::new("Nash-Strategie", strategy),
        Fact::new(&format!("EV {act_name}"), fmt::signed_bb(s.ev_act)),
        Fact::new("EV Fold", fmt::signed_bb(s.ev_fold)),
        Fact::new(
            &format!("{}-Range {position}", if open { "Shove" } else { "Call" }),
            format!("{} aller Hände", pct(solution.act_pct(s.node))),
        ),
        Fact::new("Effektiver Stack", fmt::bb(spot.effective_bb())),
    ];
    if spot.dead_bb > 0.0 {
        facts.push(Fact::new("Totes Geld", fmt::bb(spot.dead_bb)));
    }
    if let Some((cost, pot)) = spot.call_odds() {
        let label = if spot.hero + 1 == spot.players() { "Benötigte Equity" } else { "Benötigte Equity, wenn danach alle folden" };
        facts.push(
            Fact::new(label, pct(100.0 * cost / pot)).with(format!("{} ÷ {}", fmt::short(cost), fmt::short(pot))),
        );
    }
    if !solution.converged {
        facts.push(Fact::new(
            "Genauigkeit",
            format!("Lösung nicht voll konvergiert, bis zu {} Abweichung", fmt::bb(solution.exploitability)),
        ));
    }
    let (truth, text) = if f >= 0.5 { (act_id, act_name) } else { ("fold", "Fold") };
    let mut fb = feedback(correct, if correct { 1.0 } else { decision(loss) }, Answer::Choice(truth.into()), text.into(), facts);
    fb.ev_loss_bb = Some(loss);
    Ok(fb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_names_start_with_utg() {
        assert_eq!(position_names(2), ["SB", "BB"]);
        assert_eq!(position_names(3), ["BTN", "SB", "BB"]);
        assert_eq!(position_names(6), ["LJ", "HJ", "CO", "BTN", "SB", "BB"]);
        assert_eq!(position_names(7), ["UTG", "LJ", "HJ", "CO", "BTN", "SB", "BB"]);
        assert_eq!(position_names(9), ["UTG", "UTG+1", "UTG+2", "LJ", "HJ", "CO", "BTN", "SB", "BB"]);
    }
}
