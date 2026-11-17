//! The table of a hand as the engine sees it: seats in preflop action order, blinds and
//! the ante structure.

use ringside_core::model::{ActionKind, Hand, Seat, Street};
use ringside_engine::pushfold_table::Ante;
use ringside_engine::{Card, Combo, HandClass};

pub struct Table<'a> {
    /// Seats dealt in, in preflop action order: first after the big blind, big blind last.
    pub order: Vec<&'a Seat>,
    pub hero: usize,
    pub big_blind: f64,
    pub small_blind_bb: f64,
    pub ante: Ante,
    /// Ante per player and of the big blind alone, in BB, as posted.
    pub ante_bb: f64,
    pub bb_ante_bb: f64,
    pub hero_combo: Combo,
}

impl Table<'_> {
    pub fn players(&self) -> usize {
        self.order.len()
    }

    pub fn seat_of(&self, name: &str) -> Option<usize> {
        self.order.iter().position(|s| s.name == name)
    }

    pub fn class(&self) -> HandClass {
        self.hero_combo.class()
    }

    pub fn stack_bb(&self, seat: usize) -> f64 {
        self.order[seat].stack as f64 / self.big_blind
    }
}

pub fn hero_combo(hand: &Hand) -> Option<Combo> {
    let cards: Vec<Card> = hand.hero_cards.iter().map(|c| c.parse().ok()).collect::<Option<_>>()?;
    (cards.len() == 2 && cards[0] != cards[1]).then(|| Combo::new(cards[0], cards[1]))
}

/// The table, or why the engine cannot model this hand.
pub fn table<'a>(hand: &'a Hand, hero: &str) -> Result<Table<'a>, String> {
    if !hand.variant.contains("Hold'em No Limit") {
        return Err(format!("Nur No-Limit Hold'em wird bewertet, nicht „{}“.", hand.variant));
    }
    if hand.big_blind <= 0 {
        return Err("Big Blind unbekannt.".into());
    }
    let hero_combo = hero_combo(hand).ok_or("Deine Karten fehlen in der Hand-History.")?;
    let pre = || hand.actions.iter().filter(|a| a.street == Street::Preflop);
    if pre().any(|a| a.kind == ActionKind::PostBoth) {
        return Err("Totes Blind-Geld (Small und Big Blind auf einmal gesetzt) kennt das Modell nicht.".into());
    }
    let poster = |kind: ActionKind| -> Result<&str, String> {
        let mut it = pre().filter(|a| a.kind == kind);
        match (it.next(), it.next()) {
            (Some(a), None) => Ok(a.player.as_str()),
            _ => Err("Ungewöhnliche Blinds (fehlend oder mehrfach).".into()),
        }
    };
    let sb = poster(ActionKind::PostSmallBlind)?;
    let bb = poster(ActionKind::PostBigBlind)?;
    // Players sitting out in a tournament are still dealt in: they post and fold.
    let active: Vec<&Seat> =
        hand.seats.iter().filter(|s| !s.sitting_out || hand.actions.iter().any(|a| a.player == s.name)).collect();
    let n = active.len();
    let at = active.iter().position(|s| s.name == bb).ok_or("Big Blind sitzt nicht am Tisch.")?;
    let order: Vec<&Seat> = (1..=n).map(|k| active[(at + k) % n]).collect();
    if n < 2 || order[n - 2].name != sb || n > ringside_engine::preflop::MAX_PLAYERS {
        return Err("Small Blind nicht direkt vor dem Big Blind.".into());
    }
    let hero_seat = order.iter().position(|s| s.name == hero).ok_or("Du warst nicht ausgeteilt.")?;
    let big = hand.big_blind as f64;
    let antes: Vec<(&str, f64)> =
        pre().filter(|a| a.kind == ActionKind::PostAnte).map(|a| (a.player.as_str(), a.amount as f64 / big)).collect();
    let (ante, ante_bb, bb_ante_bb) = match antes.as_slice() {
        [] => (Ante::None, 0.0, 0.0),
        [(who, x)] if *who == bb => (Ante::BigBlind, 0.0, *x),
        list => (Ante::Classic, list.iter().map(|(_, x)| *x).fold(f64::INFINITY, f64::min), 0.0),
    };
    Ok(Table {
        order,
        hero: hero_seat,
        big_blind: big,
        small_blind_bb: hand.small_blind as f64 / big,
        ante,
        ante_bb,
        bb_ante_bb,
        hero_combo,
    })
}

pub fn ante_name(ante: Ante) -> &'static str {
    match ante {
        Ante::None => "ohne Ante",
        Ante::Classic => "Ante für alle",
        Ante::BigBlind => "Big-Blind-Ante",
    }
}

/// The value of `grid` closest to `x` by ratio.
pub fn nearest(grid: &[f64], x: f64) -> f64 {
    let x = x.max(1e-9);
    grid.iter().copied().min_by(|a, b| (a / x).ln().abs().total_cmp(&(b / x).ln().abs())).unwrap_or(x)
}
