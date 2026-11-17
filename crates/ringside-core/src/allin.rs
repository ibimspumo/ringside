//! The hero's all-in expectation in a finished hand: when someone still in the hand is
//! all-in before the river and every remaining hand is known, the result the cards
//! would give on average. Bounties are left out (chips only).

use ringside_engine::Card;
use ringside_engine::allin::{SidePot, all_in_ev};

use crate::model::{ActionKind, Hand, Street};

#[derive(Clone, Debug, PartialEq)]
pub struct HeroAllIn {
    pub hero_cards: Vec<Card>,
    /// Hands of the other players still in, in seat order.
    pub opponents: Vec<Vec<Card>>,
    /// Board when the money went in.
    pub board: Vec<Card>,
    /// Pots with players indexed hero first (0), then `opponents` (1..).
    pub pots: Vec<SidePot>,
    /// Hero's share of all pots, in percent.
    pub share_pct: f64,
    pub pot: i64,
    pub rake: i64,
    pub invested: i64,
    pub net: i64,
    /// Expected result in chips: share × (pot − rake) − invested.
    pub ev: f64,
    /// Index into `Hand.actions` of the hero's last decision.
    pub last_hero_action: usize,
}

fn two_cards(list: &[String]) -> Option<Vec<Card>> {
    let v: Vec<Card> = list.iter().map(|s| s.parse().ok()).collect::<Option<_>>()?;
    (v.len() == 2).then_some(v)
}

/// `None` unless the hero is still in, somebody still in is all-in before the river and
/// every remaining hand was shown.
pub fn hero_all_in(hand: &Hand) -> Option<HeroAllIn> {
    let hero = hand.hero.as_deref()?;
    let hero_cards = two_cards(&hand.hero_cards)?;
    let folded = |name: &str| hand.actions.iter().any(|a| a.player == name && a.kind == ActionKind::Fold);
    let live: Vec<bool> = hand.seats.iter().map(|s| !s.sitting_out && !folded(&s.name)).collect();
    let remaining: Vec<&str> = hand.seats.iter().zip(&live).filter(|(_, l)| **l).map(|(s, _)| s.name.as_str()).collect();
    if remaining.len() < 2 || !remaining.contains(&hero) {
        return None;
    }
    let contributed: Vec<i64> = hand.seats.iter().map(|s| hand.invested(&s.name)).collect();
    let stack = |name: &str| hand.seats.iter().find(|s| s.name == name).map(|s| s.stack).unwrap_or(0);
    let all_in = remaining
        .iter()
        .any(|&p| hand.actions.iter().any(|a| a.player == p && a.all_in) || hand.invested(p) >= stack(p));
    if !all_in {
        return None;
    }
    let last = hand.actions.iter().rposition(|a| !a.kind.is_post())?;
    let board_len = match hand.actions[last].street {
        Street::Preflop => 0,
        Street::Flop => 3,
        Street::Turn => 4,
        Street::River => return None,
    };
    let board: Vec<Card> = hand.board.get(..board_len)?.iter().map(|c| c.parse().ok()).collect::<Option<_>>()?;
    let holdings: Vec<Vec<Card>> = hand
        .seats
        .iter()
        .zip(&live)
        .map(|(s, &l)| match (l, s.name == hero) {
            (false, _) => Some(Vec::new()),
            (true, true) => Some(hero_cards.clone()),
            (true, false) => hand.shown.iter().find(|(n, _)| *n == s.name).and_then(|(_, c)| two_cards(c)),
        })
        .collect::<Option<_>>()?;
    let hero_at = hand.seats.iter().position(|s| s.name == hero)?;
    let last_hero_action = hand.actions.iter().rposition(|a| a.player == hero && !a.kind.is_post())?;
    let r = all_in_ev(&holdings, &board, &contributed, &live, hand.rake, hero_at).ok()?;

    // Hero first, then the others still in, in seat order.
    let order: Vec<usize> =
        std::iter::once(hero_at).chain((0..hand.seats.len()).filter(|&i| live[i] && i != hero_at)).collect();
    let pots = r
        .pots
        .into_iter()
        .map(|p| SidePot {
            amount: p.amount,
            players: order.iter().enumerate().filter(|(_, i)| p.players.contains(i)).map(|(k, _)| k).collect(),
        })
        .collect();
    Some(HeroAllIn {
        hero_cards,
        opponents: order[1..].iter().map(|&i| holdings[i].clone()).collect(),
        board,
        pots,
        share_pct: r.share_pct,
        pot: r.pot,
        rake: r.rake,
        invested: contributed[hero_at],
        net: hand.net(hero),
        ev: r.ev,
        last_hero_action,
    })
}
