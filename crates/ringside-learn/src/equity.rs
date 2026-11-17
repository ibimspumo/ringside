//! Equity estimates: hand against hand, hand against a named range, and all-ins from
//! the user's own hands.

use std::sync::LazyLock;

use rand::Rng;
use ringside_engine::{Card, CardSet, Combo, HandClass, Method, PreflopMatrix, Range, equity};

use crate::deal::{self, Rng8};
use crate::fmt::{self, pct};
use crate::score::band;
use crate::{Answer, AnswerSpec, Exercise, Fact, Feedback, Grading, PotView, Prompt, Task, feedback, number};

/// Estimates within 3 percentage points score fully, nothing from 15 on.
const FULL: f64 = 3.0;
const ZERO: f64 = 15.0;

fn estimate() -> AnswerSpec {
    AnswerSpec::Number { unit: "%".into(), min: 0.0, max: 100.0, step: 0.1 }
}

fn combo(cards: &[Card]) -> Combo {
    Combo::new(cards[0], cards[1])
}

/// Exact all-in equity of each player in percent.
pub fn exact_equity(players: &[Vec<Card>], board: &[Card], dead: CardSet) -> Result<Vec<f64>, String> {
    let ranges: Vec<Range> = players.iter().map(|p| Range::single(combo(p))).collect();
    let r = equity(&ranges, CardSet::from_cards(board), dead, Method::Exact).map_err(|e| e.to_string())?;
    Ok(r.players.iter().map(|p| p.equity_pct).collect())
}

/// Two hands of a classic preflop matchup.
fn classic(rng: &mut Rng8) -> (Vec<Card>, Vec<Card>) {
    loop {
        let mut used = CardSet::EMPTY;
        let a = deal::cards(rng, &mut used, 2);
        let b = deal::cards(rng, &mut used, 2);
        let (ra, rb) = ((a[0].rank(), a[1].rank()), (b[0].rank(), b[1].rank()));
        let pair = |r: (u8, u8)| r.0 == r.1;
        let lo = |r: (u8, u8)| r.0.min(r.1);
        let hi = |r: (u8, u8)| r.0.max(r.1);
        let pattern = match rng.random_range(0..4) {
            // Pair against two overcards.
            0 => pair(ra) && !pair(rb) && lo(rb) > ra.0,
            // Higher pair against lower pair.
            1 => pair(ra) && pair(rb),
            // Dominated: same top card, worse kicker.
            2 => !pair(ra) && !pair(rb) && hi(ra) == hi(rb) && lo(ra) != lo(rb),
            // Pair against two undercards.
            _ => pair(ra) && !pair(rb) && hi(rb) < ra.0,
        };
        if pattern {
            return (a, b);
        }
    }
}

pub fn hand(level: u8, rng: &mut Rng8) -> Task {
    let (hero, villain, board) = match level {
        1 => {
            let (a, b) = classic(rng);
            if rng.random_bool(0.5) { (a, b, Vec::new()) } else { (b, a, Vec::new()) }
        }
        2 => {
            let mut used = CardSet::EMPTY;
            (deal::cards(rng, &mut used, 2), deal::cards(rng, &mut used, 2), Vec::new())
        }
        _ => {
            let mut used = CardSet::EMPTY;
            (deal::cards(rng, &mut used, 2), deal::cards(rng, &mut used, 2), deal::cards(rng, &mut used, 3))
        }
    };
    let when = if board.is_empty() { "vor dem Flop" } else { "auf diesem Flop" };
    Task {
        question: format!("Beide sind {when} all-in. Wie viel Equity hat deine Hand?"),
        prompt: Prompt::EquityHand { hero, villain, board },
        answer: estimate(),
        grading: Grading::Exakt,
        model_note: None,
    }
}

