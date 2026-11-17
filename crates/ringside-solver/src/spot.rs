use postflop_solver::{
    ActionTree, BetSize, BetSizeOptions, BoardState, CardConfig, NOT_DEALT, PostFlopGame, Range as SolverRange,
    TreeConfig,
};
use ringside_engine::{Card, CardSet, Range};
use serde::{Deserialize, Serialize};

use crate::Error;

/// The solver counts whole chips; a hundredth of a big blind is fine enough for any bet size.
pub(crate) const CHIPS_PER_BB: f64 = 100.0;
/// Per street and kind (bet, raise); every extra size multiplies the tree.
pub const MAX_SIZES: usize = 4;
const MAX_POT_BB: f64 = 100_000.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StreetSizes {
    /// First bets, in percent of the pot.
    pub bet: Vec<f64>,
    /// Raises, in percent of the pot after calling.
    pub raise: Vec<f64>,
}

/// A heads-up spot at the start of a street. Both players use the same bet sizes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    /// Out of position: acts first on every street.
    pub oop_range: String,
    pub ip_range: String,
    /// Three to five cards, e.g. "Ah7d2c" or "Ah 7d 2c Ks".
    pub board: String,
    /// In big blinds.
    pub pot: f64,
    pub stack: f64,
    /// Flop, turn and river; streets already on the board are ignored.
    pub sizes: [StreetSizes; 3],
    /// All-in is offered once it is at most this percentage of the pot.
    pub allin_threshold: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Estimate {
    /// What the solve allocates: 32-bit storage if it fits the limit, else 16-bit.
    pub memory_bytes: u64,
    pub compressed: bool,
    pub memory_limit: u64,
    pub fits: bool,
    /// Combos left for OOP and IP once the board is dealt, weights summed.
    pub combos: [f64; 2],
    /// Rough time to a target of 0.5 % of the pot at `threads`, in seconds.
    pub seconds: f64,
}

pub(crate) struct Built {
    pub game: PostFlopGame,
    pub estimate: Estimate,
}

pub(crate) fn chips(bb: f64) -> i32 {
    (bb * CHIPS_PER_BB).round() as i32
}

pub(crate) fn bb(chips: f64) -> f64 {
    chips / CHIPS_PER_BB
}

pub(crate) fn parse_board(s: &str) -> Result<Vec<Card>, Error> {
    let compact: String = s.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let chars: Vec<char> = compact.chars().collect();
    if !chars.len().is_multiple_of(2) {
        return Err(Error::Invalid(format!("Board „{s}“ ist unvollständig.")));
    }
    let mut cards = Vec::new();
    for pair in chars.chunks(2) {
        let text: String = pair.iter().collect();
        let card: Card = text.parse().map_err(|_| Error::Invalid(format!("„{text}“ ist keine Karte.")))?;
        if cards.contains(&card) {
            return Err(Error::Invalid(format!("{card} liegt doppelt auf dem Board.")));
        }
        cards.push(card);
    }
    if !(3..=5).contains(&cards.len()) {
        return Err(Error::Invalid("Das Board braucht drei bis fünf Karten.".into()));
    }
    Ok(cards)
}

/// The solver's range and its combos left beside the board, weights summed.
fn parse_range(text: &str, who: &str, board: CardSet) -> Result<(SolverRange, f64), Error> {
    let range = Range::parse(text).map_err(|e| Error::Invalid(format!("Range von {who}: {e}")))?;
    let live = range.without(board);
    if live.is_empty() {
        return Err(Error::Invalid(format!("Die Range von {who} ist leer (oder liegt ganz auf dem Board).")));
    }
    let mut out = SolverRange::new();
    for (combo, weight) in range.combos() {
        let (a, b) = combo.cards();
        out.set_weight_by_cards(a.index(), b.index(), weight);
    }
    Ok((out, live.weighted_count()))
}

