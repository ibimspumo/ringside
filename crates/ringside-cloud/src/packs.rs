//! Signed download packs from the CDN (`server/packs/packs.ts` uploads them).
//!
//! `index.json` lists every pack version with each file's size and SHA-256 and is signed
//! with Ed25519 (`index.json.sig`, base64). The app checks the signature with the public
//! key below, refuses an index older than one it has seen, and checks every file against
//! the index before it uses it.

use std::collections::BTreeMap;
use std::io::Read;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::digest::{SHA256, digest};
use ring::signature::{ED25519, UnparsedPublicKey};
use serde::Deserialize;

pub const DEFAULT_BASE_URL: &str = "https://ringside-packs.b-cdn.net";
/// Public half of `RINGSIDE_PACKS_SIGNING_KEY` (`deno task packs pubkey`).
pub const PUBLIC_KEY: &str = "Jkc8Q0xJWdJ9C1g10O6qFGafqWRLFsoemdhn/0YBJ/I=";
const FORMAT: &str = "ringside-packs-index/1";
const MAX_INDEX: u64 = 4 << 20;
/// Largest pack file the app downloads.
pub const MAX_FILE: u64 = 256 << 20;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct PackFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct PackVersion {
    pub uploaded_at: String,
    pub files: Vec<PackFile>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Index {
    pub format: String,
    pub serial: u64,
    pub updated_at: String,
    /// Pack name → version → files.
    pub packs: BTreeMap<String, BTreeMap<String, PackVersion>>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// Parses `index` after checking its signature with `public_key` (base64, raw Ed25519).
pub fn verify_index(index: &[u8], signature: &str, public_key: &str) -> Result<Index, String> {
    let key = STANDARD.decode(public_key).map_err(|e| format!("public key: {e}"))?;
    let sig = STANDARD.decode(signature.trim()).map_err(|e| format!("signature: {e}"))?;
    UnparsedPublicKey::new(&ED25519, key).verify(index, &sig).map_err(|_| "index signature is invalid".to_string())?;
    let parsed: Index = serde_json::from_slice(index).map_err(|e| format!("index: {e}"))?;
    if parsed.format != FORMAT {
        return Err(format!("unknown index format {}", parsed.format));
    }
    Ok(parsed)
}

/// Whether `bytes` is the file `f` of the index.
pub fn check_file(f: &PackFile, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 != f.size || sha256_hex(bytes) != f.sha256 {
        return Err(format!("{}: size or SHA-256 differs from the index", f.path));
    }
    Ok(())
}

/// Versions "1.2.3" compared as numbers.
pub fn newest_version<'a>(versions: impl Iterator<Item = &'a String>) -> Option<&'a String> {
    let parse = |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse().ok()).collect() };
    versions.filter(|v| parse(v).is_some()).max_by_key(|v| parse(v).unwrap_or_default())
}

pub struct Packs {
    base: String,
    agent: ureq::Agent,
}

impl Packs {
    pub fn new(base_url: &str) -> Packs {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout(Duration::from_secs(120))
            .user_agent(concat!("ringside/", env!("CARGO_PKG_VERSION")))
            .build();
        Packs { base: base_url.trim_end_matches('/').to_string(), agent }
    }

    fn get(&self, path: &str, max: u64) -> Result<Vec<u8>, String> {
        let resp = self.agent.get(&format!("{}/{path}", self.base)).call().map_err(|e| format!("{path}: {e}"))?;
        let mut out = Vec::new();
        resp.into_reader().take(max + 1).read_to_end(&mut out).map_err(|e| format!("{path}: {e}"))?;
        if out.len() as u64 > max {
            return Err(format!("{path}: too large"));
        }
        Ok(out)
    }

    /// The signed index; fails when it is older than `min_serial`.
    pub fn index(&self, min_serial: u64) -> Result<Index, String> {
        let bytes = self.get("index.json", MAX_INDEX)?;
        let sig = String::from_utf8(self.get("index.json.sig", 1024)?).map_err(|_| "signature is not text")?;
        let index = verify_index(&bytes, &sig, PUBLIC_KEY)?;
        if index.serial < min_serial {
            return Err(format!("index serial {} is older than {min_serial}", index.serial));
        }
        Ok(index)
    }

    /// A file of a pack version, checked against the index.
    pub fn file(&self, name: &str, version: &str, f: &PackFile) -> Result<Vec<u8>, String> {
        if f.size > MAX_FILE {
            return Err(format!("{}: too large", f.path));
        }
        let bytes = self.get(&format!("packs/{name}/{version}/{}", f.path), f.size)?;
        check_file(f, &bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};

    fn signed(index: &str) -> (String, String) {
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        (STANDARD.encode(pair.sign(index.as_bytes())), STANDARD.encode(pair.public_key()))
    }

    const INDEX: &str = r#"{"format":"ringside-packs-index/1","serial":3,"updated_at":"x","packs":{"a":{"1.0.0":{"uploaded_at":"x","files":[{"path":"f.bin","size":3,"sha256":"039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"}]}}}}"#;

    #[test]
    fn signed_index_is_accepted_and_tampering_is_not() {
        let (sig, key) = signed(INDEX);
        let index = verify_index(INDEX.as_bytes(), &sig, &key).unwrap();
        assert_eq!(index.serial, 3);
        let tampered = INDEX.replace("\"serial\":3", "\"serial\":4");
        assert!(verify_index(tampered.as_bytes(), &sig, &key).is_err());
        let (_, other_key) = signed(INDEX);
        assert!(verify_index(INDEX.as_bytes(), &sig, &other_key).is_err());
        assert!(verify_index(INDEX.as_bytes(), &sig, PUBLIC_KEY).is_err(), "only the operator's key signs");
    }

    #[test]
    fn files_are_checked_against_the_index() {
        let (sig, key) = signed(INDEX);
        let index = verify_index(INDEX.as_bytes(), &sig, &key).unwrap();
        let f = &index.packs["a"]["1.0.0"].files[0];
        assert!(check_file(f, &[1, 2, 3]).is_ok());
        assert!(check_file(f, &[1, 2, 4]).is_err());
        assert!(check_file(f, &[1, 2]).is_err());
    }

    #[test]
    fn newest_version_compares_numbers() {
        let v = ["1.9.0".to_string(), "1.10.0".to_string(), "x".to_string()];
        assert_eq!(newest_version(v.iter()).map(String::as_str), Some("1.10.0"));
    }
}
