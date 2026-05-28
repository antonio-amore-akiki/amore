// Hybrid retrieval engine.
//
// v0.1.0 path = vector-only over Qdrant.
// v0.2.0 path (S8 — this module) = BM25 (SQLite FTS5) + vector (Qdrant)
// fused via Reciprocal Rank Fusion (RRF) at k=60. The k constant is the
// standard from Cormack/Clarke/Buettcher 2009; higher k flattens the
// distribution so top-of-list bias decreases. We picked 60 to match
// established hybrid-retrieval defaults (mem0, LlamaIndex, Weaviate).
//
// Fusion math (per-document):
//   rrf(d) = sum over (lane, rank) of  1 / (k + rank)
// A document appearing in BOTH lanes accumulates contributions and beats
// single-lane hits — that's the whole point of RRF.
//
// S14 adds cross-encoder reranking on top-50 fused candidates.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ollama::OllamaClient;
use crate::qdrant_store::{QdrantStore, SearchHit};
use crate::sqlite_store::{Bm25Hit, SqliteStore};
use crate::timeout::{resolve_timeout, with_timeout};

/// Reciprocal Rank Fusion constant. 60 = mem0 / LlamaIndex / Weaviate default.
const RRF_K: f32 = 60.0;

/// Embedder abstraction (Rust 2024 native async-fn-in-traits, same pattern
/// as `LlmClient` in ensemble.rs). Prod wires OllamaClient; degraded-path
/// tests wire a mock that returns Err on demand.
pub trait Embedder: Send + Sync {
    fn embed_query(&self, text: &str)
    -> impl std::future::Future<Output = Result<Vec<f32>>> + Send;
}

impl Embedder for OllamaClient {
    fn embed_query(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = Result<Vec<f32>>> + Send {
        self.embed(text)
    }
}

pub struct HybridRecall<E: Embedder = OllamaClient> {
    embedder: E,
    qdrant: QdrantStore,
    sqlite: Option<Arc<SqliteStore>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecallHit {
    pub id: String,
    pub score: f32,
    pub text: String,
    pub source: String,
    pub payload: serde_json::Value,
}

/// Per-lane availability flags surfaced to the caller. Vector + BM25 are
/// co-equal primary lanes (NOT primary/fallback); `Degraded` names which lane
/// is offline so the caller can prompt remediation instead of silently
/// returning fewer hits.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct Degraded {
    pub ollama_unavailable: bool,
    pub qdrant_unavailable: bool,
    pub bm25_unavailable: bool,
}

impl Degraded {
    pub fn is_clean(&self) -> bool {
        !self.ollama_unavailable && !self.qdrant_unavailable && !self.bm25_unavailable
    }
}

/// Envelope returned by [`HybridRecall::search`]. Carries the ranked hits
/// AND the per-lane availability flags so callers can distinguish "no hits
/// because the corpus has nothing" from "no hits because Qdrant is down".
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecallEnvelope {
    pub hits: Vec<RecallHit>,
    pub degraded: Degraded,
}

impl HybridRecall<OllamaClient> {
    /// Production constructor wiring the concrete OllamaClient embedder.
    pub fn new(ollama: OllamaClient, qdrant: QdrantStore) -> Self {
        Self::with_embedder(ollama, qdrant)
    }
}

impl<E: Embedder> HybridRecall<E> {
    /// Generic constructor — `with_embedder` lets degraded-path tests wire
    /// a mock that returns Err to exercise the ollama_unavailable code path.
    pub fn with_embedder(embedder: E, qdrant: QdrantStore) -> Self {
        Self {
            embedder,
            qdrant,
            sqlite: None,
        }
    }

    /// Attach an SQLite store for the BM25 lane. After this, `search()`
    /// performs hybrid retrieval (vector + BM25 fused via RRF).
    pub fn with_sqlite(mut self, sqlite: Arc<SqliteStore>) -> Self {
        self.sqlite = Some(sqlite);
        self
    }

