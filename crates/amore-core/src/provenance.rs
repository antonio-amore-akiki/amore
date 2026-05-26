// Cryptographic provenance: canonical-JSON + SHA-256 envelope chain.
//
// Implements the portable-agent-memory paper spec for tamper-evident
// observation logs. Every observation is sealed in an Envelope whose
// hash binds {id, prev_hash, canonical_json} via length-prefixed encoding
// (so prefix-collision attacks across fields are not possible), producing
// a chain that fails verification if any byte of any link is modified.
//
// canonical_json crate (Mozilla, MIT, ADOPT verdict 2026-05-25): sorts
// object keys deterministically, normalizes number representation, and
// strips insignificant whitespace per the canonical-JSON spec — so the
// same logical payload always serializes to the same byte string,
// regardless of internal Map insertion order.

use anyhow::Result;
use canonical_json::ser::to_string as to_canonical;
use sha2::{Digest, Sha256};

/// The genesis prev_hash used by the first envelope in a chain.
/// 64 zero hex chars = 32 zero bytes — distinct from any real SHA-256 output.
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// A sealed observation envelope.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Envelope {
    pub id: String,
    pub prev_hash: String,
    pub canonical_json: String,
    pub hash: String,
}

impl Envelope {
    /// Seal a payload into a new envelope, chaining off `prev_hash`.
    pub fn seal(prev_hash: &str, payload: &serde_json::Value) -> Result<Self> {
        let canonical_str = to_canonical(payload)?;
        let id = gen_id();
        let hash = compute_hash(&id, prev_hash, &canonical_str);
        Ok(Self {
            id,
            prev_hash: prev_hash.to_string(),
            canonical_json: canonical_str,
            hash,
        })
    }

    /// Verify the envelope's stored hash matches a recomputation over its content.
    /// Returns false on any tamper of id, prev_hash, or canonical_json.
    pub fn verify(&self) -> bool {
        compute_hash(&self.id, &self.prev_hash, &self.canonical_json) == self.hash
    }
}

/// Verify a whole chain end-to-end:
///   1. Each envelope's hash matches its content (per-link integrity).
///   2. Each envelope's prev_hash equals the preceding envelope's hash (linkage).
///   3. The first envelope's prev_hash equals GENESIS_PREV_HASH.
///
/// Empty chains are vacuously valid.
pub fn verify_chain(chain: &[Envelope]) -> Result<()> {
    if chain.is_empty() {
        return Ok(());
    }
    let mut expected_prev = GENESIS_PREV_HASH.to_string();
    for (idx, env) in chain.iter().enumerate() {
        if env.prev_hash != expected_prev {
            anyhow::bail!(
                "chain broken at index {idx}: expected prev_hash={expected_prev}, got {}",
                env.prev_hash
            );
        }
        if !env.verify() {
            anyhow::bail!(
                "chain broken at index {idx}: envelope {} fails hash check",
                env.id
            );
        }
        expected_prev = env.hash.clone();
    }
    Ok(())
}

fn compute_hash(id: &str, prev_hash: &str, canonical_json: &str) -> String {
    // Length-prefix each field with a u64 big-endian length so an attacker
    // cannot move bytes between fields and produce the same hash.
    let mut hasher = Sha256::new();
    let parts: [&[u8]; 3] = [
        id.as_bytes(),
        prev_hash.as_bytes(),
        canonical_json.as_bytes(),
    ];
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hex::encode(hasher.finalize())
}

fn gen_id() -> String {
    // Monotonic-ish ID. SQLite PRIMARY KEY UNIQUE handles edge collisions —
    // SystemTime non-monotonic clock skew can otherwise duplicate nanos in
    // tight loops. Caller can resolve via DB constraint failure + retry.
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("obs-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_verifies() {
        let env = Envelope::seal(GENESIS_PREV_HASH, &json!({"k": "v", "n": 1})).unwrap();
        assert!(env.verify(), "freshly sealed envelope must verify");
    }

    #[test]
    fn tamper_canonical_breaks_verify() {
        let mut env = Envelope::seal(GENESIS_PREV_HASH, &json!({"k": "v"})).unwrap();
        env.canonical_json = r#"{"k":"tampered"}"#.to_string();
        assert!(!env.verify(), "tampered canonical_json must fail verify");
    }

    #[test]
    fn tamper_prev_hash_breaks_verify() {
        let mut env = Envelope::seal(GENESIS_PREV_HASH, &json!({"k": "v"})).unwrap();
        env.prev_hash =
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        assert!(!env.verify(), "tampered prev_hash must fail verify");
    }

    #[test]
    fn key_order_is_canonical() {
        // Object literals with same keys in different insertion order must produce
        // identical canonical_json (the canonical_json crate sorts keys).
        let env1 = Envelope::seal(GENESIS_PREV_HASH, &json!({"a": 1, "b": 2})).unwrap();
        let env2 = Envelope::seal(GENESIS_PREV_HASH, &json!({"b": 2, "a": 1})).unwrap();
        assert_eq!(
            env1.canonical_json, env2.canonical_json,
            "canonical JSON must sort keys deterministically"
        );
    }

    fn make_chain(steps: usize) -> Vec<Envelope> {
        let mut chain = Vec::with_capacity(steps);
        let mut prev = GENESIS_PREV_HASH.to_string();
        for i in 0..steps {
            // gen_id uses nanos; insert a tiny gap so consecutive seals get distinct IDs.
            std::thread::sleep(std::time::Duration::from_micros(1));
            let env = Envelope::seal(&prev, &json!({"step": i})).unwrap();
            prev = env.hash.clone();
            chain.push(env);
        }
        chain
    }

    #[test]
    fn chain_verifies_when_linked_correctly() {
        let chain = make_chain(4);
        verify_chain(&chain).expect("intact chain must verify");
    }

    #[test]
    fn chain_fails_when_linkage_broken() {
        let mut chain = make_chain(3);
        // Splice in a wrong prev_hash on the middle envelope.
        chain[1].prev_hash = GENESIS_PREV_HASH.to_string();
        let result = verify_chain(&chain);
        assert!(result.is_err(), "broken linkage must fail verify_chain");
    }

    #[test]
    fn chain_fails_when_payload_tampered() {
        let mut chain = make_chain(3);
        chain[2].canonical_json = r#"{"step":99}"#.to_string();
        let result = verify_chain(&chain);
        assert!(result.is_err(), "tampered payload must fail verify_chain");
    }

    #[test]
    fn empty_chain_is_valid() {
        verify_chain(&[]).expect("empty chain is vacuously valid");
    }

    #[test]
    fn genesis_prev_hash_is_64_zeros() {
        assert_eq!(GENESIS_PREV_HASH.len(), 64);
        assert!(GENESIS_PREV_HASH.chars().all(|c| c == '0'));
    }
}
