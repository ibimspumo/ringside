//! One analysis: prepare (select hands, build the prompt, estimate the cost), confirm
//! with the token of exactly that estimate, then run the tool-calling loop.
//!
//! Nothing leaves the machine before [`Prepared::confirm`] has turned the prepared
//! request into a [`Confirmed`] one, and [`run`] only accepts the latter.

use std::fmt;
use std::hash::{BuildHasher, RandomState};
use std::time::{Duration, Instant};

use ringside_core::model::Hand;
use ringside_core::tournament::TournamentRow;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::concepts::CONCEPTS;
use crate::openrouter::{ApiError, Client, Doing, Live, ModelInfo};
use crate::prizes::{Prizes, Source, prizes};
use crate::render::{Names, num, render_hand};
use crate::report::{self, HandRef, Meta, Report, numbers_in, numbers_in_json};
use crate::select::select;
use crate::tools::{self, REPORT};

pub const MAX_HANDS: usize = 40;
pub const MAX_STEPS: usize = 12;
/// Calls the estimate assumes: first look, a round of calculators, the report.
const EXPECTED_STEPS: usize = 3;
/// Tokens a step adds to the conversation (tool calls and results).
const GROWTH_PER_STEP: u64 = 900;
const COMPLETION_PER_STEP: u64 = 1_800;
const COMPLETION_REPORT: u64 = 2_500;
const MAX_TOKENS_PER_STEP: u64 = 16_000;
/// Below this many affordable output tokens a step cannot do anything useful.
const MIN_TOKENS_PER_STEP: u64 = 1_000;
/// Gap between two progress events while nothing new happens.
const PROGRESS_EVERY: Duration = Duration::from_millis(400);
const RETRIES: [u64; 3] = [2, 5, 12];

#[derive(Debug, PartialEq)]
pub enum CoachError {
    Api(ApiError),
    NotConfirmed,
    NoHands,
    NotHoldem,
    NoReport,
    TooManySteps,
    CostLimit { spent: f64, limit: f64 },
}

impl fmt::Display for CoachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoachError::Api(e) => e.fmt(f),
            CoachError::NotConfirmed => write!(f, "Bitte bestätige zuerst die Kostenschätzung."),
            CoachError::NoHands => {
                write!(f, "In diesem Turnier gibt es keine Hände mit einer echten Entscheidung von dir.")
            }
            CoachError::NotHoldem => write!(f, "Der Coach analysiert bisher nur Hold'em-Turniere."),
            CoachError::NoReport => write!(
                f,
                "Das Modell hat keinen Bericht abgegeben. Versuch es noch einmal oder wähl ein anderes Modell."
            ),
            CoachError::TooManySteps => {
                write!(
                    f,
                    "Das Modell ist nach {MAX_STEPS} Schritten nicht fertig geworden. Versuch es noch einmal oder wähl ein anderes Modell."
                )
            }
            CoachError::CostLimit { spent, limit } => write!(
                f,
                "Abgebrochen, weil der nächste Teil die bestätigte Obergrenze von {} $ überschritten hätte. Bisher etwa {} $.",
                num(*limit, 4),
                num(*spent, 4)
            ),
        }
    }
}

impl std::error::Error for CoachError {}

impl From<ApiError> for CoachError {
    fn from(e: ApiError) -> Self {
        CoachError::Api(e)
    }
}

/// How long the model may think in each step, OpenRouter's reasoning effort.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    #[default]
    High,
    Medium,
    Low,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::High => "high",
            Effort::Medium => "medium",
            Effort::Low => "low",
        }
    }

    /// Output tokens compared with a thorough run, a rough guess for the estimate.
    fn completion_share(self) -> f64 {
        match self {
            Effort::High => 1.0,
            Effort::Medium => 0.6,
            Effort::Low => 0.35,
        }
    }
}

/// What the user sees before anything is sent.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Estimate {
    pub token: String,
    pub tournament_id: u64,
    pub model: String,
    /// None when the model does not reason.
    pub effort: Option<Effort>,
    pub hands_total: usize,
    pub hands_sent: usize,
    pub hands_without_decision: usize,
    pub hands_dropped: usize,
    /// Push/fold and all-in spots Ringside computed in advance.
    pub spots: usize,
    /// Size of the first request.
    pub input_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub max_prompt_tokens: u64,
    pub max_completion_tokens: u64,
    pub prompt_per_m: Option<f64>,
    pub completion_per_m: Option<f64>,
    pub cost_usd: Option<f64>,
    /// Upper bound; the run stops before going past it.
    pub max_cost_usd: Option<f64>,
}