pub fn grade_hand(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::EquityHand { hero, villain, board } = &ex.prompt else { unreachable!() };
    let x = number(answer)?;
    let ranges = [Range::single(combo(hero)), Range::single(combo(villain))];
    let r = equity(&ranges, CardSet::from_cards(board), CardSet::EMPTY, Method::Exact).map_err(|e| e.to_string())?;
    let me = r.players[0];
    let facts = vec![
        Fact::new("Equity", pct(me.equity_pct)).with(format!("{} + {} ÷ 2", pct(me.win_pct), pct(me.tie_pct))),
        Fact::new("Gewinnt", pct(me.win_pct)),
        Fact::new("Split", pct(me.tie_pct)),
        Fact::new("Verliert", pct(100.0 - me.win_pct - me.tie_pct)),
    ];
    let err = x - me.equity_pct;
    Ok(feedback(err.abs() <= FULL, band(err, FULL, ZERO), Answer::Number(me.equity_pct), pct(me.equity_pct), facts))
}

/// (name, notation, share of combos in percent).
static NAMED_RANGES: LazyLock<Vec<(String, String, f64)>> = LazyLock::new(|| {
    let mut v: Vec<(String, String)> = vec![
        ("Premium".into(), "QQ+, AKs, AKo".into()),
        ("Alle Paare".into(), "22+".into()),
        ("Broadways".into(), "TT+, ATs+, KTs+, QTs+, JTs, ATo+, KTo+, QTo+, JTo".into()),
    ];
    for share in [5.0, 10.0, 15.0, 25.0, 40.0] {
        v.push((format!("Stärkste {share:.0} %"), top_range(share).to_string()));
    }
    v.into_iter()
        .map(|(name, notation)| {
            let r = Range::parse(&notation).expect("valid range");
            let share = 100.0 * r.combo_count() as f64 / 1326.0;
            (name, notation, share)
        })
        .collect()
});

/// The strongest classes by equity against a random hand until `share` percent of all
/// combos are in.
fn top_range(share: f64) -> Range {
    let m = PreflopMatrix::embedded();
    let mut classes: Vec<HandClass> = HandClass::all().collect();
    classes.sort_by(|a, b| m.equity_vs_random(*b).total_cmp(&m.equity_vs_random(*a)));
    let mut r = Range::empty();
    let mut combos = 0.0;
    for c in classes {
        if combos >= share / 100.0 * 1326.0 {
            break;
        }
        r.set_class(c, 1.0);
        combos += c.combo_count() as f64;
    }
    r
}

pub fn range(level: u8, rng: &mut Rng8) -> Task {
    let named = &*NAMED_RANGES;
    let (name, notation, share) = match level {
        1 => &named[*deal::pick(rng, &[0usize, 4])],
        _ => deal::pick(rng, named),
    };
    let mut used = CardSet::EMPTY;
    let hero = deal::cards(rng, &mut used, 2);
    let board = if level == 3 { deal::cards(rng, &mut used, 3) } else { Vec::new() };
    let when = if board.is_empty() { "vor dem Flop" } else { "auf diesem Flop" };
    Task {
        question: format!(
            "Dein Gegner geht {when} mit der Range „{name}“ ({notation}) all-in. Wie viel Equity hat deine Hand?"
        ),
        prompt: Prompt::EquityRange {
            hero,
            range_name: name.clone(),
            range: notation.clone(),
            range_pct: *share,
            board,
        },
        answer: estimate(),
        grading: Grading::Modell,
        model_note: Some(
            "Annahme: Der Gegner hält jede Hand der Range mit gleicher Wahrscheinlichkeit; Karten, die du siehst, sind herausgerechnet."
                .into(),
        ),
    }
}

/// Preflop equity of one combo against a range whose classes are fully in or out. By
/// suit symmetry every combo of a class meets the compatible combos of another class
/// with the class-average equity, so the matrix gives the exact answer.
pub fn combo_vs_range_preflop(hero: Combo, range: &Range) -> f64 {
    let m = PreflopMatrix::embedded();
    let mut counts = [0u32; 169];
    for (c, w) in range.live_combos(hero.mask()) {
        debug_assert!(w == 1.0, "class-level ranges only");
        counts[c.class().index()] += 1;
    }
    let a = hero.class();
    let (mut sum, mut n) = (0.0, 0.0);
    for b in HandClass::all() {
        let k = counts[b.index()] as f64;
        sum += k * m.equity(a, b);
        n += k;
    }
    100.0 * sum / n
}

