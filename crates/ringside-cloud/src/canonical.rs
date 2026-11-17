//! Canonical JSON of solution inputs and the solution key; the same rules as
//! `server/edge/canonical.ts`, checked against the shared vectors in `testdata/`.
//!
//! Only values with exactly one spelling are allowed: integers within ±(2^53 − 1),
//! printable ASCII strings (escaping only `"` and `\`), booleans, null, arrays, and objects
//! whose keys match `[a-z0-9_]{1,64}`, written in byte order. No whitespace, depth ≤ 8.

use std::fmt::Write;

use ring::digest::{SHA256, digest};
use serde_json::Value;

const MAX_SAFE: i64 = (1 << 53) - 1;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalError(pub String);

impl std::fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not canonical: {}", self.0)
    }
}

impl std::error::Error for CanonicalError {}

pub fn canonicalize(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write_value(value, 0, &mut out)?;
    Ok(out)
}

fn write_value(v: &Value, depth: usize, out: &mut String) -> Result<(), CanonicalError> {
    if depth > MAX_DEPTH {
        return Err(CanonicalError("nested too deeply".into()));
    }
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => match n.as_i64() {
            Some(i) if (-MAX_SAFE..=MAX_SAFE).contains(&i) => write!(out, "{i}").unwrap(),
            _ => {
                return Err(CanonicalError(
                    "numbers must be integers within ±(2^53−1)".into(),
                ));
            }
        },
        Value::String(s) => write_string(s, out)?,
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, depth + 1, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                let valid = (1..=64).contains(&k.len())
                    && k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
                if !valid {
                    return Err(CanonicalError(format!("invalid key {k:?}")));
                }
                if i > 0 {
                    out.push(',');
                }
                write!(out, "\"{k}\":").unwrap();
                write_value(&map[k], depth + 1, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn write_string(s: &str, out: &mut String) -> Result<(), CanonicalError> {
    if !s.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
        return Err(CanonicalError("strings must be printable ASCII".into()));
    }
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    Ok(())
}

/// SHA-256 (hex) over kind, engine version and canonical input; what the server computes.
pub fn solution_key(kind: &str, engine_version: u32, canonical_input: &str) -> String {
    let msg = format!("ringside-solution/1\n{kind}\n{engine_version}\n{canonical_input}");
    digest(&SHA256, msg.as_bytes())
        .as_ref()
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            write!(s, "{b:02x}").unwrap();
            s
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vectors() -> Value {
        serde_json::from_str(include_str!("../testdata/canonical.json")).unwrap()
    }

    #[test]
    fn matches_shared_vectors() {
        let v = vectors();
        for case in v["valid"].as_array().unwrap() {
            let canonical = canonicalize(&case["input"]).unwrap();
            assert_eq!(canonical, case["canonical"].as_str().unwrap());
            let key = solution_key(
                case["kind"].as_str().unwrap(),
                case["engine_version"].as_u64().unwrap() as u32,
                &canonical,
            );
            assert_eq!(key, case["key"].as_str().unwrap());
        }
    }

    #[test]
    fn rejects_shared_invalid_vectors() {
        let v = vectors();
        for case in v["invalid"].as_array().unwrap() {
            assert!(canonicalize(case).is_err(), "{case}");
        }
    }

    #[test]
    fn key_order_does_not_matter() {
        let a: Value = serde_json::from_str(r#"{"b":1,"a":[2,{"d":3,"c":4}]}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":[2,{"c":4,"d":3}],"b":1}"#).unwrap();
        assert_eq!(canonicalize(&a).unwrap(), canonicalize(&b).unwrap());
    }

    #[test]
    fn floats_are_rejected() {
        let v: Value = serde_json::from_str(r#"{"a":1.0}"#).unwrap();
        assert!(canonicalize(&v).is_err());
    }
}
