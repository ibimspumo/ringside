//! Tournament buy-ins, the client's tournament summary files and results per currency.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::model::Amount;
use crate::parser::{currency_code, parse_amount};

/// Currency of play-chip tournaments, which have neither a symbol nor a code.
pub const PLAY: &str = "PLAY";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyIn {
    /// Part that goes into the prize pool.
    pub prize: Amount,
    /// Starting bounty in knockout tournaments, 0 otherwise.
    pub bounty: Amount,
    pub fee: Amount,
    /// "USD", "EUR", ..., "FPP"/"SC" for points, [`PLAY`] for play chips, None for freerolls.
    pub currency: Option<String>,
}

impl BuyIn {
    pub fn total(&self) -> Amount {
        self.prize + self.bounty + self.fee
    }
}

/// Parses a buy-in as written in hand history headers ("$10+$10+$2 USD", "Freeroll") or in
/// summaries ("$2.00/$0.50/$0.20 USD", "300/20", "250 SC"). Three parts are prize, bounty, fee.
pub fn parse_buy_in(text: &str) -> Option<BuyIn> {
    let text = text.trim();
    if text.starts_with("Freeroll") {
        return Some(BuyIn { prize: 0, bounty: 0, fee: 0, currency: None });
    }
    let mut tokens: Vec<&str> = text.split_whitespace().collect();
    let code = match tokens.last() {
        Some(t) if t.len() >= 2 && t.chars().all(|c| c.is_ascii_uppercase()) => tokens.pop(),
        _ => None,
    };
    let joined = tokens.concat();
    if joined.is_empty() {
        return None;
    }
    let amounts = joined
        .split(['+', '/'])
        .map(|p| parse_amount(p.trim_start_matches(|c: char| !c.is_ascii_digit())))
        .collect::<Option<Vec<Amount>>>()?;
    let (prize, bounty, fee) = match amounts[..] {
        [p] => (p, 0, 0),
        [p, f] => (p, 0, f),
        [p, b, f] => (p, b, f),
        _ => return None,
    };
    let symbol: String = joined.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let currency = match currency_code(&symbol, code) {
        Some(c) => Some(c),
        None if symbol.is_empty() => Some(PLAY.to_string()),
        None => None,
    };
    Some(BuyIn { prize, bounty, fee, currency })
}

/// "Hold'em No Limit" (hand histories) and "No Limit Hold'em" (summaries) both become "NL Hold'em".
pub fn game_label(text: &str) -> String {
    let text = text.trim();
    for (limit, short) in [("No Limit", "NL"), ("Pot Limit", "PL"), ("Limit", "FL")] {
        if let Some(rest) = text.strip_prefix(limit).or_else(|| text.strip_suffix(limit)) {
            return format!("{short} {}", rest.trim());
        }
    }
    text.to_string()
}

/// One tournament as written by the client into its `TournSummary` folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub id: u64,
    pub game: String,
    pub buy_in: Option<BuyIn>,
    pub entrants: Option<i64>,
    pub prize_pool: Option<Amount>,
    /// Local time as "YYYY/MM/DD HH:MM:SS".
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    /// The hero's finishing place; None while still playing.
    pub place: Option<i64>,
    /// The hero's prize, 0 outside the money.
    pub prize: Amount,
    /// Knockout money the hero won, when the summary lists it.
    pub bounties: Option<Amount>,
    /// Re-entries, rebuys and add-ons on top of the first buy-in.
    pub extra_cost: Amount,
    pub currency: Option<String>,
    /// Prize per place from first, when the summary lists every paid place; else empty.
    pub payouts: Vec<Amount>,
}

