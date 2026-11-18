//! Open or fold once everyone before folded, by Ringside's preflop model
//! ([`ringside_engine::preflop`]) from the tables shipped with the app: 6, 8 and 9
//! players, equal stacks, chip EV. The small blind (which may also limp) and the big
//! blind (which never opens) are not asked.

use rand::Rng;
use ringside_engine::preflop::{self, Kind as Act, Solution};
use ringside_engine::preflop_table::{self, Ante};
use ringside_engine::range::NUM_CLASSES;
use ringside_engine::{CardSet, Combo, HandClass};

use crate::deal::{self, Rng8};
use crate::fmt::{self, pct};
use crate::multiway::{at, position_names};
use crate::score::decision;
use crate::{Answer, Exercise, Fact, Feedback, Grading, Grid, Prompt, Task, choice, feedback, options, strategy_text};

pub const MODEL_NOTE: &str = "Preflop-Modell von Ringside: Strategie je Handklasse mit festen Einsatzgrößen, alle Stacks gleich, in Chips gerechnet (ohne ICM). Nach dem Flop rechnet es mit einer Formel statt mit echtem Postflop-Spiel.";

/// Clear decisions differ by at least this much, in big blinds per hand.
const CLEAR_BB: f64 = 0.1;

/// Players, stacks and antes of each level: 6-max deep without ante; 8 and 9 players with
/// antes; every table and depth, close spots only.
fn table(level: u8, rng: &mut Rng8) -> (Ante, usize, f64) {
    match level {
        1 => (Ante::None, 6, *deal::pick(rng, &[40.0, 50.0, 75.0, 100.0])),
        2 => (*deal::pick(rng, &[Ante::Classic, Ante::BigBlind]), *deal::pick(rng, &[8, 9]), *deal::pick(rng, &[30.0, 40.0, 50.0, 75.0, 100.0])),
        _ => (*deal::pick(rng, &Ante::ALL), *deal::pick(rng, &preflop_table::PLAYERS), *deal::pick(rng, &preflop_table::STACKS)),
    }
}

fn chart(ante: Ante, players: usize, stack_bb: f64) -> Result<Solution, String> {
    preflop_table::chart(ante, players, stack_bb).ok_or_else(|| "Für diesen Tisch gibt es keine Preflop-Tabelle".into())
}

/// Share of the class that opens (any action but fold) and the EVs of the best open and of folding.
pub struct Decision {
    pub open: f64,
    pub ev_open: f64,
    pub ev_fold: f64,
    /// (action, frequency) of the opening actions the class takes.
    pub how: Vec<(Act, f64, f64)>,
}

pub fn decide(sol: &Solution, seat: usize, class: HandClass) -> Option<Decision> {
    let v = sol.open_node(seat)?;
    let node = &sol.nodes[v];
    let c = class.index();
    let mut d = Decision { open: 0.0, ev_open: f64::NEG_INFINITY, ev_fold: 0.0, how: Vec::new() };
    for (a, act) in node.actions.iter().enumerate() {
        let f = if preflop::taken(node.freq[a][c]) { node.freq[a][c] } else { 0.0 };
        if act.kind == Act::Fold {
            d.ev_fold = node.ev[a][c];
        } else {
            d.open += f;
            d.ev_open = d.ev_open.max(node.ev[a][c]);
            if f > 0.0 {
                d.how.push((act.kind, act.to_bb, f));
            }
        }
    }
    // Snap so that "always" and "never" read as such.
    if d.open > 0.995 {
        d.open = 1.0;
    }
    Some(d)
}

pub fn generate(level: u8, rng: &mut Rng8) -> Result<Task, String> {
    let (ante, players, stack) = table(level, rng);
    let sol = chart(ante, players, stack)?;
    for _ in 0..10_000 {
        let seat = rng.random_range(0..players - 2);
        let mut used = CardSet::EMPTY;
        let hand = deal::cards(rng, &mut used, 2);
        let class = Combo::new(hand[0], hand[1]).class();
        let Some(d) = decide(&sol, seat, class) else { continue };
        let close = (d.open > 0.0 && d.open < 1.0) || (d.ev_open - d.ev_fold).abs() < CLEAR_BB;
        if close != (level == 3) {
            continue;
        }
        let (ante_bb, bb_ante_bb) = ante.amounts();
        let positions: Vec<String> = position_names(players).into_iter().map(String::from).collect();
        let open_to_bb = ringside_engine::preflop::Sizes::standard(stack).open_bb;
        let mut setup = format!("{players} Spieler, alle mit {}", fmt::bb(stack));
        if ante_bb > 0.0 {
            setup.push_str(&format!(", Ante {} bb", fmt::num(ante_bb, 3)));
        }
        if bb_ante_bb > 0.0 {
            setup.push_str(&format!(", Big-Blind-Ante {}", fmt::bb(bb_ante_bb)));
        }
        let question = format!(
            "{setup}. Du sitzt {}, alle vor dir haben gefoldet. Öffnest du mit {class}?",
            at(&positions[seat])
        );
        return Ok(Task {
            question,
            prompt: Prompt::OpenFold {
                positions,
                hero: seat,
                hand,
                class: class.to_string(),
                stack_bb: stack,
                ante_bb,
                bb_ante_bb,
                open_to_bb,
            },
            answer: options(("open", "Öffnen"), ("fold", "Fold")),
            grading: Grading::Modell,
            model_note: Some(MODEL_NOTE.into()),
        });
    }
    Err("Kein passender Spot gefunden".into())
}

