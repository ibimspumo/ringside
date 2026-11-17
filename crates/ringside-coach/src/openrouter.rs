//! A small blocking OpenRouter client: chat completions with tools (streamed as server-sent
//! events, or in one piece as a fallback), the model list and the key check. Requests run
//! on a helper thread so a cancel takes effect at once.

use std::fmt;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "openai/gpt-6.1-sol";
/// Longest pause between two packets of a stream. OpenRouter sends keep-alive comments
/// about every half second while a model thinks, so a long silence means a dead line.
pub const IDLE_LIMIT: Duration = Duration::from_secs(120);
/// A request answered in one piece sends nothing until the model is done.
const WHOLE_ANSWER_LIMIT: Duration = Duration::from_secs(600);
const POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    Network(String),
    RateLimited { retry_after: Option<u64> },
    Unauthorized,
    NoCredit,
    NoToolSupport,
    /// The model or its provider cannot stream; ask again for the whole answer.
    NoStreaming,
    /// No packet within the idle limit, in seconds.
    Idle(u64),
    Server { status: u16, message: String },
    Rejected { status: u16, message: String },
    BadResponse(String),
    Cancelled,
}

impl ApiError {
    /// Worth trying again after a pause.
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            ApiError::Network(_) | ApiError::Idle(_) | ApiError::RateLimited { .. } | ApiError::Server { .. }
        )
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Network(m) => write!(f, "Keine Verbindung zu OpenRouter ({m})."),
            ApiError::RateLimited { .. } => {
                write!(f, "OpenRouter bremst gerade (zu viele Anfragen). Versuch es gleich noch einmal.")
            }
            ApiError::Unauthorized => write!(f, "OpenRouter lehnt den API-Key ab. Prüf ihn in den Einstellungen."),
            ApiError::NoCredit => write!(f, "Auf dem OpenRouter-Konto ist kein Guthaben mehr."),
            ApiError::NoToolSupport => {
                write!(
                    f,
                    "Dieses Modell kann keine Werkzeuge aufrufen. Wähl in den Einstellungen ein Modell mit Werkzeug-Unterstützung."
                )
            }
            ApiError::NoStreaming => write!(f, "Dieses Modell kann seine Antwort nicht streamen."),
            ApiError::Idle(s) => write!(f, "OpenRouter hat {s} s lang keine Daten geschickt."),
            ApiError::Server { status, message } => {
                write!(f, "OpenRouter hat einen Fehler gemeldet ({status}: {message}).")
            }
            ApiError::Rejected { status, message } => {
                write!(f, "OpenRouter hat die Anfrage abgelehnt ({status}: {message}).")
            }
            ApiError::BadResponse(m) => write!(f, "Unerwartete Antwort von OpenRouter ({m})."),
            ApiError::Cancelled => write!(f, "Abgebrochen."),
        }
    }
}

impl std::error::Error for ApiError {}

/// Model from `/models`, prices in US dollars per million tokens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub prompt_per_m: Option<f64>,
    pub completion_per_m: Option<f64>,
    pub context_length: Option<u64>,
    pub tools: bool,
    pub reasoning: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyInfo {
    pub label: Option<String>,
    /// Spent so far, in dollars.
    pub usage: Option<f64>,
    /// Credit limit of the key, if one is set.
    pub limit: Option<f64>,
    pub free_tier: bool,
}

pub struct Client {
    base: String,
    key: Option<String>,
    idle: Duration,
    /// Reads time out after the idle limit: streams, the model list, the key check.
    agent: ureq::Agent,
    /// For answers in one piece.
    patient: ureq::Agent,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").field("base", &self.base).field("key", &self.key.as_ref().map(|_| "***")).finish()
    }
}

fn error_message(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let m = v["error"]["message"].as_str().map(String::from).unwrap_or_else(|| body.chars().take(200).collect());
    m.chars().take(300).collect()
}

