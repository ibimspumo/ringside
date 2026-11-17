//! The calculators the model may call. Every input is checked first; a bad input comes
//! back as a German error the model can read and correct, never as a crash.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use ringside_engine::{
    Card, CardSet, Combo, HandClass, Method, PushFoldParams, PushFoldResult, Range, equity, icm, outs, solve_push_fold,
};
use serde_json::{Value, json};

use crate::concepts::CONCEPTS;

pub const REPORT: &str = "report";

/// Tool definitions in the OpenAI function format.
pub fn definitions() -> Value {
    let concept_ids: Vec<&str> = CONCEPTS.iter().map(|c| c.id).collect();
    let concept_help: Vec<String> = CONCEPTS.iter().map(|c| format!("{} = {}", c.id, c.name)).collect();
    let f = |name: &str, description: &str, parameters: Value| json!({"type": "function", "function": {"name": name, "description": description, "parameters": parameters}});
    json!([
        f(
            "equity",
            "Exakte Equity (Anteil am Pot in Prozent) von 2 bis 6 Spielern bei einem All-in bis zum River. Jeder Spieler ist eine konkrete Hand ('AhKs') oder eine Range in Standardnotation ('22+,A2s+,KTo+'). board: 0, 3, 4 oder 5 Karten, z. B. '9c4h2s'.",
            json!({"type": "object", "properties": {
                "players": {"type": "array", "items": {"type": "string"}, "minItems": 2, "maxItems": 6},
                "board": {"type": "string", "description": "Leer für preflop"}
            }, "required": ["players"]})
        ),
        f(
            "pot_odds",
            "Benötigte Equity für einen Call. pot_before_call ist alles, was schon im Pot liegt, einschließlich der Bet, die du callen sollst. Beträge in bb.",
            json!({"type": "object", "properties": {
                "call": {"type": "number"},
                "pot_before_call": {"type": "number"}
            }, "required": ["call", "pot_before_call"]})
        ),
        f(
            "push_fold_nash",
            "Heads-up-Push/Fold-Gleichgewicht Small Blind gegen Big Blind in Chip-EV, nur gültig, wenn alle vor dem Small Blind gefoldet haben. Liefert für eine Hand, wie oft der Small Blind sie schiebt und der Big Blind sie gegen einen Shove callt, und den EV jeder Option in bb.",
            json!({"type": "object", "properties": {
                "stack_bb": {"type": "number", "description": "Effektiver Stack in bb vor dem Posten, 1 bis 100"},
                "hand": {"type": "string", "description": "Handklasse wie 'K6o', 'A5s', '77' oder Karten wie 'Kh6d'"},
                "ante_bb": {"type": "number", "description": "Ante, die SB und BB je zahlen, in bb"},
                "dead_bb": {"type": "number", "description": "Totes Geld im Pot in bb: Antes der anderen, Big-Blind-Ante"}
            }, "required": ["stack_bb", "hand"]})
        ),
        f(
            "icm",
            "Turnier-Equity nach Malmuth-Harville: erwartetes Preisgeld jedes Spielers aus Stacks und Auszahlungen (Platz 1 zuerst). 2 bis 10 Spieler.",
            json!({"type": "object", "properties": {
                "stacks": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 10},
                "payouts": {"type": "array", "items": {"type": "number"}, "minItems": 1}
            }, "required": ["stacks", "payouts"]})
        ),
        f(
            "outs",
            "Outs am Flop oder Turn: Karten, nach denen du mit der nächsten Karte vorne liegst (gegen eine bekannte Hand) oder die nächstbessere Kategorie triffst (ohne Gegnerhand).",
            json!({"type": "object", "properties": {
                "hero": {"type": "string", "description": "Zwei Karten, z. B. 'AhKh'"},
                "board": {"type": "string", "description": "3 oder 4 Karten"},
                "villain": {"type": "string", "description": "Optional: zwei Karten des Gegners"}
            }, "required": ["hero", "board"]})
        ),
        f(
            REPORT,
            "Den fertigen Bericht abgeben. Ganz am Ende genau einmal aufrufen.",
            json!({"type": "object", "properties": {
                "summary": {"type": "string", "description": "2 bis 4 Sätze Gesamteindruck, Du-Form."},
                "spots": {"type": "array", "minItems": 1, "maxItems": 12, "items": {"type": "object", "properties": {
                    "hand": {"type": "string", "description": "Kennung der Hand, z. B. 'H7'"},
                    "action": {"type": "string", "description": "Was du gespielt hast, kurz."},
                    "verdict": {"type": "string", "enum": ["stark", "ok", "fraglich", "fehler"]},
                    "better": {"type": "string", "description": "Bessere Alternative, leer wenn keine."},
                    "why": {"type": "string", "description": "1 bis 3 Sätze Begründung."},
                    "numbers": {"type": "array", "items": {"type": "object", "properties": {
                        "label": {"type": "string"},
                        "value": {"type": "string"},
                        "source": {"type": "string", "enum": ["berechnet", "einschätzung"]}
                    }, "required": ["label", "value", "source"]}},
                    "concept": {"type": "string", "enum": concept_ids, "description": concept_help.join("; ")}
                }, "required": ["hand", "action", "verdict", "why", "numbers", "concept"]}},
                "focus": {"type": "array", "items": {"type": "string"}, "minItems": 1, "maxItems": 3, "description": "1 bis 3 Schwerpunkte zum Üben."}
            }, "required": ["summary", "spots", "focus"]})
        ),
    ])
}

