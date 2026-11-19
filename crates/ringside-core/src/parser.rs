//! Parser for PokerStars and GGPoker hand history text (Hold'em and Omaha, cash and
//! tournaments). The clients differ in the header and a few lines; the body is shared.

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

use crate::clock::local_time;
use crate::model::{Action, ActionKind, Amount, Finish, GameKind, Hand, Seat, Site, Street, gg_hand_id, gg_tournament_id};
use crate::tournament::BuyIn;
use crate::tournament::parse_buy_in;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// A valid hand of a game Ringside does not track (stud, draw, ...), or one the client
    /// cancelled before it was played.
    Unsupported(String),
    Malformed(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Unsupported(m) => write!(f, "unsupported game: {m}"),
            ParseError::Malformed(m) => write!(f, "malformed hand: {m}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// "Hand #" since 2014, "Game #" before.
static HAND_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:Hand|Game) #(\d+):").unwrap());
static TOURNAMENT_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Tournament #(\d+)").unwrap());
static VARIANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((?:5 Card |6 Card )?Omaha(?: Hi/Lo)?|Hold'em|Courchevel(?: Hi/Lo)?) (No Limit|Pot Limit|Limit)").unwrap()
});
static STAKES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^\d()/]*)([\d.,]+)/[^\d()/]*([\d.,]+)(?: ([A-Z]{3}))?\)").unwrap());
static TABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Table '(.+)' (\d+)-max.*?Seat #(\d+) is the button").unwrap());
static SEAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Seat (\d+): (.+) \(([^()]*?) in chips(?:, ([^()]*?) bounty)?[^()]*\)(.*)$").unwrap());
static CARDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]").unwrap());
static UNCALLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Uncalled bet \(([^)]+)\) returned to (.+)$").unwrap());
static GG_HAND_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Poker Hand #([A-Z]{2}\d+):").unwrap());
static GG_VARIANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:(?:5 Card |6 Card )?Omaha(?: Hi/Lo)?|Hold'em) (?:No Limit|Pot Limit|Limit)|PLO-[56]|PLO").unwrap()
});
/// Cash stakes may name a straddle as third blind: "($200/$400/$800)".
static GG_STAKES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\(([^\d()/]*)([\d.,]+)/[^\d()/]*([\d.,]+)(?:/[^\d()/]*[\d.,]+)?\)").unwrap()
});
static GG_CASHOUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(Receives Cashout|Pays Cashout Risk|pay premium of all-in insurance) \(([^)]+)\)").unwrap()
});
static TOTAL_POT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Total pot ([^ |]+).*?\| Rake ([^ |]+)").unwrap());

/// Splits a hand history file into the text of each hand.
pub fn split_hands(text: &str) -> Vec<&str> {
    let starts = hand_starts(text);
    starts
        .iter()
        .enumerate()
        .filter_map(|(i, &start)| {
            let end = starts.get(i + 1).copied().unwrap_or(text.len());
            // A hand the next one cut off in the middle of a line stays incomplete.
            let cut_off = end < text.len() && !text[..end].ends_with('\n');
            (!cut_off).then(|| text[start..end].trim_end())
        })
        .collect()
}

/// Byte length of the leading part of `text` that holds only fully written hands.
///
/// The client appends hands while a session runs, so the tail of a file may be a hand that
/// is still being written. A hand is complete once its summary is followed by a blank line
/// or by the next hand.
pub fn complete_prefix_len(text: &str) -> usize {
    let starts = hand_starts(text);
    let mut complete = 0;
    for (i, &start) in starts.iter().enumerate() {
        match starts.get(i + 1) {
            Some(&next) => complete = next,
            None => {
                let tail = &text[start..];
                if let Some(summary) = tail.find("*** SUMMARY ***") {
                    let after = &tail[summary..];
                    let blank_end = ["\n\n", "\n\r\n"]
                        .iter()
                        .filter_map(|p| after.find(p).map(|i| i + p.len()))
                        .min();
                    if let Some(end) = blank_end {
                        let padding = after[end..].len() - after[end..].trim_start().len();
                        complete = start + summary + end + padding;
                    }
                }
            }
        }
    }
    complete
}

