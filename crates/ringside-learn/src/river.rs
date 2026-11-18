//! Bet or check on the river, solved live with `ringside-solver`: the ranges of a
//! library scenario (from the preflop model), a random board, one bet size of 75 % of
//! the pot and no raises. Such a river solves in a few milliseconds; the hero's node is
//! kept per spot so that grading does not solve again.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};

use rand::Rng;
use ringside_engine::range::NUM_CLASSES;
use ringside_engine::{Card, CardSet, Combo};
use ringside_solver::library::{PotType, Scenario, catalog};
use ringside_solver::{ActionKind, HandInfo, Settings, Spot, Step, StreetSizes};

use crate::deal::{self, Rng8};
use crate::fmt::{self, pct};
use crate::multiway::{at, subject};
use crate::score::decision;
use crate::{Answer, Exercise, Fact, Feedback, Grading, Grid, Prompt, Task, choice, feedback, options, strategy_text};

pub const MODEL_NOTE: &str = "Gleichgewicht des River-Spiels, vom Solver gerechnet (Abweichung unter 0,3 % des Pots). Annahmen: Ranges aus dem Preflop-Modell, als wäre an Flop und Turn nur gecheckt worden; eine Einsatzgröße von 75 % des Pots, keine Raises.";

const BET_PCT: f64 = 75.0;
const TARGET_PCT: f64 = 0.3;
/// Clear decisions differ by at least this share of the pot.
const CLEAR_POT: f64 = 0.06;
/// Hands that reach the node less than this are left out: rare combos of a mixed range.
const MIN_WEIGHT: f64 = 0.25;
const CACHE_LIMIT: usize = 256;

/// Single-raised pots for the first levels, 3-bet pots added on level 3.
const SRP: [&str; 6] = ["srp-btn-bb-40", "srp-co-bb-40", "srp-utg-bb-40", "srp-btn-bb-75", "srp-co-bb-75", "srp-sb-bb-40"];
const THREE_BET: [&str; 3] = ["3bp-btn-bb-40", "3bp-co-btn-40", "3bp-btn-bb-75"];

/// The hero's decision node: pot, bet size and every combo's strategy and EVs.
pub struct Node {
    pub pot: f64,
    pub bet: f64,
    /// Index of check and bet in each hand's `strategy` and `evs`.
    pub check: usize,
    pub bet_index: usize,
    pub hands: Vec<HandInfo>,
    /// Bet frequency per class, `None` for classes not in the range.
    pub classes: Vec<Option<f64>>,
    /// Share of the range that bets, 0–1.
    pub bet_freq: f64,
}

type Key = (String, String, bool);
static CACHE: LazyLock<Mutex<HashMap<Key, Arc<Node>>>> = LazyLock::new(Default::default);

fn scenario(id: &str) -> Result<&'static Scenario, String> {
    let c = catalog();
    c.scenario(id).map(|i| &c.scenarios[i]).ok_or_else(|| format!("Szenario {id} fehlt in der Bibliothek"))
}

fn board_text(board: &[Card]) -> String {
    board.iter().map(Card::to_string).collect()
}

