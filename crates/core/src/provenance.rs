// Cryptographic provenance: sha256 envelope chain.

use anyhow::Result;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Envelope {
    pub id: String,
    pub prev_hash: String,
    pub canonical_json: String,
    pub hash: String,
}

impl Envelope {
    pub fn seal(prev_hash: &str, payload: &serde_json::Value) -> Result<Self> {
        let canonical_json = serde_json::to_string(payload)?;
        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(canonical_json.as_bytes());
        let hash = hex::encode(hasher.finalize());
        Ok(Self {
            id: uuid_stub(),
            prev_hash: prev_hash.to_string(),
            canonical_json,
            hash,
        })
    }

    pub fn verify(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(self.prev_hash.as_bytes());
        hasher.update(self.canonical_json.as_bytes());
        let computed = hex::encode(hasher.finalize());
        computed == self.hash
    }
}

fn uuid_stub() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("obs-{nanos:x}")
}
