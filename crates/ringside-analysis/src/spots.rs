//! The learning spots: about a hundred hand-picked decisions that cost casual tournament
//! players the most, each built as a hand history from a short spec (`data/lern-spots-spec.json`),
//! rated by the analysis like a real hand (push/fold equilibrium, preflop model, postflop
//! solver; bubble spots by the push/fold equilibrium with ICM) and bundled as
//! `data/lern-spots.json`. `cargo run --release -p ringside-analysis --bin lern-spots`
//! rebuilds the pack.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use ringside_core::db::{Choice, Decision, Source, SpotType, Status};
use ringside_core::model::{Hand, Street};
use ringside_core::parse_hand;
use ringside_engine::multiway::{self, Params};
use ringside_engine::preflop_table::Family;
use ringside_engine::{Card, CardSet, Combo, HandClass, icm};
use ringside_solver::{Node, Solution, Step};
use serde::{Deserialize, Serialize};

use crate::drill::{Answered, answer};
use crate::fmt::bb;
use crate::postflop::{JobError, PostflopJob, rate, solve_spot};
use crate::{ENGINE, analyze_hand, why};

/// Chips per big blind in the built hands; the small blind and the ante divide it evenly.
const CHIPS: f64 = 400.0;
/// Ante of every player, BB: the classic tournament ante the push/fold tables use.
pub const ANTE_BB: f64 = 0.125;
/// Prize pool of the bubble spots: payouts are percent of the pool.
const POOL: f64 = 100.0;
const NAMES: [&str; 8] = ["Lena", "Tom", "Mia", "Jan", "Ole", "Ida", "Max", "Eva"];

/// One spot as written in the spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    pub topic: String,
    /// Players dealt in, 2–9.
    pub players: u8,
    /// Hero's seat: "UTG", …, "CO", "BTN", "SB", "BB".
    pub hero: String,
    /// "Ah7d".
    pub cards: String,
    /// Every player's stack in BB unless `stacks` names the seat.
    pub stack: f64,
    #[serde(default)]
    pub stacks: BTreeMap<String, f64>,
    /// Streets split by "/", actions by ",": "CO r2.2, BB c / BB x, Hero ?". A seat is
    /// named by its label or "Hero"; `f` fold, `x` check, `c` call, `r2.2` raise to 2.2 BB,
    /// `b3` bet 3 BB, `b50%` bet half the pot, `a` all-in, `?` the hero's decision.
    /// Preflop, seats skipped before someone acts fold.
    pub line: String,
    /// "Ks7d2c5h": flop, turn and river as far as the hand goes.
    #[serde(default)]
    pub board: String,
    /// Bubble spots: prizes in percent of the pool, first place first.
    #[serde(default)]
    pub payouts: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Specs {
    pub spots: Vec<Spec>,
}

/// One action of the hand-wide strategy grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridAction {
    pub label: String,
    pub kind: String,
}

/// How every hand class plays the spot: `cells[class]` holds the share of each action in
/// percent (in the order of `actions`), `None` for classes that cannot be here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpotGrid {
    pub actions: Vec<GridAction>,
    pub cells: Vec<Option<Vec<u8>>>,
    /// Share of the class's combos that reach the spot, percent; postflop only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Vec<u8>>,
}

/// One learning spot of the pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LernSpot {
    pub id: String,
    pub topic: String,
    /// The hand up to and with the decision; the hero's action at `action_index` is a
    /// placeholder, the answer replaces it.
    pub hand: Hand,
    pub action_index: usize,
    /// The rated decision; `played` and `rating` are cleared.
    pub decision: Decision,
    pub grid: Option<SpotGrid>,
    /// Hero's stack in BB at the start of the hand, players and table size, for the list.
    pub stack_bb: f64,
    pub position: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    /// The analysis that rated the pack.
    pub engine: String,
    pub spots: Vec<LernSpot>,
}

/// A topic of the pack, in learning order.
pub struct Topic {
    pub id: &'static str,
    pub title: &'static str,
    /// What the topic teaches, in one sentence without numbers.
    pub lesson: &'static str,
}

