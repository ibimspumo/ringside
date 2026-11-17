//! Heads-up push/fold drills against the Nash equilibrium, with dead money.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use rand::Rng;
use ringside_engine::{Card, CardSet, ClassEv, HandClass, PushFoldParams, PushFoldResult, solve_push_fold};

use crate::deal::{self, Rng8};
use crate::fmt::{self, pct};
use crate::score::decision;
use crate::{Answer, AnswerSpec, ChoiceOption, Exercise, Fact, Feedback, Grading, Kind, Prompt, Task, choice, feedback};

pub const MODEL_NOTE: &str = "Heads-up-Gleichgewicht für reines Push/Fold in Chips (ohne ICM): Alle anderen haben gefoldet, der Small Blind schiebt oder foldet, der Big Blind callt oder foldet. Equity exakt, tote Antes gehen an den Gewinner.";

/// Decisions closer than this (in bb per hand) count as close calls for level 3; levels
/// 1 and 2 only ask clearer ones.
const CLOSE_BB: f64 = 0.3;
const CACHE_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spot {
    pub stack_bb: f64,
    pub small_blind_bb: f64,
    pub ante_bb: f64,
    pub dead_bb: f64,
}

type Key = [u64; 4];

static CACHE: LazyLock<Mutex<HashMap<Key, Arc<PushFoldResult>>>> = LazyLock::new(Default::default);

pub fn solve(s: Spot) -> Result<Arc<PushFoldResult>, String> {
    let key = [s.stack_bb, s.small_blind_bb, s.ante_bb, s.dead_bb].map(f64::to_bits);
    if let Some(r) = CACHE.lock().expect("cache").get(&key) {
        return Ok(r.clone());
    }
    let mut p = PushFoldParams::new(s.stack_bb);
    p.small_blind_bb = s.small_blind_bb;
    p.ante_bb = s.ante_bb;
    p.dead_bb = s.dead_bb;
    let r = Arc::new(solve_push_fold(&p).map_err(|e| e.to_string())?);
    let mut cache = CACHE.lock().expect("cache");
    if cache.len() >= CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(key, r.clone());
    Ok(r)
}

/// Frequency of the active action and the class EVs for one side.
pub fn class_decision(r: &PushFoldResult, sb: bool, class: HandClass) -> (f64, ClassEv) {
    let i = class.index();
    if sb { (r.shove[i], r.sb_class_ev[i]) } else { (r.call[i], r.bb_class_ev[i]) }
}

/// Ante formats: (ante per player, players dealt in, big-blind ante).
const FORMATS: [(f64, u8, bool); 6] =
    [(0.1, 9, false), (0.125, 8, false), (0.1, 6, false), (0.125, 6, false), (0.0, 8, true), (0.0, 6, true)];

pub fn generate(kind: Kind, level: u8, rng: &mut Rng8) -> Result<Task, String> {
    let sb = kind == Kind::PushFoldSb;
    for _ in 0..10_000 {
        let (spot, players) = match level {
            1 => {
                let stack = *deal::pick(rng, &[4.0, 6.0, 8.0, 10.0, 12.0, 15.0]);
                (Spot { stack_bb: stack, small_blind_bb: 0.5, ante_bb: 0.0, dead_bb: 0.0 }, 2)
            }
            _ => {
                let stack = rng.random_range(if level == 2 { 3..=20 } else { 2..=20 }) as f64;
                let (ante, players, bb_ante) = if level == 3 && rng.random_bool(0.25) {
                    (0.0, 2, false)
                } else {
                    *deal::pick(rng, &FORMATS)
                };
                let dead = if bb_ante { 1.0 } else { (ante * (players as f64 - 2.0) * 1e6).round() / 1e6 };
                (Spot { stack_bb: stack, small_blind_bb: 0.5, ante_bb: ante, dead_bb: dead }, players)
            }
        };
        let r = solve(spot)?;
        let mut used = CardSet::EMPTY;
        let hand = deal::cards(rng, &mut used, 2);
        let class = ringside_engine::Combo::new(hand[0], hand[1]).class();
        let (f, ev) = class_decision(&r, sb, class);
        if !sb && r.bb_class_ev[class.index()] == (ClassEv { act: 0.0, fold: 0.0 }) {
            continue;
        }
        let gap = (ev.act - ev.fold).abs();
        let mixed = f > 0.0 && f < 1.0;
        let close = mixed || gap < CLOSE_BB;
        if close != (level == 3) {
            continue;
        }
        return Ok(task(sb, &hand, spot, players));
    }
    Err("Kein passender Push/Fold-Spot gefunden".into())
}

