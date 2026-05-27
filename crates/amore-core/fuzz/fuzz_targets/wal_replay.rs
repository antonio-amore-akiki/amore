#![no_main]
// Fuzzes the WAL replay path via open_with_key + append + iter_from.
// Coverage: serde_json serialization/deserialization of WalRecord/WalEnvelope,
// HMAC tag generation, HMAC tag verification (mismatch path exercised by
// replaying with a different key), payload-size cap rejection.
// Uses open_with_key to bypass the OS keyring (documented test path in wal.rs).
use amore_core::wal::{WalKind, WalRecord};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // First byte selects the WalKind variant; remainder is the payload_json content.
    let kind = if data[0] & 1 == 0 { WalKind::Upsert } else { WalKind::Delete };
    let payload_raw = &data[1..];

    let tmp = std::env::temp_dir().join(format!(
        "amore-fuzz-wal-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    // Write key: fixed 32-byte key.
    let write_key = vec![0xdeu8; 32];
    let wal_write = match amore_core::wal::Wal::open_with_key(&tmp, write_key) {
        Ok(w) => w,
        Err(_) => return,
    };

    let record = WalRecord {
        kind,
        doc_id: 42u64,
        ts_utc: 1_748_000_000i64,
        payload_json: String::from_utf8_lossy(payload_raw).into_owned(),
    };

    // append() may return PayloadTooLarge — that's expected; not a bug.
    let _ = wal_write.append(&record);
    drop(wal_write);

    // Re-open with a DIFFERENT key to exercise the HMAC mismatch (tamper) path.
    let read_key = vec![0xabu8; 32];
    if let Ok(wal_read) = amore_core::wal::Wal::open_with_key(&tmp, read_key) {
        for result in wal_read.iter_from(0) {
            let _ = result; // Ok, Err, or silently skipped — none may panic
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);
});
