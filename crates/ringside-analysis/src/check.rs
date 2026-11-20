//! Hard bounds every rated decision has to keep, whatever its source: what an action can
//! win or lose follows from the chips in the hand alone. The `analysis-check` tool runs
//! them over hand histories; the tests run them over random stacks.
//!
//! All amounts in BB, against the real hand:
//! - **loss**: never negative, never more than the gap from the best action to the one played.
//! - **call**: against folding, a call loses at most what it puts in (the hero can give up
//!   on every later street) and wins at most the pot plus what can still be won later;
//!   when nobody can bet after the call, at most the pot.
//! - **spread**: no two actions differ by more than the pot plus what both can still put in.
//! - **size**: no action puts in more than the hero has.
//! - **pot**: the stored pot is the pot of the hand before the decision.
//! - **depth**: the preflop model's depth is the nearest table depth of the stack really in
//!   play: the hero's, against the deepest opponent still in the hand.

use ringside_core::db::{Decision, Source, Status};
use ringside_core::model::{ActionKind, Hand, Street};
use ringside_engine::preflop_table::STACKS;

use crate::replay::spot_at;
use crate::seats::nearest;

#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
    pub rule: &'static str,
    /// How far past the bound, BB (for the depth: model depth over the real one).
    pub by: f64,
    pub detail: String,
}

/// Slack for the models' rounding: table depths, bet sizes, the EV grid.
fn slack(x: f64) -> f64 {
    0.25 + 0.05 * x.abs()
}

/// The chips really in play at the hero's action `index`: pot before it, what the hero
/// has in on this street and behind, what the hero still has to call, and what the
/// hero can still put in that an opponent can match.
#[derive(Clone, Copy, Debug)]
pub struct Chips {
    pub pot: f64,
    pub to_call: f64,
    pub behind: f64,
    pub hero_in: f64,
    pub riskable: f64,
    /// Nobody can bet any more once the hero calls.
    pub closed_by_call: bool,
    /// Stack at the start of the hand: the hero's, against the deepest opponent still in.
    pub depth: f64,
}

pub fn chips(hand: &Hand, hero: &str, index: usize) -> Option<Chips> {
    let spot = spot_at(hand, hero, index, None, &[]).ok()?;
    let to_call = spot.to_call();
    let reach = spot.opponents.iter().map(|o| o.behind + o.street_in).fold(0.0, f64::max);
    let riskable = (reach - spot.hero_in).min(spot.hero_behind).max(0.0);
    let after = spot.hero_behind - to_call;
    // The opponents who could still bet after the call: chips behind and not the one all in.
    let closed_by_call = after <= 1e-9 || spot.opponents.iter().all(|o| o.behind <= 1e-9);
    let big = hand.big_blind.max(1) as f64;
    let start = |name: &str| hand.seats.iter().find(|s| s.name == name).map_or(0.0, |s| s.stack as f64 / big);
    let folded = |name: &str| hand.actions[..index].iter().any(|a| a.player == name && a.kind == ActionKind::Fold);
    let deepest = hand
        .seats
        .iter()
        .filter(|s| s.name != hero && !s.sitting_out && !folded(&s.name))
        .map(|s| start(&s.name))
        .fold(0.0, f64::max);
    Some(Chips {
        pot: spot.pot,
        to_call,
        behind: spot.hero_behind,
        hero_in: spot.hero_in,
        riskable,
        closed_by_call,
        depth: start(hero).min(deepest),
    })
}

/// "Raise auf 4,5 BB" → 4.5.
fn label_to(label: &str) -> Option<f64> {
    let rest = label.strip_prefix("Raise auf ").or_else(|| label.strip_prefix("Bet "))?;
    rest.trim_end_matches(" BB").replace('.', "").replace(',', ".").parse().ok()
}

/// "… alle mit 40 BB …" → 40.
fn model_depth(note: &str) -> Option<f64> {
    let rest = &note[note.find("alle mit ")? + "alle mit ".len()..];
    let n = rest.split(" BB").next()?;
    n.replace('.', "").replace(',', ".").parse().ok()
}

