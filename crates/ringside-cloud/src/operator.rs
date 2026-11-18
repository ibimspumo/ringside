//! Operator side: runs on the maintainer's machine or in the repository's scheduled
//! workflow with the full database token, never in the app. Verifies submitted solutions,
//! sets their status and deletes old rows.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use serde_json::{Value, json};

use crate::canonical::canonicalize;
use crate::kinds::pushfold_mw::POOL_PPM;
use crate::kinds::{self, Kind, KindEntry, Postflop, PostflopInput, PushFoldMw, PushFoldMwInput, Verdict};

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

/// What became of one pending solution in a run.
#[derive(Debug, Clone, PartialEq)]
pub enum Attempt {
    Verdict(Verdict),
    /// Not tried: unknown kind or engine version, no time left or slow checks used up.
    PutOff,
    /// Tried without a verdict (time limit, crash): a running out clock says nothing about
    /// the solution, so it stays pending and the try is noted for the next run.
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

#[derive(Debug)]
pub struct Checked {
    pub key: String,
    pub kind: String,
    pub attempt: Attempt,
}

/// The note of a pending row after one more try without a verdict, counting the tries.
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

pub struct VerifyOptions<'a> {
    /// Only these keys, if given.
    pub keys: Option<&'a [String]>,
    /// Most solutions read in one call.
    pub limit: u32,
    /// Pending solutions to pass over first: the ones an earlier call left in line.
    pub offset: u32,
    /// Leave out rows tried without a verdict at or after this (unix seconds), i.e. in
    /// this run.
    pub tried_since: Option<i64>,
    /// Start no further check after this; the rest stays pending for the next run.
    pub deadline: Option<Instant>,
    /// Only report verdicts.
    pub dry_run: bool,
}

/// Checks pending solutions with `check`, never tried ones oldest first, then the ones
/// that went without a verdict longest ago, and stores each outcome unless `dry_run`.
pub fn verify_pending(db: &Db, opts: &VerifyOptions, check: Check) -> Result<Vec<Checked>, String> {
    let (keys, limit, dry_run) = (opts.keys, opts.limit, opts.dry_run);
    let mut sql = String::from(
        "SELECT key, kind, engine_version, input, result, submitted_at, check_note FROM solutions \
         WHERE status = 'submitted'",
    );
    let mut args: Vec<SqlValue> = Vec::new();
    if let Some(keys) = keys {
        sql.push_str(&format!(" AND key IN ({})", vec!["?"; keys.len()].join(",")));
        args.extend(keys.iter().map(|k| SqlValue::from(k.as_str())));
    }
    if let Some(t) = opts.tried_since {
        sql.push_str(" AND (checked_at IS NULL OR checked_at < ?)");
        args.push(t.into());
    }
    sql.push_str(" ORDER BY checked_at IS NOT NULL, checked_at, submitted_at LIMIT ? OFFSET ?");
    args.push(SqlValue::Integer(limit as i64));
    args.push(SqlValue::Integer(opts.offset as i64));
    let [rows] = <[Rows; 1]>::try_from(db.execute(&[(&sql, args)])?).map_err(|_| "one result expected")?;

    let mut out = Vec::new();
    for i in 0..rows.rows.len() {
        if opts.deadline.is_some_and(|d| Instant::now() >= d) {
            break;
        }
        let key = rows.col(i, "key").as_str().unwrap_or_default().to_string();
        let kind = rows.col(i, "kind").as_str().unwrap_or_default().to_string();
        let version = rows.col(i, "engine_version").as_i64().unwrap_or(-1);
        let Some(entry) = u32::try_from(version).ok().and_then(|v| kinds::find(&kind, v)) else {
            out.push(Checked { key, kind, attempt: Attempt::PutOff });
            continue;
        };
        let input = rows.col(i, "input").as_str().unwrap_or_default();
        let attempt = match rows.col(i, "result") {
            SqlValue::Blob(b) => match gunzip(b) {
                Ok(result) => check(entry, &key, input, &result),
                Err(reason) => Attempt::Verdict(Verdict::Rejected { exploitability: None, reason }),
            },
            _ => Attempt::Verdict(Verdict::Rejected { exploitability: None, reason: "result is not a blob".into() }),
        };
        if !dry_run {
            let submitted_at = rows.col(i, "submitted_at").clone();
            let stmts = match &attempt {
                Attempt::PutOff => Vec::new(),
                Attempt::NoVerdict(reason) => vec![(
                    "UPDATE solutions SET checked_at = ?, check_note = ? \
                     WHERE key = ? AND status = 'submitted' AND submitted_at = ?",
                    vec![
                        now().into(),
                        no_verdict_note(rows.col(i, "check_note").as_str(), reason).as_str().into(),
                        key.as_str().into(),
                        submitted_at,
                    ],
                )],
                Attempt::Verdict(verdict) => verdict_statements(verdict, &key, submitted_at),
            };
            if !stmts.is_empty() {
                db.execute(&stmts)?;
            }
        }
        out.push(Checked { key, kind, attempt });
    }
    Ok(out)
}