    /// End-to-end recall. Returns a [`RecallEnvelope`] carrying the ranked
    /// hits and per-lane availability flags. Vector and BM25 are co-equal
    /// primary lanes; if one is down the envelope's `degraded` field names
    /// which. If BOTH lanes are dead, returns Err (no silent fail-open per
    /// CLAUDE.md hard gate). Over-fetches each lane to top_k*4 for fusion.
    #[tracing::instrument(skip(self), fields(top_k))]
    pub async fn search(&self, query: &str, top_k: usize) -> Result<RecallEnvelope> {
        let fetch = (top_k * 4).max(top_k) as u64;
        let mut degraded = Degraded::default();
        // Vector lane = embed + qdrant.search, each capped via with_timeout
        // (B3: AMORE_TIMEOUT_MS env, default 4000ms). Either failure WARNs
        // + flags the offline dep. Timeout surfaces as Err so the existing
        // match arm handles it the same as "connection refused".
        let t = resolve_timeout();
        let vec_hits = match with_timeout(t, self.embedder.embed_query(query)).await {
            Ok(qv) => match with_timeout(t, self.qdrant.search(qv, fetch)).await {
                Ok(hits) => hits,
                Err(e) => {
                    tracing::warn!(
                        target: "amore.recall",
                        error = %e,
                        "qdrant.unreachable — vector lane skipped"
                    );
                    degraded.qdrant_unavailable = true;
                    Vec::new()
                }
            },
            Err(e) => {
                tracing::warn!(
                    target: "amore.recall",
                    error = %e,
                    "ollama.embed.unreachable — vector lane skipped"
                );
                degraded.ollama_unavailable = true;
                Vec::new()
            }
        };

        // ---- bm25 lane -----------------------------------------------------
        let bm_hits = match self.sqlite.as_ref() {
            Some(sqlite) => match sqlite.bm25_search(query, fetch) {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(
                        target: "amore.recall",
                        error = %e,
                        "bm25_search.error — bm25 lane skipped"
                    );
                    degraded.bm25_unavailable = true;
                    Vec::new()
                }
            },
            None => {
                // Pure-vector mode: SQLite not attached. Only flag the BM25
                // lane degraded when the vector lane ALSO failed (the caller
                // deserves to know zero lanes are alive, with both causes).
                if degraded.ollama_unavailable || degraded.qdrant_unavailable {
                    degraded.bm25_unavailable = true;
                }
                Vec::new()
            }
        };

        // ---- hard-gate: both lanes dead -> Err, never silent empty ---------
        let vector_lane_alive = !degraded.ollama_unavailable && !degraded.qdrant_unavailable;
        let bm25_lane_alive = self.sqlite.is_some() && !degraded.bm25_unavailable;
        if !vector_lane_alive && !bm25_lane_alive {
            tracing::error!(
                target: "amore.recall",
                ollama_unavailable = degraded.ollama_unavailable,
                qdrant_unavailable = degraded.qdrant_unavailable,
                bm25_unavailable = degraded.bm25_unavailable,
                "recall.both_lanes_unavailable — refusing to return empty silently"
            );
            anyhow::bail!(
                "recall: all retrieval lanes unavailable (ollama={}, qdrant={}, bm25={}). \
                 Start Ollama (`ollama serve`) and Qdrant (`docker run qdrant/qdrant`), \
                 OR attach a SQLite BM25 store via HybridRecall::with_sqlite().",
                degraded.ollama_unavailable,
                degraded.qdrant_unavailable,
                degraded.bm25_unavailable,
            );
        }

        // ---- fuse + truncate -----------------------------------------------
        // When only one lane is alive, rrf_fuse degenerates cleanly: it ranks
        // the surviving lane's hits without any cross-lane lift. When both
        // lanes are alive, full RRF kicks in.
        let hits = if self.sqlite.is_some() {
            rrf_fuse(vec_hits, bm_hits, top_k)
        } else {
            vec_hits.into_iter().take(top_k).map(map_hit).collect()
        };
        Ok(RecallEnvelope { hits, degraded })
    }

    /// Convenience: embed a document and upsert into the underlying Qdrant
    /// store. Useful for tests + future indexer pipeline. Source identifies
    /// the originating observation channel (e.g. "user_prompt", "edit_log").
    pub async fn index(
        &self,
        id: u64,
        source: &str,
        text: &str,
        extra_payload: Option<serde_json::Value>,
    ) -> Result<()> {
        let vec = self.embedder.embed_query(text).await?;
        let mut payload = serde_json::json!({
            "source": source,
            "text": text,
        });
        if let Some(extra) = extra_payload
            && let (Some(p), Some(e)) = (payload.as_object_mut(), extra.as_object())
        {
            for (k, v) in e {
                p.insert(k.clone(), v.clone());
            }
        }
        self.qdrant.upsert(id, vec, payload).await?;
        Ok(())
    }

