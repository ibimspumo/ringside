//! The hero's preflop decisions as plain facts for the range charts: hand class, seat,
//! table size, stacks, what happened before and what the hero did. Derived from every
//! stored hand on import, independent of the hand analysis.

use serde::{Deserialize, Serialize};

use ringside_engine::{Card, Combo};

use crate::model::{Action, ActionKind, Hand, Street};
use crate::stats::position_labels;

/// What the players before the hero did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum History {
    /// Everyone before folded.
    Unopened,
    /// One or more limps, no raise.
    Limped,
    /// One raise, nobody called it yet.
    VsOpen,
    /// One raise and at least one caller: room for a squeeze.
    Squeeze,
    /// A re-raise or more, the last one not all-in.
    VsThreeBet,
    /// The last raise was all-in.
    VsShove,
}

impl History {
    pub const ALL: [History; 6] =
        [History::Unopened, History::Limped, History::VsOpen, History::Squeeze, History::VsThreeBet, History::VsShove];

    pub fn key(self) -> &'static str {
        match self {
            History::Unopened => "unopened",
            History::Limped => "limped",
            History::VsOpen => "vs_open",
            History::Squeeze => "squeeze",
            History::VsThreeBet => "vs_three_bet",
            History::VsShove => "vs_shove",
        }
    }

    pub fn parse(s: &str) -> Option<History> {
        History::ALL.into_iter().find(|h| h.key() == s)
    }
}

/// What the hero did, coarse enough to compare with any model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Move {
    Fold,
    /// Call, limp or check.
    Call,
    Raise,
    AllIn,
}

impl Move {
    pub const ALL: [Move; 4] = [Move::Fold, Move::Call, Move::Raise, Move::AllIn];

    pub fn key(self) -> &'static str {
        match self {
            Move::Fold => "fold",
            Move::Call => "call",
            Move::Raise => "raise",
            Move::AllIn => "all_in",
        }
    }

    pub fn parse(s: &str) -> Option<Move> {
        Move::ALL.into_iter().find(|m| m.key() == s)
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Ante structure as posted: none, every player, or the big blind alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnteKind {
    None,
    Classic,
    BigBlind,
}

impl AnteKind {
    pub fn key(self) -> &'static str {
        match self {
            AnteKind::None => "none",
            AnteKind::Classic => "classic",
            AnteKind::BigBlind => "big_blind",
        }
    }

    pub fn parse(s: &str) -> AnteKind {
        match s {
            "classic" => AnteKind::Classic,
            "big_blind" => AnteKind::BigBlind,
            _ => AnteKind::None,
        }
    }
}

/// One preflop decision of the hero.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PreflopSpot {
    /// Index into `Hand.actions`.
    pub action_index: usize,
    /// `HandClass::index`: row * 13 + col, ace first, suited above the diagonal.
    pub class: u8,
    /// "UTG" … "BB", as in the HUD.
    pub position: &'static str,
    /// Seat in preflop order: 0 acts first, the big blind last.
    pub seat: u8,
    /// Players dealt in.
    pub players: u8,
    pub ante: AnteKind,
    /// The hero's stack against the deepest other stack dealt in, in BB.
    pub stack_bb: f64,
    pub history: History,
    /// Position of the last raiser before the hero (the first limper in a limped pot).
    pub vs_position: Option<&'static str>,
    /// The hero's stack against that player's, in BB.
    pub vs_bb: Option<f64>,
    /// Shortest stack among the players still to act after the hero, in BB.
    pub behind_bb: Option<f64>,
    /// Non-fold actions before, e.g. "CO:r BTN:c" (l limp, c call, r raise, a all-in).
    pub line: String,
    pub action: Move,
}

/// Whether a raise puts in (nearly) the whole stack.
fn is_shove(a: &Action, stack: i64) -> bool {
    a.all_in || (a.to.unwrap_or(a.amount) as f64) >= 0.9 * stack as f64
}