fn classify(status: u16, body: &str, retry_after: Option<u64>) -> ApiError {
    let message = error_message(body);
    let lower = message.to_lowercase();
    match status {
        401 | 403 if !lower.contains("tool") => ApiError::Unauthorized,
        402 => ApiError::NoCredit,
        429 => ApiError::RateLimited { retry_after },
        400 | 404 if lower.contains("tool") => ApiError::NoToolSupport,
        400 | 404 | 422 if lower.contains("stream") => ApiError::NoStreaming,
        408 | 500..=599 => ApiError::Server { status, message },
        _ => ApiError::Rejected { status, message },
    }
}

fn price(v: &Value) -> Option<f64> {
    let p = match v {
        Value::String(s) => s.parse::<f64>().ok()?,
        Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    (p >= 0.0).then_some((p * 1e12).round() / 1e6)
}

pub fn parse_models(v: &Value) -> Result<Vec<ModelInfo>, ApiError> {
    let data = v["data"].as_array().ok_or_else(|| ApiError::BadResponse("Modell-Liste fehlt".into()))?;
    let mut models: Vec<ModelInfo> = data
        .iter()
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let params: Vec<&str> = m["supported_parameters"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let text_out = m["architecture"]["output_modalities"]
                .as_array()
                .is_none_or(|a| a.iter().any(|x| x.as_str() == Some("text")));
            text_out.then(|| ModelInfo {
                name: m["name"].as_str().unwrap_or(&id).to_string(),
                prompt_per_m: price(&m["pricing"]["prompt"]),
                completion_per_m: price(&m["pricing"]["completion"]),
                context_length: m["context_length"].as_u64().or_else(|| m["top_provider"]["context_length"].as_u64()),
                tools: params.contains(&"tools"),
                reasoning: params.contains(&"reasoning"),
                id,
            })
        })
        .collect();
    models.sort_by_key(|m| m.name.to_lowercase());
    Ok(models)
}

/// OpenRouter reports some upstream failures inside a 200 answer or a stream packet.
fn error_in_body(v: &Value) -> Result<(), ApiError> {
    match v.get("error").filter(|e| !e.is_null()) {
        Some(err) => {
            let status = err["code"].as_u64().unwrap_or(502) as u16;
            Err(classify(status, &json!({ "error": err }).to_string(), None))
        }
        None => Ok(()),
    }
}

fn whole(body: &str) -> Result<Value, ApiError> {
    let v: Value = serde_json::from_str(body).map_err(|_| ApiError::BadResponse("kein JSON".into()))?;
    error_in_body(&v)?;
    Ok(v)
}

fn failed(e: ureq::Error) -> ApiError {
    match e {
        ureq::Error::Status(status, resp) => {
            let retry_after = resp.header("retry-after").and_then(|h| h.trim().parse().ok());
            let body = resp.into_string().unwrap_or_default();
            classify(status, &body, retry_after)
        }
        ureq::Error::Transport(t) => ApiError::Network(t.kind().to_string()),
    }
}

fn read_failed(e: &std::io::Error, idle: Duration) -> ApiError {
    match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => ApiError::Idle(idle.as_secs()),
        kind => ApiError::Network(kind.to_string()),
    }
}

/// What the model is doing in the stream right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Doing {
    /// Nothing visible yet; models with hidden reasoning stay here while they think.
    #[default]
    Waiting,
    Reasoning,
    Writing,
    /// Writing the arguments of a tool call.
    Calling,
}

/// The answer as far as it has arrived.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Live {
    pub doing: Doing,
    /// Characters of visible reasoning (often a summary of the hidden one).
    pub reasoning_chars: usize,
    pub content_chars: usize,
    pub tool_chars: usize,
    /// The tool whose call the model writes, while it does.
    pub tool: Option<String>,
    pub since_data: Duration,
}

/// Puts a streamed chat completion back together into the shape of a whole answer.
#[derive(Debug, Default)]
pub struct Assembler {
    pub live: Live,
    content: String,
    reasoning: String,
    details: Vec<Value>,
    calls: Vec<Value>,
    finish: Option<String>,
    usage: Value,
    packets: usize,
}

