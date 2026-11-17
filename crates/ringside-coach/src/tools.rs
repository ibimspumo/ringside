//! The calculators the model may call. Every input is checked first; a bad input comes
//! back as a German error the model can read and correct, never as a crash.

use ringside_engine::{Card, CardSet, Combo, HandClass, Method, Range, equity, icm, outs};
use ringside_learn::{TableSpot, lookup_or_solve, position_names};
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
            "Push/Fold-Gleichgewicht für einen Tisch mit 2 bis 9 Spielern, in Chip-EV oder mit Auszahlungen in ICM. Gilt nur für reines Push/Fold: Bis zum ersten Shove folden alle, danach wird nur gecallt oder gefoldet, ein Call setzt den ganzen Stack. Keine Limps, keine Raises unter All-in, höchstens 3 Spieler all-in (danach folden alle übrigen). Das ist ein Modell: Ab 3 Spielern sind Kartenentfernung und 3-Wege-Equity genähert, eine Lösung kann leicht ungenau sein (siehe 'konvergiert'). Liefert für eine Hand an einer Position die Frequenz von Shove bzw. Call und den EV beider Optionen. Positionen in Aktionsreihenfolge: 2 Spieler SB, BB; 3 BTN, SB, BB; 6 LJ, HJ, CO, BTN, SB, BB; 9 UTG, UTG+1, UTG+2, LJ, HJ, CO, BTN, SB, BB (dazwischen von hinten aufgefüllt). Wann ICM gilt: am Finaltisch (alle verbleibenden Spieler an diesem Tisch) exakt mit den verbleibenden Auszahlungen; nahe der Bubble und im Geld vor dem Finaltisch nur als Näherung, weil die Stacks an anderen Tischen fehlen; nimm dann die Tisch-Auszahlungen aus den Fakten von Ringside. In frühen Phasen, fern vom Geld, payouts weglassen: Chip-EV.",
            json!({"type": "object", "properties": {
                "hand": {"type": "string", "description": "Handklasse wie 'K6o', 'A5s', '77' oder Karten wie 'Kh6d'"},
                "stacks_bb": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 9, "description": "Stack jedes Spielers in bb vor dem Posten, in Aktionsreihenfolge: erster Spieler nach dem Big Blind zuerst, Small Blind vorletzter, Big Blind letzter"},
                "position": {"type": "string", "description": "Position der Hand, z. B. 'CO' oder 'BB'"},
                "shover": {"type": "string", "description": "Position des Spielers, der zuerst all-in ging; weglassen, wenn alle vor 'position' gefoldet haben"},
                "callers": {"type": "array", "items": {"type": "string"}, "description": "Positionen, die den Shove vor 'position' gecallt haben (höchstens einer)"},
                "ante_bb": {"type": "number", "description": "Ante, die jeder Spieler zahlt, in bb"},
                "bb_ante_bb": {"type": "number", "description": "Big-Blind-Ante in bb (zahlt nur der Big Blind)"},
                "dead_bb": {"type": "number", "description": "Sonstiges totes Geld im Pot in bb"},
                "small_blind_bb": {"type": "number", "description": "Small Blind in bb, Standard 0,5"},
                "payouts": {"type": "array", "items": {"type": "number"}, "description": "Optional: Auszahlungen ab Platz 1 für die Spieler am Tisch, höchstens eine je Spieler (z. B. die Tisch-Auszahlungen in % des Preispools aus den Fakten); dann rechnet das Werkzeug in ICM statt in Chips"}
            }, "required": ["hand", "stacks_bb", "position"]})
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

fn optional_numbers(args: &Value, key: &str) -> Result<Vec<f64>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(_) => numbers(args, key),
    }
}

/// Seat of a position name at a table of `players`.
fn seat(names: &[&str], text: &str, key: &str) -> Result<usize, String> {
    let t = text.trim();
    names.iter().position(|n| n.eq_ignore_ascii_case(t)).ok_or_else(|| {
        format!("'{key}': '{t}' gibt es an einem Tisch mit {} Spielern nicht. Positionen: {}.", names.len(), names.join(", "))
    })
}

