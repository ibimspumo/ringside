//! Operator side: runs on the maintainer's machine or in the repository's scheduled
//! workflow with the full database token, never in the app.
//!
//! The API serves a solution as soon as its form checks out; nobody re-solves all of
//! them. The verifier re-solves a sample, clients new to it first, and a client whose
//! sample fails is banned and all its accepted solutions are withdrawn.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use serde_json::{Value, json};

use crate::canonical::canonicalize;
use crate::client::DEFAULT_BASE_URL;
use crate::kinds::pushfold_mw::POOL_PPM;
use crate::kinds::spot::canonical_flop;
use crate::kinds::{
    self, Kind, KindEntry, ModelLine, Postflop, PostflopInput, PushFoldMw, PushFoldMwInput, SpotInput, SpotKind, Verdict,
};
use ringside_engine::preflop_table::Family;
use ringside_engine::pushfold_table::Ante;
use ringside_play::TableKey;
use ringside_play::scenario::Scenario as ModelScenario;
use ringside_play::table::{PUSH_FOLD_MAX_BB, deep};

/// Largest decompressed result the verifier reads.
const MAX_RESULT: u64 = 1 << 20;
/// Rate-limit counters older than this are deleted (the API keeps the same horizon).
const KEEP_RATE_LIMITS_S: i64 = 2 * 86_400;
/// Parser reports and the submission log are deleted after this.
pub const KEEP_LOGS_S: i64 = 90 * 86_400;
/// Variables the operator tools read; taken from the process environment when no `.env`
/// file exists (the scheduled workflow passes them as secrets).
const KNOWN_VARS: [&str; 3] = ["BUNNY_DATABASE_URL", "BUNNY_DATABASE_AUTH_TOKEN", "BUNNY_API_KEY"];

/// `.env` next to the nearest ancestor of the working directory that has one; variables
/// set in the process environment win. Without a file (and without `path`) only the
/// process environment counts.
pub fn load_env(path: Option<&Path>) -> Result<HashMap<String, String>, String> {
    let path = match path {
        Some(p) => Some(p.to_path_buf()),
        None => find_env_file(),
    };
    let Some(path) = path else {
        return Ok(KNOWN_VARS
            .iter()
            .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
            .collect());
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

/// Runs statements in order, each on its own: the database, or SQLite in tests.
pub trait Sql {
    fn execute(&self, stmts: &[(&str, Vec<SqlValue>)]) -> Result<Vec<Rows>, String>;
}

impl Sql for Db {
    fn execute(&self, stmts: &[(&str, Vec<SqlValue>)]) -> Result<Vec<Rows>, String> {
        Db::execute(self, stmts)
    }
}

fn one(mut results: Vec<Rows>) -> Result<Rows, String> {
    if results.len() != 1 {
        return Err("one result expected".into());
    }
    Ok(results.remove(0))
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
                // Only the kind: the message names the database URL, and the verifier's
                // logs are public.
                ureq::Error::Transport(t) => format!("database unreachable: {}", t.kind()),
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

/// What became of one solution the verifier took up.
#[derive(Debug, Clone, PartialEq)]
pub enum Attempt {
    Verdict(Verdict),
    /// Not tried: unknown kind or engine version, or no time left.
    PutOff,
    /// Tried without a verdict (time limit, crash): a running out clock says nothing about
    /// the solution, so it stays as it is and the try is noted.
    NoVerdict(String),
}

impl Attempt {
    pub fn verdict(self) -> Option<Verdict> {
        match self {
            Attempt::Verdict(v) => Some(v),
            Attempt::PutOff | Attempt::NoVerdict(_) => None,
        }
    }
}

/// The note of a row after one more try without a verdict, counting the tries.
pub fn no_verdict_note(previous: Option<&str>, reason: &str) -> String {
    let tries = previous.and_then(|n| n.split_once(" tries")).and_then(|(n, _)| n.parse::<u32>().ok()).unwrap_or(0);
    format!("{} tries without verdict, last: {reason}", tries + 1)
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

fn gzip(text: &str) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut e, text.as_bytes()).expect("writing to memory");
    e.finish().expect("writing to memory")
}

/// Checks a stored row: (kind, key, canonical input JSON, result JSON) → attempt.
pub type Check<'a> = &'a dyn Fn(&KindEntry, &str, &str, &str) -> Attempt;

/// Runs a kind's check in this process.
pub fn check_here(entry: &KindEntry, key: &str, input: &str, result: &str) -> Attempt {
    Attempt::Verdict((entry.check)(key, input, result))
}

const ROW_COLUMNS: &str = "key, kind, engine_version, input, result, submitter, submitted_at, check_note";

/// A solution row as the verifier reads it.
struct Row {
    key: String,
    kind: String,
    entry: Option<&'static KindEntry>,
    input: String,
    /// The result JSON, or why it cannot be read.
    result: Result<String, String>,
    submitter: String,
    submitted_at: SqlValue,
    check_note: Option<String>,
}

fn read_rows(rows: &Rows) -> Vec<Row> {
    (0..rows.rows.len())
        .map(|i| {
            let text = |c: &str| rows.col(i, c).as_str().unwrap_or_default().to_string();
            let kind = text("kind");
            let version = rows.col(i, "engine_version").as_i64().unwrap_or(-1);
            Row {
                key: text("key"),
                entry: u32::try_from(version).ok().and_then(|v| kinds::find(&kind, v)),
                kind,
                input: text("input"),
                result: match rows.col(i, "result") {
                    SqlValue::Blob(b) => gunzip(b),
                    _ => Err("result is not a blob".into()),
                },
                submitter: text("submitter"),
                submitted_at: rows.col(i, "submitted_at").clone(),
                check_note: rows.col(i, "check_note").as_str().map(str::to_string),
            }
        })
        .collect()
}

/// What the form check made of a row submitted before the API accepted at once.
#[derive(Debug, Clone, PartialEq)]
pub enum Form {
    /// Served from now on, with the exploitability the solution reports, if its kind has one.
    Accepted(Option<f64>),
    Rejected(String),
    /// Unknown kind or engine version: left as it is.
    Unknown,
}

#[derive(Debug)]
pub struct Formed {
    pub key: String,
    pub kind: String,
    pub form: Form,
}

/// Rows per read: a postflop result is up to 180 KB.
const BATCH: u32 = 50;

/// Rows still `submitted` from before the API accepted at once: checked for their form
/// as a download is ([`kinds::accept`]), without solving, and accepted or rejected.
pub fn accept_submitted(db: &impl Sql, dry_run: bool) -> Result<Vec<Formed>, String> {
    let mut out = Vec::new();
    // Rows a batch leaves `submitted` stay first in line, so the next batch passes over them.
    let mut left = 0u32;
    loop {
        let sql = format!("SELECT {ROW_COLUMNS} FROM solutions WHERE status = 'submitted' ORDER BY submitted_at, key LIMIT ? OFFSET ?");
        let rows = one(db.execute(&[(&sql, vec![(BATCH as i64).into(), (left as i64).into()])])?)?;
        let batch = read_rows(&rows);
        for row in &batch {
            let form = match (row.entry, &row.result) {
                (None, _) => Form::Unknown,
                (Some(_), Err(reason)) => Form::Rejected(reason.clone()),
                (Some(entry), Ok(result)) => match (entry.accept)(&row.key, &row.input, result) {
                    Ok(e) => Form::Accepted(e),
                    Err(reason) => Form::Rejected(reason),
                },
            };
            if dry_run || form == Form::Unknown {
                left += 1;
            } else {
                let guard = vec![row.key.as_str().into(), row.submitted_at.clone()];
                let (sql, mut args): (&str, Vec<SqlValue>) = match &form {
                    Form::Accepted(e) => (
                        "UPDATE solutions SET status = 'accepted', exploitability = ? \
                         WHERE key = ? AND status = 'submitted' AND submitted_at = ?",
                        vec![e.map_or(SqlValue::Null, SqlValue::Float)],
                    ),
                    Form::Rejected(reason) => (
                        "UPDATE solutions SET status = 'rejected', checked_at = ?, check_note = ? \
                         WHERE key = ? AND status = 'submitted' AND submitted_at = ?",
                        vec![now().into(), format!("form: {reason}").as_str().into()],
                    ),
                    Form::Unknown => unreachable!("left as it is above"),
                };
                args.extend(guard);
                db.execute(&[(sql, args)])?;
            }
            out.push(Formed { key: row.key.clone(), kind: row.kind.clone(), form });
        }
        if batch.len() < BATCH as usize {
            return Ok(out);
        }
    }
}

/// Where one client stands with the verifier.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientStats {
    /// Salted hash of its IP, as the API stores it.
    pub submitter: String,
    /// Its solutions the verifier re-solved.
    pub verified: i64,
    /// Its accepted solutions not tried in this run.
    pub open: i64,
    /// Unix seconds of its oldest served solution.
    pub first_seen: i64,
}

/// The `n` clients to sample: the ones the verifier knows least first (fewest verified
/// solutions), among equals the one that showed up last.
pub fn pick_clients(mut stats: Vec<ClientStats>, n: usize) -> Vec<ClientStats> {
    stats.retain(|c| c.open > 0);
    stats.sort_by(|a, b| a.verified.cmp(&b.verified).then(b.first_seen.cmp(&a.first_seen)).then(a.submitter.cmp(&b.submitter)));
    stats.truncate(n);
    stats
}

/// What a failed check does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Penalty {
    /// Ban the client and withdraw all its accepted solutions.
    BanClient,
    /// Withdraw only this solution (the smoke test, whose client is the operator's).
    RowOnly,
}