/// The node where the hero decides: the root out of position, after a check in position.
pub fn node(id: &str, board: &[Card], hero_ip: bool) -> Result<Arc<Node>, String> {
    let key = (id.to_string(), board_text(board), hero_ip);
    if let Some(n) = CACHE.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
        return Ok(n.clone());
    }
    let sc = scenario(id)?;
    let river = StreetSizes { bet: vec![BET_PCT], raise: Vec::new() };
    let spot = Spot {
        oop_range: sc.oop_range.clone(),
        ip_range: sc.ip_range.clone(),
        board: key.1.clone(),
        pot: sc.pot,
        stack: sc.stack,
        sizes: [StreetSizes::default(), StreetSizes::default(), river],
        allin_threshold: sc.allin_threshold,
    };
    // One thread: the same spot then always solves to the same numbers.
    let settings = Settings { target_pct: TARGET_PCT, max_iterations: 2000, threads: 1 };
    let mut sol = ringside_solver::solve(&spot, &settings, 1 << 30, |_| {}, || false).map_err(|e| e.to_string())?;
    let root = sol.node(&[]).map_err(|e| e.to_string())?;
    let n = if hero_ip {
        let check = root.actions.iter().position(|a| a.kind == ActionKind::Check).ok_or("Kein Check am River")?;
        sol.node(&[Step::Action(check)]).map_err(|e| e.to_string())?
    } else {
        root
    };
    let check = n.actions.iter().position(|a| a.kind == ActionKind::Check).ok_or("Kein Check am River")?;
    let bet_index = n.actions.iter().position(|a| a.kind == ActionKind::Bet).ok_or("Kein Bet am River")?;
    let mut classes = vec![None; NUM_CLASSES];
    for c in &n.classes {
        if c.combos > 0.0 {
            classes[c.class] = Some(c.strategy[bet_index]);
        }
    }
    let node = Arc::new(Node {
        pot: n.pot,
        bet: n.actions[bet_index].amount,
        check,
        bet_index,
        hands: n.hands,
        classes,
        bet_freq: n.actions[bet_index].frequency,
    });
    let mut cache = CACHE.lock().unwrap_or_else(PoisonError::into_inner);
    if cache.len() >= CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(key, node.clone());
    Ok(node)
}

fn cards_of(combo: &str) -> Option<Vec<Card>> {
    Some(vec![combo.get(0..2)?.parse().ok()?, combo.get(2..4)?.parse().ok()?])
}

fn find<'a>(node: &'a Node, hand: &[Card]) -> Option<&'a HandInfo> {
    node.hands.iter().find(|h| cards_of(&h.combo).is_some_and(|c| c.contains(&hand[0]) && c.contains(&hand[1])))
}

/// What kind of hand this is for level 1 and 2: value bets and showdown checks on level 1,
/// bluffs and give-ups as well on level 2.
fn fits(level: u8, h: &HandInfo, node: &Node) -> bool {
    let f = h.strategy[node.bet_index];
    let gap = (h.evs[node.bet_index] - h.evs[node.check]).abs();
    let close = (f > 0.03 && f < 0.97) || gap < CLEAR_POT * node.pot;
    if level == 3 {
        return close;
    }
    if close {
        return false;
    }
    let value_or_showdown = (f > 0.5 && h.equity >= 0.6) || (f < 0.5 && (0.3..0.75).contains(&h.equity));
    level == 2 || value_or_showdown
}

pub fn generate(level: u8, rng: &mut Rng8) -> Result<Task, String> {
    for _ in 0..200 {
        let id = if level == 3 && rng.random_bool(0.35) { *deal::pick(rng, &THREE_BET) } else { *deal::pick(rng, &SRP) };
        let hero_ip = level < 3 || rng.random_bool(0.5);
        let mut used = CardSet::EMPTY;
        let board = deal::cards(rng, &mut used, 5);
        let n = node(id, &board, hero_ip)?;
        let candidates: Vec<&HandInfo> = n.hands.iter().filter(|h| h.weight >= MIN_WEIGHT && fits(level, h, &n)).collect();
        if candidates.is_empty() {
            continue;
        }
        let h = candidates[rng.random_range(0..candidates.len())];
        let hand = cards_of(&h.combo).ok_or("Unlesbare Kombo")?;
        return task(id, board, hand, hero_ip, &n);
    }
    Err("Kein passender River-Spot gefunden".into())
}

