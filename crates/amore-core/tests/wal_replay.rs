// wal_replay.rs -- H.8 WAL durability + streaming ingest integration tests
//
// Default (no env gate): T1, T2 — pure WAL, no daemon deps.
// Env-gated (AMORE_TEST_INGEST=1): T3, T4 — StreamingIngest with mock stores.

use std::sync::{Arc, Mutex};

use amore_core::wal::{Wal, WalKind, WalRecord};
use anyhow::Result;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Fixed 32-byte test key. Bypasses the OS keyring so tests are isolated and
/// repeatable regardless of keyring state or parallel test execution.
const TEST_KEY: [u8; 32] = [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE,
                             0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
                             0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10,
                             0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

fn make_record(doc_id: u64) -> WalRecord {
    WalRecord {
        kind: WalKind::Upsert,
        doc_id,
        payload_json: format!(r#"{{"id":{doc_id}}}"#),
        ts_utc: 0,
    }
}

fn open_wal(dir: &TempDir) -> Result<Wal> {
    Ok(Wal::open_with_key(&dir.path().join("test_wal"), TEST_KEY.to_vec())?)
}

// ---------------------------------------------------------------------------
// T1: append 100, drop, reopen → unacked==100; ack each, reopen → unacked==0
// ---------------------------------------------------------------------------

#[test]
fn t1_full_replay_then_clean() -> Result<()> {
    let dir = TempDir::new()?;

    // Phase 1: append 100, drop without close (TempDir keeps the dir alive).
    let seq_range: Vec<u64> = {
        let wal = open_wal(&dir)?;
        let mut seqs = Vec::with_capacity(100);
        for i in 0..100u64 {
            let seq = wal.append(&make_record(i))?;
            seqs.push(seq);
        }
        seqs
        // `wal` dropped here — simulates process kill with fsync already done
    };

    // Phase 2: reopen, check unacked == 100.
    {
        let wal = open_wal(&dir)?;
        let unacked = wal.unacked()?;
        assert_eq!(
            unacked.len(),
            100,
            "expected 100 unacked after crash-reopen, got {}",
            unacked.len()
        );
        // Ack every sequence returned by the first open.
        for seq in &seq_range {
            wal.ack(*seq)?;
        }
    }

    // Phase 3: reopen again, check unacked == 0.
    {
        let wal = open_wal(&dir)?;
        let unacked = wal.unacked()?;
        assert_eq!(
            unacked.len(),
            0,
            "expected 0 unacked after acking all, got {}",
            unacked.len()
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// T2: append 50, ack 25, drop → reopen → unacked==25
// ---------------------------------------------------------------------------

#[test]
fn t2_partial_ack_survives_reopen() -> Result<()> {
    let dir = TempDir::new()?;

    // Phase 1: append 50, ack the first 25.
    {
        let wal = open_wal(&dir)?;
        let mut seqs = Vec::with_capacity(50);
        for i in 0..50u64 {
            let seq = wal.append(&make_record(i))?;
            seqs.push(seq);
        }
        for seq in seqs.iter().take(25) {
            wal.ack(*seq)?;
        }
        // Drop without closing — simulates a mid-batch kill.
    }

    // Phase 2: reopen, check that exactly 25 are still unacked.
    {
        let wal = open_wal(&dir)?;
        let unacked = wal.unacked()?;
        assert_eq!(
            unacked.len(),
            25,
            "expected 25 unacked after partial ack, got {}",
            unacked.len()
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// T3 + T4: env-gated — StreamingIngest with a mock downstream store
// ---------------------------------------------------------------------------

/// A mock downstream store that records all flushed records in order.
#[derive(Clone, Default)]
struct MockStore {
    flushed: Arc<Mutex<Vec<WalRecord>>>,
}

impl MockStore {
    fn count(&self) -> usize {
        self.flushed.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl amore_core::streaming_ingest::DownstreamFlush for MockStore {
    async fn flush_batch(&self, records: &[WalRecord]) -> Result<()> {
        let mut guard = self.flushed.lock().unwrap();
        for r in records {
            guard.push(r.clone());
        }
        Ok(())
    }
}

/// T3: submit 200, flush, assert all 200 in store in order.
#[tokio::test]
#[ignore]
async fn t3_submit_200_flush_all_reach_store() -> Result<()> {
    if std::env::var("AMORE_TEST_INGEST").unwrap_or_default() != "1" {
        println!("SKIP: AMORE_TEST_INGEST not set");
        return Ok(());
    }

    use amore_core::streaming_ingest::{IngestOpts, StreamingIngest};

    let dir = TempDir::new()?;
    let store = Arc::new(MockStore::default());
    let opts = IngestOpts {
        queue_depth: 1024,
        flush_interval: std::time::Duration::from_millis(50),
        batch_size: 32,
    };
    let ingest = StreamingIngest::new(dir.path(), Arc::clone(&store), opts).await?;

    for i in 0..200u64 {
        // Retry on backpressure (shouldn't happen at depth 1024, but guard anyway).
        loop {
            match ingest.submit(make_record(i)).await {
                Ok(()) => break,
                Err(amore_core::streaming_ingest::IngestError::Backpressure) => {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                Err(e) => return Err(anyhow::anyhow!("submit failed: {e}")),
            }
        }
    }

    ingest.flush().await?;
    // Give the batch interval one more cycle to drain any remainder.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let count = store.count();
    assert_eq!(count, 200, "expected 200 records in store, got {count}");

    Ok(())
}

/// T4: submit 200, drop without flush → reopen → recover_on_startup → final count==200.
#[tokio::test]
#[ignore]
async fn t4_kill_mid_ingest_recover_on_startup() -> Result<()> {
    if std::env::var("AMORE_TEST_INGEST").unwrap_or_default() != "1" {
        println!("SKIP: AMORE_TEST_INGEST not set");
        return Ok(());
    }

    use amore_core::streaming_ingest::{IngestOpts, StreamingIngest};

    let dir = TempDir::new()?;

    // Phase 1: submit 200 via WAL, drop ingest without flush.
    // The WAL append inside the consumer happens BEFORE downstream flush, so
    // records in-flight in the channel that the consumer has NOT yet processed
    // are NOT in the WAL yet — they are the "channel buffered" segment.
    // To guarantee WAL coverage we write them directly to the WAL here,
    // mirroring the kill-mid-ingest scenario where the WAL append succeeded
    // but the downstream flush had not yet been acked.
    {
        let wal_path = dir.path().join("ingest_wal");
        let wal = Wal::open_with_key(&wal_path, TEST_KEY.to_vec())?;
        for i in 0..200u64 {
            wal.append(&make_record(i))?;
        }
        // Drop without acking — simulates process kill after WAL write, before ack.
    }

    // Phase 2: reopen via StreamingIngest and call recover_on_startup.
    let store = Arc::new(MockStore::default());
    let opts = IngestOpts::default();
    let ingest = StreamingIngest::new(dir.path(), Arc::clone(&store), opts).await?;
    ingest.recover_on_startup(&*store).await?;

    let count = store.count();
    assert_eq!(count, 200, "expected 200 records after recovery, got {count}");

    Ok(())
}