fn sizes(list: &[f64], what: &str, street: &str) -> Result<Vec<BetSize>, Error> {
    if list.len() > MAX_SIZES {
        return Err(Error::Invalid(format!("Höchstens {MAX_SIZES} {what} je Straße ({street}).")));
    }
    let mut out = Vec::with_capacity(list.len());
    for &pct in list {
        if !(pct.is_finite() && (1.0..=1000.0).contains(&pct)) {
            return Err(Error::Invalid(format!("{what} ({street}) müssen zwischen 1 und 1000 % liegen.")));
        }
        out.push(BetSize::PotRelative(pct / 100.0));
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out.dedup();
    Ok(out)
}

fn options(s: &StreetSizes, street: &str) -> Result<[BetSizeOptions; 2], Error> {
    let o = BetSizeOptions { bet: sizes(&s.bet, "Einsatzgrößen", street)?, raise: sizes(&s.raise, "Raise-Größen", street)? };
    Ok([o.clone(), o])
}

/// Builds the game tree without allocating its storage.
pub(crate) fn build(spot: &Spot, memory_limit: u64, threads: usize) -> Result<Built, Error> {
    let board = parse_board(&spot.board)?;
    let dead = CardSet::from_cards(&board);
    if !(spot.pot.is_finite() && spot.pot > 0.0 && spot.pot <= MAX_POT_BB) {
        return Err(Error::Invalid("Der Pot muss größer als 0 sein.".into()));
    }
    if !(spot.stack.is_finite() && spot.stack > 0.0 && spot.stack <= MAX_POT_BB) {
        return Err(Error::Invalid("Der effektive Stack muss größer als 0 sein.".into()));
    }
    if !(spot.allin_threshold.is_finite() && spot.allin_threshold >= 0.0) {
        return Err(Error::Invalid("Die All-in-Schwelle darf nicht negativ sein.".into()));
    }
    let (oop, oop_combos) = parse_range(&spot.oop_range, "OOP", dead)?;
    let (ip, ip_combos) = parse_range(&spot.ip_range, "IP", dead)?;
    let (range, combos) = ([oop, ip], [oop_combos, ip_combos]);

    let card = |i: usize| board.get(i).map_or(NOT_DEALT, |c| c.index());
    let card_config = CardConfig { range, flop: [card(0), card(1), card(2)], turn: card(3), river: card(4) };
    let initial_state = match board.len() {
        3 => BoardState::Flop,
        4 => BoardState::Turn,
        _ => BoardState::River,
    };
    let tree_config = TreeConfig {
        initial_state,
        starting_pot: chips(spot.pot),
        effective_stack: chips(spot.stack),
        rake_rate: 0.0,
        rake_cap: 0.0,
        flop_bet_sizes: options(&spot.sizes[0], "Flop")?,
        turn_bet_sizes: options(&spot.sizes[1], "Turn")?,
        river_bet_sizes: options(&spot.sizes[2], "River")?,
        turn_donk_sizes: None,
        river_donk_sizes: None,
        add_allin_threshold: spot.allin_threshold / 100.0,
        // Bets that would leave almost nothing behind become all-ins, as in most solvers.
        force_allin_threshold: 0.15,
        merging_threshold: 0.1,
    };
    let tree = ActionTree::new(tree_config).map_err(|e| Error::Invalid(format!("Spielbaum ungültig: {e}")))?;
    let game =
        PostFlopGame::with_config(card_config, tree).map_err(|e| Error::Invalid(format!("Spot ungültig: {e}")))?;

    let (full, compressed) = game.memory_usage();
    let use_compression = full > memory_limit && compressed <= memory_limit;
    let memory_bytes = if use_compression { compressed } else { full };
    let estimate = Estimate {
        memory_bytes,
        compressed: use_compression,
        memory_limit,
        fits: memory_bytes <= memory_limit,
        combos,
        seconds: seconds(memory_bytes, board.len(), threads),
    };
    Ok(Built { game, estimate })
}

/// Seconds per iteration grow with the allocated storage; parallel speed-up flattens out.
/// Calibrated on release builds; the iterations to 0.5 % of the pot fall from flop to river.
fn seconds(memory_bytes: u64, board_len: usize, threads: usize) -> f64 {
    const SECONDS_PER_GB_ITERATION: f64 = 1.1;
    let iterations = match board_len {
        3 => 250.0,
        4 => 180.0,
        _ => 120.0,
    };
    let speedup = (threads.max(1) as f64).powf(0.8);
    memory_bytes as f64 / (1u64 << 30) as f64 * SECONDS_PER_GB_ITERATION * iterations / speedup
}

/// Memory and time a solve of `spot` would take, without allocating it.
pub fn estimate(spot: &Spot, memory_limit: u64, threads: usize) -> Result<Estimate, Error> {
    Ok(build(spot, memory_limit, threads)?.estimate)
}