fn task(id: &str, board: Vec<Card>, hand: Vec<Card>, hero_ip: bool, n: &Node) -> Result<Task, String> {
    let sc = scenario(id)?;
    let class = Combo::new(hand[0], hand[1]).class().to_string();
    let (me, other, my_note) = if hero_ip { (&sc.ip, &sc.oop, &sc.ip_note) } else { (&sc.oop, &sc.ip, &sc.oop_note) };
    let opened = my_note.starts_with("Open");
    let (me_at, them) = (at(me), subject(other));
    let preflop = match (sc.pot_type, opened) {
        (PotType::Srp, true) => format!("Du hast {me_at} vor dem Flop geöffnet, {them} hat gecallt"),
        (PotType::Srp, false) => format!("{} hat vor dem Flop geöffnet, du hast {me_at} gecallt", capitalized(&them)),
        (PotType::ThreeBet, true) => format!("Du hast {me_at} vor dem Flop geöffnet, {them} hat erhöht (3-Bet), du hast gecallt"),
        (PotType::ThreeBet, false) => {
            format!("{} hat vor dem Flop geöffnet, du hast {me_at} erhöht (3-Bet), er hat gecallt", capitalized(&them))
        }
    };
    let river = if hero_ip { format!("Am River checkt {them} zu dir") } else { "Am River bist du zuerst dran".to_string() };
    let question = format!(
        "{preflop}. Flop und Turn wurden gecheckt. {river}. Im Pot liegen {}, du hast noch {}. Setzt du {} ({} des Pots) oder checkst du?",
        fmt::bb(n.pot),
        fmt::bb(sc.stack),
        fmt::bb(n.bet),
        fmt::num(BET_PCT, 0) + " %"
    );
    Ok(Task {
        question,
        prompt: Prompt::RiverBet {
            scenario: id.to_string(),
            board,
            hand,
            class,
            oop: sc.oop.clone(),
            ip: sc.ip.clone(),
            hero_ip,
            pot_type: match sc.pot_type {
                PotType::Srp => "srp",
                PotType::ThreeBet => "3bp",
            }
            .into(),
            pot_bb: n.pot,
            stack_bb: sc.stack,
            bet_bb: n.bet,
        },
        answer: options(("bet", "Bet"), ("check", "Check")),
        grading: Grading::Gleichgewicht,
        model_note: Some(MODEL_NOTE.into()),
    })
}

