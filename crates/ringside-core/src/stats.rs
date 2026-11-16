//! Per-hand counters and the stats derived from them.
//!
//! Every stat is a ratio of two counters ("made" / "opportunity"), so stats over any set of
//! hands are just sums of per-hand counters.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::model::{ActionKind, Hand, Street};

macro_rules! counters {
    ($($field:ident),+ $(,)?) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub struct Counters {
            $(pub $field: i64,)+
        }

        impl Counters {
            /// Column names in storage order; the database schema is generated from this.
            pub const FIELDS: &'static [&'static str] = &[$(stringify!($field)),+];

            pub fn to_vec(&self) -> Vec<i64> {
                vec![$(self.$field),+]
            }

            pub fn from_slice(values: &[i64]) -> Self {
                let mut it = values.iter().copied();
                Counters { $($field: it.next().unwrap_or(0),)+ }
            }

            pub fn add(&mut self, other: &Counters) {
                $(self.$field += other.$field;)+
            }
        }
    };
}

counters! {
    hands,
    vpip_opp, vpip, pfr,
    three_bet_opp, three_bet,
    fold_to_three_bet_opp, fold_to_three_bet,
    steal_opp, steal,
    fold_to_steal_opp, fold_to_steal,
    saw_flop,
    cbet_opp, cbet,
    fold_to_cbet_opp, fold_to_cbet,
    wtsd, wsd,
    postflop_aggr, postflop_calls, postflop_folds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatId {
    Hands,
    Vpip,
    Pfr,
    ThreeBet,
    FoldToThreeBet,
    Steal,
    FoldToSteal,
    Cbet,
    FoldToCbet,
    Wtsd,
    Wsd,
    Af,
    Afq,
}

impl StatId {
    pub const ALL: [StatId; 13] = [
        StatId::Hands,
        StatId::Vpip,
        StatId::Pfr,
        StatId::ThreeBet,
        StatId::FoldToThreeBet,
        StatId::Steal,
        StatId::FoldToSteal,
        StatId::Cbet,
        StatId::FoldToCbet,
        StatId::Wtsd,
        StatId::Wsd,
        StatId::Af,
        StatId::Afq,
    ];

    /// (made, opportunities). For AF the "made" side is aggressive actions and the
    /// "opportunities" side is calls, because AF is a ratio rather than a frequency.
    pub fn parts(self, c: &Counters) -> (i64, i64) {
        match self {
            StatId::Hands => (c.hands, c.hands),
            StatId::Vpip => (c.vpip, c.vpip_opp),
            StatId::Pfr => (c.pfr, c.vpip_opp),
            StatId::ThreeBet => (c.three_bet, c.three_bet_opp),
            StatId::FoldToThreeBet => (c.fold_to_three_bet, c.fold_to_three_bet_opp),
            StatId::Steal => (c.steal, c.steal_opp),
            StatId::FoldToSteal => (c.fold_to_steal, c.fold_to_steal_opp),
            StatId::Cbet => (c.cbet, c.cbet_opp),
            StatId::FoldToCbet => (c.fold_to_cbet, c.fold_to_cbet_opp),
            StatId::Wtsd => (c.wtsd, c.saw_flop),
            StatId::Wsd => (c.wsd, c.wtsd),
            StatId::Af => (c.postflop_aggr, c.postflop_calls),
            StatId::Afq => (c.postflop_aggr, c.postflop_aggr + c.postflop_calls + c.postflop_folds),
        }
    }

    /// Percent for frequencies, plain ratio for AF, count for hands. `None` without a sample.
    pub fn value(self, c: &Counters) -> Option<f64> {
        let (made, opp) = self.parts(c);
        match self {
            StatId::Hands => Some(c.hands as f64),
            StatId::Af if opp == 0 => (made > 0).then_some(made as f64),
            _ if opp == 0 => None,
            StatId::Af => Some(made as f64 / opp as f64),
            _ => Some(100.0 * made as f64 / opp as f64),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatValue {
    pub id: StatId,
    pub value: Option<f64>,
    pub made: i64,
    pub opp: i64,
}

pub fn stat_values(c: &Counters) -> Vec<StatValue> {
    StatId::ALL
        .iter()
        .map(|&id| {
            let (made, opp) = id.parts(c);
            StatValue { id, value: id.value(c), made, opp }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Position {
    Cutoff,
    Button,
    SmallBlind,
    BigBlind,
    Other,
}

fn positions(hand: &Hand) -> Vec<(String, Position)> {
    let posted = |kind: ActionKind| {
        hand.actions
            .iter()
            .find(|a| a.street == Street::Preflop && a.kind == kind)
            .map(|a| a.player.clone())
    };
    let sb = posted(ActionKind::PostSmallBlind);
    let bb = posted(ActionKind::PostBigBlind);
    let active: Vec<&str> = hand.active_players().map(|s| s.name.as_str()).collect();
    let button = hand.seats.iter().find(|s| s.seat == hand.button_seat).map(|s| s.name.as_str());
    let cutoff = button
        .and_then(|b| active.iter().position(|&n| n == b))
        .filter(|_| active.len() >= 4)
        .map(|i| active[(i + active.len() - 1) % active.len()]);

    active
        .iter()
        .map(|&name| {
            let pos = if Some(name) == bb.as_deref() {
                Position::BigBlind
            } else if Some(name) == sb.as_deref() {
                Position::SmallBlind
            } else if Some(name) == button {
                Position::Button
            } else if Some(name) == cutoff {
                Position::Cutoff
            } else {
                Position::Other
            };
            (name.to_string(), pos)
        })
        .collect()
}

/// Counters for every player dealt into `hand`.
pub fn analyze(hand: &Hand) -> Vec<(String, Counters)> {
    let positions = positions(hand);
    let pos_of = |name: &str| {
        positions.iter().find(|(n, _)| n == name).map(|(_, p)| *p).unwrap_or(Position::Other)
    };
    let mut out: Vec<(String, Counters)> =
        positions.iter().map(|(n, _)| (n.clone(), Counters { hands: 1, ..Default::default() })).collect();
    let names: Vec<String> = positions.iter().map(|(n, _)| n.clone()).collect();
    let idx = |name: &str| names.iter().position(|n| n == name);

    // Preflop.
    let mut raises = 0;
    let mut callers_since_raise = 0;
    let mut limpers = 0;
    let mut opener: Option<String> = None;
    let mut open_was_steal = false;
    let mut last_raiser: Option<String> = None;
    let mut acted: HashSet<String> = HashSet::new();
    let mut faced_three_bet: HashSet<String> = HashSet::new();
    let mut faced_open: HashSet<String> = HashSet::new();
    let mut faced_steal: HashSet<String> = HashSet::new();
    let mut folded: HashSet<String> = HashSet::new();

    for a in hand.actions.iter().filter(|a| a.street == Street::Preflop && !a.kind.is_post()) {
        let Some(i) = idx(&a.player) else { continue };
        let c = &mut out[i].1;
        let pos = pos_of(&a.player);
        let raise = a.kind == ActionKind::Raise;

        if acted.insert(a.player.clone()) {
            c.vpip_opp += 1;
        }
        if raise {
            c.pfr = 1;
        }
        if matches!(a.kind, ActionKind::Call | ActionKind::Raise) {
            c.vpip = 1;
        }

        let stealer = matches!(pos, Position::Cutoff | Position::Button | Position::SmallBlind);
        if raises == 0 && limpers == 0 && stealer && c.steal_opp == 0 {
            c.steal_opp = 1;
            c.steal = raise as i64;
        }
        if raises == 1 && opener.as_deref() != Some(&a.player) && faced_open.insert(a.player.clone()) {
            c.three_bet_opp = 1;
            c.three_bet = raise as i64;
            let blind = matches!(pos, Position::SmallBlind | Position::BigBlind);
            if open_was_steal && blind && callers_since_raise == 0 && faced_steal.insert(a.player.clone()) {
                c.fold_to_steal_opp = 1;
                c.fold_to_steal = (a.kind == ActionKind::Fold) as i64;
            }
        }
        if raises == 2
            && opener.as_deref() == Some(&a.player)
            && faced_three_bet.insert(a.player.clone())
        {
            c.fold_to_three_bet_opp = 1;
            c.fold_to_three_bet = (a.kind == ActionKind::Fold) as i64;
        }

        match a.kind {
            ActionKind::Raise => {
                raises += 1;
                callers_since_raise = 0;
                if raises == 1 {
                    opener = Some(a.player.clone());
                    open_was_steal = stealer && limpers == 0;
                }
                last_raiser = Some(a.player.clone());
            }
            ActionKind::Call if raises == 0 => limpers += 1,
            ActionKind::Call => callers_since_raise += 1,
            ActionKind::Fold => {
                folded.insert(a.player.clone());
            }
            _ => {}
        }
    }

    // Postflop.
    let saw_flop = hand.board.len() >= 3;
    if saw_flop {
        for (name, c) in out.iter_mut() {
            if !folded.contains(name) {
                c.saw_flop = 1;
            }
        }
    }

    if let Some(aggressor) = last_raiser.filter(|_| saw_flop) {
        let flop: Vec<_> = hand.actions.iter().filter(|a| a.street == Street::Flop).collect();
        if let Some(first) = flop.iter().position(|a| a.player == aggressor) {
            let bet_before = flop[..first].iter().any(|a| a.kind.is_aggressive());
            if !bet_before {
                let made = flop[first].kind == ActionKind::Bet;
                if let Some(i) = idx(&aggressor) {
                    out[i].1.cbet_opp = 1;
                    out[i].1.cbet = made as i64;
                }
                if made {
                    let mut responded: HashSet<&str> = HashSet::new();
                    for a in &flop[first + 1..] {
                        if a.kind == ActionKind::Raise {
                            break;
                        }
                        if a.player != aggressor && responded.insert(&a.player)
                            && let Some(i) = idx(&a.player) {
                                out[i].1.fold_to_cbet_opp = 1;
                                out[i].1.fold_to_cbet = (a.kind == ActionKind::Fold) as i64;
                            }
                    }
                }
            }
        }
    }

    for a in hand.actions.iter().filter(|a| a.street > Street::Preflop) {
        let Some(i) = idx(&a.player) else { continue };
        let c = &mut out[i].1;
        match a.kind {
            ActionKind::Bet | ActionKind::Raise => c.postflop_aggr += 1,
            ActionKind::Call => c.postflop_calls += 1,
            ActionKind::Fold => c.postflop_folds += 1,
            _ => {}
        }
    }

    if hand.showdown {
        let folded_any: HashSet<&str> =
            hand.actions.iter().filter(|a| a.kind == ActionKind::Fold).map(|a| a.player.as_str()).collect();
        for (name, c) in out.iter_mut() {
            if c.saw_flop == 1 && !folded_any.contains(name.as_str()) {
                c.wtsd = 1;
                c.wsd = (hand.won(name) > 0) as i64;
            }
        }
    }

    out
}
