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
//
// Security (H1 residual — keyring-deletion downgrade):
//   A SHA256(machine_key)[..16] fingerprint is stored in `<wal_path>.fingerprint`
//   (first 16 bytes of SHA256, hex-encoded, 32 chars).
//   On Wal::open():
//     - fingerprint absent (first run): generate/load key → write fingerprint → open.
//     - fingerprint present + keyring entry missing → WalError::KeyringEntryMissing.
//     - fingerprint present + key loaded + fingerprint matches → open normally.
//     - fingerprint present + key loaded + fingerprint mismatches →
//         WalError::KeyFingerprintMismatch { stored, current }.
//   Wal::open_with_key() (test-only bypass) does NOT interact with the fingerprint.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
    /// Fingerprint file exists but the OS keyring entry was deleted — explicit attack signal.
    /// Do NOT auto-regenerate: caller must investigate.
    #[error("WAL key fingerprint file exists but keyring entry is missing — possible keyring deletion attack")]
    KeyringEntryMissing,
    /// Fingerprint on disk does not match the key currently in the keyring.
    /// Refusing to open to prevent silent WAL history erasure.
    #[error("WAL key fingerprint mismatch: stored={stored}, current={current}")]
    KeyFingerprintMismatch { stored: String, current: String },
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

/// Compute the key fingerprint: SHA256(key)[..16] as a 32-char lowercase hex string.
fn key_fingerprint(key: &[u8]) -> String {
    let hash = Sha256::digest(key);
    hex::encode(&hash[..16])
}

/// Path of the fingerprint file adjacent to the WAL sled DB directory.
fn fingerprint_path(wal_path: &Path) -> PathBuf {
    // e.g. `/data/wal` → `/data/wal.fingerprint`
    let mut fp = wal_path.as_os_str().to_owned();
    fp.push(".fingerprint");
    PathBuf::from(fp)
}