#[derive(Debug)]
pub struct Sampled {
    pub key: String,
    pub kind: String,
    pub submitter: String,
    pub attempt: Attempt,
    /// Solutions withdrawn because this one failed: (kind, key).
    pub withdrawn: Vec<(String, String)>,
}

pub struct SampleOptions {
    /// Clients to take one solution of.
    pub clients: usize,
    /// Leave out rows tried without a verdict at or after this (unix seconds), i.e. in
    /// this run.
    pub tried_since: i64,
    /// Start no further check after this.
    pub deadline: Option<Instant>,
    /// Only report verdicts.
    pub dry_run: bool,
}

/// Re-solves one random accepted solution of each of [`pick_clients`]'s clients with
/// `check`; a failed one bans its client and withdraws all its accepted solutions.
pub fn sample(db: &impl Sql, opts: &SampleOptions, check: Check) -> Result<Vec<Sampled>, String> {
    let rows = one(db.execute(&[(
        "SELECT submitter, SUM(status = 'verified') AS verified, \
         SUM(status = 'accepted' AND (checked_at IS NULL OR checked_at < ?)) AS open, MIN(submitted_at) AS first_seen \
         FROM solutions WHERE status IN ('accepted', 'verified') \
         AND submitter NOT IN (SELECT submitter FROM banned_clients) GROUP BY submitter",
        vec![opts.tried_since.into()],
    )])?)?;
    let stats = (0..rows.rows.len())
        .map(|i| ClientStats {
            submitter: rows.col(i, "submitter").as_str().unwrap_or_default().to_string(),
            verified: rows.col(i, "verified").as_i64().unwrap_or(0),
            open: rows.col(i, "open").as_i64().unwrap_or(0),
            first_seen: rows.col(i, "first_seen").as_i64().unwrap_or(0),
        })
        .collect();
    let mut out = Vec::new();
    for client in pick_clients(stats, opts.clients) {
        if opts.deadline.is_some_and(|d| Instant::now() >= d) {
            break;
        }
        let sql = format!(
            "SELECT {ROW_COLUMNS} FROM solutions WHERE submitter = ? AND status = 'accepted' \
             AND (checked_at IS NULL OR checked_at < ?) ORDER BY random() LIMIT 1"
        );
        let rows = one(db.execute(&[(&sql, vec![client.submitter.as_str().into(), opts.tried_since.into()])])?)?;
        for row in read_rows(&rows) {
            out.push(check_row(db, &row, check, Penalty::BanClient, opts.dry_run)?);
        }
    }
    Ok(out)
}

