// QA B1 + B2 (unit-level) — HybridRecall degraded paths.
//
// Proves the graceful-degradation contract WITHOUT a live Qdrant/Ollama:
//   * Ollama down (embed fails)    -> envelope.degraded.ollama_unavailable=true,
//                                     hits come through from BM25 lane (if attached).
//   * Qdrant down (search fails)   -> envelope.degraded.qdrant_unavailable=true,
//                                     hits come through from BM25 lane (if attached).
//   * Both vector deps down + no BM25 lane -> Err (no silent fail-open).
//
// Live-daemon B1 / B2 shell harness (kill the container mid-test, observe the
// structured WARN line in stderr) is a separate gate; this file proves the
// degraded-envelope contract at the data-flow level, which the shell harness
// then layers on top.

use anyhow::Result;
use obelion_core::qdrant_store::QdrantStore;
use obelion_core::recall::{Embedder, HybridRecall};
use obelion_core::sqlite_store::SqliteStore;
use std::sync::Arc;

/// Mock embedder that ALWAYS returns Err — simulates `Ollama unreachable`.
struct FlakyEmbedder {
    err_msg: &'static str,
}

impl Embedder for FlakyEmbedder {
    async fn embed_query(&self, _text: &str) -> Result<Vec<f32>> {
        anyhow::bail!("{}", self.err_msg)
    }
}

/// Mock embedder that returns Ok([0.0; 768]) — simulates a working Ollama
/// (lets us isolate the qdrant_unavailable case).
struct OkEmbedder;

impl Embedder for OkEmbedder {
    async fn embed_query(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(vec![0.0_f32; 768])
    }
}

/// Build an in-memory SQLite + FTS5 store and index a few sample observations.
fn sqlite_with_corpus() -> Arc<SqliteStore> {
    let store = SqliteStore::open_in_memory().expect("open_in_memory");
    let observations = [
        ("ollama_doc", "ollama is the local LLM runtime"),
        ("qdrant_doc", "qdrant stores vectors and does cosine search"),
        ("rust_doc", "rust borrow checker prevents data races"),
    ];
    for (source, text) in observations {
        let payload = serde_json::json!({"text": text});
        store
            .insert_observation(source, &payload)
            .expect("insert observation");
    }
    Arc::new(store)
}

#[tokio::test]
async fn b1_qdrant_unavailable_returns_bm25_hits_with_degraded_flag() {
    // Ollama mock OK, Qdrant constructed with a bogus URL on an unused port.
    // search() against that QdrantStore must fail; recall must keep going via BM25.
    let qdrant =
        QdrantStore::open_lazy("http://127.0.0.1:1", "b1_test").expect("open_lazy never connects");
    let sqlite = sqlite_with_corpus();
    let recall = HybridRecall::with_embedder(OkEmbedder, qdrant).with_sqlite(sqlite);

    let envelope = recall
        .search("ollama runtime", 5)
        .await
        .expect("recall must succeed via BM25 even with Qdrant down");

    assert!(
        envelope.degraded.qdrant_unavailable,
        "qdrant_unavailable must be flagged true; got {:?}",
        envelope.degraded
    );
    assert!(
        !envelope.degraded.ollama_unavailable,
        "ollama_unavailable must stay false in B1 scenario; got {:?}",
        envelope.degraded
    );
    assert!(
        !envelope.degraded.bm25_unavailable,
        "bm25_unavailable must stay false (BM25 served hits); got {:?}",
        envelope.degraded
    );
    assert!(
        !envelope.hits.is_empty(),
        "BM25 lane must serve hits when vector lane is down; got 0 hits"
    );
    assert!(
        envelope
            .hits
            .iter()
            .any(|h| h.source == "ollama_doc" || h.text.contains("ollama")),
        "expected an `ollama` BM25 hit, got {:?}",
        envelope.hits.iter().map(|h| &h.text).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn b2_ollama_unavailable_returns_bm25_hits_with_degraded_flag() {
    // Flaky embedder simulates Ollama unreachable; vector lane skipped before
    // Qdrant is ever called, so QdrantStore lazy construct is fine.
    let qdrant = QdrantStore::open_lazy("http://127.0.0.1:1", "b2_test").expect("open_lazy");
    let sqlite = sqlite_with_corpus();
    let recall = HybridRecall::with_embedder(
        FlakyEmbedder {
            err_msg: "ollama: connection refused",
        },
        qdrant,
    )
    .with_sqlite(sqlite);

    let envelope = recall
        .search("borrow checker rust", 5)
        .await
        .expect("recall must succeed via BM25 even with Ollama down");

    assert!(
        envelope.degraded.ollama_unavailable,
        "ollama_unavailable must be flagged true; got {:?}",
        envelope.degraded
    );
    assert!(
        !envelope.degraded.qdrant_unavailable,
        "qdrant_unavailable must stay false (we never reached Qdrant); got {:?}",
        envelope.degraded
    );
    assert!(
        !envelope.hits.is_empty(),
        "BM25 lane must serve hits when vector lane is down"
    );
    assert!(
        envelope
            .hits
            .iter()
            .any(|h| h.source == "rust_doc" || h.text.contains("rust")),
        "expected a `rust` BM25 hit, got {:?}",
        envelope.hits.iter().map(|h| &h.text).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn both_vector_lanes_dead_and_no_sqlite_returns_err_actionable() {
    // No SQLite attached + flaky embedder = zero usable lanes. CLAUDE.md hard
    // gate: refuse to return an empty envelope silently; bail with actionable
    // remediation in the error message.
    let qdrant = QdrantStore::open_lazy("http://127.0.0.1:1", "both_dead_test").expect("open_lazy");
    let recall = HybridRecall::with_embedder(
        FlakyEmbedder {
            err_msg: "ollama: connection refused",
        },
        qdrant,
    );
    let err = recall
        .search("anything", 5)
        .await
        .expect_err("both lanes dead must propagate Err, not silent empty");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("all retrieval lanes unavailable")
            || msg.contains("ollama")
            || msg.contains("qdrant"),
        "Err must mention which lanes are dead, got: {msg}"
    );
}

#[tokio::test]
async fn bm25_only_envelope_has_qdrant_flag_set_but_returns_hits() {
    // Equivalent to B1 with focus on the precise envelope: lazy qdrant fails,
    // BM25 returns hits, envelope.qdrant_unavailable=true, envelope.hits non-empty.
    // Locks the per-call semantics independently of B1's lane-coverage assert.
    let qdrant = QdrantStore::open_lazy("http://127.0.0.1:1", "happy_test").expect("open_lazy");
    let sqlite = sqlite_with_corpus();
    let recall = HybridRecall::with_embedder(OkEmbedder, qdrant).with_sqlite(sqlite);

    let envelope = recall
        .search("qdrant cosine search", 3)
        .await
        .expect("recall ok");
    assert!(
        envelope.degraded.qdrant_unavailable,
        "bogus qdrant must flag unavailable"
    );
    assert!(
        envelope.hits.iter().any(|h| h.text.contains("qdrant")),
        "BM25 must rank the qdrant doc highly; got {:?}",
        envelope.hits.iter().map(|h| &h.text).collect::<Vec<_>>()
    );
}