pub fn task(sb: bool, hand: &[Card], s: Spot, players: u8) -> Task {
    let class = ringside_engine::Combo::new(hand[0], hand[1]).class();
    let mut setup = format!("Effektiver Stack {}", fmt::bb(s.stack_bb));
    if s.ante_bb > 0.0 {
        setup.push_str(&format!(", Ante {}", fmt::bb(s.ante_bb)));
    }
    if s.dead_bb > 0.0 {
        setup.push_str(&format!(", totes Geld im Pot {}", fmt::bb(s.dead_bb)));
    }
    let (question, options) = if sb {
        (
            format!("Alle haben zu dir in den Small Blind gefoldet. {setup}. Schiebst du mit {class} all-in?"),
            vec![ChoiceOption::new("shove", "All-in"), ChoiceOption::new("fold", "Fold")],
        )
    } else {
        (
            format!("Der Small Blind geht gegen dich all-in, du bist im Big Blind. {setup}. Callst du mit {class}?"),
            vec![ChoiceOption::new("call", "Call"), ChoiceOption::new("fold", "Fold")],
        )
    };
    Task {
        question,
        prompt: Prompt::PushFold {
            hero_position: if sb { "sb" } else { "bb" }.into(),
            hand: hand.to_vec(),
            class: class.to_string(),
            stack_bb: s.stack_bb,
            small_blind_bb: s.small_blind_bb,
            ante_bb: s.ante_bb,
            dead_bb: s.dead_bb,
            players,
        },
        answer: AnswerSpec::Choice { options },
        grading: Grading::Modell,
        model_note: Some(MODEL_NOTE.into()),
    }
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::PushFold { hero_position, hand, stack_bb, small_blind_bb, ante_bb, dead_bb, .. } = &ex.prompt else {
        unreachable!()
    };
    let sb = hero_position == "sb";
    let act_id = if sb { "shove" } else { "call" };
    let act_name = if sb { "All-in" } else { "Call" };
    let picked = choice(answer)?;
    if picked != act_id && picked != "fold" {
        return Err(format!("Unbekannte Aktion '{picked}'"));
    }
    let spot = Spot { stack_bb: *stack_bb, small_blind_bb: *small_blind_bb, ante_bb: *ante_bb, dead_bb: *dead_bb };
    let r = solve(spot)?;
    let class = ringside_engine::Combo::new(hand[0], hand[1]).class();
    let (f, ev) = class_decision(&r, sb, class);
    let act = picked == act_id;
    let correct = if act { f > 0.0 } else { f < 1.0 };
    let loss = ev.loss(act);
    let strategy = if f >= 1.0 {
        format!("{act_name} (immer)")
    } else if f <= 0.0 {
        "Fold (immer)".to_string()
    } else {
        format!("Gemischt: {} {act_name}", pct(100.0 * f))
    };
    let range_pct = if sb { r.shove_pct } else { r.call_pct };
    let mut facts = vec![
        Fact::new("Nash-Strategie", strategy),
        Fact::new(&format!("EV {act_name}"), fmt::signed_bb(ev.act)),
        Fact::new("EV Fold", fmt::signed_bb(ev.fold)),
        Fact::new(if sb { "Shove-Range" } else { "Call-Range" }, format!("{} aller Hände", pct(range_pct))),
        Fact::new("Effektiver Stack", fmt::bb(*stack_bb)),
    ];
    if *dead_bb > 0.0 {
        facts.push(Fact::new("Totes Geld", fmt::bb(*dead_bb)));
    }
    if !sb {
        let pot = 2.0 * stack_bb + dead_bb;
        let need = 100.0 * (stack_bb - 1.0 - ante_bb) / pot;
        facts.push(
            Fact::new("Benötigte Equity", pct(need))
                .with(format!("{} ÷ {}", fmt::short(stack_bb - 1.0 - ante_bb), fmt::short(pot))),
        );
    }
    let (truth, text) = if f >= 0.5 { (act_id, act_name) } else { ("fold", "Fold") };
    let mut fb = feedback(correct, if correct { 1.0 } else { decision(loss) }, Answer::Choice(truth.into()), text.into(), facts);
    fb.ev_loss_bb = Some(loss);
    Ok(fb)
}