fn hand_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start_matches('\u{feff}');
        if (trimmed.starts_with("PokerStars ") && (trimmed.contains("Hand #") || trimmed.contains("Game #"))) || trimmed.starts_with("Poker Hand #") {
            starts.push(offset + (line.len() - trimmed.len()));
        } else if let Some(at) = ["PokerStars Hand #", "PokerStars Game #", "PokerStars Zoom Hand #"].iter().filter_map(|h| line.find(h)).min() {
            // The client stopped in the middle of a hand and went on with the next one in
            // the same line.
            starts.push(offset + at);
        }
        offset += line.len();
    }
    starts
}

/// Clients until 2007 wrote no "Uncalled bet" line when a bet was called for less: the part
/// nobody matched went back all the same. When the bets exceed the pot by exactly what the
/// biggest exceeds the second biggest, that part is returned.
fn return_unmatched(hand: &mut Hand) {
    let mut put: Vec<(&str, Amount)> = Vec::new();
    for a in &hand.actions {
        match put.iter_mut().find(|(p, _)| *p == a.player) {
            Some((_, x)) => *x += a.amount,
            None => put.push((&a.player, a.amount)),
        }
    }
    put.sort_by_key(|(_, x)| std::cmp::Reverse(*x));
    let ([(top, most), (_, next), ..], true) = (put.as_slice(), hand.total_pot > 0) else { return };
    let over = put.iter().map(|(_, x)| x).sum::<Amount>() - hand.total_pot;
    if over > 0 && over == most - next {
        let top = top.to_string();
        hand.returned.push((top, over));
    }
}

pub fn parse_amount(raw: &str) -> Option<Amount> {
    let cleaned: String = raw.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    if cleaned.is_empty() {
        return None;
    }
    let (int, frac) = cleaned.split_once('.').unwrap_or((&cleaned, ""));
    let int: i64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let frac = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>().ok()? * 10,
        _ => frac[..2].parse::<i64>().ok()?,
    };
    int.checked_mul(100)?.checked_add(frac)
}

pub(crate) fn currency_code(symbol: &str, code: Option<&str>) -> Option<String> {
    if let Some(code) = code {
        return Some(code.to_string());
    }
    match symbol.trim() {
        "$" => Some("USD".into()),
        "€" => Some("EUR".into()),
        "£" => Some("GBP".into()),
        "₹" => Some("INR".into()),
        "¥" => Some("CNY".into()),
        _ => None,
    }
}

fn malformed(msg: &str) -> ParseError {
    ParseError::Malformed(msg.to_string())
}

/// What the first line of a hand says.
struct Head {
    id: u64,
    site: Site,
    game: GameKind,
    tournament_id: Option<u64>,
    tournament_name: Option<String>,
    variant: String,
    buy_in: Option<BuyIn>,
    small_blind: Amount,
    big_blind: Amount,
    currency: Option<String>,
    played_at: String,
}

/// "5:00:45" becomes "05:00:45", so timestamps sort and slice as text.
fn played_at(header: &str) -> String {
    local_time(header).unwrap_or_default()
}

