// Hybrid retrieval engine.
//
// Strategy (v0.1.0 = vector-only path):
//   1. Embed query via Ollama (nomic-embed-text, 768-dim)
//   2. Search Qdrant by cosine similarity, top_k * over-fetch
//   3. Return Vec<RecallHit> ordered by score desc
//
// S8 adds BM25 fusion (SQLite FTS5) + RRF (Reciprocal Rank Fusion) +
// cross-encoder reranking on top-50 vector candidates. Today's path is
// the pure-vector substrate — proves the wire end-to-end before adding
// fusion layers.

use anyhow::Result;

use crate::ollama::OllamaClient;
use crate::qdrant_store::{QdrantStore, SearchHit};

pub struct HybridRecall {
    ollama: OllamaClient,
    qdrant: QdrantStore,
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
        Self { ollama, qdrant }
    }

    /// End-to-end vector recall. v0.1.0 path: pure cosine via Qdrant.
    /// S8 will fuse with BM25 via Reciprocal Rank Fusion.
    pub async fn search(&self, query: &str, top_k: usize) -> Result<Vec<RecallHit>> {
        let query_vec = self.ollama.embed(query).await?;
        let hits = self.qdrant.search(query_vec, top_k as u64).await?;
        Ok(hits.into_iter().map(map_hit).collect())
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