static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^PokerStars Tournament #(\d+),\s*(.*)$").unwrap());
static PLAYERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([\d,]+) players$").unwrap());
static TIME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4}/\d{2}/\d{2}) (\d{1,2}):(\d{2}):(\d{2})").unwrap());
/// "  7: Name [2] (Country), $6,514.80 (2.063%)": the text after the last "),".
static RANK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d+): .*\),\s*(.*)$").unwrap());
static FINISHED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"You finished (?:the tournament )?in (\d+)(?:st|nd|rd|th) place").unwrap());
static BOUNTIES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"You collected \d+ bount(?:y|ies) for a total of (.+?)\.?$|You have also received (.+?) in Knockout Bounties").unwrap()
});
static EXTRA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^You made .* for a total of (.+?)\.?$").unwrap());

fn padded_time(text: &str) -> Option<String> {
    let c = TIME.captures(text)?;
    Some(format!("{} {:0>2}:{}:{}", &c[1], &c[2], &c[3], &c[4]))
}

/// Amount after any currency symbol or code, e.g. "USD 875.00" or "$1,899.29".
fn money(text: &str) -> Option<Amount> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    parse_amount(text[start..].split_whitespace().next()?)
}

/// The prizes of places 1, 2, … when the ranks show all of them: paid places from first
/// without a gap, followed by a listed place without a prize (or the whole field paid).
/// Summaries written while the tournament still runs leave out places or say
/// "still playing", and give nothing.
fn payouts(ranks: &[(i64, String)], entrants: Option<i64>) -> Vec<Amount> {
    let mut sorted: Vec<&(i64, String)> = ranks.iter().collect();
    sorted.sort_by_key(|(p, _)| *p);
    let mut out = Vec::new();
    for (place, text) in sorted {
        if *place != out.len() as i64 + 1 {
            return Vec::new();
        }
        match money(text) {
            Some(a) if a > 0 => out.push(a),
            _ if text.trim().is_empty() && !out.is_empty() => return out,
            _ => return Vec::new(),
        }
    }
    if entrants == Some(out.len() as i64) { out } else { Vec::new() }
}

/// Parses every tournament in a summary file (normally one; e-mailed histories hold several).
pub fn parse_summaries(text: &str) -> Vec<Summary> {
    let mut out: Vec<Summary> = Vec::new();
    let mut ranks: Vec<(i64, String)> = Vec::new();
    let finish = |s: &mut Summary, ranks: &mut Vec<(i64, String)>| {
        if let Some(place) = s.place
            && let Some((_, prize)) = ranks.iter().find(|(p, _)| *p == place)
        {
            s.prize = money(prize).unwrap_or(0);
            if s.currency.is_none() {
                let symbol: String = prize.chars().take_while(|c| !c.is_ascii_digit()).collect();
                s.currency = currency_code(&symbol, None);
            }
        }
        s.payouts = payouts(ranks, s.entrants);
        ranks.clear();
    };
    for line in text.lines().map(|l| l.trim_start_matches('\u{feff}').trim_end()) {
        if let Some(c) = HEADER.captures(line) {
            if let Some(prev) = out.last_mut() {
                finish(prev, &mut ranks);
            }
            let Ok(id) = c[1].parse() else { continue };
            out.push(Summary {
                id,
                game: game_label(&c[2]),
                buy_in: None,
                entrants: None,
                prize_pool: None,
                started_at: None,
                finished_at: None,
                place: None,
                prize: 0,
                bounties: None,
                extra_cost: 0,
                currency: None,
                payouts: Vec::new(),
            });
            continue;
        }
        let Some(s) = out.last_mut() else { continue };
        if let Some(rest) = line.strip_prefix("Buy-In: ") {
            s.buy_in = parse_buy_in(rest);
            s.currency = s.buy_in.as_ref().and_then(|b| b.currency.clone());
        } else if line.starts_with("Freeroll") && s.buy_in.is_none() {
            s.buy_in = parse_buy_in("Freeroll");
        } else if let Some(c) = PLAYERS.captures(line) {
            s.entrants = c[1].replace(',', "").parse().ok();
        } else if let Some(rest) = line.strip_prefix("Total Prize Pool: ") {
            s.prize_pool = money(rest);
        } else if let Some(rest) = line.strip_prefix("Tournament started ") {
            s.started_at = padded_time(rest);
        } else if let Some(rest) = line.strip_prefix("Tournament finished ") {
            s.finished_at = padded_time(rest);
        } else if let Some(c) = RANK.captures(line) {
            if let Ok(place) = c[1].parse() {
                ranks.push((place, c[2].to_string()));
            }
        } else {
            if let Some(c) = FINISHED.captures(line) {
                s.place = c[1].parse().ok();
            }
            if let Some(c) = BOUNTIES.captures(line) {
                s.bounties = c.get(1).or(c.get(2)).and_then(|m| money(m.as_str()));
            }
            if let Some(c) = EXTRA.captures(line) {
                s.extra_cost += money(&c[1]).unwrap_or(0);
            }
        }
    }
    if let Some(last) = out.last_mut() {
        finish(last, &mut ranks);
    }
    out
}

