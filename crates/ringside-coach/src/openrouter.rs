//! A small blocking OpenRouter client: chat completions with tools, the model list and
//! the key check. Requests run on a helper thread so a cancel takes effect at once.

use std::fmt;
use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "openai/gpt-6.1-sol";

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    Network(String),
    RateLimited { retry_after: Option<u64> },
    Unauthorized,
    NoCredit,
    NoToolSupport,
    Server { status: u16, message: String },
    Rejected { status: u16, message: String },
    BadResponse(String),
    Cancelled,
}

impl ApiError {
    /// Worth trying again after a pause.
    pub fn retryable(&self) -> bool {
        matches!(self, ApiError::Network(_) | ApiError::RateLimited { .. } | ApiError::Server { .. })
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
    agent: ureq::Agent,
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

enum Method {
    Get,
    Post(String),
}

impl Client {
    pub fn new(base: &str, key: Option<String>) -> Client {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(600))
            .timeout_write(Duration::from_secs(60))
            .build();
        Client { base: base.trim_end_matches('/').to_string(), key, agent }
    }

    fn request(&self, path: &str, method: Method, cancel: &dyn Fn() -> bool) -> Result<Value, ApiError> {
        let mut req = match &method {
            Method::Get => self.agent.get(&format!("{}{path}", self.base)),
            Method::Post(_) => self.agent.post(&format!("{}{path}", self.base)).set("Content-Type", "application/json"),
        };
        if let Some(key) = &self.key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = match method {
                Method::Get => req.call(),
                Method::Post(body) => req.send_string(&body),
            };
            let out = match result {
                Ok(resp) => resp.into_string().map_err(|e| ApiError::Network(e.kind().to_string())),
                Err(ureq::Error::Status(status, resp)) => {
                    let retry_after = resp.header("retry-after").and_then(|h| h.trim().parse().ok());
                    let body = resp.into_string().unwrap_or_default();
                    Err(classify(status, &body, retry_after))
                }
                Err(ureq::Error::Transport(t)) => Err(ApiError::Network(t.kind().to_string())),
            };
            let _ = tx.send(out);
        });
        let body = loop {
            if cancel() {
                return Err(ApiError::Cancelled);
            }
            match rx.recv_timeout(Duration::from_millis(150)) {
                Ok(r) => break r?,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err(ApiError::Network("Verbindung abgebrochen".into()));
                }
            }
        };
        let v: Value = serde_json::from_str(&body).map_err(|_| ApiError::BadResponse("kein JSON".into()))?;
        // OpenRouter reports some upstream failures inside a 200 answer.
        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
            let status = err["code"].as_u64().unwrap_or(502) as u16;
            return Err(classify(status, &serde_json::json!({ "error": err }).to_string(), None));
        }
        Ok(v)
    }

    pub fn chat(&self, body: &Value, cancel: &dyn Fn() -> bool) -> Result<Value, ApiError> {
        self.request("/chat/completions", Method::Post(body.to_string()), cancel)
    }

    pub fn models(&self) -> Result<Vec<ModelInfo>, ApiError> {
        parse_models(&self.request("/models", Method::Get, &|| false)?)
    }

    pub fn key_info(&self) -> Result<KeyInfo, ApiError> {
        if self.key.is_none() {
            return Err(ApiError::Unauthorized);
        }
        let v = self.request("/key", Method::Get, &|| false)?;
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
    fn debug_hides_the_key() {
        let c = Client::new(BASE_URL, Some("sk-or-geheim".into()));
        assert!(!format!("{c:?}").contains("geheim"));
    }
}