/// Re-solves the accepted solutions under `keys` with `check`, as a sample would.
pub fn check_keys(db: &impl Sql, keys: &[String], check: Check, penalty: Penalty) -> Result<Vec<Sampled>, String> {
    let sql = format!(
        "SELECT {ROW_COLUMNS} FROM solutions WHERE status = 'accepted' AND key IN ({})",
        vec!["?"; keys.len()].join(",")
    );
    let rows = one(db.execute(&[(&sql, keys.iter().map(|k| SqlValue::from(k.as_str())).collect())])?)?;
    read_rows(&rows).iter().map(|row| check_row(db, row, check, penalty, false)).collect()
}

fn check_row(db: &impl Sql, row: &Row, check: Check, penalty: Penalty, dry_run: bool) -> Result<Sampled, String> {
    let attempt = match (row.entry, &row.result) {
        (None, _) => Attempt::PutOff,
        (Some(_), Err(reason)) => Attempt::Verdict(Verdict::Rejected { exploitability: None, reason: reason.clone() }),
        (Some(entry), Ok(result)) => check(entry, &row.key, &row.input, result),
    };
    let mut withdrawn = Vec::new();
    if !dry_run {
        let guard = || vec![SqlValue::from(row.key.as_str()), row.submitted_at.clone()];
        match &attempt {
            Attempt::PutOff => {}
            Attempt::NoVerdict(reason) => {
                let note = no_verdict_note(row.check_note.as_deref(), reason);
                let mut args = vec![now().into(), note.as_str().into()];
                args.extend(guard());
                db.execute(&[(
                    "UPDATE solutions SET checked_at = ?, check_note = ? WHERE key = ? AND status = 'accepted' AND submitted_at = ?",
                    args,
                )])?;
            }
            Attempt::Verdict(Verdict::Verified { exploitability, result }) => {
                let mut stmts = Vec::new();
                // The verifier's own rebuild replaces the submitted result first, so a row
                // never turns verified with the submitted one.
                if let Some(result) = result {
                    let mut args = vec![SqlValue::Blob(gzip(result)), (result.len() as i64).into()];
                    args.extend(guard());
                    stmts.push((
                        "UPDATE solutions SET result = ?, result_size = ? WHERE key = ? AND status = 'accepted' AND submitted_at = ?",
                        args,
                    ));
                }
                let mut args = vec![SqlValue::Float(*exploitability), now().into()];
                args.extend(guard());
                stmts.push((
                    "UPDATE solutions SET status = 'verified', exploitability = ?, checked_at = ?, check_note = NULL \
                     WHERE key = ? AND status = 'accepted' AND submitted_at = ?",
                    args,
                ));
                db.execute(&stmts)?;
            }
            Attempt::Verdict(Verdict::Rejected { reason, .. }) => {
                let note = format!("sample failed: {reason}");
                withdrawn = match penalty {
                    Penalty::BanClient => {
                        ban_client(db, &row.submitter, &note)?;
                        withdraw_client(db, &row.submitter, &note)?
                    }
                    Penalty::RowOnly => withdraw(db, "key = ?", row.key.as_str().into(), &note)?,
                };
            }
        }
    }
    Ok(Sampled { key: row.key.clone(), kind: row.kind.clone(), submitter: row.submitter.clone(), attempt, withdrawn })
}

/// Whether `s` is a client id as the API stores it: 32 lowercase hex digits.
pub fn is_client_id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The API refuses submissions of `submitter` from now on.
pub fn ban_client(db: &impl Sql, submitter: &str, reason: &str) -> Result<(), String> {
    db.execute(&[(
        "INSERT INTO banned_clients (submitter, banned_at, reason) VALUES (?, ?, ?) \
         ON CONFLICT (submitter) DO UPDATE SET banned_at = excluded.banned_at, reason = excluded.reason",
        vec![submitter.into(), now().into(), reason.into()],
    )])?;
    Ok(())
}

/// Lifts a ban; returns whether there was one.
pub fn unban_client(db: &impl Sql, submitter: &str) -> Result<bool, String> {
    Ok(one(db.execute(&[("DELETE FROM banned_clients WHERE submitter = ?", vec![submitter.into()])])?)?.affected > 0)
}

/// Sets every accepted (or still submitted) solution of `submitter` to rejected with
/// `reason`; verified ones were re-solved and stay. Returns the withdrawn (kind, key)s,
/// whose URLs the CDN may still hold ([`purge`]).
pub fn withdraw_client(db: &impl Sql, submitter: &str, reason: &str) -> Result<Vec<(String, String)>, String> {
    withdraw(db, "submitter = ?", submitter.into(), reason)
}

fn withdraw(db: &impl Sql, which: &str, arg: SqlValue, reason: &str) -> Result<Vec<(String, String)>, String> {
    let sql = format!(
        "UPDATE solutions SET status = 'rejected', checked_at = ?, check_note = ? \
         WHERE {which} AND status IN ('accepted', 'submitted') RETURNING kind, key"
    );
    let rows = one(db.execute(&[(&sql, vec![now().into(), reason.into(), arg])])?)?;
    Ok((0..rows.rows.len())
        .map(|i| {
            let text = |c: &str| rows.col(i, c).as_str().unwrap_or_default().to_string();
            (text("kind"), text("key"))
        })
        .collect())
}

/// How long the API lets the CDN keep an accepted solution (`ACCEPTED_MAX_AGE_S` in
/// server/edge/api.ts): a withdrawn one has left every edge after this, purged or not.
pub const ACCEPTED_MAX_AGE_S: u64 = 300;
/// Pause between a purge and looking whether it took.
const PURGE_WAIT: Duration = Duration::from_secs(5);
/// Purge rounds: until the cache time has run out, with a minute to spare.
const PURGE_ROUNDS: usize = ((ACCEPTED_MAX_AGE_S + 60) / PURGE_WAIT.as_secs()) as usize;