fn verdict_statements(verdict: &Verdict, key: &str, submitted_at: SqlValue) -> Vec<(&'static str, Vec<SqlValue>)> {
    let (status, exploitability, note) = match verdict {
        Verdict::Verified { exploitability, .. } => ("verified", Some(*exploitability), None),
        Verdict::Rejected { exploitability, reason } => ("rejected", *exploitability, Some(reason.as_str())),
    };
    let mut stmts = Vec::new();
    // The verifier's own rebuild replaces the submitted result first, so a row never
    // turns verified with the submitted one.
    if let Verdict::Verified { result: Some(result), .. } = verdict {
        stmts.push((
            "UPDATE solutions SET result = ?, result_size = ? WHERE key = ? AND status = 'submitted' AND submitted_at = ?",
            vec![SqlValue::Blob(gzip(result)), (result.len() as i64).into(), key.into(), submitted_at.clone()],
        ));
    }
    stmts.push((
        "UPDATE solutions SET status = ?, exploitability = ?, checked_at = ?, check_note = ? \
         WHERE key = ? AND status = 'submitted' AND submitted_at = ?",
        vec![
            status.into(),
            exploitability.map_or(SqlValue::Null, SqlValue::Float),
            now().into(),
            note.map_or(SqlValue::Null, SqlValue::from),
            key.into(),
            submitted_at,
        ],
    ));
    stmts
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub rate_limits: u64,
    pub reports: u64,
    pub submissions: u64,
    /// Queue entries whose solution is verified.
    pub work: u64,
}

/// Deletes rate-limit counters nobody looks at any more, and parser reports and
/// submission log entries older than [`KEEP_LOGS_S`].
pub fn prune(db: &Db) -> Result<Pruned, String> {
    let t = now();
    let r = db.execute(&[
        ("DELETE FROM rate_limits WHERE window_start < ?", vec![(t - KEEP_RATE_LIMITS_S).into()]),
        ("DELETE FROM reports WHERE received_at < ?", vec![(t - KEEP_LOGS_S).into()]),
        ("DELETE FROM submissions WHERE received_at < ?", vec![(t - KEEP_LOGS_S).into()]),
        ("DELETE FROM work WHERE key IN (SELECT key FROM solutions WHERE status = 'verified')", vec![]),
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
    /// "catalog" or "preflop".
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
pub fn seed_work(db: &Db, seeds: &[Seed]) -> Result<u64, String> {
    let t = now();
    let mut added = 0;
    for chunk in seeds.chunks(SEED_BATCH) {
        let stmts: Vec<(&str, Vec<SqlValue>)> = chunk
            .iter()
            .map(|s| {
                (
                    "INSERT INTO work (key, kind, engine_version, input, source, priority, created_at) \
                     SELECT ?, ?, ?, ?, ?, 0, ? WHERE NOT EXISTS \
                     (SELECT 1 FROM solutions WHERE key = ? AND status = 'verified') ON CONFLICT (key) DO NOTHING",
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