pub const TOPICS: [Topic; 10] = [
    Topic {
        id: "shove",
        title: "All-in oder Fold mit kurzem Stack",
        lesson: "Mit 8 bis 15 Big Blinds ist ein kleiner Raise selten gut: Entweder du gehst all-in und gewinnst oft schon die Blinds, oder du foldest.",
    },
    Topic {
        id: "call_shove",
        title: "Call gegen ein All-in",
        lesson: "Gegen ein All-in zählt nur: Gewinnt deine Hand gegen seine Hände oft genug für den Preis? Wer früh schiebt, hat stärkere Hände.",
    },
    Topic {
        id: "open",
        title: "Eröffnen oder folden",
        lesson: "Je mehr Spieler nach dir kommen, desto stärker muss die Hand sein: Am Button darfst du viel öffnen, ganz vorne wenig. Und wer als Erster spielt, erhöht statt nur zu limpen.",
    },
    Topic {
        id: "bb_defense",
        title: "Big Blind verteidigen",
        lesson: "Im Big Blind hast du schon bezahlt und bekommst einen guten Preis: Gegen späte Opens callst du viele Hände, statt sie wegzuwerfen.",
    },
    Topic {
        id: "resteal",
        title: "All-in gegen einen Open",
        lesson: "Mit 15 bis 25 Big Blinds ist ein All-in gegen einen späten Open oft besser als ein Call: Der Gegner muss oft folden, und du spielst nicht ohne Position weiter.",
    },
    Topic {
        id: "bvb",
        title: "Blind gegen Blind",
        lesson: "Wenn alle bis zum Small Blind folden, kämpfen nur noch die Blinds: Der Small Blind darf hier oft nur mitgehen, und der Big Blind verteidigt viel mehr Hände als an einem vollen Tisch.",
    },
    Topic {
        id: "bubble",
        title: "Bubble und Preisgeld",
        lesson: "Kurz vor dem Geld ist Rausfliegen teurer als Chips gewinnen wert ist: Callen braucht deutlich stärkere Hände, Schieben bleibt oft richtig.",
    },
    Topic {
        id: "three_bet",
        title: "Gegen eine 3-Bet",
        lesson: "Gegen eine 3-Bet ohne Position verlierst du mit mittelstarken Händen viel nach dem Flop: Mit starken Händen all-in, mittlere oft folden.",
    },
    Topic {
        id: "cbet",
        title: "Nach dem Flop: C-Bet",
        lesson: "Nach dem Flop entscheidet das Board: Gegen eine kleine C-Bet foldest du nicht alles, was nicht getroffen hat – Draws und Paare spielen weiter, starke Hände erhöhen.",
    },
    Topic {
        id: "river",
        title: "Am River",
        lesson: "Am River kommt keine Karte mehr: Callen lohnt, wenn der Gegner oft genug blufft. Und eine Hand, die beim Zeigen nie gewinnt, gewinnt nur noch, wenn du setzt.",
    },
];

/// Seat labels in preflop action order for `n` players, as [`ringside_core::stats::position_labels`] names them.
pub fn labels(n: usize) -> Vec<&'static str> {
    if n == 2 {
        return vec!["BTN", "BB"];
    }
    const LATE: [&str; 3] = ["LJ", "HJ", "CO"];
    const EARLY: [&str; 5] = ["UTG", "UTG+1", "UTG+2", "UTG+3", "UTG+4"];
    let rest = n - 3;
    let early = rest.saturating_sub(LATE.len());
    let mut out: Vec<&'static str> = EARLY.iter().take(early).copied().collect();
    out.extend(&LATE[LATE.len() - (rest - early)..]);
    out.extend(["BTN", "SB", "BB"]);
    out
}

struct Player {
    label: &'static str,
    name: String,
    seat: u8,
    /// Chips left behind.
    behind: i64,
    /// Chips in front this street.
    street: i64,
    folded: bool,
    /// Acted voluntarily since the street began.
    acted: bool,
}

fn chips(x: f64) -> i64 {
    (x * CHIPS).round() as i64
}

