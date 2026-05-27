// wal.rs -- H.8 sled-backed write-ahead log
// @file-size-exempt: security-critical WAL with mandatory inline unit tests (H1 fix)
//
// Every observation is appended here (durable fsync) before async downstream flush.
// On kill-mid-ingest restart, unacked() returns records not yet confirmed to downstream,
// allowing recover_on_startup to replay with zero record loss.
//
// Key scheme:
//   b"rec:{seq:020}"  -> serde_json bytes of WalEnvelope
//   b"ack:{seq:020}"  -> b"1"   (present = flushed to all downstream stores)
//
// Security (H1 fix):
//   Each WalEnvelope carries a 32-byte HMAC-SHA256 tag over
//   (doc_id || ts_utc || payload_json).  Machine key stored in OS keyring
//   ("amore" / "wal-hmac-key"); generated on first open.
//   Tag mismatch on replay → error-logged + skipped (no panic).
//   Payload bounded at MAX_WAL_PAYLOAD_BYTES (16 KiB) on append.
//   Legacy records (no tag, serde default) accepted; re-stamped on next flush.

use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Maximum serialised payload length accepted by `append`.
/// Matches `MAX_QUERY_BYTES` from the amore-mcp transport envelope.
pub const MAX_WAL_PAYLOAD_BYTES: usize = 16 * 1024; // 16_384

/// Errors specific to WAL operations.
#[derive(Debug, thiserror::Error)]
pub enum WalError {
    #[error("WAL payload exceeds {MAX_WAL_PAYLOAD_BYTES} bytes (got {0})")]
    PayloadTooLarge(usize),
    #[error("WAL machine key unavailable: {0}")]
    KeyUnavailable(String),
    #[error("WAL operation failed: {0}")]
    Other(#[from] anyhow::Error),
}

/// Kind of WAL record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WalKind {
    Upsert,
    Delete,
}

/// A single record durably stored in the WAL before downstream flush.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalRecord {
    pub kind: WalKind,
    /// Stable doc identifier shared across Qdrant / SQLite / Tantivy.
    pub doc_id: u64,
    /// Full observation payload as JSON string (embeddings excluded).
    pub payload_json: String,
    /// Unix timestamp (seconds) at ingestion time.
    pub ts_utc: i64,
}

/// On-disk serialisation envelope — wraps WalRecord with an integrity tag.
/// `tag` is `None` for legacy records; accepted on replay (forward-compat).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalEnvelope {
    #[serde(flatten)]
    record: WalRecord,
    /// HMAC-SHA256(machine_key, doc_id_le8 || ts_utc_le8 || payload_json_bytes) as hex.
    #[serde(default)]
    tag: Option<String>,
}

fn rec_key(seq: u64) -> [u8; 24] {
    let mut k = [0u8; 24];
    k[..4].copy_from_slice(b"rec:");
    let digits = format!("{seq:020}");
    k[4..].copy_from_slice(digits.as_bytes());
    k
}

fn ack_key(seq: u64) -> [u8; 24] {
    let mut k = [0u8; 24];
    k[..4].copy_from_slice(b"ack:");
    let digits = format!("{seq:020}");
    k[4..].copy_from_slice(digits.as_bytes());
    k
}

/// Compute HMAC-SHA256 tag bytes for a record.
fn compute_tag(key: &[u8], record: &WalRecord) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&record.doc_id.to_le_bytes());
    mac.update(&record.ts_utc.to_le_bytes());
    mac.update(record.payload_json.as_bytes());
    mac.finalize().into_bytes().into()
}

/// Load or generate the 32-byte machine key from the OS keyring (fail closed).
fn load_or_create_machine_key() -> Result<Vec<u8>, WalError> {
    let entry = keyring::Entry::new("amore", "wal-hmac-key")
        .map_err(|e| WalError::KeyUnavailable(format!("keyring entry: {e}")))?;

    match entry.get_password() {
        Ok(hex_key) => hex::decode(&hex_key)
            .map_err(|e| WalError::KeyUnavailable(format!("corrupt keyring value: {e}"))),
        Err(keyring::Error::NoEntry) => {
            let mut raw = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut raw);
            let hex_key = hex::encode(raw);
            entry
                .set_password(&hex_key)
                .map_err(|e| WalError::KeyUnavailable(format!("keyring set: {e}")))?;
            tracing::info!(target: "amore.wal", "generated new WAL machine key in keyring");
            Ok(raw.to_vec())
        }
        Err(e) => Err(WalError::KeyUnavailable(format!("keyring get: {e}"))),
    }
}

/// Sled-backed write-ahead log with HMAC-SHA256 per-record integrity.
pub struct Wal {
    /// Sled DB; `pub(crate)` so integration tests can directly inject tampered records.
    pub(crate) db: sled::Db,
    seq: AtomicU64,
    machine_key: Vec<u8>,
}