/// Runs a calculator. `Err` is a German message for the model.
pub fn run(name: &str, args: &Value) -> Result<Value, String> {
    if !args.is_object() {
        return Err("Die Argumente müssen ein JSON-Objekt sein.".into());
    }
    match name {
        "equity" => equity_tool(args),
        "pot_odds" => pot_odds(args),
        "push_fold_nash" => push_fold(args),
        "icm" => icm_tool(args),
        "outs" => outs_tool(args),
        other => Err(format!(
            "Unbekanntes Werkzeug '{other}'. Verfügbar: equity, pot_odds, push_fold_nash, icm, outs, report."
        )),
    }
}

fn number(args: &Value, key: &str) -> Result<f64, String> {
    let v = args.get(key).ok_or_else(|| format!("'{key}' fehlt."))?;
    let n = match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().replace(',', ".").parse().ok(),
        _ => None,
    };
    n.filter(|n| n.is_finite()).ok_or_else(|| format!("'{key}' muss eine Zahl sein."))
}

fn optional_number(args: &Value, key: &str) -> Result<f64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(0.0),
        Some(_) => number(args, key),
    }
}

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key).and_then(Value::as_str).ok_or_else(|| format!("'{key}' muss ein Text sein."))
}

fn numbers(args: &Value, key: &str) -> Result<Vec<f64>, String> {
    let list =
        args.get(key).and_then(Value::as_array).ok_or_else(|| format!("'{key}' muss eine Liste von Zahlen sein."))?;
    list.iter()
        .map(|v| {
            v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| format!("'{key}' enthält etwas, das keine Zahl ist."))
        })
        .collect()
}

fn round(v: f64, digits: i32) -> f64 {
    let f = 10f64.powi(digits);
    (v * f).round() / f
}

fn cards(s: &str, what: &str) -> Result<Vec<Card>, String> {
    CardSet::parse(s)
        .map(CardSet::to_vec)
        .map_err(|_| format!("{what} '{s}' sind keine gültigen Karten (Format 'Ah', 'Td', '9c')."))
}

fn two_cards(s: &str, what: &str) -> Result<[Card; 2], String> {
    let v = cards(s, what)?;
    <[Card; 2]>::try_from(v).map_err(|_| format!("{what} braucht genau zwei Karten."))
}