fn append(target: &mut Value, key: &str, piece: &Value) {
    if let Some(piece) = piece.as_str() {
        let joined = format!("{}{piece}", target[key].as_str().unwrap_or(""));
        target[key] = Value::String(joined);
    }
}

impl Assembler {
    /// Takes one line of the event stream; true at the end marker.
    pub fn feed(&mut self, line: &str) -> Result<bool, ApiError> {
        // Comments (": OPENROUTER PROCESSING") only keep the line alive.
        let Some(data) = line.strip_prefix("data:") else { return Ok(false) };
        let data = data.trim();
        if data == "[DONE]" {
            return Ok(true);
        }
        if data.is_empty() {
            return Ok(false);
        }
        let v: Value =
            serde_json::from_str(data).map_err(|_| ApiError::BadResponse("unlesbares Stream-Paket".into()))?;
        error_in_body(&v)?;
        self.packets += 1;
        if v["usage"].is_object() {
            self.usage = v["usage"].clone();
        }
        let choice = &v["choices"][0];
        if let Some(f) = choice["finish_reason"].as_str() {
            self.finish = Some(f.to_string());
        }
        let delta = &choice["delta"];
        let mut seen_reasoning = 0;
        if let Some(r) = delta["reasoning"].as_str() {
            self.reasoning.push_str(r);
            seen_reasoning = r.chars().count();
        }
        for d in delta["reasoning_details"].as_array().into_iter().flatten() {
            let text_key = match d["type"].as_str() {
                Some("reasoning.summary") => Some("summary"),
                Some("reasoning.text") => Some("text"),
                _ => None,
            };
            if delta["reasoning"].is_null() {
                seen_reasoning += text_key.and_then(|k| d[k].as_str()).map_or(0, |t| t.chars().count());
            }
            // Pieces of one block share type and index and are joined in order.
            match (self.details.last_mut(), text_key) {
                (Some(last), Some(key)) if last["type"] == d["type"] && last["index"] == d["index"] => {
                    append(last, key, &d[key]);
                    if !d["signature"].is_null() {
                        last["signature"] = d["signature"].clone();
                    }
                }
                _ => self.details.push(d.clone()),
            }
        }
        if seen_reasoning > 0 {
            self.live.reasoning_chars += seen_reasoning;
            self.live.doing = Doing::Reasoning;
        }
        if let Some(c) = delta["content"].as_str().filter(|c| !c.is_empty()) {
            self.content.push_str(c);
            self.live.content_chars += c.chars().count();
            self.live.doing = Doing::Writing;
            self.live.tool = None;
        }
        for (pos, d) in delta["tool_calls"].as_array().into_iter().flatten().enumerate() {
            let i = d["index"].as_u64().map_or(pos, |i| i as usize);
            while self.calls.len() <= i {
                self.calls.push(json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}}));
            }
            let call = &mut self.calls[i];
            if let Some(id) = d["id"].as_str().filter(|s| !s.is_empty()) {
                call["id"] = json!(id);
            }
            if let Some(name) = d["function"]["name"].as_str().filter(|s| !s.is_empty()) {
                call["function"]["name"] = json!(name);
            }
            append(&mut call["function"], "arguments", &d["function"]["arguments"]);
            self.live.tool_chars += d["function"]["arguments"].as_str().map_or(0, |a| a.chars().count());
            self.live.doing = Doing::Calling;
            self.live.tool = call["function"]["name"].as_str().filter(|n| !n.is_empty()).map(String::from);
        }
        Ok(false)
    }

    /// The answer in the shape of a non-streamed one.
    pub fn finish(self) -> Result<Value, ApiError> {
        if self.packets == 0 {
            return Err(ApiError::BadResponse("leerer Stream".into()));
        }
        let mut message = json!({"role": "assistant", "content": self.content});
        if !self.calls.is_empty() {
            message["tool_calls"] = Value::Array(self.calls);
        }
        if !self.details.is_empty() {
            message["reasoning_details"] = Value::Array(self.details);
        }
        if !self.reasoning.is_empty() {
            message["reasoning"] = Value::String(self.reasoning);
        }
        Ok(json!({
            "choices": [{"message": message, "finish_reason": self.finish}],
            "usage": self.usage,
        }))
    }
}

