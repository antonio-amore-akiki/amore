//! End-to-end test: HybridRecall + OllamaClient + QdrantStore.
//!
//! Indexes a small corpus via `HybridRecall::index`, queries via
//! `HybridRecall::search`, asserts top hit semantic relevance.
//!
//! Skipped by default. Run with:
//!     OBELION_TEST_E2E=1 cargo test -p obelion-core --test hybrid_e2e -- --ignored
//!
//! Prerequisites:
//!   - Qdrant daemon at http://127.0.0.1:6334 (gRPC)
//!   - Ollama daemon at http://127.0.0.1:11434 with nomic-embed-text

use obelion_core::ollama::OllamaClient;
use obelion_core::qdrant_store::QdrantStore;
use obelion_core::recall::HybridRecall;

const QDRANT_URL: &str = "http://127.0.0.1:6334";
const OLLAMA_URL: &str = "http://127.0.0.1:11434";

fn enabled() -> bool {
    std::env::var("OBELION_TEST_E2E").ok().as_deref() == Some("1")
}

fn unique_collection(suffix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("obelion_e2e_{suffix}_{nanos:x}")
}

#[tokio::test]
#[ignore = "requires OBELION_TEST_E2E=1 + Qdrant + Ollama daemons"]
async fn end_to_end_index_and_recall() {
    if !enabled() {
        eprintln!("OBELION_TEST_E2E not set to 1 — skipping");
        return;
    }
    let collection = unique_collection("recall");
    let ollama = OllamaClient::new(OLLAMA_URL);
    let qdrant = QdrantStore::open(QDRANT_URL, &collection)
        .await
        .expect("open qdrant");
    let recall = HybridRecall::new(ollama, qdrant);

    // Index a 5-doc corpus of distinct topics.
    let corpus = [
        (
            1u64,
            "rust_async_post",
            "Rust tokio async runtime and the await keyword for futures",
        ),
        (
            2u64,
            "baking_recipe",
            "Chocolate chip cookies need flour, butter, sugar, and eggs",
        ),
        (
            3u64,
            "alps_hiking",
            "Hiking the Matterhorn route requires acclimatization and crampons",
        ),
        (
            4u64,
            "rust_borrow",
            "The borrow checker in Rust prevents data races at compile time",
        ),
        (
            5u64,
            "javascript_npm",
            "Node.js packages from the npm registry use semver versioning",
        ),
    ];

    for (id, source, text) in &corpus {
        recall
            .index(*id, source, text, None)
            .await
            .expect("index doc");
    }

    // Query semantically aligned with the two Rust docs.
    let envelope = recall
        .search("async networking and memory safety in systems languages", 3)
        .await
        .expect("recall");

    assert!(
        envelope.degraded.is_clean(),
        "happy-path recall must not flag any lane degraded, got {:?}",
        envelope.degraded
    );
    let hits = &envelope.hits;
    assert_eq!(hits.len(), 3, "expected 3 hits");
    let top = &hits[0];
    assert!(
        top.text.contains("Rust") || top.text.contains("borrow"),
        "top hit should be a Rust doc, got id={} text={}",
        top.id,
        top.text
    );
    assert_eq!(
        top.source,
        if top.text.contains("borrow") {
            "rust_borrow"
        } else {
            "rust_async_post"
        },
        "source field must be preserved through the payload roundtrip"
    );
    assert!(
        top.score > hits[1].score,
        "top score {} must beat second {}",
        top.score,
        hits[1].score
    );
    assert!(
        top.score >= 0.5,
        "top score {} must beat random baseline",
        top.score
    );

    // Clean up the test collection so repeated runs don't leak storage.
    let recall_owned = recall;
    let qdrant_back = QdrantStore::open(QDRANT_URL, &collection).await.unwrap();
    qdrant_back.drop_collection().await.expect("drop");
    drop(recall_owned);
}
