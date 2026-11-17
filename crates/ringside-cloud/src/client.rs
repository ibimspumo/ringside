//! HTTP client for the Ringside Cloud API: blocking, rustls, with timeouts and retries.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::canonicalize;
use crate::kinds::{self, Kind, Verdict};

/// Public URL of the `ringside-api` Edge Script.
pub const DEFAULT_BASE_URL: &str = "https://ringside-api.bunny.run";

#[derive(Debug)]
pub enum Error {
    /// The server asked to wait longer than the client is willing to.
    RateLimited { retry_after: Option<Duration> },
    /// The server refused the request (4xx other than 404/429).
    Rejected { status: u16, message: String },
    Network(String),
    /// The server answered with something that does not fit the request.
    Protocol(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::RateLimited { retry_after: Some(d) } => write!(f, "rate limited, retry in {}s", d.as_secs()),
            Error::RateLimited { retry_after: None } => write!(f, "rate limited"),
            Error::Rejected { status, message } => write!(f, "rejected ({status}): {message}"),
            Error::Network(m) => write!(f, "network error: {m}"),
            Error::Protocol(m) => write!(f, "unexpected answer: {m}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitStatus {
    /// Queued for verification (or someone else's submission for the key already is).
    Submitted,
    /// A verified solution for the key already exists; nothing was stored.
    AlreadyVerified,
}

/// A parser failure: one hand the caller has already anonymized, nothing else.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub app_version: String,
    pub error: String,
    pub hand: String,
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    /// Tries in total, including the first.
    pub attempts: u32,
    pub base_delay: Duration,
    /// Longest single wait, also for a server's Retry-After.
    pub max_wait: Duration,
    /// Limit for one request, connecting included.
    pub timeout: Duration,
}

impl Default for RetryPolicy {
    fn default() -> RetryPolicy {
        RetryPolicy {
            attempts: 4,
            base_delay: Duration::from_millis(500),
            max_wait: Duration::from_secs(10),
            timeout: Duration::from_secs(20),
        }
    }
}

impl RetryPolicy {
    /// For lookups someone waits for: one try, a few seconds at most.
    pub fn impatient() -> RetryPolicy {
        RetryPolicy { attempts: 1, timeout: Duration::from_secs(4), ..RetryPolicy::default() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Failure {
    RateLimited(Option<Duration>),
    Server,
    Network,
}

/// Wait before try `failed + 1`, or `None` to give up. `jitter` in [0, 1) spreads clients
/// that failed together.
fn next_delay(p: &RetryPolicy, failed: u32, f: Failure, jitter: f64) -> Option<Duration> {
    if failed + 1 >= p.attempts {
        return None;
    }
    if let Failure::RateLimited(Some(d)) = f {
        return (d <= p.max_wait).then_some(d);
    }
    let backoff = p.base_delay.saturating_mul(1 << failed.min(16)).min(p.max_wait);
    Some(backoff.mul_f64(1.0 + 0.25 * jitter).min(p.max_wait))
}

fn jitter() -> f64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    (nanos % 1000) as f64 / 1000.0
}

pub struct Client {
    base: String,
    agent: ureq::Agent,
    policy: RetryPolicy,
}

#[derive(Deserialize)]
struct Served {
    kind: String,
    engine_version: u32,
    key: String,
    input: Value,
    result: Value,
}

#[derive(Deserialize)]
struct Submitted {
    key: String,
    status: String,
}

impl Client {
    pub fn new(base_url: &str) -> Client {
        Client::with_policy(base_url, RetryPolicy::default())
    }

    pub fn with_policy(base_url: &str, policy: RetryPolicy) -> Client {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(policy.timeout.min(Duration::from_secs(5)))
            .timeout(policy.timeout)
            .user_agent(concat!("ringside/", env!("CARGO_PKG_VERSION")))
            .build();
        Client { base: base_url.trim_end_matches('/').to_string(), agent, policy }
    }

    /// The verified solution for `input`, if the cloud has one. It is checked locally
    /// before it is returned, so a wrong answer from the server is an error, not a result.
    pub fn get_solution<K: Kind>(&self, input: &K::Input) -> Result<Option<K::Solution>, Error> {
        let key = K::key(input).map_err(|e| Error::Protocol(e.to_string()))?;
        let url = format!("{}/v1/solutions/{}/{key}", self.base, K::NAME);
        let Some((_, body)) = self.send(self.agent.get(&url), None, true)? else {
            return Ok(None);
        };
        let served: Served = serde_json::from_str(&body).map_err(|e| Error::Protocol(e.to_string()))?;
        let ours = serde_json::to_value(input).map_err(|e| Error::Protocol(e.to_string()))?;
        let same_input = canonicalize(&served.input).ok() == canonicalize(&ours).ok();
        if served.kind != K::NAME || served.engine_version != K::ENGINE_VERSION || served.key != key || !same_input {
            return Err(Error::Protocol("served solution is for a different input".into()));
        }
        let solution: K::Solution =
            serde_json::from_value(served.result).map_err(|e| Error::Protocol(e.to_string()))?;
        match kinds::verdict::<K>(input, &solution) {
            Verdict::Verified { .. } => Ok(Some(solution)),
            Verdict::Rejected { reason, .. } => Err(Error::Protocol(format!("served solution fails the local check: {reason}"))),
        }
    }

    /// Offers a locally computed solution; only call this with the user's consent.
    pub fn submit_solution<K: Kind>(&self, input: &K::Input, solution: &K::Solution) -> Result<SubmitStatus, Error> {
        let key = K::key(input).map_err(|e| Error::Protocol(e.to_string()))?;
        let body = kinds::submission::<K>(input, solution).to_string();
        let url = format!("{}/v1/solutions", self.base);
        let (_, text) = self
            .send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?
            .ok_or_else(|| Error::Protocol("404 on submit".into()))?;
        let r: Submitted = serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))?;
        if r.key != key {
            return Err(Error::Protocol("server computed a different key".into()));
        }
        match r.status.as_str() {
            "submitted" => Ok(SubmitStatus::Submitted),
            "verified" => Ok(SubmitStatus::AlreadyVerified),
            s => Err(Error::Protocol(format!("unknown status {s}"))),
        }
    }

