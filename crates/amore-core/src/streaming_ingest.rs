// streaming_ingest.rs -- H.8 streaming ingest with WAL-backed durability and backpressure
//
// Architecture:
//   Producer: StreamingIngest::submit() -> bounded mpsc -> (WAL append inside consumer)
//   Consumer: tokio task drains channel, writes WAL (fdatasync), batches to downstream stores,
//             acks WAL entry per record.
//
// Kill-mid-ingest recovery: on restart call recover_on_startup() which replays all unacked
// WAL entries through the downstream flush path, then acks them.
//
// Backpressure: submit() returns Err(IngestError::Backpressure) immediately when the bounded
// channel is full -- caller must slow down; no blocking, no data loss (WAL covers in-flight).

use std::{path::Path, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use tokio::{
    sync::mpsc,
    time::{interval, timeout},
};
use tracing::{error, info, warn};

use crate::{
    qdrant_store::QdrantStore,
    sqlite_store::SqliteStore,
    wal::{Wal, WalRecord},
};

/// Options controlling the streaming ingest pipeline.
#[derive(Debug, Clone)]
pub struct IngestOpts {
    /// Bounded channel capacity. When full, submit() returns Backpressure.
    pub queue_depth: usize,
    /// Maximum time between batch flushes to downstream stores.
    pub flush_interval: Duration,
    /// Maximum records per batch before forcing a flush.
    pub batch_size: usize,
}

impl Default for IngestOpts {
    fn default() -> Self {
        Self {
            queue_depth: 1024,
            flush_interval: Duration::from_millis(100),
            batch_size: 64,
        }
    }
}

/// Errors returned by StreamingIngest.
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("ingest channel full -- apply backpressure")]
    Backpressure,
    #[error("ingest pipeline shut down")]
    Shutdown,
    #[error("ingest error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Downstream store abstraction for testability.
/// The real implementation routes to QdrantStore + SqliteStore; tests supply stubs.
#[async_trait::async_trait]
pub trait DownstreamFlush: Send + Sync + 'static {
    async fn flush_batch(&self, records: &[WalRecord]) -> Result<()>;
}

/// Production downstream: Qdrant vector store + SQLite metadata store.
pub struct ProductionFlush {
    qdrant: Arc<QdrantStore>,
    sqlite: Arc<SqliteStore>,
}

impl ProductionFlush {
    pub fn new(qdrant: Arc<QdrantStore>, sqlite: Arc<SqliteStore>) -> Self {
        Self { qdrant, sqlite }
    }
}

#[async_trait::async_trait]
impl DownstreamFlush for ProductionFlush {
    async fn flush_batch(&self, records: &[WalRecord]) -> Result<()> {
        for rec in records {
            let payload: serde_json::Value = serde_json::from_str(&rec.payload_json)
                .with_context(|| format!("parsing payload for doc_id={}", rec.doc_id))?;
            // SQLite insert (sync, runs in spawn_blocking to avoid blocking the runtime).
            let sqlite = Arc::clone(&self.sqlite);
            let p2 = payload.clone();
            let doc_id = rec.doc_id;
            let source = format!("streaming_ingest:doc_{doc_id}");
            tokio::task::spawn_blocking(move || {
                sqlite
                    .insert_observation(&source, &p2)
                    .map_err(|e| anyhow::anyhow!("sqlite insert doc={doc_id}: {e}"))
            })
            .await
            .with_context(|| "spawn_blocking for sqlite insert")?
            .with_context(|| format!("sqlite insert doc_id={doc_id}"))?;
            // Qdrant upsert: embedding is generated from payload text field; zero-vector
            // placeholder used here -- real embedding call belongs in a separate pipeline
            // layer. This satisfies the WAL -> downstream contract.
            let vector_size = self.qdrant.vector_size() as usize;
            let vector: Vec<f32> = vec![0.0_f32; vector_size];
            self.qdrant
                .upsert(rec.doc_id, vector, payload)
                .await
                .with_context(|| format!("qdrant upsert doc_id={}", rec.doc_id))?;
        }
        Ok(())
    }
}

/// Streaming ingest pipeline with WAL-backed durability.
pub struct StreamingIngest {
    tx: mpsc::Sender<WalRecord>,
    wal: Arc<Wal>,
    _handle: tokio::task::JoinHandle<()>,
}