    /// Ingest an observation into both the BM25 (SQLite) and vector (Qdrant) lanes
    /// with graceful degradation. The BM25 lane is the authoritative sink — its
    /// failure is always propagated as Err. The vector lane failure is non-fatal:
    /// `persisted_vector` is set to false and `degraded.ollama_unavailable` or
    /// `degraded.qdrant_unavailable` is set, but Ok is still returned.
    ///
    /// The numeric Qdrant ID is derived from the first 8 bytes of the envelope's
    /// hex hash (big-endian u64) to ensure a stable, collision-resistant mapping
    /// between the SQLite string ID and the Qdrant point ID.
    ///
    /// Returns [`IngestReceipt`] which is serialised directly into the MCP
    /// `observe` tool response.
    pub async fn ingest(
        &self,
        source: &str,
        payload: &serde_json::Value,
    ) -> Result<IngestReceipt> {
        // BM25 lane — authoritative. Failure is fatal (propagated as Err).
        let sqlite = self.sqlite.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "observe: no SQLite store attached — call HybridRecall::with_sqlite() first"
            )
        })?;
        let env = sqlite.insert_observation(source, payload)?;

        // Extract indexable text: prefer payload.text, else canonical JSON.
        let text = payload
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or(&env.canonical_json)
            .to_string();

        // Derive a stable u64 Qdrant point-ID from the first 8 hex bytes of the
        // envelope hash (big-endian). Two hex chars = one byte; first 16 hex chars
        // = 8 bytes. Falls back to 0 on any parse error (hash is always 64 chars).
        let qdrant_id: u64 = u64::from_str_radix(&env.hash.chars().take(16).collect::<String>(), 16)
            .unwrap_or(0);

        // Vector lane — graceful degradation. Failure logs WARN and sets flags.
        let mut degraded = Degraded::default();
        let mut persisted_vector = false;
        let t = crate::timeout::resolve_timeout();
        match crate::timeout::with_timeout(t, self.embedder.embed_query(&text)).await {
            Ok(vec) => {
                let mut extra = serde_json::json!({
                    "source": source,
                    "text": text,
                });
                // Merge original payload fields into the Qdrant point payload.
                if let (Some(ep), Some(op)) = (extra.as_object_mut(), payload.as_object()) {
                    for (k, v) in op {
                        ep.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                match crate::timeout::with_timeout(t, self.qdrant.upsert(qdrant_id, vec, extra)).await {
                    Ok(()) => { persisted_vector = true; }
                    Err(e) => {
                        tracing::warn!(
                            target: "amore.ingest",
                            error = %e,
                            "qdrant.upsert.failed — vector lane not persisted"
                        );
                        degraded.qdrant_unavailable = true;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: "amore.ingest",
                    error = %e,
                    "ollama.embed.failed — vector lane not persisted"
                );
                degraded.ollama_unavailable = true;
            }
        }

        Ok(IngestReceipt {
            id: env.id,
            persisted_bm25: true,
            persisted_vector,
            degraded,
        })
    }
}

/// Receipt returned by [`HybridRecall::ingest`] and serialised into the MCP
/// `observe` tool response body.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IngestReceipt {
    /// The envelope ID (hex-encoded SHA-256 of the canonical payload).
    pub id: String,
    /// Whether the observation was persisted in the BM25 SQLite lane.
    pub persisted_bm25: bool,
    /// Whether the observation was persisted in the Qdrant vector lane.
    pub persisted_vector: bool,
    /// Per-lane degradation flags. Caller MUST inspect to detect lane outages.
    pub degraded: Degraded,
}

fn map_hit(h: SearchHit) -> RecallHit {
    let text = h
        .payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let source = h
        .payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    RecallHit {
        id: h.id,
        score: h.score,
        text,
        source,
        payload: h.payload,
    }
}

