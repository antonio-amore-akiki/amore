// wal.rs -- H.8 sled-backed write-ahead log
//
// Every observation is appended here (durable fsync) before async downstream flush.
// On kill-mid-ingest restart, unacked() returns records not yet confirmed to downstream,
// allowing recover_on_startup to replay with zero record loss.
//
// Key scheme:
//   b"rec:{seq:020}"  -> serde_json bytes of WalRecord
//   b"ack:{seq:020}"  -> b"1"   (present = flushed to all downstream stores)

use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
    /// Full observation payload as JSON string (embeddings excluded -- generated downstream).
    pub payload_json: String,
    /// Unix timestamp (seconds) at ingestion time.
    pub ts_utc: i64,
}

/// Sled-backed write-ahead log.
pub struct Wal {
    db: sled::Db,
    seq: AtomicU64,
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

impl Wal {
    /// Open or create a sled-backed WAL at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let db = sled::open(path)
            .with_context(|| format!("opening sled WAL at {}", path.display()))?;

        // Recover current max sequence so we never reuse a seq after restart.
        let max_seq: u64 = db
            .scan_prefix(b"rec:")
            .next_back()
            .transpose()
            .with_context(|| "scanning WAL for max seq")?
            .and_then(|(k, _)| {
                let s = std::str::from_utf8(&k[4..]).ok()?;
                s.parse::<u64>().ok()
            })
            .unwrap_or(0);

        Ok(Self {
            db,
            seq: AtomicU64::new(max_seq + 1),
        })
    }

    /// Append a record atomically. Returns the assigned sequence number.
    /// Calls sled::Db::flush (fdatasync) so the record survives a process kill.
    pub fn append(&self, record: &WalRecord) -> Result<u64> {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst);
        let bytes = serde_json::to_vec(record)
            .with_context(|| format!("serialising WalRecord seq={seq}"))?;
        self.db
            .insert(rec_key(seq), bytes.as_slice())
            .with_context(|| format!("sled insert rec seq={seq}"))?;
        self.db
            .flush()
            .with_context(|| format!("sled flush after rec seq={seq}"))?;
        Ok(seq)
    }

    /// Iterate all records from `start_seq` (inclusive) in ascending order.
    pub fn iter_from(
        &self,
        start_seq: u64,
    ) -> impl Iterator<Item = Result<(u64, WalRecord)>> + '_ {
        let prefix_start = rec_key(start_seq);
        self.db.range(prefix_start..).filter_map(|r| {
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
            let record: WalRecord = match serde_json::from_slice(&v) {
                Ok(r) => r,
                Err(e) => return Some(Err(anyhow::anyhow!("deserialise seq={seq}: {e}"))),
            };
            Some(Ok((seq, record)))
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