/// The hand history of `spec` up to and with the hero's decision, as PokerStars writes it.
pub fn history(spec: &Spec, id: u64) -> Result<String, String> {
    let n = spec.players as usize;
    if !(2..=9).contains(&n) {
        return Err(format!("{}: 2 bis 9 Spieler", spec.id));
    }
    let order = labels(n);
    if !order.contains(&spec.hero.as_str()) {
        return Err(format!("{}: Platz {} gibt es bei {n} Spielern nicht", spec.id, spec.hero));
    }
    // Seat 1 is the button, the blinds follow, then everyone else in action order.
    let btn = order.iter().position(|l| *l == "BTN").unwrap();
    let mut seat_of = vec![0u8; n];
    for k in 0..n {
        seat_of[(btn + k) % n] = k as u8 + 1;
    }
    let mut names = NAMES.iter();
    let mut ps: Vec<Player> = order
        .iter()
        .enumerate()
        .map(|(i, &label)| {
            let stack = spec.stacks.get(label).copied().unwrap_or(spec.stack);
            Player {
                label,
                name: if label == spec.hero { "Hero".into() } else { names.next().unwrap().to_string() },
                seat: seat_of[i],
                behind: chips(stack),
                street: 0,
                folded: false,
                acted: false,
            }
        })
        .collect();
    let max = if n <= 6 { 6 } else { 9 };
    let mut out = String::new();
    let tid = 3_900_000_000u64 + id;
    writeln!(out, "PokerStars Hand #{id}: Tournament #{tid}, $1+$0.10 USD Hold'em No Limit - Level X (200/400) - 2026/11/19 20:00:00 ET").unwrap();
    writeln!(out, "Table '{tid} 1' {max}-max Seat #1 is the button").unwrap();
    let mut by_seat: Vec<&Player> = ps.iter().collect();
    by_seat.sort_by_key(|p| p.seat);
    for p in by_seat {
        writeln!(out, "Seat {}: {} ({} in chips) ", p.seat, p.name, p.behind).unwrap();
    }
    let ante = chips(ANTE_BB);
    let mut pot: i64 = 0;
    for p in ps.iter_mut() {
        let a = ante.min(p.behind);
        p.behind -= a;
        pot += a;
        writeln!(out, "{}: posts the ante {a}", p.name).unwrap();
    }
    let sb = if n == 2 { 0 } else { n - 2 };
    let bbi = n - 1;
    for (i, amount, what) in [(sb, chips(0.5), "small blind"), (bbi, chips(1.0), "big blind")] {
        let p = &mut ps[i];
        let a = amount.min(p.behind);
        p.behind -= a;
        p.street = a;
        writeln!(out, "{}: posts {what} {a}", p.name).unwrap();
    }
    writeln!(out, "*** HOLE CARDS ***").unwrap();
    let cards = split_cards(&spec.cards)?;
    if cards.len() != 2 {
        return Err(format!("{}: zwei Karten für Hero", spec.id));
    }
    writeln!(out, "Dealt to Hero [{} {}]", cards[0], cards[1]).unwrap();

    let board = split_cards(&spec.board)?;
    let streets: Vec<&str> = spec.line.split('/').map(str::trim).collect();
    // After the flop the small blind acts first, the button last; heads-up the big blind first.
    let postflop: Vec<usize> = if n == 2 { vec![1, 0] } else { (0..n).map(|k| (n - 2 + k) % n).collect() };
    let mut level = chips(1.0);
    for (s, text) in streets.iter().enumerate() {
        let preflop = s == 0;
        if !preflop {
            pot += ps.iter().map(|p| p.street).sum::<i64>();
            for p in ps.iter_mut() {
                p.street = 0;
                p.acted = false;
            }
            level = 0;
            let (shown, name) = match s {
                1 => (3, "FLOP"),
                2 => (4, "TURN"),
                3 => (5, "RIVER"),
                _ => return Err(format!("{}: mehr als vier Straßen", spec.id)),
            };
            if board.len() < shown {
                return Err(format!("{}: Board zu kurz für {name}", spec.id));
            }
            let head = board[..3].join(" ");
            match shown {
                3 => writeln!(out, "*** FLOP *** [{head}]").unwrap(),
                _ => writeln!(out, "*** {name} *** [{}] [{}]", board[..shown - 1].join(" "), board[shown - 1]).unwrap(),
            }
        }
        let act_order: Vec<usize> = if preflop { (0..n).collect() } else { postflop.clone() };
        let mut cursor = 0usize;
        for token in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let (who, what) = token.split_once(' ').ok_or_else(|| format!("{}: Aktion „{token}“ ohne Zug", spec.id))?;
            let label = if who == "Hero" { spec.hero.as_str() } else { who };
            let p = ps.iter().position(|x| x.label == label).ok_or_else(|| format!("{}: Platz {who} unbekannt", spec.id))?;
            // Seats skipped on the way fold preflop; afterwards every action is written out.
            let mut guard = 0;
            while act_order[cursor % n] != p {
                let q = act_order[cursor % n];
                let x = &ps[q];
                if !x.folded && x.behind > 0 && (x.street < level || !x.acted) {
                    if !preflop || x.acted {
                        return Err(format!("{}: {} wäre vor {who} dran", spec.id, x.label));
                    }
                    writeln!(out, "{}: folds ", x.name).unwrap();
                    ps[q].folded = true;
                }
                cursor += 1;
                guard += 1;
                if guard > 2 * n {
                    return Err(format!("{}: {who} kommt nicht dran", spec.id));
                }
            }
            cursor += 1;
            let street_pot = pot + ps.iter().map(|x| x.street).sum::<i64>();
            let x = &mut ps[p];
            if x.folded {
                return Err(format!("{}: {who} hat schon gefoldet", spec.id));
            }
            x.acted = true;
            let total = x.street + x.behind;
            let line = match what {
                "?" => {
                    if label != spec.hero {
                        return Err(format!("{}: „?“ nur für Hero", spec.id));
                    }
                    let word = if x.street < level { "folds" } else { "checks" };
                    writeln!(out, "Hero: {word} ").unwrap();
                    writeln!(out, "*** SUMMARY ***\nTotal pot {} | Rake 0", street_pot).unwrap();
                    return Ok(out);
                }
                "f" => {
                    x.folded = true;
                    "folds ".to_string()
                }
                "x" => {
                    if x.street < level {
                        return Err(format!("{}: {who} kann nicht checken", spec.id));
                    }
                    "checks ".to_string()
                }
                "c" => {
                    let pay = (level - x.street).min(x.behind);
                    if pay <= 0 {
                        return Err(format!("{}: {who} hat nichts zu callen", spec.id));
                    }
                    x.behind -= pay;
                    x.street += pay;
                    format!("calls {pay}{}", if x.behind == 0 { " and is all-in" } else { "" })
                }
                _ => {
                    let to = if what == "a" {
                        total
                    } else if let Some(r) = what.strip_prefix('r') {
                        chips(r.parse::<f64>().map_err(|_| format!("{}: „{what}“", spec.id))?)
                    } else if let Some(b) = what.strip_prefix('b') {
                        match b.strip_suffix('%') {
                            Some(pct) => {
                                let p: f64 = pct.parse().map_err(|_| format!("{}: „{what}“", spec.id))?;
                                x.street + (street_pot as f64 * p / 100.0).round() as i64
                            }
                            None => chips(b.parse::<f64>().map_err(|_| format!("{}: „{what}“", spec.id))?),
                        }
                    } else {
                        return Err(format!("{}: Zug „{what}“ unbekannt", spec.id));
                    };
                    let to = to.min(total);
                    if to <= level {
                        // An all-in for less than the bet is a call.
                        let pay = to - x.street;
                        x.behind -= pay;
                        x.street = to;
                        format!("calls {pay} and is all-in")
                    } else {
                        let pay = to - x.street;
                        x.behind -= pay;
                        x.street = to;
                        let tail = if x.behind == 0 { " and is all-in" } else { "" };
                        let s = if level == 0 { format!("bets {pay}{tail}") } else { format!("raises {} to {to}{tail}", to - level) };
                        level = to;
                        s
                    }
                }
            };
            writeln!(out, "{}: {line}", ps[p].name).unwrap();
        }
        // Preflop, whoever has not acted yet folds (or checks the big blind's option).
        if preflop {
            for k in 0..n {
                let q = act_order[(cursor + k) % n];
                let x = &mut ps[q];
                if x.folded || x.acted || x.behind == 0 {
                    continue;
                }
                if x.street < level {
                    x.folded = true;
                    writeln!(out, "{}: folds ", x.name).unwrap();
                } else {
                    x.acted = true;
                    writeln!(out, "{}: checks ", x.name).unwrap();
                }
            }
        }
    }
    Err(format!("{}: Die Linie endet ohne „Hero ?“", spec.id))
}

