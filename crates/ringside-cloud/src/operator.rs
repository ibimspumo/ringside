//! Operator side: runs on the maintainer's machine with the full database token from the
//! repository's `.env`, never in the app. Verifies submitted solutions and sets their status.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use serde_json::{Value, json};

use crate::kinds::{self, Verdict};

/// Largest decompressed result the verifier reads.
const MAX_RESULT: u64 = 1 << 20;
/// Rate-limit counters older than this are deleted (the API keeps the same horizon).
const KEEP_RATE_LIMITS_S: i64 = 2 * 86_400;

/// `.env` next to the nearest ancestor of the working directory that has one; variables
/// set in the process environment win.
pub fn load_env(path: Option<&Path>) -> Result<HashMap<String, String>, String> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => find_env_file().ok_or("no .env found in the working directory or above")?,
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut env = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
            env.insert(k.trim().to_string(), v.to_string());
        }
    }
    for (k, v) in env.iter_mut() {
        if let Ok(own) = std::env::var(k) {
            *v = own;
        }
    }
    Ok(env)
}

fn find_env_file() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".env");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

pub fn need<'a>(env: &'a HashMap<String, String>, name: &str) -> Result<&'a str, String> {
    env.get(name)
        .map(String::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("{name} is not set (expected in .env or the environment)"))
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            SqlValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            SqlValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    fn to_wire(&self) -> Value {
        match self {
            SqlValue::Null => json!({"type": "null"}),
            SqlValue::Integer(i) => json!({"type": "integer", "value": i.to_string()}),
            SqlValue::Float(f) => json!({"type": "float", "value": f}),
            SqlValue::Text(s) => json!({"type": "text", "value": s}),
            SqlValue::Blob(b) => json!({"type": "blob", "base64": STANDARD_NO_PAD.encode(b)}),
        }
    }

    fn from_wire(v: &Value) -> Result<SqlValue, String> {
        Ok(match v["type"].as_str() {
            Some("null") => SqlValue::Null,
            Some("integer") => SqlValue::Integer(
                v["value"].as_str().and_then(|s| s.parse().ok()).ok_or("bad integer")?,
            ),
            Some("float") => SqlValue::Float(v["value"].as_f64().ok_or("bad float")?),
            Some("text") => SqlValue::Text(v["value"].as_str().ok_or("bad text")?.to_string()),
            Some("blob") => {
                let b64 = v["base64"].as_str().ok_or("bad blob")?.trim_end_matches('=');
                SqlValue::Blob(STANDARD_NO_PAD.decode(b64).map_err(|e| e.to_string())?)
            }
            _ => return Err(format!("unknown value {v}")),
        })
    }
}

impl From<&str> for SqlValue {
    fn from(s: &str) -> SqlValue {
        SqlValue::Text(s.to_string())
    }
}

impl From<i64> for SqlValue {
    fn from(i: i64) -> SqlValue {
        SqlValue::Integer(i)
    }
}

#[derive(Debug, Default)]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
    pub affected: u64,
}

impl Rows {
    pub fn col(&self, row: usize, name: &str) -> &SqlValue {
        let i = self.columns.iter().position(|c| c == name).expect("column in query");
        &self.rows[row][i]
    }
}

/// Bunny Database over its SQL API (libSQL Hrana over HTTP, `/v2/pipeline`).
pub struct Db {
    url: String,
    token: String,
    agent: ureq::Agent,
}

impl Db {
    pub fn new(database_url: &str, token: &str) -> Db {
        let base = database_url.trim().trim_end_matches('/').replacen("libsql://", "https://", 1);
        Db {
            url: format!("{base}/v2/pipeline"),
            token: token.to_string(),
            agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build(),
        }
    }

    /// With the full-access token: the verifier writes statuses.
    pub fn from_env(env: &HashMap<String, String>) -> Result<Db, String> {
        Ok(Db::new(need(env, "BUNNY_DATABASE_URL")?, need(env, "BUNNY_DATABASE_AUTH_TOKEN")?))
    }