pub struct Prepared {
    estimate: Estimate,
    messages: Vec<Value>,
    hands: Vec<(String, Hand)>,
    known: Vec<f64>,
    effort: Option<Effort>,
}

pub struct Confirmed(Prepared);

impl Prepared {
    pub fn estimate(&self) -> &Estimate {
        &self.estimate
    }

    /// The payload as it would be sent, for tests and inspection.
    pub fn messages(&self) -> &[Value] {
        &self.messages
    }

    pub fn confirm(self, token: &str) -> Result<Confirmed, CoachError> {
        if token.is_empty() || token != self.estimate.token {
            return Err(CoachError::NotConfirmed);
        }
        Ok(Confirmed(self))
    }
}

fn tokens(text: &str) -> u64 {
    (text.chars().count() as f64 / 2.8).ceil() as u64
}

const SYSTEM: &str = "Du bist der Poker-Coach in der App Ringside. Du analysierst die Hände eines Freizeitspielers aus einem abgeschlossenen Online-Turnier (No Limit Hold'em). In den Händen heißt er „Du“, die Gegner heißen „Spieler 1“, „Spieler 2“ und so weiter, über alle Hände gleich.

Regeln:
- Rechne nie selbst. Jede Zahl (benötigte Equity, Equity, Nash-Frequenz, ICM, Outs) holst du mit einem Werkzeug oder übernimmst sie aus den „Fakten von Ringside“. Solche Zahlen markierst du mit source „berechnet“ und übernimmst sie unverändert, im deutschen Format mit Komma (49,8 %).
- Annahmen über Gegner-Ranges, Tendenzen oder Bet-Größen machst du ausdrücklich und markierst sie als „einschätzung“.
- Bewerte Entscheidungen, nicht Ergebnisse. Glück oder Pech nennst du nur mit den Zahlen dazu.
- push_fold_nash gilt für reines Push/Fold an jeder Position: Bis zum ersten Shove folden alle, danach wird nur gecallt oder gefoldet, höchstens 3 Spieler all-in. Nach Limps oder Raises unter All-in ist es höchstens ein Anhaltspunkt; sag das dann. Mit 3 und mehr Spielern sind die Zahlen ein Modell mit geschätzter 3-Wege-Equity.
- ICM: Die Fakten nennen je Hand die Turnierphase und, wo ICM zählt, die Auszahlungen für die Plätze an deinem Tisch. Am Finaltisch und nahe der Bubble oder im Geld rechnet Ringside Push/Fold zusätzlich mit ICM; diese Zahlen haben Vorrang vor Chip-EV. Für eigene Rechnungen gibst du push_fold_nash genau diese Tisch-Auszahlungen als payouts mit. Heißen die Auszahlungen „angenommene Standard-Struktur“, sag, dass es eine Annahme ist. Vor dem Finaltisch ist ICM am Tisch eine Näherung, weil die Stacks an anderen Tischen unbekannt sind. In frühen Phasen oder ohne Auszahlungen gilt Chip-EV; ICM-Aussagen sind dort höchstens Einschätzungen.
- Wähle 3 bis 10 Spots mit echten Entscheidungen, die lehrreichsten zuerst. Gute Entscheidungen dürfen dabei sein.
- Jeder Spot bekommt genau ein Konzept aus der Liste im Werkzeug `report`.
- Fakten, die schon da sind, musst du nicht nachrechnen. Du kannst mehrere Werkzeuge in einem Schritt aufrufen.
- Schreib kurz und konkret, auf Deutsch, in der Du-Form.
- Zum Schluss rufst du genau einmal `report` auf.";

fn money(amount: i64, currency: Option<&str>) -> String {
    let unit = match currency {
        Some("USD") => " $",
        Some("EUR") => " €",
        Some("GBP") => " £",
        Some(_) => " Punkte",
        None => "",
    };
    format!("{}{unit}", num(amount as f64 / 100.0, 2))
}

