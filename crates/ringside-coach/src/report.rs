//! The coach's report: what the model hands in through the `report` tool, checked and
//! enriched with what Ringside knows about each hand, then stored per tournament.

use ringside_core::model::{Hand, Street};
use ringside_core::stats::position_labels;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::concepts;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Stark,
    Ok,
    Fraglich,
    Fehler,
}

impl Verdict {
    fn parse(s: &str) -> Option<Verdict> {
        match s.trim().to_lowercase().as_str() {
            "stark" => Some(Verdict::Stark),
            "ok" => Some(Verdict::Ok),
            "fraglich" => Some(Verdict::Fraglich),
            "fehler" => Some(Verdict::Fehler),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// Taken from a calculator result or from Ringside's own facts.
    #[serde(rename = "berechnet")]
    Berechnet,
    #[serde(rename = "einschätzung")]
    Einschaetzung,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Figure {
    pub label: String,
    pub value: String,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    pub hand_id: u64,
    pub played_at: String,
    pub hero_cards: Vec<String>,
    /// The board as far as it was dealt.
    pub board: Vec<String>,
    pub position: Option<String>,
    pub stack_bb: f64,
    pub action: String,
    pub verdict: Verdict,
    pub better: Option<String>,
    pub why: String,
    pub numbers: Vec<Figure>,
    /// Id from [`concepts::CONCEPTS`].
    pub concept: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub model: String,
    pub created_at: i64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// As billed by OpenRouter; `None` when the answer did not say.
    pub cost_usd: Option<f64>,
    pub seconds: f64,
    pub steps: usize,
    pub tool_calls: usize,
    pub hands_total: usize,
    pub hands_sent: usize,
    /// Figures the model called computed that no calculator produced; shown as estimates.
    pub downgraded: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub tournament_id: u64,
    pub summary: String,
    pub spots: Vec<Spot>,
    pub focus: Vec<String>,
    pub meta: Meta,
}

/// Overview row for the report list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportInfo {
    pub tournament_id: u64,
    pub created_at: i64,
    pub model: String,
    pub spots: usize,
    pub mistakes: usize,
    pub cost_usd: Option<f64>,
}

impl Report {
    pub fn info(&self) -> ReportInfo {
        ReportInfo {
            tournament_id: self.tournament_id,
            created_at: self.meta.created_at,
            model: self.meta.model.clone(),
            spots: self.spots.len(),
            mistakes: self.spots.iter().filter(|s| matches!(s.verdict, Verdict::Fehler | Verdict::Fraglich)).count(),
            cost_usd: self.meta.cost_usd,
        }
    }
}

/// Every number in a text, German or English decimals: "22,7 %" → 22.7.
pub fn numbers_in(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut flush = |cur: &mut String| {
        let t = cur.trim_end_matches(['.', ',']).replace(',', ".");
        if let Ok(v) = t.parse::<f64>() {
            out.push(v);
        }
        cur.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_digit() || ((ch == ',' || ch == '.') && !cur.is_empty()) {
            cur.push(ch);
        } else if ch == '-' || ch == '−' {
            flush(&mut cur);
            cur.push('-');
        } else {
            flush(&mut cur);
        }
    }
    flush(&mut cur);
    out
}

/// Numbers found anywhere in a JSON value.
pub fn numbers_in_json(v: &Value, out: &mut Vec<f64>) {
    match v {
        Value::Number(n) => out.extend(n.as_f64()),
        Value::String(s) => out.extend(numbers_in(s)),
        Value::Array(a) => a.iter().for_each(|x| numbers_in_json(x, out)),
        Value::Object(o) => o.values().for_each(|x| numbers_in_json(x, out)),
        _ => {}
    }
}

/// "49.8 %" → "49,8 %": calculators answer in JSON numbers, the report is German.
fn german_decimals(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let digit = |i: usize| chars.get(i).is_some_and(char::is_ascii_digit);
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| if c == '.' && i > 0 && digit(i - 1) && digit(i + 1) { ',' } else { c })
        .collect()
}

/// Whether every figure in `value` comes from `known` (rounded, or as a fraction ↔
/// percentage). A value without digits cannot be computed.
pub(crate) fn traced(value: &str, known: &[f64]) -> bool {
    let found = numbers_in(value);
    !found.is_empty()
        && found.iter().all(|v| {
            known.iter().any(|k| {
                let close = |a: f64, b: f64| (a - b).abs() <= 0.051_f64.max(b.abs() * 0.005);
                close(v.abs(), k.abs()) || close(v.abs(), (k * 100.0).abs())
            })
        })
}

pub struct HandRef<'a> {
    pub label: &'a str,
    pub hand: &'a Hand,
}

fn board_at(hand: &Hand) -> Vec<String> {
    let hero = hand.hero.as_deref().unwrap_or_default();
    let last = hand.actions.iter().rev().find(|a| a.player == hero && !a.kind.is_post()).map(|a| a.street);
    let n = match last {
        Some(Street::Flop) => 3,
        Some(Street::Turn) => 4,
        Some(Street::River) => 5,
        _ => 0,
    };
    // An all-in runs the board out; show all of it then.
    let all_in = hand.actions.iter().any(|a| a.all_in);
    let n = if all_in { hand.board.len() } else { n.min(hand.board.len()) };
    hand.board[..n].to_vec()
}