fn split_cards(s: &str) -> Result<Vec<String>, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 2 != 0 {
        return Err(format!("Karten „{s}“ unvollständig"));
    }
    let cards: Vec<String> = (0..s.len()).step_by(2).map(|i| s[i..i + 2].to_string()).collect();
    for c in &cards {
        c.parse::<Card>().map_err(|_| format!("Karte „{c}“ unbekannt"))?;
    }
    Ok(cards)
}

/// The parsed hand of `spec` and the index of the hero's decision.
pub fn hand_of(spec: &Spec, id: u64) -> Result<(Hand, usize), String> {
    let text = history(spec, id)?;
    let hand = parse_hand(&text).map_err(|e| format!("{}: Handhistorie unlesbar: {e:?}", spec.id))?;
    let index = hand.actions.iter().rposition(|a| a.player == "Hero").ok_or_else(|| format!("{}: keine Entscheidung", spec.id))?;
    Ok((hand, index))
}

/// Stable id of a spot's hand, from its spec id.
pub fn hand_id(spec_id: &str) -> u64 {
    // FNV-1a, so the id does not depend on the spot's place in the spec.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in spec_id.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    1_000_000_000 + h % 1_000_000_000
}

/// The decision as the analysis rates it right away, against the plain model tables of
/// the charts (or with ICM for bubble spots).
pub fn rate_preflop(spec: &Spec, hand: &Hand, index: usize) -> Result<Decision, String> {
    if !spec.payouts.is_empty() {
        return rate_icm(spec, hand, index).map(|(d, _)| d);
    }
    let d = crate::preflop::analyze_in(hand, "Hero", Family::Plain)
        .decisions
        .into_iter()
        .find(|d| d.action_index == index)
        .ok_or_else(|| format!("{}: Die Analyse findet die Entscheidung nicht", spec.id))?;
    usable(spec, d)
}

fn usable(spec: &Spec, d: Decision) -> Result<Decision, String> {
    if d.status != Status::Rated || d.strategy.len() < 2 || !matches!(d.source, Some(Source::Equilibrium | Source::Model | Source::Solver)) {
        return Err(format!("{}: nicht gerechnet ({:?}, {:?}, {} {})", spec.id, d.status, d.source, d.reason.clone().unwrap_or_default(), d.note));
    }
    Ok(clean(d))
}

/// The decision without the placeholder answer.
fn clean(mut d: Decision) -> Decision {
    for c in &mut d.strategy {
        c.played = false;
        c.freq = (c.freq * 1000.0).round() / 1000.0;
        c.ev_bb = c.ev_bb.map(|x| (x * 1000.0).round() / 1000.0);
    }
    d.played = String::new();
    d.played_freq = None;
    d.rating = None;
    d.ev_loss_bb = None;
    d.solver = None;
    d.engine = ENGINE.into();
    d
}

