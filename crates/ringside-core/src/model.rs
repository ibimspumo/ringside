use serde::{Deserialize, Serialize};

use crate::tournament::BuyIn;

/// Money and chips in hundredths, so cash ($0.05 → 5) and tournament chips (1500 → 150000)
/// share one exact integer type.
pub type Amount = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GameKind {
    Cash,
    Tournament,
}

impl GameKind {
    pub fn as_str(self) -> &'static str {
        match self {
            GameKind::Cash => "cash",
            GameKind::Tournament => "tournament",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cash" => Some(GameKind::Cash),
            "tournament" => Some(GameKind::Tournament),
            _ => None,
        }
    }
}

/// The poker client a hand history comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Site {
    #[default]
    PokerStars,
    GgPoker,
}

impl Site {
    pub fn as_str(self) -> &'static str {
        match self {
            Site::PokerStars => "pokerstars",
            Site::GgPoker => "ggpoker",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pokerstars" => Some(Site::PokerStars),
            "ggpoker" => Some(Site::GgPoker),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Site::PokerStars => "PokerStars",
            Site::GgPoker => "GGPoker",
        }
    }

    /// GGPoker hand and tournament ids live from here up, so they never meet PokerStars ids
    /// (about 2.6·10¹¹ today) and stay exact as JavaScript numbers (below 2⁵³).
    pub fn of_id(id: u64) -> Self {
        if id >= GG_ID_BASE { Site::GgPoker } else { Site::PokerStars }
    }
}

pub const GG_ID_BASE: u64 = 1 << 52;
/// Room per two-letter GGPoker hand prefix ("TM", "HD", "RC", ...): 676 prefixes fit below 2⁵³.
const GG_PREFIX_SPAN: u64 = 6_000_000_000_000;

/// Maps a GGPoker hand number such as "TM5216773942" into Ringside's id space.
pub fn gg_hand_id(text: &str) -> Option<u64> {
    let (prefix, digits) = text.split_at_checked(2)?;
    let mut letters = prefix.bytes();
    let (a, b) = (letters.next()?, letters.next()?);
    if !a.is_ascii_uppercase() || !b.is_ascii_uppercase() || digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let number: u64 = digits.parse().ok()?;
    if number >= GG_PREFIX_SPAN {
        return None;
    }
    let code = (a - b'A') as u64 * 26 + (b - b'A') as u64;
    Some(GG_ID_BASE + code * GG_PREFIX_SPAN + number)
}

/// Maps a GGPoker tournament number into Ringside's id space.
pub fn gg_tournament_id(number: u64) -> Option<u64> {
    (number < GG_ID_BASE).then_some(GG_ID_BASE + number)
}

/// A hand id as the client writes it: "260012345678" or "TM5216773942".
pub fn hand_label(id: u64) -> String {
    if id < GG_ID_BASE {
        return id.to_string();
    }
    let rest = id - GG_ID_BASE;
    let code = rest / GG_PREFIX_SPAN;
    let letter = |n: u64| char::from(b'A' + n as u8);
    format!("{}{}{}", letter(code / 26), letter(code % 26), rest % GG_PREFIX_SPAN)
}

/// A tournament id as the client writes it.
pub fn tournament_label(id: u64) -> String {
    id.checked_sub(GG_ID_BASE).unwrap_or(id).to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    PostSmallBlind,
    PostBigBlind,
    PostAnte,
    /// "posts small & big blinds" – a dead small blind plus a live big blind.
    PostBoth,
    /// A voluntary blind before the cards (GGPoker "straddle", "posts third blind"): live money.
    PostStraddle,
    /// A missed blind paid back as dead money (GGPoker "posts missed blind").
    PostDead,
    Fold,
    Check,
    Call,
    Bet,
    Raise,
}

impl ActionKind {
    pub fn is_post(self) -> bool {
        matches!(
            self,
            ActionKind::PostSmallBlind
                | ActionKind::PostBigBlind
                | ActionKind::PostAnte
                | ActionKind::PostBoth
                | ActionKind::PostStraddle
                | ActionKind::PostDead
        )
    }

