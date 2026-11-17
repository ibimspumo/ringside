use serde::{Deserialize, Serialize};

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
            ActionKind::PostSmallBlind | ActionKind::PostBigBlind | ActionKind::PostAnte | ActionKind::PostBoth
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
    pub id: u64,
    pub game: GameKind,
    pub tournament_id: Option<u64>,
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
    /// Knockout money won in this hand, per player.
    #[serde(default)]
    pub bounties_won: Vec<(String, Amount)>,
    #[serde(default)]
    pub bounty_currency: Option<String>,
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
        self.won(name) - self.invested(name)
    }

    /// Players dealt into the hand, in seat order.
    pub fn active_players(&self) -> impl Iterator<Item = &Seat> {
        self.seats.iter().filter(|s| !s.sitting_out)
    }

    /// Stable key that also appears in the client's window title, used to match windows.
    pub fn table_key(&self) -> String {
        table_key(self.game, self.tournament_id, &self.table_name)
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
