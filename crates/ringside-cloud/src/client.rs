//! HTTP client for the Ringside Cloud API: blocking, rustls, with timeouts and retries.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canonical::canonicalize;
use crate::kinds::{self, Kind, SpotInput, SpotKind};

/// Public URL of the `ringside-api` Edge Script.
pub const DEFAULT_BASE_URL: &str = "https://ringside-api.bunny.run";
/// The server keeps this much of a failure's reason.
const MAX_FAIL_NOTE: usize = 300;

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
    /// Served to everyone from now on (or someone else's solution for the key already is).
    Accepted,
    /// Waits for the verifier's form check: a server from before accepting at once, or
    /// someone else's submission from then.
    Submitted,
    /// A verified solution for the key already exists; nothing was stored.
    AlreadyVerified,
}

/// Where a submitted key stands on the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStatus {
    /// Served to everyone; its form was checked, it was not re-solved.
    Accepted,
    /// Re-solved by the verifier and served to everyone.
    Verified,
    /// Waits for the verifier.
    Submitted,
    /// Refused or withdrawn, with the verifier's note.
    Rejected(Option<String>),
    /// The server has nothing for the key.
    Unknown,
}

/// Keys per status request; mirrors `MAX_STATUS_KEYS` in `server/edge/api.ts`.
pub const MAX_STATUS_KEYS: usize = 200;

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

/// A spot of the community queue this computer may solve until `lease_until`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Work {
    pub key: String,
    pub kind: String,
    pub engine_version: u32,
    pub input: Value,
    /// Unix seconds; afterwards the server hands the spot to someone else.
    pub lease_until: u64,
}

#[derive(Deserialize)]
struct NextWork {
    work: Option<Work>,
}

#[derive(Deserialize)]
struct Requested {
    key: String,
    status: String,
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
struct StatusEntry {
    status: String,
    note: Option<String>,
}

#[derive(Deserialize)]
struct Statuses {
    statuses: std::collections::HashMap<String, StatusEntry>,
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