pub fn check(hand: &Hand, d: &Decision) -> Vec<Violation> {
    let mut out = Vec::new();
    if d.status != Status::Rated {
        return out;
    }
    let Some(hero) = hand.hero.as_deref() else { return out };
    let Some(c) = chips(hand, hero, d.action_index) else { return out };
    let mut v = |rule: &'static str, by: f64, detail: String| out.push(Violation { rule, by, detail });
    let ev = |kind: &str| d.strategy.iter().filter(|x| x.kind == kind).filter_map(|x| x.ev_bb).fold(None, |m: Option<f64>, e| Some(m.map_or(e, |m| m.max(e))));
    let evs: Vec<f64> = d.strategy.iter().filter_map(|x| x.ev_bb).collect();
    let best = evs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let played = d.strategy.iter().find(|x| x.played).and_then(|x| x.ev_bb);

    if let Some(loss) = d.ev_loss_bb {
        if loss < -1e-9 {
            v("loss", -loss, format!("Verlust {loss:.2}"));
        }
        if let Some(p) = played.filter(|_| best.is_finite()) {
            let gap = best - p;
            if loss > gap + slack(gap) {
                v("loss", loss - gap, format!("Verlust {loss:.2} über dem Abstand {gap:.2} zur besten Aktion"));
            }
        }
    }

    if let (Some(call), Some(fold)) = (ev("call"), ev("fold")) {
        let diff = call - fold;
        let low = -c.to_call;
        if diff < low - slack(c.to_call) {
            v("call", low - diff, format!("Call − Fold {diff:.2}, der Call kostet höchstens {:.2}", c.to_call));
        }
        let high = if c.closed_by_call { c.pot } else { c.pot + c.riskable };
        if diff > high + slack(high) {
            v("call", diff - high, format!("Call − Fold {diff:.2}, zu gewinnen höchstens {high:.2}"));
        }
    }

    if evs.len() >= 2 {
        let low = evs.iter().copied().fold(f64::INFINITY, f64::min);
        let spread = best - low;
        let bound = c.pot + 2.0 * c.riskable + c.to_call;
        if spread > bound + slack(bound) {
            v("spread", spread - bound, format!("Aktionen {spread:.2} auseinander, möglich höchstens {bound:.2}"));
        }
    }

    for x in &d.strategy {
        if let Some(to) = label_to(&x.label) {
            let most = c.hero_in + c.behind;
            if to > most + slack(most) {
                v("size", to - most, format!("{} bei nur {most:.2} im Stack", x.label));
            }
        }
    }

    if let Some(pot) = d.pot_bb.filter(|pot| (pot - c.pot).abs() > 0.05 + 0.01 * c.pot) {
        v("pot", (pot - c.pot).abs(), format!("Pot {pot:.2} gespeichert, in der Hand {:.2}", c.pot));
    }

    let preflop_model = d.source == Some(Source::Model) && hand.actions[d.action_index].street == Street::Preflop;
    if let Some(m) = model_depth(&d.note).filter(|_| preflop_model) {
        let want = nearest(&STACKS, c.depth);
        // Below the shallowest table every depth rounds up to it.
        if m > want + 1e-9 && c.depth >= STACKS[0] * 2.0 / 3.0 {
            v("depth", m - c.depth, format!("Modell mit {m} BB, im Spiel {:.1} BB (Tabelle {want})", c.depth));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_labels_and_notes() {
        assert_eq!(label_to("Raise auf 4,5 BB"), Some(4.5));
        assert_eq!(label_to("Raise auf 1.234,5 BB"), Some(1234.5));
        assert_eq!(label_to("Fold"), None);
        assert_eq!(model_depth("Preflop-Modell (kein Gleichgewicht des echten Spiels): 8 Spieler, alle mit 40 BB, Ante"), Some(40.0));
    }
}