fn pokerstars_head(header: &str) -> Result<Head, ParseError> {
    let id: u64 = HAND_ID
        .captures(header)
        .and_then(|c| c[1].parse().ok())
        .ok_or_else(|| malformed("missing hand id"))?;
    let tournament = TOURNAMENT_ID.captures(header);
    let tournament_id: Option<u64> = tournament.as_ref().and_then(|c| c[1].parse().ok());
    let game = if tournament_id.is_some() { GameKind::Tournament } else { GameKind::Cash };
    let variant_match = VARIANT.captures(header).ok_or_else(|| ParseError::Unsupported(header.to_string()))?;
    let variant = format!("{} {}", &variant_match[1], &variant_match[2]);
    // "Tournament #123, $10+$10+$2 USD Hold'em No Limit": the buy-in sits between id and game.
    let buy_in = tournament.and_then(|c| {
        let (from, to) = (c.get(0)?.end(), variant_match.get(0)?.start());
        header.get(from..to)?.strip_prefix(',').and_then(parse_buy_in)
    });
    let stakes = STAKES.captures(header).ok_or_else(|| malformed("missing stakes"))?;
    let small_blind = parse_amount(&stakes[2]).ok_or_else(|| malformed("small blind"))?;
    let big_blind = parse_amount(&stakes[3]).ok_or_else(|| malformed("big blind"))?;
    let currency = match game {
        GameKind::Cash => currency_code(&stakes[1], stakes.get(4).map(|m| m.as_str())),
        GameKind::Tournament => None,
    };
    Ok(Head {
        id,
        site: Site::PokerStars,
        game,
        tournament_id,
        tournament_name: None,
        variant,
        buy_in,
        small_blind,
        big_blind,
        currency,
        played_at: played_at(header),
    })
}

/// A GGPoker header, for tournaments `Poker Hand #TM599808286: Tournament #25313424, L-03: $52.50
/// Bounty Hunters Hold'em No Limit - Level1(100/200) - 2021/04/04 16:06:27`, for cash games
/// `Poker Hand #HD2777175730: Hold'em No Limit ($0.01/$0.02) - 2026/03/15 18:12:42`. Tournament
/// names may themselves name a game, so the game is the last one written.
fn ggpoker_head(header: &str) -> Result<Head, ParseError> {
    let number = GG_HAND_ID.captures(header).ok_or_else(|| malformed("missing hand id"))?;
    let number = number.get(1).map(|m| m.as_str()).unwrap_or_default();
    // Short Deck and All-in or Fold play by other rules than any model Ringside has.
    if number.starts_with("SD") || number.starts_with("AF") {
        return Err(ParseError::Unsupported(header.to_string()));
    }
    let id = gg_hand_id(number).ok_or_else(|| malformed("hand id"))?;
    let variant_match = GG_VARIANT.find_iter(header).last().ok_or_else(|| ParseError::Unsupported(header.to_string()))?;
    let variant = match variant_match.as_str() {
        "PLO" => "Omaha Pot Limit".to_string(),
        "PLO-5" => "5 Card Omaha Pot Limit".to_string(),
        "PLO-6" => "6 Card Omaha Pot Limit".to_string(),
        other => other.to_string(),
    };
    let tournament = TOURNAMENT_ID.captures(header);
    let tournament_id = match &tournament {
        Some(c) => Some(c[1].parse().ok().and_then(gg_tournament_id).ok_or_else(|| malformed("tournament id"))?),
        None => None,
    };
    let tournament_name = tournament.and_then(|c| {
        let name = header.get(c.get(0)?.end()..variant_match.start())?.trim_start_matches(',').trim();
        (!name.is_empty()).then(|| name.to_string())
    });
    let game = if tournament_id.is_some() { GameKind::Tournament } else { GameKind::Cash };
    let stakes = GG_STAKES
        .captures(&header[variant_match.end()..])
        .ok_or_else(|| malformed("missing stakes"))?;
    let small_blind = parse_amount(&stakes[2]).ok_or_else(|| malformed("small blind"))?;
    let big_blind = parse_amount(&stakes[3]).ok_or_else(|| malformed("big blind"))?;
    let currency = match game {
        GameKind::Cash => currency_code(&stakes[1], None),
        GameKind::Tournament => None,
    };
    Ok(Head {
        id,
        site: Site::GgPoker,
        game,
        tournament_id,
        tournament_name,
        variant,
        buy_in: None,
        small_blind,
        big_blind,
        currency,
        played_at: played_at(header),
    })
}

