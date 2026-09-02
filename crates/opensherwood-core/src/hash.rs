//! Canonical state hashing (ADR-0004): explicit little-endian encoding per subsystem, BLAKE3.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Hash schema version; bump when the encoding changes.
pub const HASH_SCHEMA_VERSION: u32 = 4;

/// Subsystem hashes plus the total, as lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Hashes {
    /// `world`, `actors`, `orders`, `rng`, ... and `total`.
    #[serde(flatten)]
    pub parts: BTreeMap<String, String>,
}

impl Hashes {
    /// Lookup.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.parts.get(name).map(String::as_str)
    }

    /// Total hash.
    #[must_use]
    pub fn total(&self) -> &str {
        self.get("total").unwrap_or("")
    }

    /// Names of parts that differ between two hash sets.
    #[must_use]
    pub fn diff(&self, other: &Hashes) -> Vec<String> {
        let mut names: Vec<&String> = self.parts.keys().chain(other.parts.keys()).collect();
        names.sort();
        names.dedup();
        names
            .into_iter()
            .filter(|n| self.parts.get(*n) != other.parts.get(*n))
            .cloned()
            .collect()
    }
}

/// Builder for one subsystem's canonical bytes.
#[derive(Debug)]
pub struct Encoder {
    hasher: blake3::Hasher,
}

impl Encoder {
    /// Start a subsystem encoding with a domain prefix.
    #[must_use]
    pub fn new(domain: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"opensherwood-state\0");
        hasher.update(&HASH_SCHEMA_VERSION.to_le_bytes());
        hasher.update(domain.as_bytes());
        hasher.update(b"\0");
        Self { hasher }
    }

    /// Append a `u8`.
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.hasher.update(&[v]);
        self
    }

    /// Append a `u32`.
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.hasher.update(&v.to_le_bytes());
        self
    }

    /// Append an `i32`.
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.hasher.update(&v.to_le_bytes());
        self
    }

    /// Append a `u64`.
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.hasher.update(&v.to_le_bytes());
        self
    }

    /// Append raw bytes with a length prefix.
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.hasher.update(&(v.len() as u64).to_le_bytes());
        self.hasher.update(v);
        self
    }

    /// Append a string with a length prefix.
    pub fn str(&mut self, v: &str) -> &mut Self {
        self.bytes(v.as_bytes())
    }

    /// Finish as hex.
    #[must_use]
    pub fn finish(self) -> String {
        self.hasher.finalize().to_hex().to_string()
    }
}

/// Combine subsystem hashes into a total, in name order.
#[must_use]
pub fn total(parts: &BTreeMap<String, String>) -> String {
    let mut e = Encoder::new("total");
    for (k, v) in parts {
        e.str(k).str(v);
    }
    e.finish()
}
