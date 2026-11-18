//! "Frag den Coach": one question about figures Ringside has already computed (a
//! tournament report or the decisions of a hand). The model only explains them; it gets no
//! calculators, and every figure in its answer that is not among the facts is flagged.
//!
//! Like an analysis, nothing leaves the machine before [`PreparedAsk::confirm`] took the
//! token of exactly the estimate the user saw.

use std::hash::{BuildHasher, RandomState};

use serde::Serialize;
use serde_json::{Value, json};

use crate::openrouter::{ApiError, Client, ModelInfo};
use crate::report::{numbers_in, traced};
use crate::session::CoachError;

/// Output tokens an answer usually takes; reasoning models think a little before it.
const COMPLETION_GUESS: u64 = 900;
/// The provider stops here, hidden reasoning included, which also caps the cost.
const MAX_COMPLETION: u64 = 3_000;
const MAX_QUESTION_CHARS: usize = 600;
/// Counting words up to here are no computed figures ("3 Fehler", "Schritt 2").
const SMALL: f64 = 10.0;

const SYSTEM: &str = "Du bist der Poker-Coach in der App Ringside. Ein Aufsteiger fragt dich zu Zahlen, die Ringside schon berechnet hat: aus seinem Turnier-Bericht oder aus der Bewertung einer Hand. Der Spieler heißt „Du“, Gegner heißen „Spieler 1“, „Spieler 2“ und so weiter.

Regeln:
- Rechne nie selbst. Jede Zahl in deiner Antwort steht genau so in den „Fakten von Ringside“. Fehlt eine Zahl, sag, dass Ringside sie nicht berechnet hat, statt sie zu schätzen.
- Die Fakten stimmen. Widersprich ihnen nicht; erkläre, warum sie so sind.
- Bewerte Entscheidungen, nicht Ergebnisse.
- Die Antwort steht im ersten Satz. Danach höchstens drei kurze Absätze in einfachem Deutsch, in der Du-Form, ohne Tabellen und ohne Überschriften. Erklär jeden Fachbegriff in ein paar Worten.
- Höchstens 150 Wörter.";

fn tokens(text: &str) -> u64 {
    (text.chars().count() as f64 / 2.8).ceil() as u64
}

/// What the user sees before the question is sent.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AskEstimate {
    pub token: String,
    pub model: String,
    pub question: String,
    /// Exactly what goes to the model besides the fixed instructions.
    pub sent: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub max_completion_tokens: u64,
    pub cost_usd: Option<f64>,
    /// The provider stops at the token limit, so the answer cannot cost more.
    pub max_cost_usd: Option<f64>,
}

pub struct PreparedAsk {
    estimate: AskEstimate,
    messages: Vec<Value>,
    known: Vec<f64>,
    reasoning: bool,
}

pub struct ConfirmedAsk(PreparedAsk);

impl PreparedAsk {
    pub fn estimate(&self) -> &AskEstimate {
        &self.estimate
    }

    /// The payload as it would be sent, for tests and inspection.
    pub fn messages(&self) -> &[Value] {
        &self.messages
    }