impl Wal {
    /// Open or create a sled-backed WAL at `path`.
    /// Loads (or generates on first run) the HMAC machine key from the OS keyring.
    /// Returns `WalError::KeyUnavailable` if the OS keyring is unreachable (fail closed).
    pub fn open(path: &Path) -> Result<Self, WalError> {
        let machine_key = load_or_create_machine_key()?;
        Self::open_with_key(path, machine_key)
    }

    /// Open with an explicit machine key, bypassing the OS keyring.
    ///
    /// Use only in integration tests where keyring isolation is required.
    /// Production code MUST use `Wal::open`.
    pub fn open_with_key(path: &Path, machine_key: Vec<u8>) -> Result<Self, WalError> {
        let db = sled::open(path)
            .with_context(|| format!("opening sled WAL at {}", path.display()))
            .map_err(WalError::Other)?;

        let max_seq: u64 = db
            .scan_prefix(b"rec:")
            .next_back()
            .transpose()
            .with_context(|| "scanning WAL for max seq")
            .map_err(WalError::Other)?
            .and_then(|(k, _)| {
                let s = std::str::from_utf8(&k[4..]).ok()?;
                s.parse::<u64>().ok()
            })
            .unwrap_or(0);

        Ok(Self { db, seq: AtomicU64::new(max_seq + 1), machine_key })
    }

    /// Append a record. Returns `WalError::PayloadTooLarge` above 16 KiB.
    /// Stamps HMAC-SHA256 tag on the stored envelope and fdatasyncs.
    #[tracing::instrument(skip(self, record), fields(kind = ?record.kind, doc_id = record.doc_id))]
    pub fn append(&self, record: &WalRecord) -> Result<u64, WalError> {
        if record.payload_json.len() > MAX_WAL_PAYLOAD_BYTES {
            return Err(WalError::PayloadTooLarge(record.payload_json.len()));
        }
        let tag_bytes = compute_tag(&self.machine_key, record);
        let envelope = WalEnvelope {
            record: record.clone(),
            tag: Some(hex::encode(tag_bytes)),
        };
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let bytes = serde_json::to_vec(&envelope)
            .with_context(|| format!("serialising WalEnvelope seq={seq}"))
            .map_err(WalError::Other)?;
        self.db
            .insert(rec_key(seq), bytes.as_slice())
            .with_context(|| format!("sled insert rec seq={seq}"))
            .map_err(WalError::Other)?;
        self.db
            .flush()
            .with_context(|| format!("sled flush after rec seq={seq}"))
            .map_err(WalError::Other)?;
        Ok(seq)
    }