fn push_fold(args: &Value) -> Result<Value, String> {
    let class = hand_class(text(args, "hand")?)?;
    let stacks = numbers(args, "stacks_bb")?;
    let n = stacks.len();
    if !(2..=9).contains(&n) {
        return Err("'stacks_bb' braucht 2 bis 9 Stacks, einen je Spieler am Tisch.".into());
    }
    if stacks.iter().any(|s| !(0.0..=200.0).contains(s)) {
        return Err("Jeder Stack muss zwischen 0 und 200 bb liegen.".into());
    }
    let names = position_names(n);
    let hero = seat(&names, text(args, "position")?, "position")?;
    let shover = match args.get("shover").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        Some(s) => Some(seat(&names, s, "shover")?),
        None => None,
    };
    let callers = match args.get("callers") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list
            .iter()
            .map(|v| v.as_str().ok_or("'callers' muss eine Liste von Positionen sein.".to_string()))
            .map(|v| v.and_then(|s| seat(&names, s, "callers")))
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err("'callers' muss eine Liste von Positionen sein.".into()),
    };
    match shover {
        None if !callers.is_empty() => return Err("Ohne 'shover' kann es keine 'callers' geben.".into()),
        None if hero + 1 == n => {
            return Err("Der Big Blind entscheidet nur gegen einen Shove; gib 'shover' an.".into());
        }
        Some(s) if s >= hero => return Err("'shover' muss vor 'position' handeln.".into()),
        Some(s) if callers.windows(2).any(|w| w[0] >= w[1]) || callers.iter().any(|&c| c <= s || c >= hero) => {
            return Err("'callers' müssen zwischen 'shover' und 'position' sitzen, in Aktionsreihenfolge.".into());
        }
        Some(_) if callers.len() > 1 => {
            return Err("Höchstens 3 Spieler all-in: Nach einem Shove und zwei Calls folden im Modell alle übrigen.".into());
        }
        _ => {}
    }
    let small_blind = match args.get("small_blind_bb") {
        None | Some(Value::Null) => 0.5,
        Some(_) => number(args, "small_blind_bb")?,
    };
    let spot = TableSpot {
        stacks_bb: stacks.iter().map(|s| round(*s, 1)).collect(),
        small_blind_bb: round(small_blind, 3),
        ante_bb: round(optional_number(args, "ante_bb")?, 3),
        bb_ante_bb: round(optional_number(args, "bb_ante_bb")?, 3),
        dead_bb: round(optional_number(args, "dead_bb")?, 3),
        hero,
        shover,
        callers,
    };
    if [spot.ante_bb, spot.bb_ante_bb, spot.dead_bb].iter().any(|v| *v < 0.0) || !spot.valid() {
        return Err("Antes und totes Geld dürfen nicht negativ sein, der Small Blind liegt über 0 und höchstens bei 1 bb, und jeder Stack muss Ante und Blind decken.".into());
    }
    let payouts = optional_numbers(args, "payouts")?;
    if payouts.len() > n || payouts.iter().any(|p| *p < 0.0) || (!payouts.is_empty() && payouts.iter().sum::<f64>() <= 0.0) {
        return Err("'payouts': höchstens eine Auszahlung je Spieler, nicht negativ, zusammen über 0.".into());
    }
    let icm = !payouts.is_empty();
    let mut params = spot.params();
    if icm {
        params = params.with_payouts(payouts);
    }
    let solution = lookup_or_solve(&params)?;
    let s = solution
        .spot(spot.hero, class, spot.shover, &spot.callers)
        .ok_or("Diese Entscheidung gibt es im Push/Fold-Modell nicht.")?;
    let (act, act_name) = if shover.is_some() { ("call", "Call") } else { ("shove", "All-in") };
    let best = if s.freq >= 1.0 {
        act_name
    } else if s.freq <= 0.0 {
        "Fold"
    } else {
        "gemischt"
    };
    let mut situation = match shover {
        None => format!("Alle vor {} haben gefoldet", names[hero]),
        Some(s) => format!("{} geht all-in", names[s]),
    };
    for &c in &spot.callers {
        situation.push_str(&format!(", {} callt", names[c]));
    }
    let digits = if icm { 4 } else { 3 };
    Ok(json!({
        "hand": class.to_string(),
        "spieler": n,
        "position": names[hero],
        "positionen": names,
        "situation": situation,
        "aktion": act,
        "frequenz_pct": round(100.0 * s.freq, 1),
        "ev_aktion": round(s.ev_act, digits),
        "ev_fold": round(s.ev_fold, digits),
        "beste_option": best,
        "range_pct": round(solution.act_pct(s.node), 1),
        "effektiver_stack_bb": round(spot.effective_bb(), 1),
        "ev_einheit": if icm { "ICM-Equity in Einheiten der Auszahlungen" } else { "bb gewonnen oder verloren gegenüber dem Stack vor dem Posten" },
        "konvergiert": solution.converged,
        "rest_ausnutzbarkeit": round(solution.exploitability, 4),
        "mehrweg_all_in_pct": round(100.0 * solution.approximation.multiway_probability, 1),
        "modell": "Reines Push/Fold, höchstens 3 Spieler all-in, 3-Wege-Equity per Monte Carlo geschätzt, Kartenentfernung paarweise genähert.",
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
            ("push_fold_nash", json!({"stacks_bb": [10, 10], "position": "SB", "hand": "AK"})),
            ("push_fold_nash", json!({"stacks_bb": [10, 10], "position": "SB", "hand": "Zz"})),
            ("push_fold_nash", json!({"stacks_bb": [10], "position": "SB", "hand": "AA"})),
            ("push_fold_nash", json!({"stacks_bb": vec![10; 10], "position": "SB", "hand": "AA"})),
            ("push_fold_nash", json!({"stacks_bb": [10, 0.5], "position": "SB", "hand": "AA"})),
            ("push_fold_nash", json!({"stacks_bb": [10, 10, 10], "position": "UTG", "hand": "AA"})),
            ("push_fold_nash", json!({"stacks_bb": [10, 10, 10], "position": "BB", "hand": "AA"})),
            ("push_fold_nash", json!({"stacks_bb": [10, 10, 10], "position": "SB", "shover": "BB", "hand": "AA"})),
            ("push_fold_nash", json!({"stacks_bb": [10, 10, 10], "position": "SB", "callers": ["BTN"], "hand": "AA"})),
            (
                "push_fold_nash",
                json!({"stacks_bb": [10, 10, 10, 10], "position": "BB", "shover": "CO", "callers": ["BTN", "SB"], "hand": "AA"}),
            ),
            ("push_fold_nash", json!({"stacks_bb": [10, 10], "position": "SB", "hand": "AA", "payouts": [5, 3, 2]})),
            ("push_fold_nash", json!({"stacks_bb": [10, 10], "position": "SB", "hand": "AA", "ante_bb": -1})),
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

        let r = run("push_fold_nash", &json!({"stacks_bb": [8, 8], "position": "SB", "hand": "Kh6d"})).unwrap();
        assert_eq!(r["hand"], "K6o");
        assert_eq!(r["frequenz_pct"], 100.0);
        assert_eq!(r["beste_option"], "All-in");

        let r = run("icm", &json!({"stacks": [5000, 5000], "payouts": [70, 30]})).unwrap();
        assert_eq!(r["equity"][0], 50.0);

        let r = run("outs", &json!({"hero": "AhKh", "board": "2h7h9c", "villain": "QsQd"})).unwrap();
        assert!(r["outs"].as_u64().unwrap() >= 9);
    }

    /// The tool answers with the solver's numbers for the same table, rounded.
    #[test]
    fn push_fold_matches_the_solver_at_a_full_table() {
        let args = json!({
            "stacks_bb": [12, 25, 7, 18, 9, 11], "position": "BB", "shover": "LJ", "callers": ["BTN"],
            "hand": "AQo", "ante_bb": 0.125
        });
        let r = run("push_fold_nash", &args).unwrap();
        assert_eq!((r["spieler"].as_u64(), r["aktion"].as_str()), (Some(6), Some("call")));
        assert_eq!(r["situation"], "LJ geht all-in, BTN callt");
        let spot = TableSpot {
            stacks_bb: vec![12.0, 25.0, 7.0, 18.0, 9.0, 11.0],
            small_blind_bb: 0.5,
            ante_bb: 0.125,
            bb_ante_bb: 0.0,
            dead_bb: 0.0,
            hero: 5,
            shover: Some(0),
            callers: vec![3],
        };
        let class: HandClass = "AQo".parse().unwrap();
        let s = lookup_or_solve(&spot.params()).unwrap().spot(5, class, Some(0), &[3]).unwrap();
        assert_eq!(r["frequenz_pct"], round(100.0 * s.freq, 1));
        assert_eq!(r["ev_aktion"], round(s.ev_act, 3));
        assert_eq!(r["ev_fold"], round(s.ev_fold, 3));

        let icm = run("push_fold_nash", &json!({"stacks_bb": [6, 9, 15], "position": "BTN", "hand": "Q9s", "payouts": [50, 30, 20]}))
            .unwrap();
        assert_eq!(icm["ev_einheit"], "ICM-Equity in Einheiten der Auszahlungen");
        assert!(icm["ev_fold"].as_f64().unwrap() > 0.0 && icm["ev_fold"].as_f64().unwrap() < 100.0);
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