pub fn grade_range(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::EquityRange { hero, range, board, .. } = &ex.prompt else { unreachable!() };
    let x = number(answer)?;
    let villain = Range::parse(range).map_err(|e| e.to_string())?;
    let me = combo(hero);
    let dead = CardSet::from_cards(board).union(me.mask());
    let live = villain.live_combos(dead).len();
    let eq = if board.is_empty() {
        combo_vs_range_preflop(me, &villain)
    } else {
        let r = equity(&[Range::single(me), villain], CardSet::from_cards(board), CardSet::EMPTY, Method::Exact)
            .map_err(|e| e.to_string())?;
        r.players[0].equity_pct
    };
    let mut facts = vec![
        Fact::new("Equity", pct(eq)),
        Fact::new("Mögliche Kombos des Gegners", live.to_string()),
    ];
    if board.is_empty() {
        let random = 100.0 * PreflopMatrix::embedded().equity_vs_random(me.class());
        facts.push(Fact::new("Zum Vergleich: gegen eine Zufallshand", pct(random)));
    }
    let err = x - eq;
    Ok(feedback(err.abs() <= FULL, band(err, FULL, ZERO), Answer::Number(eq), pct(eq), facts))
}

/// Hero's expected share of all pots, in percent, and per pot.
pub fn all_in_shares(hero: &[Card], opponents: &[Vec<Card>], board: &[Card], pots: &[PotView]) -> Result<(f64, Vec<f64>), String> {
    let players: Vec<Vec<Card>> = std::iter::once(hero.to_vec()).chain(opponents.iter().cloned()).collect();
    let total: f64 = pots.iter().map(|p| p.amount_bb).sum();
    let mut won = 0.0;
    let mut per_pot = Vec::new();
    for pot in pots {
        let share = if !pot.players.contains(&0) {
            0.0
        } else if pot.players.len() == 1 {
            100.0
        } else {
            let in_pot: Vec<Vec<Card>> = pot.players.iter().map(|&i| players[i].clone()).collect();
            let dead: Vec<Card> = (0..players.len())
                .filter(|i| !pot.players.contains(i))
                .flat_map(|i| players[i].clone())
                .collect();
            let eq = exact_equity(&in_pot, board, CardSet::from_cards(&dead))?;
            eq[pot.players.iter().position(|&i| i == 0).expect("hero in pot")]
        };
        won += share / 100.0 * pot.amount_bb;
        per_pot.push(share);
    }
    Ok((100.0 * won / total, per_pot))
}

pub fn grade_all_in(ex: &Exercise, answer: &Answer) -> Result<Feedback, String> {
    let Prompt::AllIn { hero, opponents, board, pots } = &ex.prompt else { unreachable!() };
    let x = number(answer)?;
    let (share, per_pot) = all_in_shares(hero, opponents, board, pots)?;
    let mut facts = vec![Fact::new("Equity", pct(share))];
    if pots.len() > 1 {
        for (i, (pot, eq)) in pots.iter().zip(&per_pot).enumerate() {
            let label = if i == 0 { "Hauptpot".to_string() } else { format!("Nebenpot {i}") };
            facts.push(Fact::new(&label, format!("{} von {}", pct(*eq), fmt::bb(pot.amount_bb))));
        }
    }
    let err = x - share;
    Ok(feedback(err.abs() <= FULL, band(err, FULL, ZERO), Answer::Number(share), pct(share), facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_ranges_parse_and_grow() {
        let shares: Vec<f64> = NAMED_RANGES.iter().skip(3).map(|r| r.2).collect();
        assert!(shares.windows(2).all(|w| w[0] < w[1]), "{shares:?}");
        assert!((5.0..7.0).contains(&shares[0]), "{shares:?}");
    }

    /// The matrix shortcut agrees with an independent Monte Carlo run.
    #[test]
    fn preflop_range_equity_matches_monte_carlo() {
        let hero = Combo::new("Ah".parse().unwrap(), "Kd".parse().unwrap());
        let villain = Range::parse("TT+, AQs+, AQo+").unwrap();
        let fast = combo_vs_range_preflop(hero, &villain);
        let r = equity(
            &[Range::single(hero), villain],
            CardSet::EMPTY,
            CardSet::EMPTY,
            Method::MonteCarlo { samples: 300_000, seed: 5 },
        )
        .unwrap();
        assert!((fast - r.players[0].equity_pct).abs() < 0.4, "{fast} vs {}", r.players[0].equity_pct);
    }
}