/// Asks the CDN to drop solution URLs. Its answer comes before every edge has dropped
/// them, and now and then one never does, so withdrawals go through [`purge_confirmed`].
/// Only for a ringside-* host.
pub fn purge(base: &str, api_key: &str, keys: &[(String, String)]) -> Result<(), String> {
    let host = base.trim_start_matches("https://");
    if !host.starts_with("ringside-") || host.contains('/') {
        return Err(format!("refusing to purge {base}: not a ringside-* host"));
    }
    for (kind, key) in keys {
        let url = format!("{base}/v1/solutions/{kind}/{key}");
        ureq::post("https://api.bunny.net/purge")
            .set("AccessKey", api_key)
            .query("url", &url)
            .call()
            .map_err(|e| format!("purge {url}: {}", e.kind()))?;
    }
    Ok(())
}

/// Purges `keys` until the CDN answers each URL with what `fresh(status, cache_control)`
/// accepts, here at the edge this process reaches; other edges drop them within
/// [`ACCEPTED_MAX_AGE_S`]. A look that re-fills the cache from a database replica still
/// behind is purged again in the next round.
pub fn purge_confirmed(
    base: &str,
    api_key: &str,
    keys: &[(String, String)],
    fresh: impl Fn(u16, &str) -> bool,
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).build();
    let stale = |(kind, key): &(String, String)| {
        let (status, cache) = match agent.get(&format!("{base}/v1/solutions/{kind}/{key}")).call() {
            Ok(r) => (r.status(), r.header("Cache-Control").unwrap_or_default().to_string()),
            Err(ureq::Error::Status(code, _)) => (code, String::new()),
            Err(ureq::Error::Transport(_)) => return true,
        };
        !fresh(status, &cache)
    };
    purge_until(keys, |k| purge(base, api_key, k), stale, std::thread::sleep, PURGE_ROUNDS)
}

/// [`purge_confirmed`] of withdrawn solutions on the public API: done once they answer 404.
pub fn purge_withdrawn(api_key: &str, keys: &[(String, String)]) -> Result<(), String> {
    purge_confirmed(DEFAULT_BASE_URL, api_key, keys, |status, _| status == 404)
}

