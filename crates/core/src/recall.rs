// Hybrid retrieval: BM25 + vector + graph.
//
// Strategy:
//   1. Issue parallel queries: BM25 (SQLite FTS5) + vector (Qdrant) + graph traversal (SQLite)
//   2. Reciprocal Rank Fusion of the three result lists
//   3. Cross-encoder reranking on top-K survivors
//   4. Return top-N with provenance + scores

use anyhow::Result;

pub struct HybridRecall {
    // TODO: SqliteStore, QdrantStore, OllamaClient
}

impl HybridRecall {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn search(&self, _query: &str, _top_k: usize) -> Result<Vec<RecallHit>> {
        // TODO: implement
        Ok(vec![])
    }
}

impl Default for HybridRecall {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecallHit {
    pub id: String,
    pub score: f32,
    pub text: String,
    pub source: String,
}