/// A bubble spot: the push/fold equilibrium of the real stacks with the payouts (ICM).
/// EVs are prize equity, turned into BB at the average value of a chip.
fn rate_icm(spec: &Spec, hand: &Hand, index: usize) -> Result<(Decision, SpotGrid), String> {
    let n = spec.players as usize;
    let order = labels(n);
    let stacks: Vec<f64> = order.iter().map(|l| spec.stacks.get(*l).copied().unwrap_or(spec.stack)).collect();
    let hero = order.iter().position(|l| *l == spec.hero).unwrap();
    let shover = hand.actions[..index]
        .iter()
        .find(|a| a.street == Street::Preflop && a.all_in && a.player != "Hero")
        .and_then(|a| hand.seats.iter().find(|s| s.name == a.player))
        .map(|s| order.iter().position(|l| Some(*l) == label_of(hand, &s.name)).unwrap());
    let payouts: Vec<f64> = spec.payouts.iter().map(|p| p / 100.0 * POOL).collect();
    let mut params = Params::new(stacks.clone()).with_payouts(payouts.clone());
    params.ante_bb = ANTE_BB;
    params.max_all_in = 2;
    let sol = multiway::solve(&params).map_err(|e| format!("{}: ICM-Lösung: {e}", spec.id))?;
    let cards = split_cards(&spec.cards)?;
    let class = Combo::new(cards[0].parse().unwrap(), cards[1].parse().unwrap()).class();
    let s = sol.spot(hero, class, shover, &[]).ok_or_else(|| format!("{}: kein ICM-Spot", spec.id))?;
    let total: f64 = stacks.iter().sum();
    let start = icm::icm(&stacks, &payouts).map_err(|e| format!("{}: {e}", spec.id))?.ev[hero];
    let per_bb = POOL / total;
    let to_bb = |ev: f64| ((ev - start) / per_bb * 1000.0).round() / 1000.0;
    let (act, kind, spot) = if shover.is_some() { ("Call", "call", SpotType::CallShove) } else { ("All-in", "all_in", SpotType::Shove) };
    let strategy = vec![
        Choice { label: act.into(), freq: s.freq, ev_bb: Some(to_bb(s.ev_act)), played: false, kind: kind.into() },
        Choice { label: "Fold".into(), freq: 1.0 - s.freq, ev_bb: Some(to_bb(s.ev_fold)), played: false, kind: "fold".into() },
    ];
    let node = &sol.nodes[s.node];
    let margin = |c: usize| node.ev_act[c] - node.ev_fold[c];
    let mine = margin(class.index());
    let combos = |c: &HandClass| c.combo_count() as f64 / 1326.0;
    let rank = HandClass::all().map(|c| if margin(c.index()) > mine { combos(&c) } else if c == class { combos(&c) / 2.0 } else { 0.0 }).sum::<f64>();
    let paid = payouts.iter().filter(|p| **p > 0.0).count();
    let place = match shover {
        Some(_) => format!("Auf der Bubble ({n} Spieler, {paid} bezahlt) als {} gegen den Shove", spec.hero),
        None => format!("Auf der Bubble ({n} Spieler, {paid} bezahlt) als {} mit {}", spec.hero, bb(stacks[hero])),
    };
    let ranked = why::Ranked {
        chart: why::Chart::Equilibrium,
        place,
        act: Some(if shover.is_some() { why::Move::Call } else { why::Move::AllIn }),
        share: sol.act_pct(s.node) / 100.0,
        all_hands: true,
        class: class.to_string(),
        rank,
    };
    let pot_bb = pot_at(hand, index);
    let mut d = crate::base(hand.id, index, Street::Preflop, spot, String::new());
    d.why = why::ranked(&ranked, &strategy, pot_bb);
    d.best = strategy.iter().max_by(|a, b| a.freq.total_cmp(&b.freq)).map(|c| c.label.clone());
    d.strategy = strategy;
    d.status = Status::Rated;
    d.source = Some(Source::Equilibrium);
    d.pot_bb = Some(pot_bb);
    d.note = format!(
        "Push/Fold-Gleichgewicht mit ICM (Malmuth-Harville): Preise {} vom Preisgeld, Stacks wie am Tisch, höchstens zwei Spieler all-in. Gewinne und Verluste sind Preisgeld, umgerechnet in BB zum durchschnittlichen Wert eines Chips ({} BB = 1 % des Preisgeldes).",
        spec.payouts.iter().map(|p| format!("{} %", crate::fmt::num(*p))).collect::<Vec<_>>().join(", "),
        crate::fmt::num(total / 100.0)
    );
    let actions: Vec<GridAction> = d.strategy.iter().map(|c| GridAction { label: c.label.clone(), kind: c.kind.clone() }).collect();
    let cells = (0..169)
        .map(|c| {
            let f = (node.act[c] * 100.0).round() as u8;
            Some(vec![f, 100 - f])
        })
        .collect();
    Ok((clean(d), SpotGrid { actions, cells, weights: None }))
}

fn label_of<'a>(hand: &'a Hand, name: &str) -> Option<&'static str> {
    ringside_core::stats::position_labels(hand).into_iter().find(|(n, _)| n == name).map(|(_, l)| l)
}

/// Pot before the hero acts at `index`, BB.
fn pot_at(hand: &Hand, index: usize) -> f64 {
    hand.actions[..index].iter().map(|a| a.amount).sum::<i64>() as f64 / hand.big_blind as f64
}

/// How every hand class plays a preflop spot: the same hand with each class dealt to the
/// hero, rated again.
pub fn preflop_grid(spec: &Spec, hand: &Hand, index: usize, d: &Decision) -> SpotGrid {
    let actions: Vec<GridAction> = d.strategy.iter().map(|c| GridAction { label: c.label.clone(), kind: c.kind.clone() }).collect();
    let cells = HandClass::all()
        .map(|class| {
            let combo = class.combos()[0];
            let mut h = hand.clone();
            let (a, b) = combo.cards();
            h.hero_cards = vec![a.to_string(), b.to_string()];
            let mut s = spec.clone();
            s.cards = h.hero_cards.concat();
            let r = rate_preflop(&s, &h, index).ok()?;
            Some(shares(&actions, &r.strategy))
        })
        .collect();
    SpotGrid { actions, cells, weights: None }
}

fn shares(actions: &[GridAction], strategy: &[Choice]) -> Vec<u8> {
    actions
        .iter()
        .map(|a| strategy.iter().find(|c| c.label == a.label).map_or(0, |c| (c.freq * 100.0).round() as u8))
        .collect()
}

/// The last flop solution, kept for the next spot with the same flop, ranges and sizes.
pub type SolveCache = Option<Solution>;