/// A tournament as listed on the tournaments page, merged from its summary and its hands.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TournamentRow {
    pub id: u64,
    pub game: String,
    pub currency: Option<String>,
    pub buy_in: Option<BuyIn>,
    /// Everything paid in: buy-in plus re-entries; None when the buy-in is unknown.
    pub cost: Option<Amount>,
    pub entrants: Option<i64>,
    /// None while the result is unknown, or when the elimination hand names no place.
    pub place: Option<i64>,
    /// None while the result is unknown.
    pub prize: Option<Amount>,
    pub bounties: Amount,
    /// Prize plus bounties minus cost, once both result and cost are known.
    pub profit: Option<Amount>,
    pub started_at: String,
    pub hands: i64,
    /// The hero's chip change over the imported hands.
    pub chips: Amount,
    pub has_summary: bool,
    /// Prize per place from first, from a summary that lists every paid place.
    #[serde(skip)]
    pub payouts: Vec<Amount>,
    /// As shown in the table window title ("$5.50 Mini Early Battle [Progressive KO], $1.5K Gtd").
    pub name: Option<String>,
}

/// Results in one currency. Money figures cover only tournaments whose result and buy-in are
/// known; `tournaments` counts all of them.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Totals {
    /// Empty when unknown (freerolls without a summary).
    pub currency: String,
    pub tournaments: i64,
    pub finished: i64,
    pub cashes: i64,
    pub cost: Amount,
    pub prizes: Amount,
    pub bounties: Amount,
    pub profit: Amount,
    /// Profit per buy-in, in percent; None without any cost.
    pub roi: Option<f64>,
    /// Share of finished tournaments with a prize, in percent.
    pub itm: Option<f64>,
}

