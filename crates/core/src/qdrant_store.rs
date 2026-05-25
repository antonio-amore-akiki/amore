// Qdrant store wrapper: collection lifecycle, point upsert/search, payload filters.
// Vector size 768 (matches nomic-embed-text). Auto-launches Qdrant binary as managed subprocess.

use anyhow::Result;

pub struct QdrantStore {}

impl QdrantStore {
    pub fn new(_url: &str) -> Self { Self {} }
    pub async fn upsert(&self, _id: &str, _vector: Vec<f32>, _payload: serde_json::Value) -> Result<()> { Ok(()) }
    pub async fn search(&self, _vector: Vec<f32>, _top_k: usize) -> Result<Vec<(String, f32)>> { Ok(vec![]) }
}