fn equity_tool(args: &Value) -> Result<Value, String> {
    let players = args.get("players").and_then(Value::as_array).ok_or("'players' muss eine Liste sein.")?;
    if !(2..=6).contains(&players.len()) {
        return Err("Equity braucht 2 bis 6 Spieler.".into());
    }
    let ranges = players
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let s = p.as_str().ok_or_else(|| format!("Spieler {} ist kein Text.", i + 1))?;
            Range::parse(s).map_err(|_| format!("Spieler {}: '{s}' ist weder eine Hand noch eine Range.", i + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let board_text = args.get("board").and_then(Value::as_str).unwrap_or("");
    let board = CardSet::parse(board_text).map_err(|_| format!("Board '{board_text}' ist ungültig."))?;
    if ![0, 3, 4, 5].contains(&board.len()) {
        return Err(format!("Das Board hat {} Karten, erlaubt sind 0, 3, 4 oder 5.", board.len()));
    }
    let method = Method::Auto { max_exact_showdowns: 60_000_000, samples: 2_000_000, seed: 7 };
    let r = equity(&ranges, board, CardSet::default(), method)
        .map_err(|e| format!("Equity nicht berechenbar (Karten doppelt oder Range leer?): {e}"))?;
    Ok(json!({
        "equity_pct": r.players.iter().map(|p| round(p.equity_pct, 1)).collect::<Vec<_>>(),
        "exakt": r.exact,
        "methode": if r.exact { "vollständig ausgezählt" } else { "Monte Carlo, 2 Mio. Boards" },
    }))
}

fn pot_odds(args: &Value) -> Result<Value, String> {
    let call = number(args, "call")?;
    let pot = number(args, "pot_before_call")?;
    if call <= 0.0 || pot < 0.0 {
        return Err("'call' muss größer als 0 sein und 'pot_before_call' darf nicht negativ sein.".into());
    }
    Ok(json!({
        "benoetigte_equity_pct": round(100.0 * call / (pot + call), 2),
        "odds": format!("{} : 1", round(pot / call, 2)),
        "formel": "call ÷ (pot_before_call + call)",
    }))
}

type Key = [u64; 3];
static SOLVED: LazyLock<Mutex<HashMap<Key, Arc<PushFoldResult>>>> = LazyLock::new(Default::default);

fn solve(stack: f64, ante: f64, dead: f64) -> Result<Arc<PushFoldResult>, String> {
    let key = [stack, ante, dead].map(f64::to_bits);
    if let Some(r) = SOLVED.lock().map_err(|_| "Rechenkern blockiert")?.get(&key) {
        return Ok(r.clone());
    }
    let mut p = PushFoldParams::new(stack);
    p.ante_bb = ante;
    p.dead_bb = dead;
    let r = Arc::new(solve_push_fold(&p).map_err(|e| format!("Push/Fold nicht lösbar: {e}"))?);
    let mut cache = SOLVED.lock().map_err(|_| "Rechenkern blockiert")?;
    if cache.len() > 256 {
        cache.clear();
    }
    cache.insert(key, r.clone());
    Ok(r)
}

/// "K6o", "77", "A5s" or two cards like "Kh6d".
fn hand_class(s: &str) -> Result<HandClass, String> {
    let t = s.trim();
    if let Ok([a, b]) = two_cards(t, "Hand")
        && a != b
    {
        return Ok(Combo::new(a, b).class());
    }
    let range = Range::parse(t).map_err(|_| format!("'{s}' ist keine Handklasse wie 'K6o', 'A5s' oder '77'."))?;
    let mut classes = range.combos().map(|(c, _)| c.class());
    let first = classes.next().ok_or_else(|| format!("'{s}' enthält keine Hand."))?;
    if classes.any(|c| c != first) {
        return Err(format!("'{s}' ist mehr als eine Handklasse; gib genau eine an."));
    }
    Ok(first)
}

fn push_fold(args: &Value) -> Result<Value, String> {
    let stack = number(args, "stack_bb")?;
    let ante = optional_number(args, "ante_bb")?;
    let dead = optional_number(args, "dead_bb")?;
    if !(1.0..=100.0).contains(&stack) {
        return Err("'stack_bb' muss zwischen 1 und 100 liegen.".into());
    }
    if ante < 0.0 || dead < 0.0 || stack < 1.0 + ante {
        return Err(
            "Ante und totes Geld dürfen nicht negativ sein, und der Stack muss Big Blind plus Ante decken.".into()
        );
    }
    let class = hand_class(text(args, "hand")?)?;
    let r = solve(round(stack, 2), round(ante, 3), round(dead, 3))?;
    let i = class.index();
    Ok(json!({
        "hand": class.to_string(),
        "sb_shove_frequenz_pct": round(100.0 * r.shove[i], 1),
        "sb_ev_shove_bb": round(r.sb_class_ev[i].act, 3),
        "sb_ev_fold_bb": round(r.sb_class_ev[i].fold, 3),
        "bb_call_frequenz_pct": round(100.0 * r.call[i], 1),
        "bb_ev_call_bb": round(r.bb_class_ev[i].act, 3),
        "bb_ev_fold_bb": round(r.bb_class_ev[i].fold, 3),
        "sb_shove_range_pct": round(r.shove_pct, 1),
        "bb_call_range_pct": round(r.call_pct, 1),
        "modell": "Heads-up SB gegen BB, Chip-EV ohne ICM; EV je Option ab vor dem Posten gerechnet.",
    }))
}

fn icm_tool(args: &Value) -> Result<Value, String> {
    let stacks = numbers(args, "stacks")?;
    let payouts = numbers(args, "payouts")?;
    if !(2..=10).contains(&stacks.len()) {
        return Err("ICM braucht 2 bis 10 Stacks.".into());
    }
    if payouts.is_empty() || payouts.len() > stacks.len() {
        return Err("Es braucht mindestens eine und höchstens so viele Auszahlungen wie Spieler.".into());
    }
    if stacks.iter().chain(&payouts).any(|v| *v < 0.0) {
        return Err("Stacks und Auszahlungen dürfen nicht negativ sein.".into());
    }
    let r = icm(&stacks, &payouts).map_err(|e| format!("ICM nicht berechenbar: {e}"))?;
    let pool: f64 = payouts.iter().sum();
    let chips: f64 = stacks.iter().sum();
    Ok(json!({
        "equity": r.ev.iter().map(|v| round(*v, 2)).collect::<Vec<_>>(),
        "equity_pct_vom_preispool": r.ev.iter().map(|v| round(100.0 * v / pool.max(f64::MIN_POSITIVE), 1)).collect::<Vec<_>>(),
        "chip_anteil_pct": stacks.iter().map(|s| round(100.0 * s / chips.max(f64::MIN_POSITIVE), 1)).collect::<Vec<_>>(),
    }))
}

fn outs_tool(args: &Value) -> Result<Value, String> {
    let hero = two_cards(text(args, "hero")?, "hero")?;
    let board = cards(text(args, "board")?, "board")?;
    if !(3..=4).contains(&board.len()) {
        return Err("Outs gibt es nur am Flop (3 Karten) oder Turn (4 Karten).".into());
    }
    let villain = match args.get("villain").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(v) => Some(two_cards(v, "villain")?),
        None => None,
    };
    let r = outs(hero, villain, &board).map_err(|e| format!("Outs nicht berechenbar (Karte doppelt?): {e}"))?;
    let unseen = 52 - 2 - board.len() - if villain.is_some() { 2 } else { 0 };
    let list = |v: &[Card]| v.iter().map(Card::to_string).collect::<Vec<_>>();
    Ok(json!({
        "outs": r.outs.len(),
        "halbe_outs": r.half_outs.len(),
        "effektiv": r.effective(),
        "karten": list(&r.outs),
        "halbe_karten": list(&r.half_outs),
        "chance_naechste_karte_pct": round(100.0 * r.effective() / unseen as f64, 1),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pot_odds_formula() {
        let r = run("pot_odds", &json!({"call": 4.6, "pot_before_call": 15.7})).unwrap();
        assert_eq!(r["benoetigte_equity_pct"], 22.66);
    }

    #[test]
    fn bad_inputs_become_german_errors() {
        let cases = [
            ("pot_odds", json!({"call": -1, "pot_before_call": 3})),
            ("pot_odds", json!({"pot_before_call": 3})),
            ("pot_odds", json!("kein Objekt")),
            ("equity", json!({"players": ["AhKh"]})),
            ("equity", json!({"players": ["AhKh", "AhQd"]})),
            ("equity", json!({"players": ["AhKh", "xx"]})),
            ("equity", json!({"players": ["AhKh", "QQ"], "board": "2c3c"})),
            ("push_fold_nash", json!({"stack_bb": 0.5, "hand": "AA"})),
            ("push_fold_nash", json!({"stack_bb": 10, "hand": "AK"})),
            ("push_fold_nash", json!({"stack_bb": 10, "hand": "Zz"})),
            ("icm", json!({"stacks": [100], "payouts": [1]})),
            ("icm", json!({"stacks": [100, 50], "payouts": [1, 2, 3]})),
            ("outs", json!({"hero": "AhKh", "board": "2h"})),
            ("outs", json!({"hero": "AhKh", "board": "2h3h4c", "villain": "Ah2c"})),
            ("zaubern", json!({})),
        ];
        for (name, args) in cases {
            let e = run(name, &args).expect_err(&format!("{name} {args}"));
            assert!(!e.is_empty());
        }
    }

    #[test]
    fn engine_answers() {
        let r = run("equity", &json!({"players": ["AhAd", "KcKs"]})).unwrap();
        assert_eq!(r["exakt"], true);
        let eq = r["equity_pct"][0].as_f64().unwrap();
        assert!((81.0..83.0).contains(&eq), "{eq}");

        let r = run("push_fold_nash", &json!({"stack_bb": 8, "hand": "Kh6d"})).unwrap();
        assert_eq!(r["hand"], "K6o");
        assert_eq!(r["sb_shove_frequenz_pct"], 100.0);

        let r = run("icm", &json!({"stacks": [5000, 5000], "payouts": [70, 30]})).unwrap();
        assert_eq!(r["equity"][0], 50.0);

        let r = run("outs", &json!({"hero": "AhKh", "board": "2h7h9c", "villain": "QsQd"})).unwrap();
        assert!(r["outs"].as_u64().unwrap() >= 9);
    }

    #[test]
    fn report_schema_lists_every_concept() {
        let defs = definitions();
        let report = defs.as_array().unwrap().iter().find(|d| d["function"]["name"] == REPORT).unwrap();
        let concepts =
            &report["function"]["parameters"]["properties"]["spots"]["items"]["properties"]["concept"]["enum"];
        assert_eq!(concepts.as_array().unwrap().len(), CONCEPTS.len());
    }
}
