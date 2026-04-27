// compaction.rs — background compaction worker for amore-core.
//
// Responsibilities:
//   1. SHA-256 dedup: observations with identical payload hashes keep only
//      the newest row; stale duplicates are deleted from SQLite + Qdrant.
//   2. Age eviction (optional): delete rows older than `max_age`.
//   3. SQLite incremental_vacuum: reclaim freed pages after deletions.
//
// H.9 — new file; no touch to reranker.rs / cache.rs / wal.rs.

use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::qdrant_store::QdrantStore;
use crate::sqlite_store::SqliteStore;

// ─── Configuration ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CompactionOpts {
    /// How often the background loop runs. Default: 1 hour.
    pub interval: Duration,
    /// Dedup window (seconds). Observations inserted within this many seconds
    /// of *now* are eligible for dedup. Default: 86400 (24 h).
    pub dedup_window_secs: u64,
    /// If Some, also evict observations older than this duration.
    pub max_age: Option<Duration>,
}

impl Default for CompactionOpts {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3600),
            dedup_window_secs: 86400,
            max_age: None,
        }
    }
}

// ─── Stats returned per compaction pass ───────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct CompactionStats {
    /// Number of duplicate observation rows removed.
    pub docs_deduped: u64,
    /// Approximate bytes freed in SQLite (pages * page_size).
    pub bytes_freed: u64,
    /// Number of age-evicted rows (only non-zero when max_age is Some).
    pub rows_evicted: u64,
}

// ─── Worker ───────────────────────────────────────────────────────────────────

pub struct CompactionWorker {
    qdrant: Arc<QdrantStore>,
    sqlite: Arc<SqliteStore>,
    opts: CompactionOpts,
    handle: Option<JoinHandle<()>>,
}

impl CompactionWorker {
    pub fn new(qdrant: Arc<QdrantStore>, sqlite: Arc<SqliteStore>, opts: CompactionOpts) -> Self {
        Self { qdrant, sqlite, opts, handle: None }
    }

    /// Start the background compaction loop. Idempotent: calling twice
    /// replaces the old handle after aborting it.
    pub async fn start(&mut self) {
        // Abort any previous task before spawning a new one.
        if let Some(h) = self.handle.take() {
            h.abort();
        }
        let qdrant = Arc::clone(&self.qdrant);
        let sqlite = Arc::clone(&self.sqlite);
        let opts = self.opts.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(opts.interval);
            loop {
                interval.tick().await;
                match do_compact(&qdrant, &sqlite, &opts).await {
                    Ok(stats) => {
                        tracing::info!(
                            target: "amore.compaction",
                            docs_deduped = stats.docs_deduped,
                            rows_evicted = stats.rows_evicted,
                            bytes_freed = stats.bytes_freed,
                            "compaction pass complete"
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            target: "amore.compaction",
                            error = %e,
                            "compaction pass failed — will retry next interval"
                        );
                    }
                }
            }
        });
        self.handle = Some(handle);
    }

    /// Graceful shutdown: abort the background task and wait for it to finish.
    pub async fn stop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
            // Await to confirm the task is gone; ignore the expected Cancelled error.
            let _ = h.await;
        }
    }

    /// Run one compaction pass synchronously (callable from tests or admin endpoints).
    #[tracing::instrument(skip(self))]
    pub async fn compact_once(&self) -> Result<CompactionStats> {
        do_compact(&self.qdrant, &self.sqlite, &self.opts).await
    }
}

// ─── Core algorithm ───────────────────────────────────────────────────────────

async fn do_compact(
    qdrant: &QdrantStore,
    sqlite: &SqliteStore,
    opts: &CompactionOpts,
) -> Result<CompactionStats> {
    let mut stats = CompactionStats::default();

    // ── Step 1: collect duplicate doc_ids within the dedup window ─────────────
    let stale_ids: Vec<String> = sqlite
        .compaction_find_stale_duplicates(opts.dedup_window_secs)
        .with_context(|| "finding stale duplicate observations")?;

    let deduped_count = stale_ids.len() as u64;

    // ── Step 2: remove stale duplicates from Qdrant (best-effort) ─────────────
    if !stale_ids.is_empty() {
        qdrant
            .delete_by_ids(&stale_ids)
            .await
            .with_context(|| "deleting stale vectors from Qdrant")?;

        // ── Step 3: remove stale duplicates from SQLite ────────────────────────
        sqlite
            .compaction_delete_by_ids(&stale_ids)
            .with_context(|| "deleting stale observations from SQLite")?;

        stats.docs_deduped = deduped_count;
    }

    // ── Step 4: age eviction ───────────────────────────────────────────────────
    if let Some(max_age) = opts.max_age {
        let evict_ids: Vec<String> = sqlite
            .compaction_find_aged_rows(max_age)
            .with_context(|| "finding aged observations for eviction")?;

        if !evict_ids.is_empty() {
            qdrant
                .delete_by_ids(&evict_ids)
                .await
                .with_context(|| "deleting aged vectors from Qdrant")?;

            sqlite
                .compaction_delete_by_ids(&evict_ids)
                .with_context(|| "deleting aged observations from SQLite")?;

            stats.rows_evicted = evict_ids.len() as u64;
        }
    }

    // ── Step 5: incremental_vacuum to reclaim freed SQLite pages ──────────────
    let freed = sqlite
        .compaction_incremental_vacuum(1000)
        .with_context(|| "running SQLite incremental_vacuum")?;
    stats.bytes_freed = freed;

    Ok(stats)
}