fn header(row: &TournamentRow, prizes: Option<&Prizes>) -> String {
    let mut parts = vec!["No Limit Hold'em".to_string()];
    if let Some(b) = &row.buy_in {
        let cur = b.currency.as_deref();
        if b.bounty > 0 {
            parts.push(format!(
                "Buy-in {} + {} Bounty + {} Gebühr, Bounty-Turnier",
                money(b.prize, cur),
                money(b.bounty, cur),
                money(b.fee, cur)
            ));
        } else {
            parts.push(format!("Buy-in {} + {} Gebühr", money(b.prize, cur), money(b.fee, cur)));
        }
    }
    if let Some(n) = row.entrants {
        parts.push(format!("{n} Teilnehmer"));
    }
    match (row.place, row.prize) {
        (Some(p), Some(prize)) if prize > 0 => {
            parts.push(format!("dein Platz {p} mit {}", money(prize, row.currency.as_deref())))
        }
        (Some(p), _) => parts.push(format!("dein Platz {p}, ohne Preisgeld")),
        (None, Some(0)) => parts.push("ohne Preisgeld ausgeschieden".into()),
        _ => {}
    }
    parts.push(match prizes {
        Some(p) => {
            let top: Vec<String> = p.pct.iter().take(9).map(|x| format!("{} %", num(*x, 2))).collect();
            let more = if p.pct.len() > 9 { ", …" } else { "" };
            let source = match p.source {
                Source::Summary => "aus der Turnier-Zusammenfassung",
                Source::Standard => "angenommene Standard-Struktur (15 % bezahlt), nicht die echten Auszahlungen",
            };
            format!("Auszahlungen {source}: {} Plätze bezahlt, ab Platz 1 in % des Preispools {}{more}", p.pct.len(), top.join(", "))
        }
        None => "Auszahlungen unbekannt, Chip-EV".into(),
    });
    parts.join(", ")
}

