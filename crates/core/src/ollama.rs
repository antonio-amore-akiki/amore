// Ollama client wrapper (LLM + embeddings).
// Default models: qwen2.5:7b (LLM), nomic-embed-text (768-dim embeddings).

use anyhow::Result;

pub struct OllamaClient {}

impl OllamaClient {
    pub fn new(_base_url: &str) -> Self { Self {} }
    pub async fn embed(&self, _text: &str) -> Result<Vec<f32>> { Ok(vec![]) }
    pub async fn extract_facts(&self, _observation: &str) -> Result<Vec<String>> { Ok(vec![]) }
}