pub fn parse_hand(text: &str) -> Result<Hand, ParseError> {
    let mut lines = text.lines().map(|l| l.trim_start_matches('\u{feff}').trim_end());
    let header = lines.next().ok_or_else(|| malformed("empty"))?;
    // The blinds go back: nothing was played.
    if text.lines().any(|l| l.trim_end() == "Hand cancelled") {
        return Err(ParseError::Unsupported("hand cancelled".into()));
    }
    let head = if header.starts_with("Poker Hand #") { ggpoker_head(header)? } else { pokerstars_head(header)? };
    let Head { id, site, game, tournament_id, tournament_name, variant, buy_in, small_blind, big_blind, currency, played_at } =
        head;
    let gg = site == Site::GgPoker;

    let table_line = lines.next().ok_or_else(|| malformed("missing table line"))?;
    let table = TABLE.captures(table_line).ok_or_else(|| malformed("table line"))?;
    let table_name = table[1].to_string();
    let max_seats: u8 = table[2].parse().map_err(|_| malformed("max seats"))?;
    let button_seat: u8 = table[3].parse().map_err(|_| malformed("button seat"))?;

    let mut seats = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for line in lines.by_ref() {
        match SEAT.captures(line) {
            Some(c) => seats.push(Seat {
                seat: c[1].parse().map_err(|_| malformed("seat number"))?,
                name: c[2].to_string(),
                stack: parse_amount(&c[3]).ok_or_else(|| malformed("stack"))?,
                bounty: c.get(4).and_then(|b| parse_amount(b.as_str())),
                sitting_out: c[5].contains("sitting out") || c[5].contains("out of hand"),
            }),
            None => {
                rest.push(line);
                break;
            }
        }
    }
    if seats.is_empty() {
        return Err(malformed("no seats"));
    }
    rest.extend(lines);
    let bounty_currency = text
        .lines()
        .find_map(|l| SEAT.captures(l).and_then(|c| c.get(4).map(|b| b.as_str().to_string())))
        .and_then(|b| currency_code(b.trim_end_matches(|ch: char| ch.is_ascii_digit() || ch == '.' || ch == ','), None));

    // Longest names first so "Bob 2" is not matched as "Bob".
    let mut names: Vec<String> = seats.iter().map(|s| s.name.clone()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let names_by_length = names.clone();
    let find_player = |line: &str, sep: &str| -> Option<(String, String)> {
        names.iter().find_map(|n| {
            line.strip_prefix(n.as_str())
                .and_then(|r| r.strip_prefix(sep))
                .map(|r| (n.clone(), r.to_string()))
        })
    };

    let mut hand = Hand {
        id,
        site,
        game,
        tournament_id,
        tournament_name,
        variant,
        table_name,
        max_seats,
        button_seat,
        small_blind,
        big_blind,
        currency,
        played_at,
        seats,
        hero: None,
        hero_cards: Vec::new(),
        board: Vec::new(),
        actions: Vec::new(),
        returned: Vec::new(),
        shown: Vec::new(),
        collected: Vec::new(),
        showdown: false,
        total_pot: 0,
        rake: 0,
        cashouts: Vec::new(),
        bounties_won: Vec::new(),
        bounty_currency,
        buy_in,
        finishes: Vec::new(),
    };

    let mut street = Street::Preflop;
    let mut street_put: HashMap<String, Amount> = HashMap::new();
    let mut in_summary = false;
    let mut second_board = false;

    for line in rest {
        if line.is_empty() {
            continue;
        }
        if let Some(marker) = line.strip_prefix("*** ") {
            let new_street = if marker.starts_with("SUMMARY") {
                in_summary = true;
                None
            } else if !gg && (marker.starts_with("SHOW DOWN") || marker.starts_with("FIRST SHOW DOWN")) {
                hand.showdown = true;
                None
            } else if marker.starts_with("SECOND ") {
                second_board = true;
                None
            } else if marker.starts_with("PRE-FLOP") {
                // Courchevel deals the first board card before preflop betting.
                if let Some(c) = CARDS.captures(line) {
                    hand.board.extend(c[1].split_whitespace().map(String::from));
                }
                None
            } else if marker.contains("FLOP ***") {
                Some(Street::Flop)
            } else if marker.contains("TURN ***") {
                Some(Street::Turn)
            } else if marker.contains("RIVER ***") {
                Some(Street::River)
            } else {
                None
            };
            if let Some(s) = new_street {
                street = s;
                street_put.clear();
                if !second_board
                    && let Some(last) = CARDS.captures_iter(line).last() {
                        hand.board.extend(last[1].split_whitespace().map(String::from));
                    }
            }
            continue;
        }

        if in_summary {
            // "Total pot $4.64 | Rake $0.22 | Jackpot $0.15 | Bingo $0 | Fortune $0 | Tax $0": every
            // fee leaves the pot, so all of them count as rake.
            if gg && let Some(rest) = line.strip_prefix("Total pot ") {
                let mut parts = rest.split('|');
                hand.total_pot = parts.next().and_then(parse_amount).unwrap_or(0);
                hand.rake = parts.filter_map(|p| p.split_whitespace().last().and_then(parse_amount)).sum();
            } else if let Some(c) = TOTAL_POT.captures(line) {
                hand.total_pot = parse_amount(&c[1]).unwrap_or(0);
                hand.rake = parse_amount(&c[2]).unwrap_or(0);
            } else if let Some(cards) = line.strip_prefix("Board ") {
                if hand.board.is_empty()
                    && let Some(c) = CARDS.captures(cards) {
                        hand.board = c[1].split_whitespace().map(String::from).collect();
                    }
            } else if line.starts_with("Seat ") && line.contains(" mucked [")
                && let (Some(name), Some(c)) = (summary_seat_name(&hand, line), CARDS.captures(line)) {
                    add_shown(&mut hand, &name, &c[1]);
                }
            continue;
        }

        if let Some(rest) = line.strip_prefix("Dealt to ") {
            if let Some((name, cards)) = find_player(rest, " [") {
                hand.hero = Some(name);
                hand.hero_cards = cards.trim_end_matches(']').split_whitespace().map(String::from).collect();
            }
            continue;
        }
        if let Some(c) = UNCALLED.captures(line) {
            let amount = parse_amount(&c[1]).ok_or_else(|| malformed("uncalled bet"))?;
            hand.returned.push((c[2].to_string(), amount));
            continue;
        }
        if let Some((name, rest)) = find_player(line, " collected ") {
            let amount = parse_amount(rest.split(" from").next().unwrap_or(&rest))
                .ok_or_else(|| malformed("collected amount"))?;
            hand.collected.push((name, amount));
            continue;
        }
        // Without a place the line is just "X finished the tournament"; a paid finish always
        // names place and prize, so that means out of the money.
        if let Some((name, rest)) = find_player(line, " finished the tournament") {
            let rest = rest.strip_prefix(" in ").unwrap_or("");
            let place = rest.split(|c: char| !c.is_ascii_digit()).next().and_then(|p| p.parse().ok());
            let prize = rest.split_once(" and received ").map(|(_, p)| p.trim_end_matches('.'));
            add_finish(&mut hand, name, place, prize);
            continue;
        }
        if let Some((name, rest)) = find_player(line, " wins the tournament and receives ") {
            add_finish(&mut hand, name, Some(1), rest.split_whitespace().next());
            continue;
        }
        // "A, B split the $50 bounty for eliminating C"
        if let Some((names, rest)) = line.split_once(" split the ")
            && rest.contains(" bounty for eliminating ")
            && let Some(amount) = rest.split_whitespace().next().and_then(parse_amount)
        {
            // Names may themselves contain ", ", so match against the seated players.
            if let Some(winners) = split_names(names, &names_by_length) {
                let share = amount / winners.len() as Amount;
                let remainder = amount - share * winners.len() as Amount;
                for (i, w) in winners.iter().enumerate() {
                    hand.bounties_won.push((w.to_string(), share + if i == 0 { remainder } else { 0 }));
                }
            }
            continue;
        }
        // "A wins $0.31 for eliminating B and their own bounty increases ..." (progressive)
        // "A wins the $5 bounty for eliminating B" (classic knockout)
        if let Some((name, rest)) = find_player(line, " wins ")
            && rest.contains(" for eliminating ")
        {
            let amount = rest.trim_start_matches("the ").split_whitespace().next().and_then(parse_amount);
            if let Some(amount) = amount {
                hand.bounties_won.push((name, amount));
            }
            continue;
        }
        // "Hero cashed out the hand for $56.99 | Cash Out Fee $0.58": that is what the player
        // gets instead of the pot.
        if let Some((name, rest)) = find_player(line, " cashed out the hand for ") {
            if let Some(amount) = rest.split_whitespace().next().and_then(parse_amount) {
                hand.cashouts.push((name, amount));
            }
            continue;
        }
        let Some((player, rest)) = find_player(line, ": ") else {
            continue;
        };
        if let Some(cards) = rest.strip_prefix("shows [") {
            let cards = cards.split(']').next().unwrap_or("");
            // GGPoker lets a player who folded show a single card; that is no hand.
            let whole = !gg || hand.hero_cards.is_empty() || cards.split_whitespace().count() == hand.hero_cards.len();
            if whole {
                add_shown(&mut hand, &player, cards);
            }
            continue;
        }
        if gg && let Some(c) = GG_CASHOUT.captures(&rest) {
            let amount = parse_amount(&c[2]).ok_or_else(|| malformed("cashout"))?;
            hand.cashouts.push((player, if &c[1] == "Receives Cashout" { amount } else { -amount }));
            continue;
        }
        if let Some(action) = parse_action(&player, &rest, street, &mut street_put, big_blind) {
            hand.actions.push(action);
        }
    }

    if !gg && hand.returned.is_empty() {
        return_unmatched(&mut hand);
    }
    if gg {
        // GGPoker writes a showdown marker into every hand: a showdown is two players left.
        let folded = |name: &str| hand.actions.iter().any(|a| a.player == name && a.kind == ActionKind::Fold);
        hand.showdown = hand.active_players().filter(|s| !folded(&s.name)).count() >= 2;
    }
    Ok(hand)
}

/// `prize` is the text after "received", e.g. "$5.40" or "2700"; a ticket counts as no money.
fn add_finish(hand: &mut Hand, player: String, place: Option<i64>, prize: Option<&str>) {
    let prize = prize.filter(|p| p.starts_with(|c: char| !c.is_ascii_alphabetic()));
    let symbol: String = prize.unwrap_or("").chars().take_while(|c| !c.is_ascii_digit()).collect();
    hand.finishes.push(Finish {
        player,
        place,
        prize: prize.and_then(parse_amount),
        currency: currency_code(&symbol, None),
    });
}

/// Splits "A, B, C" into seated player names, longest match first.
fn split_names<'a>(list: &str, names: &'a [String]) -> Option<Vec<&'a str>> {
    let mut out = Vec::new();
    let mut rest = list;
    while !rest.is_empty() {
        let name = names.iter().find(|n| {
            rest.strip_prefix(n.as_str()).is_some_and(|after| after.is_empty() || after.starts_with(", "))
        })?;
        out.push(name.as_str());
        rest = rest[name.len()..].strip_prefix(", ").unwrap_or("");
    }
    Some(out)
}