fn spot_from(raw: &Value, i: usize, refs: &[HandRef], known: &[f64], downgraded: &mut usize) -> Result<Spot, String> {
    let at = format!("Spot {}", i + 1);
    let field = |k: &str| raw.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("");
    let label = field("hand");
    let r = refs
        .iter()
        .find(|r| r.label.eq_ignore_ascii_case(label))
        .ok_or_else(|| format!("{at}: Hand '{label}' gibt es nicht; nimm eine Kennung wie 'H3' aus der Liste."))?;
    let verdict = Verdict::parse(field("verdict"))
        .ok_or_else(|| format!("{at}: 'verdict' muss stark, ok, fraglich oder fehler sein."))?;
    let concept = concepts::resolve(field("concept"))
        .ok_or_else(|| format!("{at}: Konzept '{}' ist nicht in der Liste.", field("concept")))?;
    let (action, why) = (field("action"), field("why"));
    if action.is_empty() || why.is_empty() {
        return Err(format!("{at}: 'action' und 'why' dürfen nicht leer sein."));
    }
    let numbers = match raw.get("numbers") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) => list
            .iter()
            .map(|n| {
                let get = |k: &str| n.get(k).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
                let (label, value) = (get("label"), get("value"));
                if label.is_empty() || value.is_empty() {
                    return Err(format!("{at}: jede Zahl braucht 'label' und 'value'."));
                }
                let source = match get("source").to_lowercase().as_str() {
                    "berechnet" if traced(&value, known) => Source::Berechnet,
                    "berechnet" => {
                        *downgraded += 1;
                        Source::Einschaetzung
                    }
                    "einschätzung" | "einschaetzung" => Source::Einschaetzung,
                    _ => return Err(format!("{at}: 'source' muss berechnet oder einschätzung sein.")),
                };
                Ok(Figure { label, value: german_decimals(&value), source })
            })
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(format!("{at}: 'numbers' muss eine Liste sein.")),
    };
    let hand = r.hand;
    let hero = hand.hero.as_deref().unwrap_or_default();
    let stack = hand.seats.iter().find(|s| s.name == hero).map(|s| s.stack).unwrap_or(0);
    let better = field("better");
    Ok(Spot {
        hand_id: hand.id,
        played_at: hand.played_at.clone(),
        hero_cards: hand.hero_cards.clone(),
        board: board_at(hand),
        position: position_labels(hand).into_iter().find(|(n, _)| n == hero).map(|(_, p)| p.to_string()),
        stack_bb: if hand.big_blind > 0 { stack as f64 / hand.big_blind as f64 } else { 0.0 },
        action: action.to_string(),
        verdict,
        better: (!better.is_empty() && better != "-").then(|| better.to_string()),
        why: why.to_string(),
        numbers,
        concept: concept.id.to_string(),
    })
}

/// Checks the arguments of a `report` call. `known` holds every number Ringside gave
/// the model (facts and calculator results); figures marked computed that are not among
/// them are shown as estimates. All problems are listed at once, so the model can fix
/// them in one go.
pub fn parse(args: &Value, tournament_id: u64, refs: &[HandRef], known: &[f64], meta: Meta) -> Result<Report, String> {
    let mut errors = Vec::new();
    let summary = args.get("summary").and_then(Value::as_str).map(str::trim).unwrap_or("");
    if summary.is_empty() {
        errors.push("'summary' fehlt.".to_string());
    }
    let mut meta = meta;
    let mut spots = Vec::new();
    match args.get("spots").and_then(Value::as_array) {
        Some(list) if !list.is_empty() && list.len() <= 12 => {
            for (i, raw) in list.iter().enumerate() {
                match spot_from(raw, i, refs, known, &mut meta.downgraded) {
                    Ok(s) => spots.push(s),
                    Err(e) => errors.push(e),
                }
            }
        }
        _ => errors.push("'spots' muss 1 bis 12 Einträge haben.".into()),
    }
    let focus: Vec<String> = args
        .get("focus")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    if focus.is_empty() {
        errors.push("'focus' braucht mindestens einen Schwerpunkt.".into());
    }
    if !errors.is_empty() {
        return Err(format!(
            "Der Bericht ist noch nicht gültig. Bitte korrigieren und `report` erneut aufrufen:\n- {}",
            errors.join("\n- ")
        ));
    }
    Ok(Report { tournament_id, summary: summary.to_string(), spots, focus: focus.into_iter().take(3).collect(), meta })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_numbers_in_both_notations() {
        assert_eq!(numbers_in("22,7 % bei 8 bb"), vec![22.7, 8.0]);
        assert_eq!(numbers_in("−0.92 bb"), vec![-0.92]);
        assert_eq!(numbers_in("1.234"), vec![1.234]);
        assert!(numbers_in("immer").is_empty());
    }

    #[test]
    fn decimals_become_german() {
        assert_eq!(german_decimals("49.8 % bis 1.5 bb."), "49,8 % bis 1,5 bb.");
    }

    #[test]
    fn traces_figures_to_tool_results() {
        let known = [22.66, 1.0, 61.9];
        assert!(traced("22,7 %", &known));
        assert!(traced("100 %", &known), "Frequenz 1.0 als Prozent");
        assert!(!traced("35 %", &known));
        assert!(!traced("hoch", &known));
    }

    #[test]
    fn invalid_report_lists_all_problems() {
        let e =
            parse(&json!({"spots": [{"hand": "H9", "verdict": "super"}]}), 1, &[], &[], Meta::default()).unwrap_err();
        assert!(e.contains("summary"));
        assert!(e.contains("H9"));
        assert!(e.contains("focus"));
    }

    #[test]
    fn serde_names_are_german() {
        let f = Figure { label: "x".into(), value: "1".into(), source: Source::Einschaetzung };
        assert_eq!(serde_json::to_value(&f).unwrap()["source"], "einschätzung");
        assert_eq!(serde_json::to_value(Verdict::Fraglich).unwrap(), "fraglich");
    }
}