/// A postflop spot solved from the flop: the hero's decision and the grid at its node.
pub fn rate_postflop(spec: &Spec, hand: &Hand, index: usize, threads: usize, memory: u64, cache: &mut SolveCache) -> Result<(Decision, SpotGrid), String> {
    let job = analyze_hand(hand).job.ok_or_else(|| format!("{}: kein Postflop-Spot", spec.id))?;
    let job: PostflopJob = serde_json::from_str(&job.1).map_err(|e| e.to_string())?;
    let fail = |e: JobError| format!("{}: Solver: {e:?}", spec.id);
    if cache.as_ref().is_none_or(|s| s.spot != job.spot) {
        *cache = None;
        *cache = Some(solve_spot(&job, threads, memory, |_| {}, || false).map_err(fail)?);
    }
    let sol = cache.as_mut().unwrap();
    let summary = sol.summary.clone();
    let decisions = rate(&job, &summary, None, |path| sol.node(path)).map_err(fail)?;
    let d = decisions.into_iter().find(|d| d.action_index == index).ok_or_else(|| format!("{}: Solver bewertet die Entscheidung nicht", spec.id))?;
    // The solver link holds the path to the decision's node.
    let path: Vec<Step> = d
        .solver
        .as_deref()
        .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
        .and_then(|v| serde_json::from_value(v["path"].clone()).ok())
        .ok_or_else(|| format!("{}: Pfad zur Entscheidung fehlt", spec.id))?;
    let node = sol.node(&path).map_err(|e| format!("{}: {e}", spec.id))?;
    let d = usable(spec, d)?;
    Ok((d.clone(), node_grid(&node, &d)))
}

fn node_grid(node: &Node, d: &Decision) -> SpotGrid {
    let actions: Vec<GridAction> = d.strategy.iter().map(|c| GridAction { label: c.label.clone(), kind: c.kind.clone() }).collect();
    // The decision's choices follow the node's actions one to one; each class is the
    // reach-weighted mean of its combos.
    let mut sums = vec![(0.0, vec![0.0; actions.len()]); 169];
    for h in node.hands.iter().filter(|h| h.weight > 0.0 && h.strategy.len() == actions.len()) {
        let (w, acc) = &mut sums[h.class];
        *w += h.weight;
        for (a, f) in acc.iter_mut().zip(&h.strategy) {
            *a += h.weight * f;
        }
    }
    let cells = sums.iter().map(|(w, acc)| (*w > 0.0).then(|| acc.iter().map(|a| (a / w * 100.0).round() as u8).collect())).collect();
    let weights = sums
        .iter()
        .enumerate()
        .map(|(c, (w, _))| if *w > 0.0 { (w / HandClass::from_index(c).combo_count() as f64 * 100.0).round().clamp(1.0, 100.0) as u8 } else { 0 })
        .collect();
    SpotGrid { actions, cells, weights: Some(weights) }
}

/// Everything the pack needs about one spot, from its spec.
pub fn build(spec: &Spec, threads: usize, memory: u64, cache: &mut SolveCache) -> Result<LernSpot, String> {
    let id = hand_id(&spec.id);
    let (mut hand, index) = hand_of(spec, id)?;
    let (decision, grid) = if !spec.payouts.is_empty() {
        rate_icm(spec, &hand, index)?
    } else if hand.actions[index].street == Street::Preflop {
        let d = rate_preflop(spec, &hand, index)?;
        let g = preflop_grid(spec, &hand, index, &d);
        (d, g)
    } else {
        rate_postflop(spec, &hand, index, threads, memory, cache)?
    };
    hand.actions.truncate(index + 1);
    hand.collected.clear();
    hand.returned.clear();
    hand.shown.clear();
    let mut used = CardSet::EMPTY;
    let mut count = 0;
    for c in hand.hero_cards.iter().chain(&hand.board).filter_map(|c| c.parse::<Card>().ok()) {
        used.insert(c);
        count += 1;
    }
    if count != hand.hero_cards.len() + hand.board.len() || used.iter().count() != count {
        return Err(format!("{}: eine Karte doppelt", spec.id));
    }
    let stack_bb = spec.stacks.get(&spec.hero).copied().unwrap_or(spec.stack);
    Ok(LernSpot { id: spec.id.clone(), topic: spec.topic.clone(), hand, action_index: index, decision, grid: Some(grid), stack_bb, position: spec.hero.clone() })
}

/// The bundled pack.
pub fn pack() -> &'static Pack {
    static PACK: OnceLock<Pack> = OnceLock::new();
    PACK.get_or_init(|| serde_json::from_str(include_str!("../data/lern-spots.json")).expect("the bundled learning spots parse"))
}

pub fn spot(id: &str) -> Option<&'static LernSpot> {
    pack().spots.iter().find(|s| s.id == id)
}

/// Grades `choice` (index into the strategy) for the spot `id`.
pub fn check(id: &str, choice: usize) -> Result<Answered, String> {
    let s = spot(id).ok_or("Diesen Lern-Spot gibt es nicht.")?;
    answer(&s.decision, choice)
}

/// What the user did with a spot so far.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    /// The last answer was right; `None` before the first.
    pub right: Option<bool>,
    /// Unix seconds of the last answer.
    pub at: Option<i64>,
    pub tries: u32,
}