fn summary_seat_name(hand: &Hand, line: &str) -> Option<String> {
    let rest = line.strip_prefix("Seat ")?;
    let (_, rest) = rest.split_once(": ")?;
    hand.seats
        .iter()
        .filter(|s| rest.starts_with(&format!("{} ", s.name)))
        .max_by_key(|s| s.name.len())
        .map(|s| s.name.clone())
}

fn add_shown(hand: &mut Hand, player: &str, cards: &str) {
    if hand.shown.iter().any(|(p, _)| p == player) {
        return;
    }
    let cards: Vec<String> = cards.split_whitespace().map(String::from).collect();
    if !cards.is_empty() {
        hand.shown.push((player.to_string(), cards));
    }
}

fn parse_action(
    player: &str,
    rest: &str,
    street: Street,
    street_put: &mut HashMap<String, Amount>,
    big_blind: Amount,
) -> Option<Action> {
    let all_in = rest.ends_with("and is all-in");
    let first_amount = |s: &str| s.split_whitespace().next().and_then(parse_amount);
    let put_before = street_put.get(player).copied().unwrap_or(0);

    let (kind, amount, to, live) = if let Some(r) = rest.strip_prefix("posts small & big blinds ") {
        let total = first_amount(r)?;
        (ActionKind::PostBoth, total, None, total.min(big_blind))
    } else if let Some(r) = rest.strip_prefix("posts small blind ") {
        let a = first_amount(r)?;
        (ActionKind::PostSmallBlind, a, None, a)
    } else if let Some(r) = rest.strip_prefix("posts big blind ") {
        let a = first_amount(r)?;
        (ActionKind::PostBigBlind, a, None, a)
    } else if let Some(r) = rest.strip_prefix("straddle ").or_else(|| rest.strip_prefix("posts third blind ")) {
        let a = first_amount(r)?;
        (ActionKind::PostStraddle, a, None, a)
    } else if let Some(r) = rest.strip_prefix("posts missed blind ") {
        (ActionKind::PostDead, first_amount(r)?, None, 0)
    } else if let Some(r) = rest.strip_prefix("posts the ante ") {
        (ActionKind::PostAnte, first_amount(r)?, None, 0)
    } else if rest.starts_with("folds") {
        (ActionKind::Fold, 0, None, 0)
    } else if rest.starts_with("checks") {
        (ActionKind::Check, 0, None, 0)
    } else if let Some(r) = rest.strip_prefix("calls ") {
        let a = first_amount(r)?;
        (ActionKind::Call, a, None, a)
    } else if let Some(r) = rest.strip_prefix("bets ") {
        let a = first_amount(r)?;
        (ActionKind::Bet, a, None, a)
    } else {
        let r = rest.strip_prefix("raises ")?;
        let (_, after) = r.split_once(" to ")?;
        let to = first_amount(after)?;
        let added = to - put_before;
        (ActionKind::Raise, added, Some(to), added)
    };

    *street_put.entry(player.to_string()).or_default() += live;
    Some(Action { street, player: player.to_string(), kind, amount, to, all_in })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts() {
        assert_eq!(parse_amount("$0.05"), Some(5));
        assert_eq!(parse_amount("€1,234.5"), Some(123450));
        assert_eq!(parse_amount("1500"), Some(150000));
        assert_eq!(parse_amount("$1"), Some(100));
        assert_eq!(parse_amount("chips"), None);
        assert_eq!(parse_amount("99999999999999999999"), None, "too large for i64");
        assert_eq!(parse_amount("92233720368547758.07"), Some(i64::MAX));
        assert_eq!(parse_amount("92233720368547758.08"), None, "overflows when adding cents");
        assert_eq!(parse_amount("92233720368547759"), None, "overflows when scaling");
    }

    #[test]
    fn incomplete_tail_is_not_consumed() {
        let done = "PokerStars Hand #1: x\nTable\n*** SUMMARY ***\nTotal pot 1\n\n\n";
        let partial = "PokerStars Hand #2: x\nTable\n*** HOLE CARDS ***\n";
        let text = format!("{done}{partial}");
        assert_eq!(complete_prefix_len(&text), done.len());
        assert_eq!(complete_prefix_len(done), done.len());
        assert_eq!(complete_prefix_len(partial), 0);
    }

    #[test]
    fn crlf_complete_hand() {
        let done = "PokerStars Hand #1: x\r\nTable\r\n*** SUMMARY ***\r\nTotal pot 1\r\n\r\n";
        assert_eq!(complete_prefix_len(done), done.len());
    }
}
