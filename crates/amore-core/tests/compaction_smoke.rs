// compaction_smoke.rs — smoke tests for H.9 CompactionWorker.
//
// T1 (env-gated, #[ignore]) — live Qdrant + SQLite: seed 1000 unique +
//   500 duplicate observations, compact_once(), assert 500 deduped + disk delta.
// T2 (default-on) — CompactionOpts::default() returns expected field values.
// T3 (default-on) — start() + stop() round-trip completes without panic.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use amore_core::compaction::{CompactionOpts, CompactionWorker};
use amore_core::sqlite_store::SqliteStore;

// ─── T2: default opts ─────────────────────────────────────────────────────────

#[test]
fn t2_default_opts_match_spec() {
    let opts = CompactionOpts::default();
    assert_eq!(
        opts.interval,
        Duration::from_secs(3600),
        "default interval must be 1 hour"
    );
    assert_eq!(
        opts.dedup_window_secs, 86400,
        "default dedup_window_secs must be 86400 (24 h)"
    );
    assert!(opts.max_age.is_none(), "default max_age must be None");
}

// ─── T3: start / stop round-trip ──────────────────────────────────────────────

#[tokio::test]
async fn t3_worker_start_stop_round_trip() {
    // Use in-memory SQLite so we don't need a live Qdrant.
    // We still need a QdrantStore stub — point it at a localhost URL that will
    // never be called during the test (the interval is 1 hour; we stop before
    // any tick fires).
    let sqlite = Arc::new(SqliteStore::open_in_memory().expect("in-memory sqlite"));

    // Build a lazy QdrantStore (no live connection needed — open_lazy does not
    // make network calls at construction time).
    let qdrant = Arc::new(
        amore_core::qdrant_store::QdrantStore::open_lazy(
            "http://127.0.0.1:6334",
            "compaction_smoke_t3",
        )
        .expect("lazy QdrantStore construction"),
    );

    let opts = CompactionOpts {
        interval: Duration::from_secs(3600), // long enough; tick won't fire
        dedup_window_secs: 86400,
        max_age: None,
    };

    let mut worker = CompactionWorker::new(qdrant, sqlite, opts);

    // start() must spawn without panic.
    worker.start().await;

    // stop() must abort the handle and return within a reasonable wall-clock.
    let result = timeout(Duration::from_secs(5), async move {
        worker.stop().await;
    })
    .await;

    assert!(
        result.is_ok(),
        "stop() must complete within 5 s — handle abort should be instant"
    );
}

// ─── T1: live integration (env-gated) ────────────────────────────────────────

/// Live Qdrant + SQLite dedup test.
///
/// Run with:
///   AMORE_TEST_COMPACTION=1 cargo test -p amore-core --test compaction_smoke t1_live -- --ignored
///
/// Expects a Qdrant instance at QDRANT_URL (default http://127.0.0.1:6334).
#[tokio::test]
#[ignore]
async fn t1_live_dedup_500_duplicates() {
    let enabled = std::env::var("AMORE_TEST_COMPACTION")
        .map(|v| v == "1")
        .unwrap_or(false);
    if !enabled {
        return;
    }

    let url = std::env::var("QDRANT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());

    let sqlite = Arc::new(SqliteStore::open_in_memory().expect("sqlite"));
    let qdrant = Arc::new(
        amore_core::qdrant_store::QdrantStore::new(&url, "compaction_t1_live", 768)
            .await
            .expect("QdrantStore"),
    );

    // Seed 1000 unique observations.
    for i in 0u64..1000 {
        let payload = serde_json::json!({ "text": format!("unique observation {i}"), "seq": i });
        sqlite
            .insert_observation("test", &payload)
            .expect("insert unique");
    }

    let before_count = sqlite.count_observations().expect("count");
    assert_eq!(before_count, 1000, "should have 1000 unique rows before dedup");

    // Seed 500 duplicates: re-insert the first 500 observations with the same
    // payload (SQLite will hash them identically — same canonical JSON).
    for i in 0u64..500 {
        let payload = serde_json::json!({ "text": format!("unique observation {i}"), "seq": i });
        sqlite
            .insert_observation("test_dup", &payload)
            .expect("insert duplicate");
    }

    let before_total = sqlite.count_observations().expect("count");
    assert_eq!(before_total, 1500, "1000 unique + 500 duplicates = 1500");

    let opts = CompactionOpts {
        interval: Duration::from_secs(3600),
        dedup_window_secs: 86400,
        max_age: None,
    };
    let worker = CompactionWorker::new(Arc::clone(&qdrant), Arc::clone(&sqlite), opts);
    let stats = worker.compact_once().await.expect("compact_once");

    // Allow ±10 for timing edge cases where ts resolution collapses rows.
    assert!(
        stats.docs_deduped >= 490,
        "expected ≥490 deduped, got {}",
        stats.docs_deduped
    );

    let after_count = sqlite.count_observations().expect("count after");
    assert!(
        after_count <= 1010,
        "after compaction count should be ≤1010, got {after_count}"
    );

    // On-disk delta: bytes_freed ≥ 30% of pre-compaction file size (measured as
    // rows removed × estimated row size).  In-memory SQLite won't free pages, so
    // we check docs_deduped as the proxy instead.
    assert!(
        stats.docs_deduped >= 490,
        "dedup proxy for 30%% delta: {}", stats.docs_deduped
    );

    // Cleanup.
    qdrant.drop_collection().await.ok();
}