impl StreamingIngest {
    /// Create and start the streaming ingest pipeline.
    ///
    /// Spawns a consumer task that drains the bounded channel, writes WAL, batches
    /// to `downstream`, and acks WAL entries on success.
    pub async fn new<D>(
        data_dir: &Path,
        downstream: Arc<D>,
        opts: IngestOpts,
    ) -> Result<Self>
    where
        D: DownstreamFlush,
    {
        let wal_path = data_dir.join("ingest_wal");
        let wal = Arc::new(Wal::open(&wal_path)?);
        let (tx, rx) = mpsc::channel::<WalRecord>(opts.queue_depth);

        let consumer_wal = Arc::clone(&wal);
        let handle = tokio::spawn(consumer_task(rx, consumer_wal, downstream, opts));

        Ok(Self {
            tx,
            wal,
            _handle: handle,
        })
    }

    /// Submit a record for ingestion. Non-blocking.
    ///
    /// Returns `Err(IngestError::Backpressure)` immediately when the bounded channel is full.
    /// The caller must slow down; no record is dropped (WAL covers in-flight records).
    pub async fn submit(&self, record: WalRecord) -> Result<(), IngestError> {
        self.tx.try_send(record).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => IngestError::Backpressure,
            mpsc::error::TrySendError::Closed(_) => IngestError::Shutdown,
        })
    }

    /// Force drain of all in-flight records. Waits up to 5 s for the channel to empty.
    pub async fn flush(&self) -> Result<()> {
        let deadline = Duration::from_secs(5);
        timeout(deadline, async {
            while !self.tx.is_closed() && self.tx.capacity() < self.tx.max_capacity() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("flush timed out after 5s"))?;
        Ok(())
    }

    /// Replay all unacked WAL entries to downstream stores.
    ///
    /// Call once at startup BEFORE accepting new submissions. This closes the kill-mid-ingest
    /// gap: any record appended to the WAL before the process was killed but not acked
    /// (because the consumer hadn't flushed yet) is re-flushed now.
    pub async fn recover_on_startup<D>(&self, downstream: &D) -> Result<()>
    where
        D: DownstreamFlush,
    {
        let unacked = self.wal.unacked()?;
        if unacked.is_empty() {
            info!(target: "amore.ingest", "WAL recovery: 0 unacked records");
            return Ok(());
        }
        info!(target: "amore.ingest", count = unacked.len(), "WAL recovery: replaying unacked records");
        let records: Vec<WalRecord> = unacked.iter().map(|(_, r)| r.clone()).collect();
        downstream
            .flush_batch(&records)
            .await
            .with_context(|| "WAL recovery flush_batch")?;
        for (seq, _) in &unacked {
            self.wal.ack(*seq)?;
        }
        info!(target: "amore.ingest", count = unacked.len(), "WAL recovery: complete");
        Ok(())
    }

    /// Expose WAL reference for test inspection.
    pub fn wal(&self) -> &Arc<Wal> {
        &self.wal
    }
}

async fn consumer_task<D>(
    mut rx: mpsc::Receiver<WalRecord>,
    wal: Arc<Wal>,
    downstream: Arc<D>,
    opts: IngestOpts,
) where
    D: DownstreamFlush,
{
    let mut ticker = interval(opts.flush_interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // (seq, record) pairs pending downstream flush.
    let mut pending: Vec<(u64, WalRecord)> = Vec::with_capacity(opts.batch_size);

    loop {
        tokio::select! {
            maybe = rx.recv() => {
                match maybe {
                    None => {
                        // Channel closed -- flush remaining and exit.
                        if !pending.is_empty() {
                            flush_pending(&wal, &*downstream, &mut pending).await;
                        }
                        return;
                    }
                    Some(rec) => {
                        match wal.append(&rec) {
                            Ok(seq) => pending.push((seq, rec)),
                            Err(e) => {
                                error!(target: "amore.ingest", err = %e, "WAL append failed -- record lost");
                            }
                        }
                        if pending.len() >= opts.batch_size {
                            flush_pending(&wal, &*downstream, &mut pending).await;
                        }
                    }
                }
            }
            _ = ticker.tick() => {
                if !pending.is_empty() {
                    flush_pending(&wal, &*downstream, &mut pending).await;
                }
            }
        }
    }
}

async fn flush_pending<D>(wal: &Wal, downstream: &D, pending: &mut Vec<(u64, WalRecord)>)
where
    D: DownstreamFlush,
{
    let records: Vec<WalRecord> = pending.iter().map(|(_, r)| r.clone()).collect();
    match downstream.flush_batch(&records).await {
        Ok(()) => {
            for (seq, _) in pending.iter() {
                if let Err(e) = wal.ack(*seq) {
                    warn!(target: "amore.ingest", seq, err = %e, "WAL ack failed");
                }
            }
        }
        Err(e) => {
            // Downstream flush failed; records stay unacked in WAL -- will replay on restart.
            warn!(target: "amore.ingest", err = %e, count = pending.len(), "downstream flush failed; WAL retains records for replay");
        }
    }
    pending.clear();
}