fn ante_of(ante_bb: f64, bb_ante_bb: f64) -> Ante {
    Ante::ALL.into_iter().find(|a| a.amounts() == (ante_bb, bb_ante_bb)).unwrap_or(Ante::None)
}

fn act_text(kind: Act, to_bb: f64) -> String {
    match kind {
        Act::AllIn => "All-in".into(),
        Act::Limp => "Limp".into(),
        _ => format!("Raise auf {}", fmt::bb(to_bb)),
    }
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::OpenFold { positions, hero, hand, stack_bb, ante_bb, bb_ante_bb, .. } = &ex.prompt else { unreachable!() };
    let picked = choice(answer)?;
    if picked != "open" && picked != "fold" {
        return Err(format!("Unbekannte Aktion '{picked}'"));
    }
    let sol = chart(ante_of(*ante_bb, *bb_ante_bb), positions.len(), *stack_bb)?;
    let class = Combo::new(hand[0], hand[1]).class();
    let d = decide(&sol, *hero, class).ok_or("Diesen Spot gibt es im Modell nicht")?;
    let open = picked == "open";
    let correct = if open { d.open > 0.0 } else { d.open < 1.0 };
    let loss = (d.ev_open.max(d.ev_fold) - if open { d.ev_open } else { d.ev_fold }).max(0.0);
    let ranges: Vec<(String, f64)> = (0..positions.len() - 2)
        .filter_map(|s| Some((positions[s].clone(), sol.play_pct(sol.open_node(s)?))))
        .collect();
    let mine = ranges[*hero].1;
    let mut facts = vec![
        Fact::new("Modell-Strategie", strategy_text(d.open, "Öffnen", "Fold")),
        Fact::new("EV Öffnen", fmt::signed_bb(d.ev_open)),
        Fact::new("EV Fold", fmt::signed_bb(d.ev_fold)),
        Fact::new(&format!("Open-Range {}", positions[*hero]), format!("{} aller Hände", pct(mine))),
        Fact::new(
            "Je später, desto breiter",
            ranges.iter().map(|(p, x)| format!("{p} {}", pct(*x))).collect::<Vec<_>>().join(" · "),
        ),
    ];
    if d.open > 0.0 {
        facts.push(Fact::new(
            "So öffnet das Modell",
            d.how.iter().map(|(k, to, f)| format!("{} ({})", act_text(*k, *to), pct(100.0 * f))).collect::<Vec<_>>().join(", "),
        ));
    }
    let v = sol.open_node(*hero).ok_or("Diesen Spot gibt es im Modell nicht")?;
    let node = &sol.nodes[v];
    let cells = (0..NUM_CLASSES)
        .map(|c| {
            let open: f64 = node
                .actions
                .iter()
                .enumerate()
                .filter(|(_, a)| a.kind != Act::Fold)
                .map(|(a, _)| node.freq[a][c])
                .filter(|&f| preflop::taken(f))
                .sum();
            Some(open.min(1.0))
        })
        .collect();
    let (truth, text) = if d.open >= 0.5 { ("open", "Öffnen") } else { ("fold", "Fold") };
    let mut fb = feedback(correct, if correct { 1.0 } else { decision(loss) }, Answer::Choice(truth.into()), text.into(), facts);
    fb.ev_loss_bb = Some(loss);
    fb.grid = Some(Grid {
        title: format!("Open-Range {}", positions[*hero]),
        action: "Öffnen".into(),
        cells,
        hero: class.index(),
        pct: mine,
    });
    Ok(fb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every spot is an open decision of a seat before the blinds, graded by the model's
    /// table for exactly that table; the grid shows the same frequency for the hero's class.
    #[test]
    fn spots_follow_the_preflop_tables() {
        for level in 1..=3 {
            for seed in 0..8 {
                let ex = crate::generate(crate::Kind::OpenFold, level, seed).unwrap();
                let Prompt::OpenFold { positions, hero, hand, stack_bb, ante_bb, bb_ante_bb, class, .. } = &ex.prompt else {
                    unreachable!()
                };
                let n = positions.len();
                assert!(*hero < n - 2, "no blinds");
                match level {
                    1 => assert!(n == 6 && *ante_bb == 0.0 && *bb_ante_bb == 0.0 && *stack_bb >= 40.0),
                    2 => assert!(n >= 8 && ante_bb + bb_ante_bb > 0.0),
                    _ => {}
                }
                let c = Combo::new(hand[0], hand[1]).class();
                assert_eq!(c.to_string(), *class);
                let sol = chart(ante_of(*ante_bb, *bb_ante_bb), n, *stack_bb).unwrap();
                let d = decide(&sol, *hero, c).unwrap();
                let close = (d.open > 0.0 && d.open < 1.0) || (d.ev_open - d.ev_fold).abs() < CLEAR_BB;
                assert_eq!(close, level == 3);
                let (_, fb) = crate::check(crate::Kind::OpenFold, level, seed, &Answer::Choice("open".into())).unwrap();
                assert_eq!(fb.correct, d.open > 0.0);
                let grid = fb.grid.unwrap();
                assert!((grid.cells[c.index()].unwrap() - d.open).abs() < 0.006);
                assert_eq!(grid.hero, c.index());
            }
        }
    }
}
