//! What may never leave the computer with a shared solution: PokerStars forbids shared
//! hand databases, so uploads carry the abstract spot only. Tests of every upload path
//! run their bodies through [`personal_fields`].

use serde_json::Value;

/// Field names (or parts of them) that would tie an upload to a person, a hand, a
/// tournament, a time or money.
pub const FORBIDDEN: &[&str] = &[
    "name", "player", "hero", "user", "nick", "screen", "alias", "hand_id", "hand_no", "handid", "tournament", "tourney",
    "table", "site", "room", "time", "date", "stamp", "created", "played", "buyin", "buy_in", "money", "dollar", "usd",
    "eur", "currency", "cash", "prize", "bounty", "amount", "chips", "seat_name", "position_name", "ip", "email",
];

/// Paths of fields whose names contain a forbidden word, and string values that look
/// like a hand or tournament number; empty for an anonymous body.
pub fn personal_fields(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk(v, "", &mut out);
    out
}

fn walk(v: &Value, path: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let lower = k.to_ascii_lowercase();
                let parts: Vec<&str> = lower.split('_').collect();
                let hit = FORBIDDEN.iter().any(|f| if f.contains('_') { lower.contains(f) } else { parts.contains(f) || lower == *f });
                let here = format!("{path}.{k}");
                if hit {
                    out.push(here.clone());
                }
                walk(x, &here, out);
            }
        }
        Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| walk(x, &format!("{path}[{i}]"), out)),
        // PokerStars hand and tournament numbers have nine digits and more.
        Value::String(s) if s.len() >= 9 && s.chars().filter(|c| c.is_ascii_digit()).count() >= 9 && !s.contains('=') => {
            out.push(format!("{path} = {s:.20}"))
        }
        Value::Number(n) if n.as_u64().is_some_and(|x| x >= 100_000_000) => out.push(format!("{path} = {n}")),
        _ => {}
    }
}

/// Panics with the offending fields unless `body` is anonymous.
pub fn assert_anonymous(body: &Value) {
    let found = personal_fields(body);
    assert!(found.is_empty(), "personal fields in an upload: {found:?}\n{body}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn personal_fields_are_found() {
        let bad = json!({"input": {"player_name": "x", "board": "AsKs2h", "hand_id": 1, "deep": [{"buyin": 5}]}, "hand": "260000000002"});
        let found = personal_fields(&bad);
        assert_eq!(found.len(), 4, "{found:?}");
        assert!(personal_fields(&json!({"kind": "spot", "input": {"pot_cbb": 550, "stack_bb": 40, "line": [[6, "raise", 2300]]}})).is_empty());
    }
}