    pub fn report(&self, report: &Report) -> Result<(), Error> {
        let body = serde_json::to_string(report).map_err(|e| Error::Protocol(e.to_string()))?;
        let url = format!("{}/v1/reports", self.base);
        self.send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?
            .ok_or_else(|| Error::Protocol("404 on report".into()))?;
        Ok(())
    }

    /// Sends `req` (with `body`, if any) with retries. `Ok(None)` for a 404 when `not_found_ok`.
    fn send(&self, req: ureq::Request, body: Option<&str>, not_found_ok: bool) -> Result<Option<(u16, String)>, Error> {
        let mut failed = 0;
        loop {
            let sent = match body {
                Some(b) => req.clone().send_string(b),
                None => req.clone().call(),
            };
            let (failure, error) = match sent {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
                    return Ok(Some((status, text)));
                }
                Err(ureq::Error::Status(404, _)) if not_found_ok => return Ok(None),
                Err(ureq::Error::Status(429, resp)) => {
                    let retry_after = resp
                        .header("Retry-After")
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .map(Duration::from_secs);
                    (Failure::RateLimited(retry_after), Error::RateLimited { retry_after })
                }
                Err(ureq::Error::Status(status, resp)) if status >= 500 => {
                    let message = resp.into_string().unwrap_or_default();
                    (Failure::Server, Error::Rejected { status, message })
                }
                Err(ureq::Error::Status(status, resp)) => {
                    let text = resp.into_string().unwrap_or_default();
                    let message = serde_json::from_str::<Value>(&text)
                        .ok()
                        .and_then(|v| v["message"].as_str().map(str::to_string))
                        .unwrap_or(text);
                    return Err(Error::Rejected { status, message });
                }
                Err(ureq::Error::Transport(t)) => (Failure::Network, Error::Network(t.to_string())),
            };
            match next_delay(&self.policy, failed, failure, jitter()) {
                Some(d) => std::thread::sleep(d),
                None => return Err(error),
            }
            failed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: RetryPolicy = RetryPolicy {
        attempts: 4,
        base_delay: Duration::from_millis(500),
        max_wait: Duration::from_secs(10),
        timeout: Duration::from_secs(20),
    };

    #[test]
    fn backs_off_exponentially_then_gives_up() {
        let d: Vec<_> = (0..4).map(|n| next_delay(&P, n, Failure::Server, 0.0)).collect();
        assert_eq!(d, [Some(Duration::from_millis(500)), Some(Duration::from_secs(1)), Some(Duration::from_secs(2)), None]);
        assert_eq!(next_delay(&P, 0, Failure::Network, 0.999), Some(Duration::from_millis(500).mul_f64(1.24975)));
    }

    #[test]
    fn honours_retry_after_only_when_short() {
        assert_eq!(next_delay(&P, 0, Failure::RateLimited(Some(Duration::from_secs(3))), 0.5), Some(Duration::from_secs(3)));
        assert_eq!(next_delay(&P, 0, Failure::RateLimited(Some(Duration::from_secs(3600))), 0.5), None);
        assert_eq!(next_delay(&P, 1, Failure::RateLimited(None), 0.0), Some(Duration::from_secs(1)));
    }

    #[test]
    fn backoff_is_capped() {
        let long = RetryPolicy { attempts: 40, ..P };
        assert_eq!(next_delay(&long, 30, Failure::Server, 0.9), Some(Duration::from_secs(10)));
    }
}
