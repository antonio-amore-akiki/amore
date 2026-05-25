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

/// Reciprocal Rank Fusion constant. 60 = mem0 / LlamaIndex / Weaviate default.
const RRF_K: f32 = 60.0;

pub struct HybridRecall {
    ollama: OllamaClient,
    qdrant: QdrantStore,
    /// Optional BM25 lane via SQLite FTS5. None = pure-vector mode (back-
    /// compat path for callers that haven't wired SQLite yet).
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

impl HybridRecall {
    pub fn new(ollama: OllamaClient, qdrant: QdrantStore) -> Self {
        Self {
            ollama,
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

    /// End-to-end recall. If only Qdrant is wired -> pure cosine. If SQLite
    /// is also wired (`with_sqlite`) -> RRF fusion of vector + BM25 ranks.
    /// Over-fetches each lane to `top_k * 4` for richer fusion overlap, then
    /// returns the top `top_k` fused hits.
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<RecallHit>> {
        let fetch = (top_k * 4).max(top_k) as u64;
        let query_vec = self.ollama.embed(query).await?;
        let vec_hits = self.qdrant.search(query_vec, fetch).await?;
        let Some(sqlite) = self.sqlite.as_ref() else {
            // Pure-vector path — return Qdrant's top_k directly.
            return Ok(vec_hits.into_iter().take(top_k).map(map_hit).collect());
        };
        let bm_hits = sqlite.bm25_search(query, fetch)?;
        Ok(rrf_fuse(vec_hits, bm_hits, top_k))
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
        let vec = self.ollama.embed(text).await?;
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