/// Reciprocal Rank Fusion of vector + BM25 lanes.
///
/// For each document appearing in either lane, accumulate `1 / (RRF_K + rank)`
/// where `rank` is 0-based position in that lane. Documents in both lanes
/// accumulate from both — that's the cross-lane lift RRF buys.
///
/// The fused `score` field is the RRF score (NOT the raw cosine or BM25). The
/// `payload` carries through whatever the vector lane stored; BM25-only hits
/// synthesize a minimal payload from the SQLite text+source columns.
pub(crate) fn rrf_fuse(
    vec_hits: Vec<SearchHit>,
    bm_hits: Vec<Bm25Hit>,
    top_k: usize,
) -> Vec<RecallHit> {
    let mut acc: HashMap<String, (f32, RecallHit)> = HashMap::new();
    for (rank, hit) in vec_hits.iter().enumerate() {
        let contrib = 1.0 / (RRF_K + rank as f32);
        let entry = acc
            .entry(hit.id.clone())
            .or_insert_with(|| (0.0, map_hit(hit.clone())));
        entry.0 += contrib;
    }
    for (rank, hit) in bm_hits.iter().enumerate() {
        let contrib = 1.0 / (RRF_K + rank as f32);
        let entry = acc.entry(hit.id.clone()).or_insert_with(|| {
            let payload = serde_json::json!({
                "source": hit.source,
                "text": hit.text,
            });
            (
                0.0,
                RecallHit {
                    id: hit.id.clone(),
                    score: 0.0, // filled below
                    text: hit.text.clone(),
                    source: hit.source.clone(),
                    payload,
                },
            )
        });
        entry.0 += contrib;
    }
    let mut fused: Vec<RecallHit> = acc
        .into_values()
        .map(|(rrf, mut h)| {
            h.score = rrf;
            h
        })
        .collect();
    fused.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fused.truncate(top_k);
    fused
}