    /// Runs the statements in order, each on its own.
    pub fn execute(&self, stmts: &[(&str, Vec<SqlValue>)]) -> Result<Vec<Rows>, String> {
        let mut requests: Vec<Value> = stmts
            .iter()
            .map(|(sql, args)| {
                json!({"type": "execute", "stmt": {"sql": sql, "args": args.iter().map(SqlValue::to_wire).collect::<Vec<_>>()}})
            })
            .collect();
        requests.push(json!({"type": "close"}));
        let resp = self
            .agent
            .post(&self.url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/json")
            .send_string(&json!({ "requests": requests }).to_string())
            .map_err(|e| match e {
                ureq::Error::Status(code, r) => format!("database HTTP {code}: {}", r.into_string().unwrap_or_default()),
                ureq::Error::Transport(t) => format!("database unreachable: {t}"),
            })?;
        let body: Value = serde_json::from_str(&resp.into_string().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let results = body["results"].as_array().ok_or("no results in pipeline response")?;
        stmts
            .iter()
            .zip(results)
            .map(|((sql, _), r)| {
                if r["type"] != "ok" {
                    return Err(format!("{sql}: {}", r["error"]["message"]));
                }
                let res = &r["response"]["result"];
                let columns = res["cols"]
                    .as_array()
                    .map(|c| c.iter().map(|c| c["name"].as_str().unwrap_or("").to_string()).collect())
                    .unwrap_or_default();
                let rows = res["rows"]
                    .as_array()
                    .map(|rows| {
                        rows.iter()
                            .map(|row| row.as_array().into_iter().flatten().map(SqlValue::from_wire).collect())
                            .collect::<Result<Vec<Vec<SqlValue>>, String>>()
                    })
                    .transpose()?
                    .unwrap_or_default();
                Ok(Rows { columns, rows, affected: res["affected_row_count"].as_u64().unwrap_or(0) })
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct Checked {
    pub key: String,
    pub kind: String,
    /// `None` if this verifier does not know the kind or engine version (left pending).
    pub verdict: Option<Verdict>,
}

fn gunzip(bytes: &[u8]) -> Result<String, String> {
    let mut out = String::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_RESULT + 1)
        .read_to_string(&mut out)
        .map_err(|e| format!("result does not decompress: {e}"))?;
    if out.len() as u64 > MAX_RESULT {
        return Err("result too large".into());
    }
    Ok(out)
}

/// Checks up to `limit` pending solutions (only `keys` if given), oldest first, and stores
/// each verdict unless `dry_run`.
pub fn verify_pending(db: &Db, keys: Option<&[String]>, limit: u32, dry_run: bool) -> Result<Vec<Checked>, String> {
    let mut sql = String::from(
        "SELECT key, kind, engine_version, input, result, submitted_at FROM solutions WHERE status = 'submitted'",
    );
    let mut args: Vec<SqlValue> = Vec::new();
    if let Some(keys) = keys {
        sql.push_str(&format!(" AND key IN ({})", vec!["?"; keys.len()].join(",")));
        args.extend(keys.iter().map(|k| SqlValue::from(k.as_str())));
    }
    sql.push_str(" ORDER BY submitted_at LIMIT ?");
    args.push(SqlValue::Integer(limit as i64));
    let [rows] = <[Rows; 1]>::try_from(db.execute(&[(&sql, args)])?).map_err(|_| "one result expected")?;

    let mut out = Vec::new();
    for i in 0..rows.rows.len() {
        let key = rows.col(i, "key").as_str().unwrap_or_default().to_string();
        let kind = rows.col(i, "kind").as_str().unwrap_or_default().to_string();
        let version = rows.col(i, "engine_version").as_i64().unwrap_or(-1);
        let Some(entry) = u32::try_from(version).ok().and_then(|v| kinds::find(&kind, v)) else {
            out.push(Checked { key, kind, verdict: None });
            continue;
        };
        let input = rows.col(i, "input").as_str().unwrap_or_default();
        let verdict = match rows.col(i, "result") {
            SqlValue::Blob(b) => match gunzip(b) {
                Ok(result) => (entry.check)(&key, input, &result),
                Err(reason) => Verdict::Rejected { exploitability: None, reason },
            },
            _ => Verdict::Rejected { exploitability: None, reason: "result is not a blob".into() },
        };
        if !dry_run {
            let (status, exploitability, note) = match &verdict {
                Verdict::Verified { exploitability } => ("verified", Some(*exploitability), None),
                Verdict::Rejected { exploitability, reason } => ("rejected", *exploitability, Some(reason.as_str())),
            };
            db.execute(&[(
                "UPDATE solutions SET status = ?, exploitability = ?, checked_at = ?, check_note = ? \
                 WHERE key = ? AND status = 'submitted' AND submitted_at = ?",
                vec![
                    status.into(),
                    exploitability.map_or(SqlValue::Null, SqlValue::Float),
                    now().into(),
                    note.map_or(SqlValue::Null, SqlValue::from),
                    key.as_str().into(),
                    rows.col(i, "submitted_at").clone(),
                ],
            )])?;
        }
        out.push(Checked { key, kind, verdict: Some(verdict) });
    }
    Ok(out)
}

/// Deletes rate-limit counters nobody looks at any more; returns how many.
pub fn prune_rate_limits(db: &Db) -> Result<u64, String> {
    let r = db.execute(&[("DELETE FROM rate_limits WHERE window_start < ?", vec![(now() - KEEP_RATE_LIMITS_S).into()])])?;
    Ok(r[0].affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn wire_values_round_trip() {
        for v in [
            SqlValue::Null,
            SqlValue::Integer(-(1 << 60)),
            SqlValue::Float(0.25),
            SqlValue::Text("x".into()),
            SqlValue::Blob(vec![0, 255, 7]),
        ] {
            assert_eq!(SqlValue::from_wire(&v.to_wire()).unwrap(), v);
        }
        let padded = json!({"type": "blob", "base64": "AP8H"});
        assert_eq!(SqlValue::from_wire(&padded).unwrap(), SqlValue::Blob(vec![0, 255, 7]));
    }

    #[test]
    fn gunzip_caps_size() {
        let gz = |data: &[u8]| {
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).unwrap();
            e.finish().unwrap()
        };
        assert_eq!(gunzip(&gz(b"{\"a\":1}")).unwrap(), "{\"a\":1}");
        assert!(gunzip(&gz(&vec![b' '; MAX_RESULT as usize + 10])).is_err());
        assert!(gunzip(b"not gzip").is_err());
    }
}