/// Totals per currency, the currency with the most tournaments first. Amounts in different
/// currencies are never added together.
pub fn totals(rows: &[TournamentRow]) -> Vec<Totals> {
    let mut by: BTreeMap<String, Totals> = BTreeMap::new();
    for r in rows {
        let key = r.currency.clone().unwrap_or_default();
        let t = by.entry(key.clone()).or_insert_with(|| Totals { currency: key, ..Default::default() });
        t.tournaments += 1;
        let (Some(cost), Some(prize), Some(profit)) = (r.cost, r.prize, r.profit) else { continue };
        t.finished += 1;
        t.cashes += (prize > 0) as i64;
        t.cost += cost;
        t.prizes += prize;
        t.bounties += r.bounties;
        t.profit += profit;
    }
    let mut out: Vec<Totals> = by
        .into_values()
        .map(|mut t| {
            t.roi = (t.cost > 0).then(|| t.profit as f64 / t.cost as f64 * 100.0);
            t.itm = (t.finished > 0).then(|| t.cashes as f64 / t.finished as f64 * 100.0);
            t
        })
        .collect();
    out.sort_by_key(|t| (t.currency.is_empty(), std::cmp::Reverse(t.tournaments)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(prize: Amount, bounty: Amount, fee: Amount, currency: Option<&str>) -> Option<BuyIn> {
        Some(BuyIn { prize, bounty, fee, currency: currency.map(String::from) })
    }

    #[test]
    fn buy_in_variants() {
        assert_eq!(parse_buy_in("$5+$0.50 USD"), b(500, 0, 50, Some("USD")));
        assert_eq!(parse_buy_in("$10+$10+$2 USD"), b(1000, 1000, 200, Some("USD")));
        assert_eq!(parse_buy_in("€1.80+€0.20 EUR"), b(180, 0, 20, Some("EUR")));
        assert_eq!(parse_buy_in("Freeroll"), b(0, 0, 0, None));
        assert_eq!(parse_buy_in("300+20"), b(30000, 0, 2000, Some(PLAY)));
        assert_eq!(parse_buy_in("100 FPP"), b(10000, 0, 0, Some("FPP")));
        assert_eq!(parse_buy_in("$2.00/$0.50/$0.20 USD"), b(200, 50, 20, Some("USD")));
        assert_eq!(parse_buy_in("£1,000/£50"), b(100000, 0, 5000, Some("GBP")));
        assert_eq!(parse_buy_in("₹136.50/₹13.50 INR"), b(13650, 0, 1350, Some("INR")));
        assert_eq!(parse_buy_in(""), None);
        assert_eq!(parse_buy_in("$1+$2+$3+$4 USD"), None);
        assert_eq!(b(1000, 1000, 200, None).unwrap().total(), 2200);
    }

    #[test]
    fn game_labels() {
        assert_eq!(game_label("Hold'em No Limit"), "NL Hold'em");
        assert_eq!(game_label("No Limit Hold'em"), "NL Hold'em");
        assert_eq!(game_label("Pot Limit 5 Card Omaha"), "PL 5 Card Omaha");
        assert_eq!(game_label("Omaha Hi/Lo Limit"), "FL Omaha Hi/Lo");
        assert_eq!(game_label("HORSE"), "HORSE");
    }

    fn row(currency: Option<&str>, cost: Option<Amount>, prize: Option<Amount>, bounties: Amount) -> TournamentRow {
        TournamentRow {
            id: 1,
            game: String::new(),
            currency: currency.map(String::from),
            buy_in: None,
            cost,
            entrants: None,
            place: prize.map(|_| 1),
            prize,
            bounties,
            profit: match (cost, prize) {
                (Some(c), Some(p)) => Some(p + bounties - c),
                _ => None,
            },
            started_at: String::new(),
            hands: 0,
            chips: 0,
            has_summary: true,
            payouts: Vec::new(),
            name: None,
        }
    }

    #[test]
    fn roi_and_itm_per_currency() {
        let rows = [
            row(Some("USD"), Some(1100), Some(0), 0),
            row(Some("USD"), Some(1100), Some(3300), 0),
            row(Some("USD"), Some(2200), Some(0), 1500),
            row(Some("USD"), Some(550), None, 300),
            row(Some("EUR"), Some(200), Some(0), 0),
            row(None, Some(0), Some(500), 0),
        ];
        let t = totals(&rows);
        assert_eq!(t.iter().map(|t| t.currency.as_str()).collect::<Vec<_>>(), ["USD", "EUR", ""]);
        let usd = &t[0];
        assert_eq!((usd.tournaments, usd.finished, usd.cashes), (4, 3, 1));
        assert_eq!((usd.cost, usd.prizes, usd.bounties, usd.profit), (4400, 3300, 1500, 400));
        assert_eq!(usd.roi, Some(400.0 / 4400.0 * 100.0));
        assert_eq!(usd.itm, Some(1.0 / 3.0 * 100.0));
        let eur = &t[1];
        assert_eq!((eur.cost, eur.profit, eur.roi, eur.itm), (200, -200, Some(-100.0), Some(0.0)));
        let free = &t[2];
        assert_eq!((free.profit, free.roi, free.itm), (500, None, Some(100.0)));
    }
}