fn class_of(cards: &[String]) -> Option<u8> {
    let cards: Vec<Card> = cards.iter().map(|c| c.parse().ok()).collect::<Option<_>>()?;
    let [a, b] = cards[..] else { return None };
    (a != b).then(|| Combo::new(a, b).class().index() as u8)
}

fn ante_of(hand: &Hand, big_blind_player: Option<&str>) -> AnteKind {
    let posters: Vec<&str> = hand
        .actions
        .iter()
        .filter(|a| a.street == Street::Preflop && a.kind == ActionKind::PostAnte)
        .map(|a| a.player.as_str())
        .collect();
    match posters.as_slice() {
        [] => AnteKind::None,
        [one] if Some(*one) == big_blind_player => AnteKind::BigBlind,
        _ => AnteKind::Classic,
    }
}

/// Every preflop decision of the hero in `hand`, in order. Empty without a hero, hole
/// cards or a big blind.
pub fn hero_preflop(hand: &Hand) -> Vec<PreflopSpot> {
    let Some(hero) = hand.hero.as_deref() else { return Vec::new() };
    let Some(class) = class_of(&hand.hero_cards) else { return Vec::new() };
    if hand.big_blind <= 0 {
        return Vec::new();
    }
    // In preflop order, the big blind last.
    let order = position_labels(hand);
    let n = order.len();
    let Some(hero_seat) = order.iter().position(|(name, _)| name == hero) else { return Vec::new() };
    let big = hand.big_blind as f64;
    let stack = |name: &str| hand.seats.iter().find(|s| s.name == name).map_or(0, |s| s.stack);
    let label = |name: &str| order.iter().find(|(p, _)| p == name).map(|(_, l)| *l);
    let seat_of = |name: &str| order.iter().position(|(p, _)| p == name);
    let own = stack(hero);
    let deepest = order.iter().filter(|(p, _)| p != hero).map(|(p, _)| stack(p)).max().unwrap_or(0);
    let stack_bb = own.min(deepest) as f64 / big;
    let big_blind_player = hand
        .actions
        .iter()
        .find(|a| a.street == Street::Preflop && a.kind == ActionKind::PostBigBlind)
        .map(|a| a.player.as_str());
    let ante = ante_of(hand, big_blind_player);

    let mut folded = vec![false; n];
    let mut steps: Vec<(&'static str, char)> = Vec::new();
    let mut raises = 0;
    let mut last_shove = false;
    let mut callers_since_raise = 0;
    let mut hero_in = false;
    let mut aggressor: Option<&str> = None;
    let mut first_limper: Option<&str> = None;
    let mut out = Vec::new();

    for (index, a) in hand.actions.iter().enumerate().filter(|(_, a)| a.street == Street::Preflop && !a.kind.is_post()) {
        let Some(seat) = seat_of(&a.player) else { continue };
        if a.player == hero {
            let history = match raises {
                0 if steps.iter().any(|s| s.1 == 'l') => History::Limped,
                0 => History::Unopened,
                _ if last_shove => History::VsShove,
                1 if callers_since_raise > 0 && !hero_in => History::Squeeze,
                1 => History::VsOpen,
                _ => History::VsThreeBet,
            };
            let action = match a.kind {
                ActionKind::Fold => Move::Fold,
                ActionKind::Raise | ActionKind::Bet if is_shove(a, own) => Move::AllIn,
                ActionKind::Raise | ActionKind::Bet => Move::Raise,
                _ => Move::Call,
            };
            let opponent = aggressor.or(first_limper);
            let behind = order
                .iter()
                .enumerate()
                .filter(|&(i, (p, _))| i > hero_seat && !folded[i] && p != hero)
                .map(|(_, (p, _))| stack(p))
                .min();
            out.push(PreflopSpot {
                action_index: index,
                class,
                position: order[hero_seat].1,
                seat: hero_seat as u8,
                players: n as u8,
                ante,
                stack_bb,
                history,
                vs_position: opponent.and_then(label),
                vs_bb: opponent.map(|p| own.min(stack(p)) as f64 / big),
                behind_bb: behind.map(|s| s as f64 / big),
                line: steps.iter().map(|(l, k)| format!("{l}:{k}")).collect::<Vec<_>>().join(" "),
                action,
            });
        }
        let name = label(&a.player).unwrap_or("?");
        match a.kind {
            ActionKind::Fold => folded[seat] = true,
            ActionKind::Call if raises == 0 => {
                steps.push((name, 'l'));
                first_limper.get_or_insert(a.player.as_str());
            }
            ActionKind::Call => {
                steps.push((name, 'c'));
                callers_since_raise += 1;
            }
            ActionKind::Raise | ActionKind::Bet => {
                let shove = is_shove(a, stack(&a.player));
                steps.push((name, if shove { 'a' } else { 'r' }));
                raises += 1;
                last_shove = shove;
                callers_since_raise = 0;
                aggressor = Some(a.player.as_str());
            }
            _ => {}
        }
        if a.player == hero && a.kind != ActionKind::Fold && a.kind != ActionKind::Check {
            hero_in = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_hand;

    fn hand(seats: &str, actions: &str, cards: &str) -> Hand {
        let raw = format!(
            "PokerStars Hand #1: Hold'em No Limit ($0.05/$0.10 USD) - 2024/01/15 20:30:00 CET [2024/01/15 14:30:00 ET]
Table 'T' 6-max Seat #4 is the button
{seats}
Hero: posts small blind $0.05
F: posts big blind $0.10
*** HOLE CARDS ***
Dealt to Hero [{cards}]
{actions}
*** SUMMARY ***
Total pot $0.15 | Rake $0"
        );
        parse_hand(&raw).expect("test hand parses")
    }

    const SEATS: &str = "Seat 1: A ($10 in chips)
Seat 2: B ($10 in chips)
Seat 3: C ($3 in chips)
Seat 4: D ($10 in chips)
Seat 5: Hero ($8 in chips)
Seat 6: F ($12 in chips)";

    #[test]
    fn open_from_the_small_blind_with_a_short_stack_behind() {
        let h = hand(SEATS, "A: folds\nB: folds\nC: folds\nD: folds\nHero: raises $0.20 to $0.30\nF: folds", "Ah Kd");
        let spots = hero_preflop(&h);
        assert_eq!(spots.len(), 1);
        let s = &spots[0];
        assert_eq!((s.position, s.seat, s.players), ("SB", 4, 6));
        assert_eq!((s.history, s.action, s.line.as_str()), (History::Unopened, Move::Raise, ""));
        assert_eq!(s.stack_bb, 80.0);
        assert_eq!(s.behind_bb, Some(120.0));
        assert_eq!(s.class as usize, Combo::new("Ah".parse().unwrap(), "Kd".parse().unwrap()).class().index());
    }

    #[test]
    fn squeeze_then_facing_a_shove() {
        let h = hand(
            SEATS,
            "A: raises $0.20 to $0.30\nB: calls $0.30\nC: folds\nD: folds\nHero: raises $1 to $1.30\nF: raises $10.70 to $12 and is all-in\nA: folds\nB: folds\nHero: calls $6.70 and is all-in",
            "Qs Qc",
        );
        let spots = hero_preflop(&h);
        assert_eq!(spots.len(), 2);
        assert_eq!((spots[0].history, spots[0].action), (History::Squeeze, Move::Raise));
        assert_eq!((spots[0].vs_position, spots[0].line.as_str()), (Some("LJ"), "LJ:r HJ:c"));
        assert_eq!((spots[1].history, spots[1].action, spots[1].vs_position), (History::VsShove, Move::Call, Some("BB")));
        assert_eq!(spots[1].vs_bb, Some(80.0));
    }

    #[test]
    fn limps_and_open_shoves() {
        let h = hand(SEATS, "A: calls $0.10\nB: folds\nC: folds\nD: folds\nHero: raises $7.90 to $8 and is all-in\nF: folds\nA: folds", "7h 2c");
        let s = &hero_preflop(&h)[0];
        assert_eq!((s.history, s.action, s.vs_position), (History::Limped, Move::AllIn, Some("LJ")));
    }
}