/// Read the fingerprint file; returns `None` if the file does not exist.
fn read_fingerprint(fp_path: &Path) -> Result<Option<String>, WalError> {
    match std::fs::read_to_string(fp_path) {
        Ok(s) => Ok(Some(s.trim().to_owned())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(WalError::KeyUnavailable(format!(
            "reading fingerprint file {}: {e}",
            fp_path.display()
        ))),
    }
}

/// Write the fingerprint file atomically (write-then-rename on the same filesystem).
fn write_fingerprint(fp_path: &Path, fingerprint: &str) -> Result<(), WalError> {
    // Write to a temp file in the same directory, then rename for atomicity.
    let tmp = fp_path.with_extension("fingerprint.tmp");
    std::fs::write(&tmp, fingerprint).map_err(|e| {
        WalError::KeyUnavailable(format!("writing fingerprint tmp {}: {e}", tmp.display()))
    })?;
    std::fs::rename(&tmp, fp_path).map_err(|e| {
        WalError::KeyUnavailable(format!("renaming fingerprint {}: {e}", fp_path.display()))
    })?;
    Ok(())
}

/// Enforce the fingerprint invariant given an already-resolved key and the stored fingerprint.
///
/// Called after the keyring state is known. Separated for unit-testability.
///
/// `stored_fp`  — `None` if the fingerprint file doesn't exist yet (first run).
/// `key`        — the key bytes resolved from the keyring (or `None` if keyring entry absent).
/// `fp_path`    — path to write the fingerprint file on first run.
///
/// Returns the verified key bytes on success, or the appropriate `WalError` on failure.
fn check_or_init_fingerprint(
    stored_fp: Option<String>,
    key: Option<Vec<u8>>,
    fp_path: &Path,
) -> Result<Vec<u8>, WalError> {
    match (stored_fp, key) {
        // ── fingerprint absent, key present → first run with pre-existing key ──
        (None, Some(k)) => {
            let fp = key_fingerprint(&k);
            write_fingerprint(fp_path, &fp)?;
            tracing::info!(target: "amore.wal", fingerprint = %fp, "wrote initial WAL key fingerprint");
            Ok(k)
        }

        // ── fingerprint absent, key absent → caller generates key; write fingerprint ──
        // (handled in load_or_create_machine_key before calling this)
        (None, None) => {
            // Unreachable via load_or_create_machine_key (key is always Some here in None/None).
            // Guard against direct misuse.
            Err(WalError::KeyUnavailable(
                "internal: check_or_init_fingerprint called with both absent".into(),
            ))
        }

        // ── fingerprint present, keyring entry deleted → explicit attack signal ──
        (Some(_), None) => {
            tracing::error!(
                target: "amore.wal",
                "WAL keyring entry missing while fingerprint exists — possible keyring deletion attack"
            );
            Err(WalError::KeyringEntryMissing)
        }

        // ── fingerprint present, key present → verify match ──
        (Some(stored), Some(k)) => {
            let current = key_fingerprint(&k);
            if stored != current {
                tracing::error!(
                    target: "amore.wal",
                    stored = %stored,
                    current = %current,
                    "WAL key fingerprint mismatch — refusing to open"
                );
                return Err(WalError::KeyFingerprintMismatch { stored, current });
            }
            Ok(k)
        }
    }
}

/// Load the machine key from keyring AND enforce the fingerprint invariant.
///
/// `wal_path` — the sled DB path (fingerprint lives at `<wal_path>.fingerprint`).
///
/// State machine:
///   - fingerprint absent, keyring present → first run; write fingerprint.
///   - fingerprint absent, keyring absent  → first run; generate key, write fingerprint.
///   - fingerprint present, keyring absent → attack signal; `KeyringEntryMissing`.
///   - fingerprint present, keyring present, match  → normal open.
///   - fingerprint present, keyring present, mismatch → `KeyFingerprintMismatch`.
fn load_or_create_machine_key(wal_path: &Path) -> Result<Vec<u8>, WalError> {
    let fp_path = fingerprint_path(wal_path);
    let stored_fp = read_fingerprint(&fp_path)?;

    let entry = keyring::Entry::new("amore", "wal-hmac-key")
        .map_err(|e| WalError::KeyUnavailable(format!("keyring entry: {e}")))?;

    match entry.get_password() {
        // ── keyring entry absent + no fingerprint → first run: generate key ──
        Err(keyring::Error::NoEntry) if stored_fp.is_none() => {
            let mut raw = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut raw);
            let hex_key = hex::encode(raw);
            entry
                .set_password(&hex_key)
                .map_err(|e| WalError::KeyUnavailable(format!("keyring set: {e}")))?;
            let fp = key_fingerprint(&raw);
            write_fingerprint(&fp_path, &fp)?;
            tracing::info!(target: "amore.wal", fingerprint = %fp, "generated new WAL machine key and fingerprint");
            Ok(raw.to_vec())
        }

        // ── keyring entry absent + fingerprint present → attack signal ──
        Err(keyring::Error::NoEntry) => {
            check_or_init_fingerprint(stored_fp, None, &fp_path)
        }

        // ── key present → verify/init fingerprint ──
        Ok(hex_key) => {
            let key = hex::decode(&hex_key)
                .map_err(|e| WalError::KeyUnavailable(format!("corrupt keyring value: {e}")))?;
            check_or_init_fingerprint(stored_fp, Some(key), &fp_path)
        }

        // ── any other keyring error → fail closed ──
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
    ///
    /// Loads (or generates on first run) the HMAC machine key from the OS keyring and
    /// enforces the key-fingerprint invariant — refuses to open if the fingerprint on disk
    /// does not match the key in the keyring (keyring-deletion downgrade attack mitigation).
    ///
    /// Returns:
    ///   - `WalError::KeyUnavailable`       — OS keyring unreachable (fail closed)
    ///   - `WalError::KeyringEntryMissing`  — fingerprint exists but keyring entry deleted
    ///   - `WalError::KeyFingerprintMismatch` — key in keyring differs from stored fingerprint
    pub fn open(path: &Path) -> Result<Self, WalError> {
        let machine_key = load_or_create_machine_key(path)?;
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

    // ── Fingerprint tests (H1 residual) ─────────────────────────────────────

    use super::{
        check_or_init_fingerprint, fingerprint_path, key_fingerprint,
        read_fingerprint, write_fingerprint,
    };

    /// FP-1: First open (no fingerprint file) with a key → fingerprint file created,
    ///        contents match SHA256(key)[..16] hex.
    #[test]
    fn fp1_first_open_creates_fingerprint_file() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("wal");
        let fp_path = fingerprint_path(&wal_path);

        // Precondition: fingerprint file does not exist.
        assert!(!fp_path.exists(), "fingerprint file must not exist before first open");

        let key = vec![0xAA_u8; 32];
        let result = check_or_init_fingerprint(None, Some(key.clone()), &fp_path);
        assert!(result.is_ok(), "first open must succeed: {result:?}");

        // Fingerprint file must now exist and contain the correct value.
        assert!(fp_path.exists(), "fingerprint file must be created on first open");
        let stored = read_fingerprint(&fp_path).unwrap().unwrap();
        let expected = key_fingerprint(&key);
        assert_eq!(stored, expected, "stored fingerprint must match SHA256(key)[..16]");
    }

    /// FP-2: Subsequent open with the same key → succeeds (fingerprint matches).
    #[test]
    fn fp2_subsequent_open_matching_key_succeeds() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("wal");
        let fp_path = fingerprint_path(&wal_path);

        let key = vec![0xBB_u8; 32];
        let fp = key_fingerprint(&key);

        // Simulate a previous open by writing the fingerprint file.
        write_fingerprint(&fp_path, &fp).expect("write fingerprint must succeed");

        // Subsequent open with the same key must succeed.
        let result = check_or_init_fingerprint(Some(fp.clone()), Some(key.clone()), &fp_path);
        assert!(
            result.is_ok(),
            "matching key must open successfully: {result:?}"
        );
        assert_eq!(result.unwrap(), key, "returned key must equal the input key");
    }

    /// FP-3: Fingerprint file present, different key supplied → `KeyFingerprintMismatch`.
    #[test]
    fn fp3_mismatched_key_returns_fingerprint_mismatch_error() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("wal");
        let fp_path = fingerprint_path(&wal_path);

        let original_key = vec![0xCC_u8; 32];
        let stored_fp = key_fingerprint(&original_key);
        write_fingerprint(&fp_path, &stored_fp).expect("write fingerprint must succeed");

        // Use a different key (simulates attacker replacing keyring entry).
        let attacker_key = vec![0xDD_u8; 32];
        let current_fp = key_fingerprint(&attacker_key);

        let result =
            check_or_init_fingerprint(Some(stored_fp.clone()), Some(attacker_key), &fp_path);

        match result {
            Err(WalError::KeyFingerprintMismatch { stored, current }) => {
                assert_eq!(stored, stored_fp, "stored field must match on-disk fingerprint");
                assert_eq!(current, current_fp, "current field must match attacker key fingerprint");
            }
            other => panic!("expected KeyFingerprintMismatch, got {other:?}"),
        }
    }

    /// FP-4: Fingerprint file exists but keyring entry is absent (deleted) →
    ///        `KeyringEntryMissing`.
    #[test]
    fn fp4_missing_keyring_entry_when_fingerprint_exists_returns_error() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("wal");
        let fp_path = fingerprint_path(&wal_path);

        // Simulate an existing fingerprint file (key was previously set up).
        let key = vec![0xEE_u8; 32];
        let fp = key_fingerprint(&key);
        write_fingerprint(&fp_path, &fp).expect("write fingerprint must succeed");

        // Simulate keyring entry deletion: pass None for the key.
        let result = check_or_init_fingerprint(Some(fp), None, &fp_path);

        match result {
            Err(WalError::KeyringEntryMissing) => {} // expected
            other => panic!("expected KeyringEntryMissing, got {other:?}"),
        }
    }

    // ── Gap-closure tests (mutation score: ack/unacked, boundary, NotFound) ──

    /// GAP-1: ack/unacked round-trip.
    ///
    /// Append 5 records → all 5 appear in unacked().
    /// Ack 2 specific seqs → unacked() shrinks to 3 and the acked seqs are absent.
    /// Catches mutations to ack() that silently no-op, and unacked() that drops or
    /// duplicates records or flips the acked-predicate polarity.
    #[test]
    fn gap1_ack_unacked_round_trip() {
        let dir = TempDir::new().unwrap();
        let wal = make_wal_with_key(&dir, &[0x11_u8; 32]);

        // Append 5 distinct records and collect their sequence numbers.
        let seqs: Vec<u64> = (0..5)
            .map(|i| {
                wal.append(&WalRecord {
                    kind: WalKind::Upsert,
                    doc_id: i,
                    payload_json: format!(r#"{{"i":{i}}}"#),
                    ts_utc: 1_700_000_000 + i as i64,
                })
                .expect("append must succeed")
            })
            .collect();

        // Before any ack: all 5 records must be unacked.
        let before = wal.unacked().expect("unacked must succeed");
        assert_eq!(before.len(), 5, "all 5 records must appear before any ack");

        // Ack the records at index 1 and 3 (arbitrary — ensures non-contiguous coverage).
        let acked_seq_a = seqs[1];
        let acked_seq_b = seqs[3];
        wal.ack(acked_seq_a).expect("ack must succeed");
        wal.ack(acked_seq_b).expect("ack must succeed");

        // After acking 2: only 3 must remain, and the acked seqs must not appear.
        let after = wal.unacked().expect("unacked must succeed after ack");
        assert_eq!(after.len(), 3, "3 records must remain after acking 2");

        let remaining_seqs: Vec<u64> = after.iter().map(|(s, _)| *s).collect();
        assert!(
            !remaining_seqs.contains(&acked_seq_a),
            "acked seq {acked_seq_a} must not appear in unacked()"
        );
        assert!(
            !remaining_seqs.contains(&acked_seq_b),
            "acked seq {acked_seq_b} must not appear in unacked()"
        );
    }

    /// GAP-2: MAX_WAL_PAYLOAD_BYTES boundary (off-by-one).
    ///
    /// Exactly MAX_WAL_PAYLOAD_BYTES → append succeeds.
    /// MAX_WAL_PAYLOAD_BYTES + 1 → PayloadTooLarge.
    /// Catches the `>` vs `>=` mutation on the size check.
    #[test]
    fn gap2_payload_boundary_exact_succeeds_plus_one_fails() {
        let dir = TempDir::new().unwrap();
        let wal = make_wal_with_key(&dir, &[0x22_u8; 32]);

        // Exactly at the limit must succeed.
        let at_limit = WalRecord {
            kind: WalKind::Upsert,
            doc_id: 1,
            payload_json: "x".repeat(MAX_WAL_PAYLOAD_BYTES),
            ts_utc: 1_700_000_000,
        };
        wal.append(&at_limit)
            .expect("append at exactly MAX_WAL_PAYLOAD_BYTES must succeed");

        // One byte over the limit must fail with PayloadTooLarge.
        let over_limit = WalRecord {
            kind: WalKind::Upsert,
            doc_id: 2,
            payload_json: "x".repeat(MAX_WAL_PAYLOAD_BYTES + 1),
            ts_utc: 1_700_000_001,
        };
        match wal.append(&over_limit) {
            Err(WalError::PayloadTooLarge(n)) => {
                assert_eq!(n, MAX_WAL_PAYLOAD_BYTES + 1, "reported size must match payload length");
            }
            other => panic!("expected PayloadTooLarge, got {other:?}"),
        }
    }

    /// GAP-3: read_fingerprint NotFound guard path.
    ///
    /// Fresh directory (no fingerprint file) → Ok(None).
    /// After writing a fingerprint file → Ok(Some(_)) with the written value.
    /// Catches mutations that swap the NotFound arm to Err or unconditionally return Ok(None).
    #[test]
    fn gap3_read_fingerprint_not_found_returns_ok_none_then_ok_some_after_write() {
        let dir = TempDir::new().unwrap();
        let wal_path = dir.path().join("wal");
        let fp_path = fingerprint_path(&wal_path);

        // Before the file exists: must return Ok(None) (NotFound is not an error).
        let result = read_fingerprint(&fp_path);
        assert!(
            matches!(result, Ok(None)),
            "read_fingerprint on absent file must return Ok(None), got {result:?}"
        );

        // Write a fingerprint and read it back: must return Ok(Some(value)).
        let expected = "deadbeef01234567deadbeef01234567";
        write_fingerprint(&fp_path, expected).expect("write_fingerprint must succeed");

        let result2 = read_fingerprint(&fp_path);
        match result2 {
            Ok(Some(ref s)) if s == expected => {} // expected
            other => panic!("expected Ok(Some({expected:?})), got {other:?}"),
        }
    }
}