fn capitalized(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// Why the better action is better, in one sentence.
fn purpose(bet: bool, equity: f64) -> (&'static str, &'static str) {
    match (bet, equity) {
        (true, e) if e >= 0.5 => ("Value-Bet", "Schlechtere Hände zahlen dich aus."),
        (true, _) => ("Bluff", "Bessere Hände sollen folden. Deine Hand gewinnt selten am Showdown."),
        (false, e) if e >= 0.75 => ("Check trotz starker Hand", "Auf einen Bet würden fast nur bessere Hände callen."),
        (false, e) if e >= 0.3 => ("Check mit Showdown-Wert", "Die Hand gewinnt oft ohne Bet. Mit Bet callen meist nur bessere Hände."),
        (false, _) => ("Aufgeben", "Für einen Bluff passt die Hand hier nicht. Andere Hände bluffen besser."),
    }
}

pub fn grade(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::RiverBet { scenario, board, hand, hero_ip, oop, ip, .. } = &ex.prompt else { unreachable!() };
    let picked = choice(answer)?;
    if picked != "bet" && picked != "check" {
        return Err(format!("Unbekannte Aktion '{picked}'"));
    }
    let n = node(scenario, board, *hero_ip)?;
    let h = find(&n, hand).ok_or("Diese Hand ist nicht in der Range")?;
    let f = h.strategy[n.bet_index];
    let (ev_bet, ev_check) = (h.evs[n.bet_index], h.evs[n.check]);
    let bet = picked == "bet";
    let correct = if bet { f > 0.0 } else { f < 1.0 };
    let loss = (ev_bet.max(ev_check) - if bet { ev_bet } else { ev_check }).max(0.0);
    let best_bet = f >= 0.5;
    let (why, reason) = purpose(best_bet, h.equity);
    let me = if *hero_ip { ip } else { oop };
    let other = if *hero_ip { oop } else { ip };
    let facts = vec![
        Fact::new("Gleichgewicht", strategy_text(f, "Bet", "Check")),
        Fact::new("EV Bet", format!("{} bb", fmt::num(ev_bet, 2))),
        Fact::new("EV Check", format!("{} bb", fmt::num(ev_check, 2))),
        Fact::new(&format!("Equity gegen die Range von {other}"), pct(100.0 * h.equity)),
        Fact::new(why, reason),
        Fact::new(&format!("Bet-Anteil {me} an diesem River"), format!("{} der Range", pct(100.0 * n.bet_freq))),
    ];
    let class = Combo::new(hand[0], hand[1]).class();
    let (truth, text) = if best_bet { ("bet", "Bet") } else { ("check", "Check") };
    let mut fb = feedback(correct, if correct { 1.0 } else { decision(loss) }, Answer::Choice(truth.into()), text.into(), facts);
    fb.ev_loss_bb = Some(loss);
    fb.grid = Some(Grid {
        title: format!("Bet-Anteil {me} je Hand"),
        action: "Bet".into(),
        cells: n.classes.clone(),
        hero: class.index(),
        pct: 100.0 * n.bet_freq,
    });
    Ok(fb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scenario_is_in_the_library() {
        for id in SRP.iter().chain(&THREE_BET) {
            let sc = scenario(id).unwrap();
            assert!(sc.stack > sc.pot * BET_PCT / 100.0, "{id}: a 75 % bet must not be all-in");
        }
    }

    /// The node's strategies are probabilities and the hero's EV is never above the pot plus what a call adds.
    #[test]
    fn solved_nodes_are_consistent() {
        let board: Vec<Card> = ["Ks", "7d", "2c", "3h", "9s"].iter().map(|c| c.parse().unwrap()).collect();
        for ip in [true, false] {
            let n = node("srp-btn-bb-40", &board, ip).unwrap();
            assert!((n.bet - 0.75 * n.pot).abs() < 0.02, "{} of {}", n.bet, n.pot);
            for h in &n.hands {
                assert!((h.strategy.iter().sum::<f64>() - 1.0).abs() < 1e-3);
                assert!((0.0..=1.0).contains(&h.equity));
                assert!(h.evs.iter().all(|&ev| ev > -n.bet - 1e-6 && ev < 2.0 * n.pot + 1e-6), "{h:?}");
            }
            // Same spot, same numbers.
            CACHE.lock().unwrap().clear();
            let again = node("srp-btn-bb-40", &board, ip).unwrap();
            assert_eq!(again.hands, n.hands);
        }
    }

    /// Generated spots are real deals graded by the solved strategy of the dealt combo.
    #[test]
    fn spots_follow_the_solver() {
        for level in 1..=3 {
            for seed in 0..6 {
                let ex = crate::generate(crate::Kind::RiverBet, level, seed).unwrap();
                let Prompt::RiverBet { scenario, board, hand, hero_ip, .. } = &ex.prompt else { unreachable!() };
                let mut all = board.clone();
                all.extend(hand);
                assert_eq!(CardSet::from_cards(&all).len(), 7, "{all:?}");
                assert!(level == 3 || *hero_ip);
                let n = node(scenario, board, *hero_ip).unwrap();
                let h = find(&n, hand).unwrap();
                assert!(h.weight >= MIN_WEIGHT);
                let f = h.strategy[n.bet_index];
                let (_, fb) = crate::check(crate::Kind::RiverBet, level, seed, &Answer::Choice("bet".into())).unwrap();
                assert_eq!(fb.correct, f > 0.0);
                assert_eq!(fb.truth, Answer::Choice(if f >= 0.5 { "bet" } else { "check" }.into()));
                // Mixed hands are worth the same either way, up to what the solve leaves.
                let (_, best) = crate::check(crate::Kind::RiverBet, level, seed, &fb.truth).unwrap();
                assert!(best.correct && best.ev_loss_bb.unwrap() < TARGET_PCT / 100.0 * n.pot, "{best:?}");
            }
        }
    }
}