    pub fn is_aggressive(self) -> bool {
        matches!(self, ActionKind::Bet | ActionKind::Raise)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub street: Street,
    pub player: String,
    pub kind: ActionKind,
    /// Chips this action moves into the pot.
    pub amount: Amount,
    /// For raises: the total the player raised to on this street.
    pub to: Option<Amount>,
    pub all_in: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Seat {
    pub seat: u8,
    pub name: String,
    pub stack: Amount,
    /// Bounty on this player's head in knockout tournaments, in `Hand::bounty_currency`.
    #[serde(default)]
    pub bounty: Option<Amount>,
    pub sitting_out: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hand {
    /// Unique across clients, see [`Site::of_id`].
    pub id: u64,
    #[serde(default)]
    pub site: Site,
    pub game: GameKind,
    pub tournament_id: Option<u64>,
    /// Tournament name from the header (GGPoker writes it there; PokerStars only in the window title).
    #[serde(default)]
    pub tournament_name: Option<String>,
    /// Game line as written by the client, e.g. "Hold'em No Limit".
    pub variant: String,
    /// Raw table name from `Table '...'`.
    pub table_name: String,
    pub max_seats: u8,
    pub button_seat: u8,
    pub small_blind: Amount,
    pub big_blind: Amount,
    pub currency: Option<String>,
    /// Local timestamp exactly as written, "YYYY/MM/DD HH:MM:SS".
    pub played_at: String,
    pub seats: Vec<Seat>,
    pub hero: Option<String>,
    pub hero_cards: Vec<String>,
    pub board: Vec<String>,
    pub actions: Vec<Action>,
    /// Uncalled bets returned, per player.
    pub returned: Vec<(String, Amount)>,
    /// Cards revealed at showdown (or voluntarily shown).
    pub shown: Vec<(String, Vec<String>)>,
    pub collected: Vec<(String, Amount)>,
    pub showdown: bool,
    pub total_pot: Amount,
    pub rake: Amount,
    /// Money a player got (+) or paid (−) outside the pot: GGPoker's EV cashout and all-in
    /// insurance. It changes the result, not the decision.
    #[serde(default)]
    pub cashouts: Vec<(String, Amount)>,
    /// Knockout money won in this hand, per player.
    #[serde(default)]
    pub bounties_won: Vec<(String, Amount)>,
    #[serde(default)]
    pub bounty_currency: Option<String>,
    /// Tournament buy-in from the header.
    #[serde(default)]
    pub buy_in: Option<BuyIn>,
    /// Players who left the tournament in this hand.
    #[serde(default)]
    pub finishes: Vec<Finish>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finish {
    pub player: String,
    /// None when the hand history leaves it out.
    pub place: Option<i64>,
    /// Prize money, None outside the money.
    pub prize: Option<Amount>,
    pub currency: Option<String>,
}

impl Hand {
    pub fn seat_of(&self, name: &str) -> Option<u8> {
        self.seats.iter().find(|s| s.name == name).map(|s| s.seat)
    }

    /// Total chips a player put in, minus what came back uncalled.
    pub fn invested(&self, name: &str) -> Amount {
        let put: Amount = self.actions.iter().filter(|a| a.player == name).map(|a| a.amount).sum();
        let back: Amount = self.returned.iter().filter(|(p, _)| p == name).map(|(_, a)| a).sum();
        put - back
    }

    pub fn won(&self, name: &str) -> Amount {
        self.collected.iter().filter(|(p, _)| p == name).map(|(_, a)| a).sum()
    }

    pub fn net(&self, name: &str) -> Amount {
        self.won(name) + self.cashout(name) - self.invested(name)
    }

    pub fn cashout(&self, name: &str) -> Amount {
        self.cashouts.iter().filter(|(p, _)| p == name).map(|(_, a)| a).sum()
    }

    /// Players dealt into the hand, in seat order.
    pub fn active_players(&self) -> impl Iterator<Item = &Seat> {
        self.seats.iter().filter(|s| !s.sitting_out)
    }

    /// Stable key that also appears in the client's window title, used to match windows.
    pub fn table_key(&self) -> String {
        let key = table_key(self.game, self.tournament_id, &self.table_name);
        match self.site {
            Site::PokerStars => key,
            Site::GgPoker => format!("gg:{key}"),
        }
    }
}

pub fn table_key(game: GameKind, tournament_id: Option<u64>, table_name: &str) -> String {
    match (game, tournament_id) {
        (GameKind::Tournament, Some(tid)) => {
            // Tournament tables are written as "<tournament id> <table number>".
            let number = table_name.rsplit(' ').next().unwrap_or(table_name);
            format!("t:{tid}:{number}")
        }
        _ => format!("c:{table_name}"),
    }
}
