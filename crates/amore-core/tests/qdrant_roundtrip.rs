//! Integration test for QdrantStore upsert+search against a live Qdrant daemon.
//!
//! Skipped by default. Run with:
//!     AMORE_TEST_QDRANT=1 cargo test -p amore-core --test qdrant_roundtrip -- --ignored
//!
//! Prerequisites:
//!   - Qdrant daemon at http://127.0.0.1:6333 (download from
//!     github.com/qdrant/qdrant/releases)
//!   - Ollama daemon at http://127.0.0.1:11434 with nomic-embed-text
//!     (this test embeds real text rather than using synthetic vectors, so
//!     the vector path is end-to-end + we don't pollute the test with
//!     hand-tuned fake vectors that may accidentally hide bugs)

use amore_core::ollama::OllamaClient;
use amore_core::qdrant_store::QdrantStore;
use serde_json::json;

// qdrant-client (Rust) uses gRPC on port 6334; REST API on 6333 is separate.
// Both are exposed by the default Qdrant config.
const TEST_QDRANT_URL: &str = "http://127.0.0.1:6334";
const TEST_OLLAMA_URL: &str = "http://127.0.0.1:11434";

fn enabled() -> bool {
    std::env::var("AMORE_TEST_QDRANT").ok().as_deref() == Some("1")
}

/// Each test uses a unique collection name so parallel test runs don't
/// collide and so a previous failure doesn't poison later runs.
fn unique_collection(suffix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("amore_test_{suffix}_{nanos:x}")
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_QDRANT=1 + Qdrant + Ollama daemons"]
async fn upsert_then_search_returns_top_match() {
    if !enabled() {
        eprintln!("AMORE_TEST_QDRANT not set to 1 — skipping");
        return;
    }
    let collection = unique_collection("roundtrip");
    let store = QdrantStore::open(TEST_QDRANT_URL, &collection)
        .await
        .expect("connect + ensure collection");
    let ollama = OllamaClient::new(TEST_OLLAMA_URL);

    // Three semantically distinct documents.
    let docs = [
        (1u64, "Rust async runtime tokio for concurrent IO"),
        (2u64, "Chocolate chip cookies baking recipe"),
        (3u64, "Mountain hiking trail conditions in the Alps"),
    ];

    for (id, text) in &docs {
        let vec = ollama.embed(text).await.expect("embed doc");
        store
            .upsert(*id, vec, json!({"text": text}))
            .await
            .expect("upsert doc");
    }

    // Query semantically aligned with doc #1.
    let query_vec = ollama
        .embed("async Rust networking framework")
        .await
        .expect("embed query");
    let hits = store.search(query_vec, 3).await.expect("search");

    assert_eq!(hits.len(), 3, "expected 3 hits, got {}", hits.len());

    // Primary signal: the top hit must be the semantically-aligned doc.
    let top_text = hits[0]
        .payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        top_text.contains("Rust") || top_text.contains("tokio"),
        "top match should be the Rust doc, got: {top_text}"
    );

    // Sanity bound: top score must clearly beat random (~0). nomic-embed-text
    // semantic similarity for "Rust async runtime tokio" vs "async Rust networking
    // framework" is typically 0.6-0.7; a stricter threshold would be brittle.
    assert!(
        hits[0].score >= 0.5,
        "top hit score {} must be >= 0.5 (well above random)",
        hits[0].score
    );

    // Top hit must out-score the off-topic docs by a clear margin.
    assert!(
        hits[0].score > hits[1].score + 0.05,
        "top hit ({}) must clearly beat second hit ({}) by >= 0.05 margin",
        hits[0].score,
        hits[1].score
    );

    // Cleanup
    store.drop_collection().await.expect("drop collection");
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_QDRANT=1 + Qdrant daemon"]
async fn ensure_collection_is_idempotent() {
    if !enabled() {
        return;
    }
    let collection = unique_collection("idempotent");

    let store1 = QdrantStore::open(TEST_QDRANT_URL, &collection)
        .await
        .expect("first open");
    let _store2 = QdrantStore::open(TEST_QDRANT_URL, &collection)
        .await
        .expect("second open of same collection must succeed");

    store1.drop_collection().await.expect("cleanup");
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_QDRANT=1 + Qdrant daemon"]
async fn search_on_empty_collection_returns_empty() {
    if !enabled() {
        return;
    }
    let collection = unique_collection("empty");
    let store = QdrantStore::open(TEST_QDRANT_URL, &collection)
        .await
        .expect("open empty");
    let zero_vec = vec![0.0f32; 768];
    let hits = store.search(zero_vec, 5).await.expect("search empty");
    assert!(hits.is_empty(), "empty collection must return no hits");
    store.drop_collection().await.expect("cleanup");
}