enum Packet {
    Line(String),
    /// The server answered a stream request in one piece.
    Whole(String),
    End,
    Failed(ApiError),
}

enum Method {
    Get,
    Post(String),
}

impl Client {
    pub fn new(base: &str, key: Option<String>) -> Client {
        Client::with_idle_limit(base, key, IDLE_LIMIT)
    }

    pub fn with_idle_limit(base: &str, key: Option<String>, idle: Duration) -> Client {
        let agent = |read| {
            ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(15))
                .timeout_read(read)
                .timeout_write(Duration::from_secs(60))
                .build()
        };
        Client {
            base: base.trim_end_matches('/').to_string(),
            key,
            idle,
            agent: agent(idle),
            patient: agent(WHOLE_ANSWER_LIMIT.max(idle)),
        }
    }

    pub fn idle_limit(&self) -> Duration {
        self.idle
    }

    fn prepare(&self, agent: &ureq::Agent, path: &str, post: bool) -> ureq::Request {
        let url = format!("{}{path}", self.base);
        let mut req = if post { agent.post(&url).set("Content-Type", "application/json") } else { agent.get(&url) };
        if let Some(key) = &self.key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        req
    }

    /// One request answered in one piece. `tick` runs while waiting; false stops.
    fn request(
        &self,
        agent: &ureq::Agent,
        path: &str,
        method: Method,
        cancel: &dyn Fn() -> bool,
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<Value, ApiError> {
        let req = self.prepare(agent, path, matches!(method, Method::Post(_)));
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = match method {
                Method::Get => req.call(),
                Method::Post(body) => req.send_string(&body),
            };
            let out = match result {
                Ok(resp) => resp.into_string().map_err(|e| ApiError::Network(e.kind().to_string())),
                Err(e) => Err(failed(e)),
            };
            let _ = tx.send(out);
        });
        loop {
            if cancel() || !tick() {
                return Err(ApiError::Cancelled);
            }
            match rx.recv_timeout(POLL) {
                Ok(r) => return whole(&r?),
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(ApiError::Network("Verbindung abgebrochen".into()));
                }
            }
        }
    }

    /// A chat completion in one piece, for models that cannot stream.
    pub fn chat(
        &self,
        body: &Value,
        cancel: &dyn Fn() -> bool,
        tick: &mut dyn FnMut() -> bool,
    ) -> Result<Value, ApiError> {
        self.request(&self.patient, "/chat/completions", Method::Post(body.to_string()), cancel, tick)
    }

    /// A streamed chat completion, returned in the shape of a whole one. `update` sees the
    /// answer after every packet and at least every 100 ms; false stops the stream.
    /// Leaving the function drops the connection: the reading thread ends with the next
    /// packet, and OpenRouter's keep-alives arrive about twice a second.
    pub fn chat_stream(
        &self,
        body: &Value,
        cancel: &dyn Fn() -> bool,
        update: &mut dyn FnMut(&Live) -> bool,
    ) -> Result<Value, ApiError> {
        let mut body = body.clone();
        body["stream"] = json!(true);
        let text = body.to_string();
        let req = self.prepare(&self.agent, "/chat/completions", true);
        let idle = self.idle;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let resp = match req.send_string(&text) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Packet::Failed(failed(e)));
                    return;
                }
            };
            if !resp.content_type().contains("event-stream") {
                let body = resp.into_string().map_err(|e| read_failed(&e, idle));
                let _ = tx.send(body.map_or_else(Packet::Failed, Packet::Whole));
                return;
            }
            let mut reader = BufReader::new(resp.into_reader());
            let mut line = String::new();
            loop {
                line.clear();
                let packet = match reader.read_line(&mut line) {
                    Ok(0) => Packet::End,
                    Ok(_) => Packet::Line(line.trim_end_matches(['\r', '\n']).to_string()),
                    Err(e) => Packet::Failed(read_failed(&e, idle)),
                };
                let last = !matches!(packet, Packet::Line(_));
                if tx.send(packet).is_err() || last {
                    return;
                }
            }
        });
        let mut asm = Assembler::default();
        let mut last_data = Instant::now();
        loop {
            if cancel() {
                return Err(ApiError::Cancelled);
            }
            match rx.recv_timeout(POLL) {
                Ok(Packet::Line(line)) => {
                    last_data = Instant::now();
                    if asm.feed(&line)? {
                        return asm.finish();
                    }
                }
                Ok(Packet::Whole(body)) => return whole(&body),
                Ok(Packet::End) => return asm.finish(),
                Ok(Packet::Failed(e)) => return Err(e),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(ApiError::Network("Verbindung abgebrochen".into()));
                }
            }
            asm.live.since_data = last_data.elapsed();
            if asm.live.since_data > idle {
                return Err(ApiError::Idle(idle.as_secs()));
            }
            if !update(&asm.live) {
                return Err(ApiError::Cancelled);
            }
        }
    }

    pub fn models(&self) -> Result<Vec<ModelInfo>, ApiError> {
        parse_models(&self.request(&self.agent, "/models", Method::Get, &|| false, &mut || true)?)
    }

    pub fn key_info(&self) -> Result<KeyInfo, ApiError> {
        if self.key.is_none() {
            return Err(ApiError::Unauthorized);
        }
        let v = self.request(&self.agent, "/key", Method::Get, &|| false, &mut || true)?;
        let d = &v["data"];
        Ok(KeyInfo {
            label: d["label"].as_str().map(String::from),
            usage: d["usage"].as_f64(),
            limit: d["limit"].as_f64(),
            free_tier: d["is_free_tier"].as_bool().unwrap_or(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn errors_are_classified() {
        assert_eq!(classify(401, "{}", None), ApiError::Unauthorized);
        assert_eq!(classify(429, "{}", Some(3)), ApiError::RateLimited { retry_after: Some(3) });
        let no_tools = json!({"error": {"message": "No endpoints found that support tool use"}}).to_string();
        assert_eq!(classify(404, &no_tools, None), ApiError::NoToolSupport);
        assert!(classify(503, "{}", None).retryable());
        assert!(!classify(400, "{}", None).retryable());
    }

    #[test]
    fn models_with_prices_per_million() {
        let v = json!({"data": [
            {"id": "b/x", "name": "B", "pricing": {"prompt": "0.000002", "completion": "0.00001"}, "context_length": 128000,
             "supported_parameters": ["tools", "reasoning"]},
            {"id": "a/img", "name": "A", "architecture": {"output_modalities": ["image"]}, "pricing": {}},
            {"id": "a/y", "name": "A", "pricing": {"prompt": "0", "completion": "0"}}
        ]});
        let m = parse_models(&v).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].id, "a/y");
        assert!(!m[0].tools);
        assert_eq!(m[1].prompt_per_m, Some(2.0));
        assert_eq!(m[1].completion_per_m, Some(10.0));
        assert!(m[1].tools && m[1].reasoning);
    }

    #[test]
    fn stream_lines() {
        let mut a = Assembler::default();
        assert!(!a.feed(": OPENROUTER PROCESSING").unwrap());
        assert!(!a.feed("event: message").unwrap());
        assert!(!a.feed(r#"data:{"choices":[{"delta":{"content":"Hallo"}}]}"#).unwrap());
        assert_eq!(a.live.doing, Doing::Writing);
        assert!(a.feed("data: [DONE]").unwrap());
        assert_eq!(a.finish().unwrap()["choices"][0]["message"]["content"], "Hallo");

        let failed = r#"data: {"error":{"code":502,"message":"Provider disconnected"},"choices":[{"finish_reason":"error"}]}"#;
        assert!(matches!(Assembler::default().feed(failed), Err(ApiError::Server { status: 502, .. })));
        assert!(Assembler::default().finish().is_err(), "ein leerer Stream ist keine Antwort");
    }

    #[test]
    fn debug_hides_the_key() {
        let c = Client::new(BASE_URL, Some("sk-or-geheim".into()));
        assert!(!format!("{c:?}").contains("geheim"));
    }
}
