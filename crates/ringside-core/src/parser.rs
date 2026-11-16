//! Parser for PokerStars hand history text (Hold'em and Omaha, cash and tournaments).

use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Action, ActionKind, Amount, GameKind, Hand, Seat, Street};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// A valid hand of a game Ringside does not track (stud, draw, ...).
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

static HAND_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Hand #(\d+):").unwrap());
static TOURNAMENT_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"Tournament #(\d+)").unwrap());
static VARIANT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"((?:5 Card |6 Card )?Omaha(?: Hi/Lo)?|Hold'em|Courchevel(?: Hi/Lo)?) (No Limit|Pot Limit|Limit)").unwrap()
});
static STAKES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^\d()/]*)([\d.,]+)/[^\d()/]*([\d.,]+)(?: ([A-Z]{3}))?\)").unwrap());
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4}/\d{2}/\d{2} \d{1,2}:\d{2}:\d{2})").unwrap());
static TABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Table '(.+)' (\d+)-max.*?Seat #(\d+) is the button").unwrap());
static SEAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Seat (\d+): (.+) \(([^()]*?) in chips[^()]*\)(.*)$").unwrap());
static CARDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]*)\]").unwrap());
static UNCALLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Uncalled bet \(([^)]+)\) returned to (.+)$").unwrap());
static TOTAL_POT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Total pot ([^ |]+).*?\| Rake ([^ |]+)").unwrap());

/// Splits a hand history file into the text of each hand.
pub fn split_hands(text: &str) -> Vec<&str> {
    let starts = hand_starts(text);
    starts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = starts.get(i + 1).copied().unwrap_or(text.len());
            text[start..end].trim_end()
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
        if trimmed.starts_with("PokerStars ") && trimmed.contains("Hand #") {
            starts.push(offset + (line.len() - trimmed.len()));
        }
        offset += line.len();
    }
    starts
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
    Some(int * 100 + frac)
}

fn currency_code(symbol: &str, code: Option<&str>) -> Option<String> {
    if let Some(code) = code {
        return Some(code.to_string());
    }
    match symbol.trim() {
        "$" => Some("USD".into()),
        "€" => Some("EUR".into()),
        "£" => Some("GBP".into()),
        _ => None,
    }
}

fn malformed(msg: &str) -> ParseError {
    ParseError::Malformed(msg.to_string())
}

pub fn parse_hand(text: &str) -> Result<Hand, ParseError> {
    let mut lines = text.lines().map(|l| l.trim_start_matches('\u{feff}').trim_end());
    let header = lines.next().ok_or_else(|| malformed("empty"))?;

    let id: u64 = HAND_ID
        .captures(header)
        .and_then(|c| c[1].parse().ok())
        .ok_or_else(|| malformed("missing hand id"))?;
    let tournament_id: Option<u64> = TOURNAMENT_ID.captures(header).and_then(|c| c[1].parse().ok());
    let game = if tournament_id.is_some() { GameKind::Tournament } else { GameKind::Cash };
    let variant = VARIANT
        .captures(header)
        .map(|c| format!("{} {}", &c[1], &c[2]))
        .ok_or_else(|| ParseError::Unsupported(header.to_string()))?;
    let stakes = STAKES.captures(header).ok_or_else(|| malformed("missing stakes"))?;
    let small_blind = parse_amount(&stakes[2]).ok_or_else(|| malformed("small blind"))?;
    let big_blind = parse_amount(&stakes[3]).ok_or_else(|| malformed("big blind"))?;
    let currency = match game {
        GameKind::Cash => currency_code(&stakes[1], stakes.get(4).map(|m| m.as_str())),
        GameKind::Tournament => None,
    };
    let played_at = DATE.captures(header).map(|c| c[1].to_string()).unwrap_or_default();

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
                sitting_out: c[4].contains("sitting out") || c[4].contains("out of hand"),
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

    // Longest names first so "Bob 2" is not matched as "Bob".
    let mut names: Vec<String> = seats.iter().map(|s| s.name.clone()).collect();
    names.sort_by_key(|n| std::cmp::Reverse(n.len()));
    let find_player = |line: &str, sep: &str| -> Option<(String, String)> {
        names.iter().find_map(|n| {
            line.strip_prefix(n.as_str())
                .and_then(|r| r.strip_prefix(sep))
                .map(|r| (n.clone(), r.to_string()))
        })
    };

    let mut hand = Hand {
        id,
        game,
        tournament_id,
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
            } else if marker.starts_with("SHOW DOWN") || marker.starts_with("FIRST SHOW DOWN") {
                hand.showdown = true;
                None
            } else if marker.starts_with("SECOND ") {
                second_board = true;
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
            if let Some(c) = TOTAL_POT.captures(line) {
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
        let Some((player, rest)) = find_player(line, ": ") else {
            continue;
        };
        if let Some(cards) = rest.strip_prefix("shows [") {
            add_shown(&mut hand, &player, cards.split(']').next().unwrap_or(""));
            continue;
        }
        if let Some(action) = parse_action(&player, &rest, street, &mut street_put, big_blind) {
            hand.actions.push(action);
        }
    }

    Ok(hand)
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
    } else if let Some(r) = rest.strip_prefix("raises ") {
        let (_, after) = r.split_once(" to ")?;
        let to = first_amount(after)?;
        let added = to - put_before;
        (ActionKind::Raise, added, Some(to), added)
    } else {
        return None;
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