    /// Iterate records from `start_seq`. Verifies HMAC before yielding payload.
    /// Tampered records are skipped with error log + metric increment.
    /// Legacy tagless records are yielded as-is (forward-compat migration path).
    pub fn iter_from(
        &self,
        start_seq: u64,
    ) -> impl Iterator<Item = Result<(u64, WalRecord)>> + '_ {
        let prefix_start = rec_key(start_seq);
        let key = self.machine_key.clone();
        self.db.range(prefix_start..).filter_map(move |r| {
            let (k, v) = match r {
                Ok(kv) => kv,
                Err(e) => return Some(Err(anyhow::anyhow!("sled scan: {e}"))),
            };
            if !k.starts_with(b"rec:") {
                return None;
            }
            let seq = match std::str::from_utf8(&k[4..])
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
            {
                Some(s) => s,
                None => return Some(Err(anyhow::anyhow!("malformed WAL key: {k:?}"))),
            };
            let envelope: WalEnvelope = match serde_json::from_slice(&v) {
                Ok(e) => e,
                Err(e) => return Some(Err(anyhow::anyhow!("deserialise seq={seq}: {e}"))),
            };
            match &envelope.tag {
                None => {
                    tracing::debug!(
                        target: "amore.wal", seq,
                        "legacy WAL record without tag; accepting for migration"
                    );
                }
                Some(stored_hex) => {
                    let expected_hex = hex::encode(compute_tag(&key, &envelope.record));
                    if *stored_hex != expected_hex {
                        metrics::counter!("amore_wal_tampered_records_total").increment(1);
                        tracing::error!(
                            target: "amore.wal", seq,
                            "WAL record HMAC mismatch — skipping tampered record"
                        );
                        return None;
                    }
                }
            }
            Some(Ok((seq, envelope.record)))
        })
    }

    /// Mark a record as durably flushed to all downstream stores.
    pub fn ack(&self, seq: u64) -> Result<()> {
        self.db
            .insert(ack_key(seq), b"1".as_ref())
            .with_context(|| format!("sled insert ack seq={seq}"))?;
        self.db
            .flush()
            .with_context(|| format!("sled flush after ack seq={seq}"))?;
        Ok(())
    }

    /// All records appended but not yet acked. Used at startup for crash recovery replay.
    pub fn unacked(&self) -> Result<Vec<(u64, WalRecord)>> {
        let mut out = Vec::new();
        for item in self.iter_from(0) {
            let (seq, record) = item?;
            let acked = self
                .db
                .contains_key(ack_key(seq))
                .with_context(|| format!("checking ack for seq={seq}"))?;
            if !acked {
                out.push((seq, record));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{
        rec_key, Wal, WalError, WalKind, WalRecord, MAX_WAL_PAYLOAD_BYTES,
    };

    fn make_wal_with_key(dir: &TempDir, key: &[u8]) -> Wal {
        Wal::open_with_key(&dir.path().join("wal"), key.to_vec())
            .expect("open_with_key should succeed")
    }

    fn sample_record(payload: &str) -> WalRecord {
        WalRecord {
            kind: WalKind::Upsert,
            doc_id: 42,
            payload_json: payload.to_string(),
            ts_utc: 1_700_000_000,
        }
    }

    /// Test 1: append + replay round-trip (tag verifies).
    #[test]
    fn test_roundtrip_tag_verifies() {
        let dir = TempDir::new().unwrap();
        let wal = make_wal_with_key(&dir, &[0xAB_u8; 32]);
        let rec = sample_record(r#"{"text":"hello"}"#);
        let seq = wal.append(&rec).expect("append should succeed");
        let replayed: Vec<_> =
            wal.iter_from(0).collect::<anyhow::Result<Vec<_>>>().unwrap();
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].0, seq);
        assert_eq!(replayed[0].1.doc_id, 42);
        assert_eq!(replayed[0].1.payload_json, r#"{"text":"hello"}"#);
    }

    /// Test 2: payload > 16 KiB → WalError::PayloadTooLarge.
    #[test]
    fn test_payload_too_large() {
        let dir = TempDir::new().unwrap();
        let wal = make_wal_with_key(&dir, &[0xCD_u8; 32]);
        let rec = sample_record(&"x".repeat(MAX_WAL_PAYLOAD_BYTES + 1));
        match wal.append(&rec) {
            Err(WalError::PayloadTooLarge(n)) => assert_eq!(n, MAX_WAL_PAYLOAD_BYTES + 1),
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    /// Test 3: corrupted tag → record skipped, replay continues for clean records.
    #[test]
    fn test_corrupted_tag_skipped() {
        let dir = TempDir::new().unwrap();
        let wal = make_wal_with_key(&dir, &[0xEF_u8; 32]);
        let seq_bad = wal.append(&sample_record(r#"{"text":"good"}"#)).unwrap();
        // Overwrite stored bytes with a wrong tag.
        let k = rec_key(seq_bad);
        let raw = wal.db.get(k).unwrap().unwrap();
        let mut env: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        env["tag"] = serde_json::Value::String("00".repeat(32));
        wal.db.insert(k, serde_json::to_vec(&env).unwrap().as_slice()).unwrap();
        wal.append(&WalRecord {
            kind: WalKind::Upsert,
            doc_id: 99,
            payload_json: r#"{"text":"clean"}"#.to_string(),
            ts_utc: 1_700_000_001,
        })
        .unwrap();
        let replayed: Vec<_> =
            wal.iter_from(0).collect::<anyhow::Result<Vec<_>>>().unwrap();
        assert_eq!(replayed.len(), 1, "corrupted record must be skipped");
        assert_eq!(replayed[0].1.doc_id, 99);
    }

    /// Test 4: legacy record without tag → accepted, replay proceeds.
    ///
    /// Simulates a pre-upgrade WAL by writing a tagless JSON envelope directly into
    /// a raw sled DB, then opening the Wal on the same path to verify replay accepts it.
    #[test]
    fn test_legacy_record_without_tag_accepted() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("wal");

        // Write a legacy tagless record directly into sled (no Wal involved).
        {
            let db = sled::open(&wal_path).unwrap();
            let legacy = serde_json::json!({
                "kind": "upsert",
                "doc_id": 7_u64,
                "payload_json": r#"{"text":"legacy"}"#,
                "ts_utc": 1_600_000_000_i64
                // no "tag" field
            });
            db.insert(rec_key(0), serde_json::to_vec(&legacy).unwrap().as_slice()).unwrap();
            db.flush().unwrap();
        } // raw sled db dropped, releasing the lock

        // Open via Wal — it should accept the tagless record.
        let wal = Wal::open_with_key(&wal_path, vec![0x12_u8; 32])
            .expect("open_with_key should succeed");
        let replayed: Vec<_> =
            wal.iter_from(0).collect::<anyhow::Result<Vec<_>>>().unwrap();
        assert_eq!(replayed.len(), 1, "legacy record must be accepted");
        assert_eq!(replayed[0].1.doc_id, 7);
        assert_eq!(replayed[0].1.payload_json, r#"{"text":"legacy"}"#);
    }
}