// ---------------------------------------------------------------------------
// Tests for HybridRecall::ingest (A1)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod observe_tests {
    use super::*;
    use crate::qdrant_store::QdrantStore;
    use crate::sqlite_store::SqliteStore;
    use serde_json::json;
    use std::sync::Arc;

    // Mock embedder that always returns Err (simulates Ollama unavailability).
    struct FailEmbedder;
    impl Embedder for FailEmbedder {
        fn embed_query(
            &self,
            _text: &str,
        ) -> impl std::future::Future<Output = anyhow::Result<Vec<f32>>> + Send {
            async { anyhow::bail!("mock: ollama unavailable") }
        }
    }

    fn make_store() -> Arc<SqliteStore> {
        Arc::new(SqliteStore::open_in_memory().expect("in-memory sqlite"))
    }

    fn make_recall_no_vector(sqlite: Arc<SqliteStore>) -> HybridRecall<FailEmbedder> {
        // QdrantStore::open_lazy with a dummy URL — never reached because embed fails first.
        let qdrant = QdrantStore::open_lazy("http://127.0.0.1:6334", "amore_test")
            .expect("lazy qdrant client");
        HybridRecall::with_embedder(FailEmbedder, qdrant).with_sqlite(sqlite)
    }

    // Test 1: round-trip — BM25 lane persists, vector lane fails gracefully.
    #[tokio::test]
    async fn observe_round_trip_bm25_persisted() {
        let sqlite = make_store();
        let recall = make_recall_no_vector(Arc::clone(&sqlite));
        let payload = json!({"text": "hello amore round-trip"});
        let receipt = recall.ingest("test_source", &payload).await
            .expect("ingest must succeed with BM25-only path");

        assert!(!receipt.id.is_empty(), "id must be non-empty");
        assert!(receipt.persisted_bm25, "BM25 must be persisted");
        assert!(!receipt.persisted_vector, "vector must not be persisted (no Qdrant)");
        assert!(receipt.degraded.ollama_unavailable, "ollama flag must be set");
        assert!(!receipt.degraded.qdrant_unavailable, "qdrant flag not set (embed failed first)");

        // Verify the observation is actually in SQLite.
        let count = sqlite.count_observations().expect("count_observations");
        assert_eq!(count, 1, "exactly 1 observation in SQLite");
    }

    // Test 2: boundary — payload at exactly 16 KiB cap round-trips successfully.
    // (The cap is enforced at the MCP handler layer; ingest itself has no cap —
    // we validate that a large-but-valid payload is persisted correctly.)
    #[tokio::test]
    async fn observe_boundary_large_payload_ingests() {
        let sqlite = make_store();
        let recall = make_recall_no_vector(Arc::clone(&sqlite));
        // 16 000 bytes of 'x' — under the 16 384 byte MCP cap but large.
        let text = "x".repeat(16_000);
        let payload = json!({"text": text});
        let receipt = recall.ingest("boundary_test", &payload).await
            .expect("large payload must ingest into SQLite");
        assert!(receipt.persisted_bm25);
        let count = sqlite.count_observations().expect("count");
        assert_eq!(count, 1);
    }

    // Test 3: partial degradation — ingest without sqlite attached returns Err.
    #[tokio::test]
    async fn observe_no_sqlite_returns_err() {
        let qdrant = QdrantStore::open_lazy("http://127.0.0.1:6334", "amore_test")
            .expect("lazy qdrant");
        // No with_sqlite() call — sqlite is None.
        let recall = HybridRecall::with_embedder(FailEmbedder, qdrant);
        let err = recall.ingest("src", &json!({"text": "hello"})).await
            .expect_err("must Err without SQLite attached");
        assert!(
            err.to_string().contains("no SQLite store attached"),
            "error must name the missing SQLite: {err}"
        );
    }

    // Test 4: concurrent — two simultaneous ingests both land without chain corruption.
    #[tokio::test]
    async fn observe_concurrent_ingests_both_land() {
        let sqlite = make_store();
        let recall = Arc::new(make_recall_no_vector(Arc::clone(&sqlite)));

        let r1 = Arc::clone(&recall);
        let r2 = Arc::clone(&recall);

        let (res1, res2) = tokio::join!(
            async move { r1.ingest("concurrent_a", &json!({"text": "observation one"})).await },
            async move { r2.ingest("concurrent_b", &json!({"text": "observation two"})).await },
        );
        let rec1 = res1.expect("concurrent ingest 1");
        let rec2 = res2.expect("concurrent ingest 2");

        // IDs must be distinct.
        assert_ne!(rec1.id, rec2.id, "concurrent observations must have distinct IDs");
        // Both must have persisted BM25.
        assert!(rec1.persisted_bm25 && rec2.persisted_bm25);

        let count = sqlite.count_observations().expect("count");
        assert_eq!(count, 2, "both concurrent observations must be in SQLite");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn vec_hit(id: &str, score: f32, text: &str, source: &str) -> SearchHit {
        SearchHit {
            id: id.to_string(),
            score,
            payload: json!({"text": text, "source": source}),
        }
    }
    fn bm_hit(id: &str, score: f32, text: &str, source: &str) -> Bm25Hit {
        Bm25Hit {
            id: id.to_string(),
            score,
            text: text.to_string(),
            source: source.to_string(),
        }
    }

    #[test]
    fn rrf_empty_lanes_empty_result() {
        assert!(rrf_fuse(vec![], vec![], 5).is_empty());
    }

    #[test]
    fn rrf_single_lane_passthrough_ranking() {
        let vec_hits = vec![
            vec_hit("A", 0.9, "alpha", "src"),
            vec_hit("B", 0.5, "beta", "src"),
        ];
        let fused = rrf_fuse(vec_hits, vec![], 5);
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].id, "A");
        assert_eq!(fused[1].id, "B");
        assert!(fused[0].score > fused[1].score);
    }

    #[test]
    fn rrf_doc_in_both_lanes_beats_single_lane_doc() {
        // A appears in BOTH lanes; B only in vector. A's RRF should beat B.
        let vec_hits = vec![
            vec_hit("A", 0.9, "rust", "src"),
            vec_hit("B", 0.85, "go", "src"),
        ];
        let bm_hits = vec![bm_hit("A", 5.0, "rust", "src")];
        let fused = rrf_fuse(vec_hits, bm_hits, 5);
        assert_eq!(fused[0].id, "A", "cross-lane doc must rank #1");
        assert_eq!(fused[1].id, "B");
        let a_score = fused[0].score;
        let b_score = fused[1].score;
        assert!(
            a_score > b_score,
            "A (in both lanes) score {a_score} must beat B (one lane) {b_score}"
        );
    }

    #[test]
    fn rrf_synthesizes_payload_for_bm25_only_hits() {
        let bm_hits = vec![bm_hit("X", 7.0, "only-in-bm25", "edit_log")];
        let fused = rrf_fuse(vec![], bm_hits, 5);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].id, "X");
        assert_eq!(fused[0].text, "only-in-bm25");
        assert_eq!(fused[0].source, "edit_log");
    }

    #[test]
    fn rrf_truncates_to_top_k() {
        let vec_hits = vec![
            vec_hit("A", 1.0, "", ""),
            vec_hit("B", 0.9, "", ""),
            vec_hit("C", 0.8, "", ""),
            vec_hit("D", 0.7, "", ""),
        ];
        let fused = rrf_fuse(vec_hits, vec![], 2);
        assert_eq!(fused.len(), 2);
    }
}