    pub fn confirm(self, token: &str) -> Result<ConfirmedAsk, CoachError> {
        if token.is_empty() || token != self.estimate.token {
            return Err(CoachError::NotConfirmed);
        }
        Ok(ConfirmedAsk(self))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Answer {
    pub text: String,
    /// Figures of the answer that are not among the facts, as written.
    pub unchecked: Vec<String>,
    pub cost_usd: f64,
    pub model: String,
}

/// Builds the request for a question about `facts` (already anonymous) and estimates its
/// cost. `model` carries the price when the model list could be loaded.
pub fn prepare_ask(facts: &str, question: &str, model_id: &str, model: Option<&ModelInfo>) -> Result<PreparedAsk, CoachError> {
    let question: String = question.trim().chars().take(MAX_QUESTION_CHARS).collect();
    if question.is_empty() {
        return Err(CoachError::Api(ApiError::BadResponse("Keine Frage gestellt.".into())));
    }
    let user = format!("Fakten von Ringside:\n{facts}\n\nFrage: {question}");
    let messages = vec![json!({"role": "system", "content": SYSTEM}), json!({"role": "user", "content": &user})];
    let mut known = numbers_in(&user);
    known.sort_by(f64::total_cmp);
    known.dedup();
    let prompt_tokens = tokens(SYSTEM) + tokens(&user);
    let price = |c: u64| {
        let m = model?;
        Some(prompt_tokens as f64 / 1e6 * m.prompt_per_m? + c as f64 / 1e6 * m.completion_per_m?)
    };
    let payload = serde_json::to_string(&messages).unwrap_or_default();
    let token = format!("{:016x}", RandomState::new().hash_one((&payload, model_id)));
    Ok(PreparedAsk {
        estimate: AskEstimate {
            token,
            model: model_id.to_string(),
            question,
            sent: user,
            prompt_tokens,
            completion_tokens: COMPLETION_GUESS,
            max_completion_tokens: MAX_COMPLETION,
            cost_usd: price(COMPLETION_GUESS),
            max_cost_usd: price(MAX_COMPLETION),
        },
        messages,
        known,
        reasoning: model.is_none_or(|m| m.reasoning),
    })
}

/// The figures of `text` that are not among `known`, as written. Small whole numbers count
/// things and pass.
pub fn unchecked(text: &str, known: &[f64]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut flush = |cur: &mut String| {
        let t = cur.trim_end_matches(['.', ',']).to_string();
        cur.clear();
        let Some(v) = numbers_in(&t).first().copied() else { return };
        let small = v.fract() == 0.0 && v.abs() <= SMALL;
        if !small && !traced(&t, known) && !out.contains(&t) {
            out.push(t);
        }
    };
    for ch in text.chars() {
        if ch.is_ascii_digit() || ((ch == ',' || ch == '.') && !cur.is_empty()) {
            cur.push(ch);
        } else {
            flush(&mut cur);
        }
    }
    flush(&mut cur);
    out
}

/// Sends the confirmed question and returns the answer with the figures it could not trace.
pub fn ask(confirmed: ConfirmedAsk, client: &Client, cancel: &dyn Fn() -> bool) -> Result<Answer, CoachError> {
    let ConfirmedAsk(p) = confirmed;
    let mut body = json!({
        "model": p.estimate.model,
        "messages": p.messages,
        "max_tokens": MAX_COMPLETION,
        "usage": {"include": true},
    });
    if p.reasoning {
        body["reasoning"] = json!({"effort": "low"});
    }
    let resp = client.chat(&body, cancel, &mut || true)?;
    let text = resp["choices"][0]["message"]["content"].as_str().unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err(CoachError::Api(ApiError::BadResponse("Das Modell hat nicht geantwortet.".into())));
    }
    Ok(Answer {
        unchecked: unchecked(&text, &p.known),
        cost_usd: resp["usage"]["cost"].as_f64().unwrap_or(0.0),
        model: p.estimate.model,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> ModelInfo {
        ModelInfo {
            id: "m".into(),
            name: "M".into(),
            prompt_per_m: Some(2.0),
            completion_per_m: Some(8.0),
            context_length: Some(100_000),
            tools: true,
            reasoning: true,
        }
    }

    #[test]
    fn nothing_is_sent_without_the_token_of_the_estimate() {
        let p = prepare_ask("Score 82 %", "Was heißt das?", "m", Some(&model())).unwrap();
        let token = p.estimate().token.clone();
        assert!(matches!(prepare_ask("x", "y", "m", None).unwrap().confirm(""), Err(CoachError::NotConfirmed)));
        assert!(matches!(prepare_ask("x", "y", "m", None).unwrap().confirm("abc"), Err(CoachError::NotConfirmed)));
        assert!(p.confirm(&token).is_ok());
    }

    #[test]
    fn the_estimate_caps_the_cost_with_the_token_limit() {
        let p = prepare_ask("Verlust 4,2 BB", "Warum?", "m", Some(&model())).unwrap();
        let e = p.estimate();
        assert!(e.cost_usd.unwrap() > 0.0);
        assert!(e.max_cost_usd.unwrap() > e.cost_usd.unwrap());
        let want = e.prompt_tokens as f64 * 2.0 / 1e6 + MAX_COMPLETION as f64 * 8.0 / 1e6;
        assert!((e.max_cost_usd.unwrap() - want).abs() < 1e-12);
        assert_eq!(prepare_ask("x", "y", "m", None).unwrap().estimate().cost_usd, None);
        assert!(prepare_ask("x", "   ", "m", None).is_err());
    }

    #[test]
    fn the_facts_and_the_question_are_what_is_sent() {
        let p = prepare_ask("Du: Ah Kd im Big Blind", "Warum ist das ein Fold?", "m", None).unwrap();
        let user = p.messages()[1]["content"].as_str().unwrap();
        assert!(user.contains("Du: Ah Kd im Big Blind") && user.ends_with("Frage: Warum ist das ein Fold?"));
        assert_eq!(p.messages().len(), 2);
    }

    #[test]
    fn figures_not_among_the_facts_are_flagged() {
        let known = numbers_in("Verlust 4,2 BB, gute Entscheidungen 82 %, Equity 0.31");
        assert_eq!(unchecked("Du hast 4,2 BB verloren, 82 % waren gut.", &known), Vec::<String>::new());
        assert_eq!(unchecked("Deine Equity lag bei 31 %, 3 Fehler.", &known), Vec::<String>::new());
        assert_eq!(unchecked("Mit 57,5 % Equity und 12 BB wäre es ein Call.", &known), vec!["57,5", "12"]);
    }
}