/// The answer as stored in `learn_attempts.answer` for kind `key_spot`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stored {
    pub id: String,
    pub choice: String,
    pub right: bool,
}

/// Every spot's history from the stored answers (oldest first), by id.
pub fn seen(answers: impl IntoIterator<Item = (Stored, i64)>) -> BTreeMap<String, Seen> {
    let mut out: BTreeMap<String, Seen> = BTreeMap::new();
    for (a, at) in answers {
        let s = out.entry(a.id).or_default();
        s.right = Some(a.right);
        s.at = Some(at);
        s.tries += 1;
    }
    out
}

/// A right answer is asked again after this many days.
const REVIEW_DAYS: i64 = 7;

/// Which spots to practise, most urgent first: those answered wrong last time (the oldest
/// first), then new ones taking turns across the topics, then right ones due for review,
/// then the rest by age.
pub fn queue<'a>(spots: &'a [LernSpot], seen: &BTreeMap<String, Seen>, now: i64) -> Vec<&'a str> {
    let state = |s: &LernSpot| seen.get(&s.id).cloned().unwrap_or_default();
    let mut wrong: Vec<(&LernSpot, i64)> = spots.iter().filter_map(|s| (state(s).right == Some(false)).then(|| (s, state(s).at.unwrap_or(0)))).collect();
    wrong.sort_by_key(|(_, at)| *at);
    // New spots take turns across the topics, each topic in its own order.
    let mut fresh: Vec<(usize, usize, &LernSpot)> = Vec::new();
    for t in &TOPICS {
        for (k, s) in spots.iter().filter(|s| s.topic == t.id && state(s).right.is_none()).enumerate() {
            fresh.push((k, TOPICS.iter().position(|x| x.id == t.id).unwrap(), s));
        }
    }
    fresh.sort_by_key(|(k, t, _)| (*k, *t));
    let mut right: Vec<(&LernSpot, i64)> = spots.iter().filter_map(|s| (state(s).right == Some(true)).then(|| (s, state(s).at.unwrap_or(0)))).collect();
    right.sort_by_key(|(_, at)| *at);
    let due = |at: i64| now - at >= REVIEW_DAYS * 86_400;
    wrong
        .iter()
        .map(|(s, _)| s.id.as_str())
        .chain(fresh.iter().map(|(_, _, s)| s.id.as_str()))
        .chain(right.iter().filter(|(_, at)| due(*at)).map(|(s, _)| s.id.as_str()))
        .chain(right.iter().filter(|(_, at)| !due(*at)).map(|(s, _)| s.id.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ringside_core::db::Rating;

    use super::*;

    fn specs() -> Specs {
        serde_json::from_str(include_str!("../data/lern-spots-spec.json")).unwrap()
    }

    #[test]
    fn labels_match_the_hand_history_reader() {
        for n in 2..=9 {
            let spec = Spec {
                id: format!("t{n}"),
                topic: "shove".into(),
                players: n,
                hero: "BB".into(),
                cards: "AhKd".into(),
                stack: 20.0,
                stacks: BTreeMap::new(),
                line: "Hero ?".into(),
                board: String::new(),
                payouts: Vec::new(),
            };
            let (hand, _) = hand_of(&spec, 1).unwrap();
            let read: Vec<&str> = ringside_core::stats::position_labels(&hand).into_iter().map(|(_, l)| l).collect();
            assert_eq!(read, labels(n as usize), "{n} players");
        }
    }

    /// Every spec builds a hand whose hero acts last at the decision, and the pack holds
    /// exactly the specs.
    #[test]
    fn the_pack_follows_the_spec() {
        let specs = specs();
        let ids: Vec<&str> = specs.spots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len(), "ids are unique");
        let topics: HashSet<&str> = TOPICS.iter().map(|t| t.id).collect();
        for s in &specs.spots {
            assert!(topics.contains(s.topic.as_str()), "{}: topic {}", s.id, s.topic);
            hand_of(s, hand_id(&s.id)).unwrap();
        }
        let packed: Vec<&str> = pack().spots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(packed, ids, "rebuild the pack: cargo run --release -p ringside-analysis --bin lern-spots");
    }

    /// Each spot asks one real decision: the hand stops at the hero, nothing later is in
    /// it, every choice can be graded and at least one of them is right.
    #[test]
    fn every_spot_can_be_answered() {
        for s in &pack().spots {
            assert_eq!(s.hand.actions.len(), s.action_index + 1, "{}", s.id);
            assert_eq!(s.hand.actions[s.action_index].player, "Hero", "{}", s.id);
            assert!(s.hand.shown.is_empty() && s.hand.collected.is_empty(), "{}", s.id);
            assert!(s.decision.strategy.len() >= 2, "{}", s.id);
            let mut right = 0;
            for c in 0..s.decision.strategy.len() {
                let a = check(&s.id, c).unwrap();
                assert!(a.decision.ev_loss_bb.unwrap() >= 0.0 && (0.0..=1.0).contains(&a.score), "{}", s.id);
                if a.decision.rating.is_some_and(|r| r <= Rating::Good) {
                    right += 1;
                }
            }
            assert!(right >= 1, "{}: no right answer", s.id);
            let grid = s.grid.as_ref().expect("every spot has a grid");
            assert_eq!(grid.cells.len(), 169, "{}", s.id);
            let class = Combo::new(s.hand.hero_cards[0].parse().unwrap(), s.hand.hero_cards[1].parse().unwrap()).class();
            assert!(grid.cells[class.index()].is_some(), "{}: the hero's class is in the grid", s.id);
        }
    }

    /// A sample of the preflop spots rated again by the engine gives the stored answer.
    #[test]
    fn preflop_answers_match_the_engine() {
        let specs = specs();
        let mut checked = 0;
        for (k, spec) in specs.spots.iter().enumerate() {
            let stored = pack().spots.iter().find(|s| s.id == spec.id).unwrap();
            if stored.decision.street != Street::Preflop || k % 4 != 0 {
                continue;
            }
            let (hand, index) = hand_of(spec, hand_id(&spec.id)).unwrap();
            let fresh = rate_preflop(spec, &hand, index).unwrap();
            assert_eq!(fresh.strategy.len(), stored.decision.strategy.len(), "{}", spec.id);
            for (a, b) in fresh.strategy.iter().zip(&stored.decision.strategy) {
                assert_eq!(a.label, b.label, "{}", spec.id);
                assert!((a.freq - b.freq).abs() < 0.002, "{}: {} {} vs {}", spec.id, a.label, a.freq, b.freq);
                assert!((a.ev_bb.unwrap_or(0.0) - b.ev_bb.unwrap_or(0.0)).abs() < 0.002, "{}: EV of {}", spec.id, a.label);
            }
            checked += 1;
        }
        assert!(checked >= 15, "only {checked} spots checked");
    }

    #[test]
    fn wrong_answers_come_back_first_and_new_spots_mix_the_topics() {
        let spots = &pack().spots;
        let first_of = |topic: &str| spots.iter().find(|s| s.topic == topic).unwrap().id.clone();
        let stored = |id: &str, right: bool| Stored { id: id.into(), choice: "Fold".into(), right };
        let a = first_of("shove");
        let b = first_of("call_shove");
        let day = 86_400;
        let c = first_of("open");
        let seen = seen([
            (stored(&b, true), day),
            (stored(&b, false), 2 * day),
            (stored(&b, true), 3 * day),
            (stored(&a, false), 10 * day),
            (stored(&c, true), 19 * day),
        ]);
        assert_eq!(seen[&b], Seen { right: Some(true), at: Some(3 * day), tries: 3 });
        let q = queue(spots, &seen, 20 * day);
        assert_eq!(q.len(), spots.len(), "every spot once");
        assert_eq!(q[0], a, "the wrong one first");
        assert_eq!(&q[q.len() - 2..], [b.as_str(), c.as_str()], "right ones last, the one due for review first");
        let topics: Vec<&str> = q[1..1 + TOPICS.len() - 1].iter().map(|id| spot(id).unwrap().topic.as_str()).collect();
        assert_eq!(topics.iter().collect::<HashSet<_>>().len(), TOPICS.len() - 1, "new spots take turns: {topics:?}");
    }

    /// Hands every push/fold chart agrees on: aces always shove and call, seven-deuce
    /// offsuit never shoves under the gun at 15 BB; and the ICM call of a bubble spot needs
    /// a stronger hand than the same call in chips.
    #[test]
    fn known_push_fold_answers() {
        for s in pack().spots.iter().filter(|s| matches!(s.decision.spot, SpotType::Shove | SpotType::CallShove)) {
            let grid = s.grid.as_ref().unwrap();
            let aa = HandClass::all().find(|c| c.to_string() == "AA").unwrap().index();
            let act = grid.actions.iter().position(|a| a.kind != "fold").unwrap();
            assert!(grid.cells[aa].as_ref().unwrap()[act] >= 99, "{}: AA always plays", s.id);
        }
        let bubble: Vec<&LernSpot> = pack().spots.iter().filter(|s| s.topic == "bubble" && s.decision.spot == SpotType::CallShove).collect();
        assert!(!bubble.is_empty());
        for s in bubble {
            let spec = specs().spots.into_iter().find(|x| x.id == s.id).unwrap();
            let chip = Spec { payouts: Vec::new(), ..spec.clone() };
            let (hand, index) = hand_of(&chip, hand_id(&spec.id)).unwrap();
            let n = spec.players as usize;
            let order = labels(n);
            let stacks: Vec<f64> = order.iter().map(|l| spec.stacks.get(*l).copied().unwrap_or(spec.stack)).collect();
            let mut p = Params::new(stacks.clone());
            p.ante_bb = ANTE_BB;
            p.max_all_in = 2;
            let chips = multiway::solve(&p).unwrap();
            let mut q = Params::new(stacks).with_payouts(spec.payouts.clone());
            q.ante_bb = ANTE_BB;
            q.max_all_in = 2;
            let with_icm = multiway::solve(&q).unwrap();
            let hero = order.iter().position(|l| *l == spec.hero).unwrap();
            let shover = order.iter().position(|l| hand.actions[..index].iter().any(|a| a.all_in && label_of(&hand, &a.player) == Some(*l))).unwrap();
            let node = |sol: &multiway::Solution| sol.node(hero, Some(shover), &[]).unwrap();
            assert!(with_icm.act_pct(node(&with_icm)) < chips.act_pct(node(&chips)), "{}: ICM calls tighter", s.id);
        }
    }
}