fn purge_until(
    keys: &[(String, String)],
    mut purge: impl FnMut(&[(String, String)]) -> Result<(), String>,
    mut stale: impl FnMut(&(String, String)) -> bool,
    mut sleep: impl FnMut(Duration),
    rounds: usize,
) -> Result<(), String> {
    let mut left = keys.to_vec();
    for _ in 0..rounds {
        if left.is_empty() {
            break;
        }
        purge(&left)?;
        sleep(PURGE_WAIT);
        left.retain(&mut stale);
    }
    match left.as_slice() {
        [] => Ok(()),
        [(kind, key), ..] => Err(format!("{} URL(s) still served after {rounds} purges, first {kind}/{key}", left.len())),
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub rate_limits: u64,
    pub reports: u64,
    pub submissions: u64,
    /// Queue entries whose solution is served.
    pub work: u64,
}

/// Deletes rate-limit counters nobody looks at any more, and parser reports and
/// submission log entries older than [`KEEP_LOGS_S`].
pub fn prune(db: &impl Sql) -> Result<Pruned, String> {
    let t = now();
    let r = db.execute(&[
        ("DELETE FROM rate_limits WHERE window_start < ?", vec![(t - KEEP_RATE_LIMITS_S).into()]),
        ("DELETE FROM reports WHERE received_at < ?", vec![(t - KEEP_LOGS_S).into()]),
        ("DELETE FROM submissions WHERE received_at < ?", vec![(t - KEEP_LOGS_S).into()]),
        ("DELETE FROM work WHERE key IN (SELECT key FROM solutions WHERE status IN ('accepted', 'verified'))", vec![]),
    ])?;
    Ok(Pruned { rate_limits: r[0].affected, reports: r[1].affected, submissions: r[2].affected, work: r[3].affected })
}

// -----------------------------------------------------------------------------------
// The community queue ("Mitrechnen").

/// A spot the operator puts into the queue.
#[derive(Debug, Clone, PartialEq)]
pub struct Seed {
    pub key: String,
    pub kind: &'static str,
    pub engine_version: u32,
    /// Canonical JSON.
    pub input: String,
    /// "catalog" (catalog spots and practice lines on the catalog's flops) or "preflop".
    pub source: &'static str,
}

fn seed<K: Kind>(input: &K::Input, source: &'static str) -> Result<Seed, String> {
    K::check_input(input)?;
    let value = serde_json::to_value(input).map_err(|e| e.to_string())?;
    let canonical = canonicalize(&value).map_err(|e| e.to_string())?;
    Ok(Seed { key: K::key(input).map_err(|e| e.to_string())?, kind: K::NAME, engine_version: K::ENGINE_VERSION, input: canonical, source })
}

/// Final-table prize shares of a nine-handed table, first place first, in percent.
const FINAL_TABLE: [u32; 9] = [2500, 1700, 1250, 950, 750, 600, 450, 450, 350];
/// Average stacks of the push/fold grid, BB, and how unequal the stacks are: the
/// shortest at `1 - spread`, the deepest at `1 + spread` times the average.
const DEPTHS: [(u32, u32); 8] = [(5, 0), (8, 0), (10, 0), (12, 0), (15, 0), (20, 0), (10, 50), (15, 50)];

/// Shares of `weights` as parts of the ICM pool, rounding left over in first place.
fn ppm(weights: &[u32]) -> Vec<u32> {
    let total: u64 = weights.iter().map(|&w| w as u64).sum();
    let mut out: Vec<u32> = weights.iter().map(|&w| (w as u64 * POOL_PPM as u64 / total) as u32).collect();
    out[0] += POOL_PPM - out.iter().sum::<u32>();
    out
}

/// Push/fold spots the app does not ship (its tables are chip EV): ICM at final tables
/// (3 to 9 players left, the final-table payouts of those places) and on the bubble
/// (one place fewer paid than players left), at common depths with equal and unequal
/// stacks, big-blind ante and classic ante.
pub fn preflop_seeds() -> Result<Vec<Seed>, String> {
    let mut out = Vec::new();
    for n in 3..=9usize {
        let final_table = ppm(&FINAL_TABLE[..n]);
        let bubble = ppm(&(1..n as u32).rev().collect::<Vec<_>>());
        for (ante_mbb, bb_ante_mbb) in [(0, 1000), (125, 0)] {
            for (avg, spread) in DEPTHS {
                let stacks_mbb = (0..n)
                    .map(|i| {
                        let f = if n == 1 { 0 } else { (2 * i as i64 - (n as i64 - 1)) * spread as i64 / (n as i64 - 1) };
                        (avg as i64 * (1000 + 10 * f)) as u32
                    })
                    .collect::<Vec<u32>>();
                for payouts_ppm in [final_table.clone(), bubble.clone()] {
                    let input = PushFoldMwInput {
                        stacks_mbb: stacks_mbb.clone(),
                        sb_mbb: 500,
                        ante_mbb,
                        bb_ante_mbb,
                        dead_mbb: 0,
                        max_all_in: 2,
                        payouts_ppm,
                    };
                    out.push(seed::<PushFoldMw>(&input, "preflop")?);
                }
            }
        }
    }
    Ok(out)
}

/// Every spot of the library catalog whose scenario no pack ships (`shipped`: scenario ids).
pub fn catalog_seeds(shipped: &[String]) -> Result<Vec<Seed>, String> {
    let c = ringside_solver::library::catalog();
    c.ids()
        .filter(|id| !shipped.contains(&c.scenarios[id.scenario].id))
        .map(|id| seed::<Postflop>(&PostflopInput::new(&c.scenarios[id.scenario].id, &c.flops[id.flop]), "catalog"))
        .collect()
}

/// A line "Hände spielen" deals at a table: the model line, its heads-up flop and its share
/// of the table's flops.
pub struct PracticeLine {
    pub table: TableKey,
    pub model: ModelLine,
    pub scenario: ModelScenario,
    pub share: f64,
    /// A catalog scenario: the stock looks its catalog flops up as `postflop` spots.
    pub catalog: bool,
}

impl PracticeLine {
    /// E.g. "CO raise 2.2, BB call".
    pub fn label(&self) -> String {
        let names = ringside_engine::multiway::position_names(self.table.players);
        self.model
            .line
            .iter()
            .map(|s| {
                let action = format!("{:?}", s.action.kind).to_lowercase();
                format!("{} {action} {}", names.get(s.seat).copied().unwrap_or("?"), s.action.to_bb)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The lines of a practice table (big-blind ante, as practice starts), most dealt first:
/// the practice stock draws a heads-up flop of the plain model by how often the model
/// plays it (`ringside_play::Pool`). Up to 20 BB practice plays push/fold and deals no
/// flop, so those tables have no lines.
pub fn practice_lines(players: usize, stack_bb: f64) -> Result<Vec<PracticeLine>, String> {
    if stack_bb <= PUSH_FOLD_MAX_BB {
        return Err(format!("{stack_bb} BB: practice plays push/fold up to {PUSH_FOLD_MAX_BB} BB and deals no flop"));
    }
    let table = TableKey { players, ante: Ante::BigBlind, stack_bb };
    let deep = deep(table).ok_or(format!("no preflop model for {players} players at {stack_bb} BB"))?;
    let total: f64 = deep.scenarios.iter().map(|s| s.reach).sum();
    let c = ringside_solver::library::catalog();
    let mut lines: Vec<PracticeLine> = deep
        .scenarios
        .iter()
        .map(|sc| PracticeLine {
            table,
            model: ModelLine { family: Family::Plain, ante: table.ante, players, stack_bb, line: sc.line.clone() },
            scenario: sc.clone(),
            share: sc.reach / total,
            catalog: c.scenario_of(&sc.spot(&c.flops[0])).is_some(),
        })
        .collect();
    lines.sort_by(|a, b| b.share.total_cmp(&a.share));
    Ok(lines)
}

/// `n` of the catalog's flops, spread evenly over its order (texture, then rank) so a
/// smaller set keeps the textures' shares.
pub fn practice_flops(n: usize) -> Vec<String> {
    let flops = &ringside_solver::library::catalog().flops;
    let n = n.min(flops.len());
    (0..n).map(|i| flops[(2 * i + 1) * flops.len() / (2 * n)].clone()).collect()
}

/// The `spot` work of a line on `flops`: the input the practice stock looks each flop up
/// with. A catalog line has none; its spots are catalog seeds.
pub fn practice_seeds(line: &PracticeLine, flops: &[String]) -> Result<Vec<Seed>, String> {
    if line.catalog || flops.is_empty() {
        return Ok(Vec::new());
    }
    let base = SpotInput::from_model(&line.model, &line.scenario.spot(&flops[0]))?;
    flops
        .iter()
        .map(|f| seed::<SpotKind>(&SpotInput { board: canonical_flop(f)?, ..base.clone() }, "catalog"))
        .collect()
}

/// Scenario ids of the library packs in the signed index (`library-v1-<scenario>`).
pub fn shipped_scenarios() -> Result<Vec<String>, String> {
    let index = crate::packs::Packs::new(crate::packs::DEFAULT_BASE_URL).index(0)?;
    Ok(index.packs.keys().filter_map(|n| n.strip_prefix("library-v1-")).map(str::to_string).collect())
}

/// Takes the catalog spots of `shipped` scenarios out of the queue; returns how many.
pub fn unseed_shipped(db: &Db, shipped: &[String]) -> Result<u64, String> {
    let stmts: Vec<(&str, Vec<SqlValue>)> = shipped
        .iter()
        .map(|s| {
            (
                "DELETE FROM work WHERE kind = 'postflop' AND source = 'catalog' AND json_extract(input, '$.scenario') = ?",
                vec![s.as_str().into()],
            )
        })
        .collect();
    if stmts.is_empty() {
        return Ok(0);
    }
    Ok(db.execute(&stmts)?.iter().map(|r| r.affected).sum())
}

/// Rows per request when seeding; the SQL API takes a pipeline of statements.
const SEED_BATCH: usize = 200;

/// Puts `seeds` into the queue; seeds already queued keep their row. Returns how many
/// are new.
pub fn seed_work(db: &impl Sql, seeds: &[Seed]) -> Result<u64, String> {
    let t = now();
    let mut added = 0;
    for chunk in seeds.chunks(SEED_BATCH) {
        let stmts: Vec<(&str, Vec<SqlValue>)> = chunk
            .iter()
            .map(|s| {
                (
                    "INSERT INTO work (key, kind, engine_version, input, source, priority, created_at) \
                     SELECT ?, ?, ?, ?, ?, 0, ? WHERE NOT EXISTS \
                     (SELECT 1 FROM solutions WHERE key = ? AND status IN ('accepted', 'verified')) ON CONFLICT (key) DO NOTHING",
                    vec![
                        s.key.as_str().into(),
                        s.kind.into(),
                        (s.engine_version as i64).into(),
                        s.input.as_str().into(),
                        s.source.into(),
                        t.into(),
                        s.key.as_str().into(),
                    ],
                )
            })
            .collect();
        added += db.execute(&stmts)?.iter().map(|r| r.affected).sum::<u64>();
    }
    Ok(added)
}

/// Open entries of the queue by kind and source: (kind, source, rows, leased now).
pub fn queue_counts(db: &Db) -> Result<Vec<(String, String, i64, i64)>, String> {
    let [rows] = <[Rows; 1]>::try_from(db.execute(&[(
        "SELECT kind, source, COUNT(*) AS n, SUM(lease_until > ?) AS leased FROM work GROUP BY kind, source ORDER BY kind, source",
        vec![now().into()],
    )])?)
    .map_err(|_| "one result expected")?;
    Ok((0..rows.rows.len())
        .map(|i| {
            (
                rows.col(i, "kind").as_str().unwrap_or_default().to_string(),
                rows.col(i, "source").as_str().unwrap_or_default().to_string(),
                rows.col(i, "n").as_i64().unwrap_or(0),
                rows.col(i, "leased").as_i64().unwrap_or(0),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use ringside_engine::range::HandClass;
    use ringside_engine::solve_push_fold;
    use rusqlite::types::{Value as Sqlite, ValueRef};

    use crate::kinds::{PushFoldHu, PushFoldHuInput, PushFoldHuSolution};

    /// SQLite in memory with the server's migrations.
    struct Memory(rusqlite::Connection);

    impl Memory {
        fn new() -> Memory {
            let c = rusqlite::Connection::open_in_memory().unwrap();
            for sql in [
                include_str!("../../../server/db/migrations/0001_init.sql"),
                include_str!("../../../server/db/migrations/0002_work.sql"),
                include_str!("../../../server/db/migrations/0003_accept.sql"),
            ] {
                c.execute_batch(sql).unwrap();
            }
            Memory(c)
        }

        fn status(&self, key: &str) -> (String, Option<String>) {
            self.0
                .query_row("SELECT status, check_note FROM solutions WHERE key = ?", [key], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
        }

        fn banned(&self) -> Vec<String> {
            let mut st = self.0.prepare("SELECT submitter FROM banned_clients ORDER BY submitter").unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
        }
    }

    impl Sql for Memory {
        fn execute(&self, stmts: &[(&str, Vec<SqlValue>)]) -> Result<Vec<Rows>, String> {
            let e = |e: rusqlite::Error| e.to_string();
            stmts
                .iter()
                .map(|(sql, args)| {
                    let mut st = self.0.prepare(sql).map_err(e)?;
                    let params = args.iter().map(|a| match a {
                        SqlValue::Null => Sqlite::Null,
                        SqlValue::Integer(i) => Sqlite::Integer(*i),
                        SqlValue::Float(f) => Sqlite::Real(*f),
                        SqlValue::Text(t) => Sqlite::Text(t.clone()),
                        SqlValue::Blob(b) => Sqlite::Blob(b.clone()),
                    });
                    let columns: Vec<String> = st.column_names().iter().map(|c| c.to_string()).collect();
                    let mut rows = Vec::new();
                    let mut q = st.query(rusqlite::params_from_iter(params)).map_err(e)?;
                    while let Some(r) = q.next().map_err(e)? {
                        rows.push(
                            (0..columns.len())
                                .map(|i| match r.get_ref(i).unwrap() {
                                    ValueRef::Null => SqlValue::Null,
                                    ValueRef::Integer(i) => SqlValue::Integer(i),
                                    ValueRef::Real(f) => SqlValue::Float(f),
                                    ValueRef::Text(t) => SqlValue::Text(String::from_utf8_lossy(t).into()),
                                    ValueRef::Blob(b) => SqlValue::Blob(b.to_vec()),
                                })
                                .collect(),
                        );
                    }
                    drop(q);
                    Ok(Rows { columns, rows, affected: self.0.changes() })
                })
                .collect()
        }
    }

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    /// A heads-up push/fold row: solved, or with aces folded (`honest` false). Returns its key.
    fn hu(db: &Memory, stack_mbb: u32, honest: bool, status: &str, submitter: &str, at: i64) -> String {
        let input = PushFoldHuInput { stack_mbb, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 };
        let mut solution = PushFoldHuSolution::from(&solve_push_fold(&input.params()).unwrap());
        if !honest {
            solution.shove["AA".parse::<HandClass>().unwrap().index()] = 0.0;
        }
        let result = serde_json::to_string(&solution).unwrap();
        let key = PushFoldHu::key(&input).unwrap();
        row(db, &key, PushFoldHu::ENGINE_VERSION, &input, &result, status, submitter, at);
        key
    }

    #[allow(clippy::too_many_arguments)]
    fn row(db: &Memory, key: &str, version: u32, input: &PushFoldHuInput, result: &str, status: &str, submitter: &str, at: i64) {
        let input = canonicalize(&serde_json::to_value(input).unwrap()).unwrap();
        db.0.execute(
            "INSERT INTO solutions (key, kind, engine_version, input, result, result_size, status, submitter, submitted_at) \
             VALUES (?, 'pushfold-hu', ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![key, version, input, gzip(result), result.len(), status, submitter, at],
        )
        .unwrap();
    }

    fn opts(clients: usize) -> SampleOptions {
        SampleOptions { clients, tried_since: now(), deadline: None, dry_run: false }
    }

    #[test]
    fn withdrawing_a_client_takes_all_its_unverified_solutions() {
        let db = Memory::new();
        let accepted: Vec<String> = (0..3).map(|i| hu(&db, 10_000 + i, true, "accepted", A, 1)).collect();
        let waiting = hu(&db, 11_000, true, "submitted", A, 1);
        let verified = hu(&db, 12_000, true, "verified", A, 1);
        let other = hu(&db, 13_000, true, "accepted", B, 1);

        let mut withdrawn = withdraw_client(&db, A, "withdrawn by the operator").unwrap();
        withdrawn.sort();
        let mut expected: Vec<(String, String)> =
            accepted.iter().chain([&waiting]).map(|k| ("pushfold-hu".to_string(), k.clone())).collect();
        expected.sort();
        assert_eq!(withdrawn, expected);
        for k in accepted.iter().chain([&waiting]) {
            assert_eq!(db.status(k), ("rejected".into(), Some("withdrawn by the operator".into())));
        }
        assert_eq!(db.status(&verified).0, "verified", "a re-solved solution stays");
        assert_eq!(db.status(&other).0, "accepted", "other clients keep theirs");
        assert!(db.banned().is_empty(), "withdrawing alone bans nobody");
        ban_client(&db, A, "x").unwrap();
        assert_eq!(db.banned(), [A]);
        assert!(unban_client(&db, A).unwrap());
        assert!(db.banned().is_empty());
    }

    #[test]
    fn a_failed_sample_bans_its_client_and_withdraws_all_it_sent() {
        let db = Memory::new();
        let cheats = [hu(&db, 10_000, false, "accepted", A, 1), hu(&db, 11_000, false, "accepted", A, 2)];
        let honest = hu(&db, 12_000, true, "accepted", B, 3);

        let sampled = sample(&db, &opts(5), &check_here).unwrap();
        assert_eq!(sampled.len(), 2, "one solution of each client");
        let of = |c: &str| sampled.iter().find(|s| s.submitter == c).unwrap();
        assert!(matches!(of(A).attempt, Attempt::Verdict(Verdict::Rejected { .. })), "{:?}", of(A).attempt);
        assert_eq!(of(A).withdrawn.len(), 2);
        assert!(matches!(of(B).attempt, Attempt::Verdict(Verdict::Verified { .. })), "{:?}", of(B).attempt);
        assert!(of(B).withdrawn.is_empty());

        assert_eq!(db.banned(), [A]);
        for k in &cheats {
            let (status, note) = db.status(k);
            assert_eq!(status, "rejected");
            assert!(note.unwrap().starts_with("sample failed: exploitability"));
        }
        assert_eq!(db.status(&honest), ("verified".into(), None));
        assert!(sample(&db, &opts(5), &check_here).unwrap().is_empty(), "a banned client is not sampled again");
    }

    #[test]
    fn samples_go_to_new_clients_first() {
        let stats = |submitter: &str, verified, open, first_seen| ClientStats { submitter: submitter.into(), verified, open, first_seen };
        let picked = pick_clients(
            vec![stats("known", 12, 40, 1), stats("new", 0, 3, 100), stats("newest", 0, 1, 200), stats("done", 0, 0, 300)],
            2,
        );
        assert_eq!(picked.iter().map(|c| c.submitter.as_str()).collect::<Vec<_>>(), ["newest", "new"]);

        // In the database: B has two verified solutions, A none, so A is sampled.
        let db = Memory::new();
        let a = hu(&db, 10_000, true, "accepted", A, 5);
        hu(&db, 11_000, true, "verified", B, 1);
        hu(&db, 12_000, true, "verified", B, 1);
        hu(&db, 13_000, true, "accepted", B, 1);
        let seen = std::cell::RefCell::new(Vec::new());
        let check = |_: &KindEntry, key: &str, _: &str, _: &str| {
            seen.borrow_mut().push(key.to_string());
            Attempt::NoVerdict("test".into())
        };
        sample(&db, &SampleOptions { dry_run: true, ..opts(1) }, &check).unwrap();
        assert_eq!(*seen.borrow(), [a]);
    }

    #[test]
    fn rows_waiting_from_before_are_accepted_by_their_form_alone() {
        let db = Memory::new();
        let good = hu(&db, 10_000, true, "submitted", A, 1);
        let input = PushFoldHuInput { stack_mbb: 11_000, sb_mbb: 500, ante_mbb: 0, dead_mbb: 0 };
        let malformed = PushFoldHu::key(&input).unwrap();
        row(&db, &malformed, 1, &input, "{\"shove\":[]}", "submitted", A, 1);
        let other_key = "0".repeat(64);
        row(&db, &other_key, 1, &input, "{}", "submitted", A, 1);
        let unknown_input = PushFoldHuInput { stack_mbb: 12_000, ..input };
        row(&db, "1".repeat(64).as_str(), 99, &unknown_input, "{}", "submitted", A, 1);

        let formed = accept_submitted(&db, false).unwrap();
        let form = |k: &str| formed.iter().find(|f| f.key == k).unwrap().form.clone();
        assert_eq!(form(&good), Form::Accepted(None));
        assert!(matches!(form(&malformed), Form::Rejected(r) if r.starts_with("result")));
        assert_eq!(form(&other_key), Form::Rejected("key does not match input".into()));
        assert_eq!(form(&"1".repeat(64)), Form::Unknown);
        assert_eq!(db.status(&good).0, "accepted");
        assert!(db.status(&malformed).1.unwrap().starts_with("form: result"));
        assert_eq!(db.status(&"1".repeat(64)).0, "submitted", "an unknown version waits");
        assert_eq!(accept_submitted(&db, false).unwrap().len(), 1, "only the unknown one is left");
    }

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
    fn tries_without_verdict_are_counted_in_the_note() {
        let first = no_verdict_note(None, "check took longer than 900s");
        assert_eq!(first, "1 tries without verdict, last: check took longer than 900s");
        assert_eq!(no_verdict_note(Some(&first), "check crashed"), "2 tries without verdict, last: check crashed");
        assert_eq!(no_verdict_note(Some("something else"), "x"), "1 tries without verdict, last: x");
    }

    #[test]
    fn seeds_are_valid_unique_and_cover_the_gaps() {
        let pre = preflop_seeds().unwrap();
        let cat = catalog_seeds(&[]).unwrap();
        assert_eq!(pre.len(), 7 * 2 * DEPTHS.len() * 2);
        assert_eq!(cat.len(), ringside_solver::library::catalog().len());
        let c = ringside_solver::library::catalog();
        let without = catalog_seeds(&["srp-btn-bb-40".into()]).unwrap();
        assert_eq!(without.len(), cat.len() - c.flops.len(), "a shipped scenario is not open");
        let mut keys: Vec<&str> = pre.iter().chain(&cat).map(|s| s.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), pre.len() + cat.len(), "no spot twice");
        for s in &pre {
            let input: PushFoldMwInput = serde_json::from_str(&s.input).unwrap();
            assert_eq!(input.payouts_ppm.iter().sum::<u32>(), POOL_PPM);
            assert!(input.payouts_ppm.len() < input.stacks_mbb.len() || input.payouts_ppm.len() == input.stacks_mbb.len());
        }
    }

    fn url(key: char) -> (String, String) {
        ("pushfold-hu".to_string(), key.to_string().repeat(64))
    }

    #[test]
    fn a_purge_that_did_not_take_is_repeated_until_the_edge_answers_fresh() {
        let (a, b) = (url('a'), url('b'));
        let mut purged: Vec<Vec<(String, String)>> = Vec::new();
        let mut looks = HashMap::new();
        let mut waited = Duration::ZERO;
        // `a` is still served twice: the first purge never took, the next look re-filled
        // the cache from a replica behind; `b` is gone after the first purge.
        let r = purge_until(
            &[a.clone(), b.clone()],
            |k| {
                purged.push(k.to_vec());
                Ok(())
            },
            |k| {
                let n = looks.entry(k.clone()).or_insert(0);
                *n += 1;
                *k == a && *n <= 2
            },
            |d| waited += d,
            PURGE_ROUNDS,
        );
        assert_eq!(r, Ok(()));
        assert_eq!(purged, vec![vec![a.clone(), b], vec![a.clone()], vec![a]]);
        assert_eq!(waited, PURGE_WAIT * 3, "it looks only after waiting for the purge");
    }

    #[test]
    fn an_edge_that_keeps_serving_fails_after_the_cache_time() {
        let mut purges = 0;
        let mut waited = Duration::ZERO;
        let purge = |_: &[(String, String)]| {
            purges += 1;
            Ok(())
        };
        let r = purge_until(&[url('c')], purge, |_| true, |d| waited += d, PURGE_ROUNDS);
        assert!(r.is_err_and(|e| e.contains(&"c".repeat(64))));
        assert_eq!(purges, PURGE_ROUNDS);
        assert!(waited > Duration::from_secs(ACCEPTED_MAX_AGE_S), "by then the edge's copy has run out");
    }

    #[test]
    fn the_cache_time_is_the_apis() {
        let api = include_str!("../../../server/edge/api.ts");
        assert!(api.contains(&format!("export const ACCEPTED_MAX_AGE_S = {ACCEPTED_MAX_AGE_S};")));
    }

    #[test]
    fn practice_seeds_cover_the_most_dealt_lines_and_spare_queued_and_served_spots() {
        assert!(practice_lines(6, 20.0).is_err(), "practice plays push/fold at 20 BB");
        let lines = practice_lines(6, 40.0).unwrap();
        assert!(lines.windows(2).all(|w| w[0].share >= w[1].share));
        assert!((lines.iter().map(|l| l.share).sum::<f64>() - 1.0).abs() < 1e-9);
        let c = ringside_solver::library::catalog();
        assert_eq!(practice_flops(c.flops.len()), c.flops);
        let flops = practice_flops(3);
        assert!(flops.iter().all(|f| c.flop(f).is_some()));
        let seeds: Vec<Seed> = lines.iter().take(2).flat_map(|l| practice_seeds(l, &flops).unwrap()).collect();
        let mut keys: Vec<&str> = seeds.iter().map(|s| s.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        assert_eq!((seeds.len(), keys.len()), (6, 6));
        assert!(seeds.iter().all(|s| s.kind == SpotKind::NAME));
        let catalog_line = practice_lines(9, 40.0).unwrap().into_iter().find(|l| l.catalog).expect("a 9-max catalog line");
        assert!(practice_seeds(&catalog_line, &flops).unwrap().is_empty(), "catalog lines are catalog seeds");

        let db = Memory::new();
        db.0.execute(
            "INSERT INTO work (key, kind, engine_version, input, source, priority, created_at) VALUES (?, 'spot', 1, ?, 'request', 3, 0)",
            [&seeds[0].key, &seeds[0].input],
        )
        .unwrap();
        db.0.execute(
            "INSERT INTO solutions (key, kind, engine_version, input, result, result_size, status, submitter, submitted_at) \
             VALUES (?, 'spot', 1, ?, x'00', 1, 'accepted', 'x', 0)",
            [&seeds[1].key, &seeds[1].input],
        )
        .unwrap();
        assert_eq!(seed_work(&db, &seeds).unwrap(), 4);
        assert_eq!(seed_work(&db, &seeds).unwrap(), 0, "queued once");
        let request: (String, i64) =
            db.0.query_row("SELECT source, priority FROM work WHERE key = ?", [&seeds[0].key], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(request, ("request".into(), 3), "a requested spot keeps its row");
        let served: i64 = db.0.query_row("SELECT COUNT(*) FROM work WHERE key = ?", [&seeds[1].key], |r| r.get(0)).unwrap();
        assert_eq!(served, 0, "a served spot is not queued");
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