/// Selects the hands of a tournament, builds the request and estimates its cost.
/// `model` carries the price when the model list could be loaded.
pub fn prepare(
    row: &TournamentRow,
    hands: Vec<Hand>,
    model_id: &str,
    model: Option<&ModelInfo>,
    max_hands: usize,
    effort: Effort,
) -> Result<Prepared, CoachError> {
    if model.is_some_and(|m| !m.tools) {
        return Err(CoachError::Api(ApiError::NoToolSupport));
    }
    let holdem: Vec<Hand> = hands.into_iter().filter(|h| h.variant.contains("Hold'em")).collect();
    if holdem.is_empty() {
        return Err(CoachError::NotHoldem);
    }
    let prizes = prizes(row, &holdem);
    let selection = select(holdem, max_hands, prizes.as_ref());
    if selection.hands.is_empty() {
        return Err(CoachError::NoHands);
    }
    let mut names = Names::default();
    let mut body = String::new();
    let mut kept = Vec::new();
    let mut spots = 0;
    for s in selection.hands {
        let label = format!("H{}", kept.len() + 1);
        let Some(text) = render_hand(&s.hand, &label, &mut names) else { continue };
        body.push_str(&text);
        let why: Vec<&str> = s.reasons.iter().map(|r| r.text()).collect();
        body.push_str(&format!("Ausgewählt wegen: {}\n", why.join(", ")));
        for f in &s.facts {
            body.push_str(&format!("Fakt von Ringside: {f}\n"));
        }
        body.push('\n');
        spots += s.spots;
        kept.push((label, s.hand));
    }
    if kept.is_empty() {
        return Err(CoachError::NoHands);
    }
    let user = format!(
        "Turnier: {}.\nHände insgesamt: {}. Hier stehen {} Hände mit einer echten Entscheidung von dir. Weggelassen: {} Hände, in denen du vor dem Flop nur gefoldet hast{}.\nBeträge in Big Blinds (bb), Stacks zu Beginn der Hand.\n\n{body}Analysiere diese Hände.",
        header(row, prizes.as_ref()),
        selection.total,
        kept.len(),
        selection.without_decision,
        if selection.dropped > 0 {
            format!(", und {} weniger wichtige, damit die Anfrage klein bleibt", selection.dropped)
        } else {
            String::new()
        },
    );
    let messages = vec![json!({"role": "system", "content": SYSTEM}), json!({"role": "user", "content": user})];
    let mut known = numbers_in(&user);
    known.sort_by(f64::total_cmp);
    known.dedup();

    let base = tokens(SYSTEM) + tokens(&user) + tokens(&tools::definitions().to_string());
    let prompt_for = |steps: usize| (0..steps as u64).map(|i| base + i * GROWTH_PER_STEP).sum::<u64>();
    // Unknown models are asked to reason; OpenRouter ignores the field where it does not apply.
    let effort = model.is_none_or(|m| m.reasoning).then_some(effort);
    let completion_for = |steps: usize| {
        let thorough = steps as u64 * COMPLETION_PER_STEP + COMPLETION_REPORT;
        (thorough as f64 * effort.map_or(1.0, Effort::completion_share)).round() as u64
    };
    let prompt_tokens = prompt_for(EXPECTED_STEPS);
    let completion_tokens = completion_for(EXPECTED_STEPS);
    let max_prompt_tokens = prompt_for(MAX_STEPS);
    let max_completion_tokens = completion_for(MAX_STEPS);
    let price = |p: u64, c: u64| {
        let m = model?;
        Some(p as f64 / 1e6 * m.prompt_per_m? + c as f64 / 1e6 * m.completion_per_m?)
    };
    let payload = serde_json::to_string(&messages).unwrap_or_default();
    let token = format!("{:016x}", RandomState::new().hash_one((&payload, model_id, row.id)));
    Ok(Prepared {
        estimate: Estimate {
            token,
            tournament_id: row.id,
            model: model_id.to_string(),
            effort,
            hands_total: selection.total,
            hands_sent: kept.len(),
            hands_without_decision: selection.without_decision,
            hands_dropped: selection.dropped,
            spots,
            input_tokens: base,
            prompt_tokens,
            completion_tokens,
            max_prompt_tokens,
            max_completion_tokens,
            prompt_per_m: model.and_then(|m| m.prompt_per_m),
            completion_per_m: model.and_then(|m| m.completion_per_m),
            cost_usd: price(prompt_tokens, completion_tokens),
            max_cost_usd: price(max_prompt_tokens, max_completion_tokens),
        },
        messages,
        hands: kept,
        known,
        effort,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Waiting for the model, which reads and reasons.
    Thinking,
    /// The model writes text.
    Writing,
    /// The model writes a tool call (`tool` names it).
    Calling,
    /// Ringside runs a calculator.
    Tool,
    /// Waiting before trying again.
    Retry,
    /// The model's report failed validation and goes back for a fix.
    Fixing,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Progress {
    pub tournament_id: u64,
    pub step: usize,
    pub max_steps: usize,
    pub stage: Stage,
    /// German line for the UI.
    pub message: String,
    pub tool: Option<String>,
    pub tool_calls: usize,
    /// Billed so far, from OpenRouter's usage after each step.
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    /// Part of `completion_tokens` the model spent reasoning.
    pub reasoning_tokens: u64,
    pub cost_usd: f64,
    /// Counted in the stream of the current step (rough, from characters). Hidden
    /// reasoning only shows up in the billed numbers after the step.
    pub live_reasoning_tokens: u64,
    pub live_output_tokens: u64,
    pub step_seconds: f64,
    pub total_seconds: f64,
    /// Seconds since the last packet of the stream.
    pub idle_seconds: f64,
    /// The step fails after this many seconds without data; None without streaming.
    pub idle_limit: Option<u64>,
}

/// Sleeps in small slices so a cancel is noticed.
fn pause(seconds: u64, cancel: &dyn Fn() -> bool) -> Result<(), CoachError> {
    let end = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < end {
        if cancel() {
            return Err(ApiError::Cancelled.into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// The confirmed upper limit with prices per token.
#[derive(Clone, Copy)]
struct Budget {
    limit: f64,
    prompt: f64,
    completion: f64,
}

impl Budget {
    fn of(e: &Estimate) -> Option<Budget> {
        Some(Budget { limit: e.max_cost_usd?, prompt: e.prompt_per_m? / 1e6, completion: e.completion_per_m? / 1e6 })
    }
}

fn char_tokens(chars: usize) -> u64 {
    (chars as f64 / 2.8).ceil() as u64
}

/// Turns stream updates into progress events (each change, otherwise every
/// [`PROGRESS_EVERY`]) and stops the stream before it passes the cost limit.
struct Watch {
    run_started: Instant,
    step_started: Instant,
    emitted: Option<(Instant, Stage, Option<String>)>,
    budget: Option<Budget>,
    /// Estimated price of the current request's input.
    prompt_cost: f64,
    over: Option<f64>,
}

impl Watch {
    fn emit(&mut self, p: &mut Progress, progress: &mut dyn FnMut(&Progress)) {
        p.step_seconds = self.step_started.elapsed().as_secs_f64();
        p.total_seconds = self.run_started.elapsed().as_secs_f64();
        self.emitted = Some((Instant::now(), p.stage.clone(), p.tool.clone()));
        progress(p);
    }

    fn update(&mut self, p: &mut Progress, live: &Live, progress: &mut dyn FnMut(&Progress)) -> bool {
        p.live_reasoning_tokens = char_tokens(live.reasoning_chars);
        p.live_output_tokens = char_tokens(live.content_chars + live.tool_chars);
        if let Some(b) = self.budget {
            let running = p.cost_usd
                + self.prompt_cost
                + (p.live_reasoning_tokens + p.live_output_tokens) as f64 * b.completion;
            if running > b.limit {
                self.over = Some(running);
                return false;
            }
        }
        p.idle_seconds = live.since_data.as_secs_f64();
        (p.stage, p.tool, p.message) = match live.doing {
            Doing::Waiting | Doing::Reasoning if p.step == 1 => {
                (Stage::Thinking, None, "Das Modell liest die Hände und denkt nach …".into())
            }
            Doing::Waiting | Doing::Reasoning => (Stage::Thinking, None, "Das Modell denkt nach …".into()),
            Doing::Writing => (Stage::Writing, None, "Das Modell schreibt …".into()),
            Doing::Calling => {
                let message = match live.tool.as_deref() {
                    Some(REPORT) => "Das Modell schreibt den Bericht …".to_string(),
                    Some(t) => format!("Das Modell fragt Ringside, {} …", tool_purpose(t)),
                    None => "Das Modell ruft ein Werkzeug auf …".to_string(),
                };
                (Stage::Calling, live.tool.clone(), message)
            }
        };
        let fresh = self
            .emitted
            .as_ref()
            .is_some_and(|(at, stage, tool)| *stage == p.stage && *tool == p.tool && at.elapsed() < PROGRESS_EVERY);
        if !fresh {
            self.emit(p, progress);
        }
        true
    }
}

/// What a calculator answers, in the player's words, for the live progress line.
fn tool_purpose(name: &str) -> &str {
    match name {
        "equity" => "wie oft deine Hand gegen die Gegner gewinnt",
        "pot_odds" => "ob sich der Call vom Preis her lohnt",
        "push_fold_nash" => "ob All-in oder Fold besser war",
        "icm" => "was die Chips in Preisgeld wert sind",
        "outs" => "welche Karten deine Hand noch verbessern",
        REPORT => "ob der Bericht vollständig ist",
        other => other,
    }
}

/// Runs the tool-calling loop until the model hands in a valid report.
/// `now` is the creation time stored with the report, in seconds.
pub fn run(
    confirmed: Confirmed,
    client: &Client,
    now: i64,
    progress: &mut dyn FnMut(&Progress),
    cancel: &dyn Fn() -> bool,
) -> Result<Report, CoachError> {
    let Confirmed(prepared) = confirmed;
    let Prepared { estimate, mut messages, hands, mut known, effort } = prepared;
    let refs: Vec<HandRef> = hands.iter().map(|(label, hand)| HandRef { label, hand }).collect();
    let started = Instant::now();
    let budget = Budget::of(&estimate);
    let mut p = Progress {
        tournament_id: estimate.tournament_id,
        step: 0,
        max_steps: MAX_STEPS,
        stage: Stage::Thinking,
        message: String::new(),
        tool: None,
        tool_calls: 0,
        prompt_tokens: 0,
        completion_tokens: 0,
        reasoning_tokens: 0,
        cost_usd: 0.0,
        live_reasoning_tokens: 0,
        live_output_tokens: 0,
        step_seconds: 0.0,
        total_seconds: 0.0,
        idle_seconds: 0.0,
        idle_limit: None,
    };
    let mut nudged = false;
    let mut streaming = true;
    let defs = tools::definitions();
    let defs_tokens = tokens(&defs.to_string());
    for step in 1..=MAX_STEPS {
        p.step = step;
        let last = step == MAX_STEPS;
        let prompt_guess = tokens(&serde_json::to_string(&messages).unwrap_or_default()) + defs_tokens;
        let mut watch = Watch {
            run_started: started,
            step_started: Instant::now(),
            emitted: None,
            budget,
            prompt_cost: budget.map_or(0.0, |b| prompt_guess as f64 * b.prompt),
            over: None,
        };
        // The provider stops at `max_tokens`, hidden reasoning included, so capping it at what
        // the limit still allows keeps the run under it even when nothing can be counted.
        let mut max_tokens = MAX_TOKENS_PER_STEP;
        if let Some(b) = budget {
            let left = b.limit - p.cost_usd - watch.prompt_cost;
            let affordable = if b.completion > 0.0 { (left.max(0.0) / b.completion) as u64 } else { u64::MAX };
            if affordable < MIN_TOKENS_PER_STEP {
                return Err(CoachError::CostLimit { spent: p.cost_usd, limit: b.limit });
            }
            max_tokens = max_tokens.min(affordable);
        }
        let mut body = json!({
            "model": estimate.model,
            "messages": messages,
            "tools": defs,
            "tool_choice": if last { json!({"type": "function", "function": {"name": REPORT}}) } else { json!("auto") },
            "max_tokens": max_tokens,
            "usage": {"include": true},
        });
        if let Some(effort) = effort {
            body["reasoning"] = json!({"effort": effort.as_str()});
        }
        let mut attempt = 0;
        let resp = loop {
            if cancel() {
                return Err(ApiError::Cancelled.into());
            }
            p.idle_limit = streaming.then(|| client.idle_limit().as_secs());
            p.idle_seconds = 0.0;
            watch.emitted = None;
            let result = if streaming {
                client.chat_stream(&body, cancel, &mut |live| watch.update(&mut p, live, progress))
            } else {
                client.chat(&body, cancel, &mut || watch.update(&mut p, &Live::default(), progress))
            };
            p.live_reasoning_tokens = 0;
            p.live_output_tokens = 0;
            match result {
                Ok(r) => break r,
                Err(ApiError::Cancelled) if watch.over.is_some() => {
                    let limit = budget.map_or(0.0, |b| b.limit);
                    return Err(CoachError::CostLimit { spent: watch.over.unwrap_or(limit), limit });
                }
                Err(ApiError::NoStreaming) if streaming => {
                    streaming = false;
                    p.stage = Stage::Retry;
                    p.message = "Das Modell kann nicht streamen. Ringside wartet auf die ganze Antwort.".into();
                    watch.emit(&mut p, progress);
                }
                Err(e) if e.retryable() && attempt < RETRIES.len() => {
                    let wait = match e {
                        ApiError::RateLimited { retry_after: Some(s) } => s.min(60),
                        _ => RETRIES[attempt],
                    };
                    attempt += 1;
                    p.stage = Stage::Retry;
                    p.message = format!("{e} Neuer Versuch in {wait} s ({attempt}/{}).", RETRIES.len());
                    watch.emit(&mut p, progress);
                    pause(wait, cancel)?;
                }
                Err(e) => return Err(e.into()),
            }
        };
        let usage = &resp["usage"];
        p.prompt_tokens += usage["prompt_tokens"].as_u64().unwrap_or(0);
        p.completion_tokens += usage["completion_tokens"].as_u64().unwrap_or(0);
        p.reasoning_tokens += usage["completion_tokens_details"]["reasoning_tokens"].as_u64().unwrap_or(0);
        p.cost_usd += usage["cost"].as_f64().unwrap_or(0.0);
        if let Some(limit) = estimate.max_cost_usd
            && p.cost_usd > limit
        {
            return Err(CoachError::CostLimit { spent: p.cost_usd, limit });
        }
        let msg = &resp["choices"][0]["message"];
        if !msg.is_object() {
            return Err(ApiError::BadResponse("Antwort ohne Nachricht".into()).into());
        }
        let mut keep = json!({"role": "assistant", "content": msg["content"].as_str().unwrap_or("")});
        for key in ["tool_calls", "reasoning_details"] {
            if !msg[key].is_null() {
                keep[key] = msg[key].clone();
            }
        }
        messages.push(keep);
        let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        if calls.is_empty() {
            if nudged || last {
                return Err(CoachError::NoReport);
            }
            nudged = true;
            messages
                .push(json!({"role": "user", "content": "Bitte gib den Bericht jetzt über das Werkzeug `report` ab."}));
            continue;
        }
        let mut done = None;
        for call in &calls {
            let id = call["id"].as_str().unwrap_or_default().to_string();
            let name = call["function"]["name"].as_str().unwrap_or_default().to_string();
            let raw = call["function"]["arguments"].as_str().unwrap_or("{}");
            p.tool_calls += 1;
            let args: Result<Value, String> = serde_json::from_str(if raw.trim().is_empty() { "{}" } else { raw })
                .map_err(|_| "Die Argumente sind kein gültiges JSON. Bitte erneut aufrufen.".to_string());
            let result = match args {
                Err(e) => json!({"fehler": e}),
                Ok(args) if name == REPORT => {
                    let meta = Meta {
                        model: estimate.model.clone(),
                        created_at: now,
                        hands_total: estimate.hands_total,
                        hands_sent: estimate.hands_sent,
                        ..Meta::default()
                    };
                    match report::parse(&args, estimate.tournament_id, &refs, &known, meta) {
                        Ok(r) => {
                            done = Some(r);
                            json!({"ok": true})
                        }
                        Err(e) => {
                            p.stage = Stage::Fixing;
                            p.message = "Der Bericht war unvollständig, das Modell bessert nach …".into();
                            watch.emit(&mut p, progress);
                            json!({"fehler": e})
                        }
                    }
                }
                Ok(args) => {
                    p.stage = Stage::Tool;
                    p.tool = Some(name.clone());
                    p.message = format!("Ringside prüft, {} …", tool_purpose(&name));
                    watch.emit(&mut p, progress);
                    match tools::run(&name, &args) {
                        Ok(v) => {
                            numbers_in_json(&v, &mut known);
                            v
                        }
                        Err(e) => json!({"fehler": e}),
                    }
                }
            };
            messages.push(json!({"role": "tool", "tool_call_id": id, "content": result.to_string()}));
        }
        if let Some(mut r) = done {
            r.meta.prompt_tokens = p.prompt_tokens;
            r.meta.completion_tokens = p.completion_tokens;
            r.meta.cost_usd = (p.cost_usd > 0.0).then_some(p.cost_usd);
            r.meta.seconds = (started.elapsed().as_secs_f64() * 10.0).round() / 10.0;
            r.meta.steps = step;
            r.meta.tool_calls = p.tool_calls;
            return Ok(r);
        }
    }
    Err(CoachError::TooManySteps)
}

/// How often each concept shows up in stored reports, weighted towards mistakes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConceptCount {
    pub id: &'static str,
    pub name: &'static str,
    pub exercise: Option<&'static str>,
    pub articles: &'static [&'static str],
    pub spots: usize,
    pub mistakes: usize,
    pub questionable: usize,
    /// Tournaments it appeared in.
    pub tournaments: usize,
}

/// Concepts of all reports, the weakest first (most mistakes, then questionable spots).
pub fn concept_counts(reports: &[Report]) -> Vec<ConceptCount> {
    use crate::report::Verdict;
    let mut out: Vec<ConceptCount> = CONCEPTS
        .iter()
        .map(|c| ConceptCount {
            id: c.id,
            name: c.name,
            exercise: c.exercise,
            articles: c.articles,
            spots: 0,
            mistakes: 0,
            questionable: 0,
            tournaments: 0,
        })
        .collect();
    for r in reports {
        for c in &mut out {
            let spots: Vec<_> = r.spots.iter().filter(|s| s.concept == c.id).collect();
            c.spots += spots.len();
            c.mistakes += spots.iter().filter(|s| s.verdict == Verdict::Fehler).count();
            c.questionable += spots.iter().filter(|s| s.verdict == Verdict::Fraglich).count();
            c.tournaments += usize::from(!spots.is_empty());
        }
    }
    out.retain(|c| c.spots > 0);
    out.sort_by_key(|c| std::cmp::Reverse((c.mistakes, c.questionable, c.spots)));
    out
}