    /// The solution for `input`, if the cloud has one. It is checked locally before it is
    /// returned (form, reported exploitability, for push/fold the full check), so a wrong
    /// answer from the server is an error, not a result.
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
        kinds::accept::<K>(input, &solution)
            .map(|_| Some(solution))
            .map_err(|reason| Error::Protocol(format!("served solution fails the local check: {reason}")))
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
            "accepted" => Ok(SubmitStatus::Accepted),
            "submitted" => Ok(SubmitStatus::Submitted),
            "verified" => Ok(SubmitStatus::AlreadyVerified),
            s => Err(Error::Protocol(format!("unknown status {s}"))),
        }
    }

    /// Where each of up to [`MAX_STATUS_KEYS`] keys stands, in one request.
    pub fn statuses(&self, keys: &[String]) -> Result<Vec<(String, KeyStatus)>, Error> {
        if keys.is_empty() || keys.len() > MAX_STATUS_KEYS {
            return Err(Error::Protocol(format!("1 to {MAX_STATUS_KEYS} keys per request")));
        }
        let body = serde_json::json!({ "keys": keys }).to_string();
        let url = format!("{}/v1/solutions/status", self.base);
        let (_, text) = self
            .send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?
            .ok_or_else(|| Error::Protocol("404 on status".into()))?;
        let mut r: Statuses = serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))?;
        keys.iter()
            .map(|k| {
                let e = r.statuses.remove(k).ok_or_else(|| Error::Protocol(format!("no status for {k}")))?;
                let status = match e.status.as_str() {
                    "accepted" => KeyStatus::Accepted,
                    "verified" => KeyStatus::Verified,
                    "submitted" => KeyStatus::Submitted,
                    "rejected" => KeyStatus::Rejected(e.note),
                    "unknown" => KeyStatus::Unknown,
                    s => return Err(Error::Protocol(format!("unknown status {s}"))),
                };
                Ok((k.clone(), status))
            })
            .collect()
    }

    /// The flops on which the line of `input` (a model flop on any flop) is served, as
    /// canonical flops, in one request. `None` from a server without the endpoint (API
    /// before 1.5.0): the caller asks [`Client::statuses`] instead.
    pub fn served_flops(&self, input: &SpotInput) -> Result<Option<Vec<String>>, Error> {
        let body = serde_json::json!({ "kind": SpotKind::NAME, "engine_version": SpotKind::ENGINE_VERSION, "input": input }).to_string();
        let url = format!("{}/v1/solutions/flops", self.base);
        let Some((_, text)) = self.send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), true)? else {
            return Ok(None);
        };
        #[derive(Deserialize)]
        struct Flops {
            flops: Vec<String>,
        }
        let r: Flops = serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))?;
        Ok(Some(r.flops))
    }

    /// The next spot of the community queue among `kinds` (name and engine version),
    /// leased to this computer, or `None` when nothing is open.
    pub fn next_work(&self, kinds: &[(&str, u32)]) -> Result<Option<Work>, Error> {
        let mut map = serde_json::Map::new();
        for (name, version) in kinds {
            let entry = map.entry(name.to_string()).or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(v) = entry {
                v.push((*version).into());
            }
        }
        let body = serde_json::json!({ "kinds": map }).to_string();
        let url = format!("{}/v1/work/next", self.base);
        let (_, text) = self
            .send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?
            .ok_or_else(|| Error::Protocol("404 on work".into()))?;
        let r: NextWork = serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))?;
        if let Some(w) = &r.work {
            let key = crate::canonical::canonicalize(&w.input)
                .map(|c| crate::canonical::solution_key(&w.kind, w.engine_version, &c))
                .map_err(|e| Error::Protocol(e.to_string()))?;
            if key != w.key {
                return Err(Error::Protocol("work key does not match its input".into()));
            }
        }
        Ok(r.work)
    }

    /// Gives the lease on `key` back before it runs out, so the spot is handed out again
    /// at once. The server only frees a lease this client holds.
    pub fn release_work(&self, key: &str) -> Result<(), Error> {
        let body = serde_json::json!({ "key": key }).to_string();
        let url = format!("{}/v1/work/release", self.base);
        self.send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?
            .ok_or_else(|| Error::Protocol("404 on release".into()))?;
        Ok(())
    }

    /// Reports that this computer cannot solve the spot it holds on `key` (`note` says why):
    /// the server does not hand it out here again for a while and parks it after a few such
    /// reports. False from a server without the endpoint (API before 1.6).
    pub fn fail_work(&self, key: &str, note: &str) -> Result<bool, Error> {
        let note: String = note.chars().take(MAX_FAIL_NOTE).collect();
        let body = serde_json::json!({ "key": key, "note": note }).to_string();
        let url = format!("{}/v1/work/fail", self.base);
        Ok(self.send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?.is_some())
    }

    /// Asks the community to solve `input`, which the cloud does not have; only with the
    /// user's consent to share. Returns the server's status: "queued", or the status of the
    /// solution it already has ("accepted", "submitted", "verified").
    pub fn request_work<K: Kind>(&self, input: &K::Input) -> Result<String, Error> {
        let key = K::key(input).map_err(|e| Error::Protocol(e.to_string()))?;
        let body = serde_json::json!({ "kind": K::NAME, "engine_version": K::ENGINE_VERSION, "input": input }).to_string();
        let url = format!("{}/v1/work/request", self.base);
        let (_, text) = self
            .send(self.agent.post(&url).set("Content-Type", "application/json"), Some(&body), false)?
            .ok_or_else(|| Error::Protocol("404 on request".into()))?;
        let r: Requested = serde_json::from_str(&text).map_err(|e| Error::Protocol(e.to_string()))?;
        if r.key != key {
            return Err(Error::Protocol("server computed a different key".into()));
        }
        Ok(r.status)
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
