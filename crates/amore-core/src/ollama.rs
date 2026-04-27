use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::circuit_breaker::{BreakerError, CircuitBreaker};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_EMBED_MODEL: &str = "nomic-embed-text";
const DEFAULT_LLM_MODEL: &str = "qwen3:8b";
const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub struct OllamaClient {
    base_url: String, embed_model: String, llm_model: String, http: Client,
    breaker: Option<CircuitBreaker>,
}
#[derive(Debug, Serialize)] struct EmbedRequest<'a> { model: &'a str, prompt: &'a str }
#[derive(Debug, Deserialize)] struct EmbedResponse { embedding: Vec<f32> }
#[derive(Debug, Serialize)] struct GenerateRequest<'a> {
    model: &'a str, prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")] system: Option<&'a str>, stream: bool,
}
#[derive(Debug, Deserialize)] struct GenerateResponse { response: String }

impl OllamaClient {
    pub fn new(base_url: &str) -> Self {
        let url = if base_url.is_empty() { DEFAULT_BASE_URL.to_string() } else { base_url.trim_end_matches('/').to_string() };
        Self {
            base_url: url, embed_model: DEFAULT_EMBED_MODEL.to_string(), llm_model: DEFAULT_LLM_MODEL.to_string(),
            http: Client::builder().timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS)).build()
                .expect("reqwest client build (infallible defaults)"),
            breaker: None,
        }
    }
    pub fn with_models(mut self, embed: &str, llm: &str) -> Self { self.embed_model = embed.to_string(); self.llm_model = llm.to_string(); self }
    pub fn with_breaker(mut self, breaker: CircuitBreaker) -> Self { self.breaker = Some(breaker); self }
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if let Some(breaker) = &self.breaker {
            return breaker.call(|| async { self.embed_inner(text).await }).await
                .map_err(|e| match e {
                    BreakerError::Open { dep, open_for } => {
                        tracing::warn!(target: "amore.ollama", dep = %dep, open_for = ?open_for, "ollama.embed breaker open");
                        anyhow::anyhow!("circuit breaker open dep={dep} open_for={open_for:?}")
                    }
                    BreakerError::Inner(inner) => inner,
                });
        }
        self.embed_inner(text).await
    }
    async fn embed_inner(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);
        let resp = self.http.post(&url).json(&EmbedRequest { model: &self.embed_model, prompt: text })
            .send().await.with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        if !status.is_success() { let body = resp.text().await.unwrap_or_default(); anyhow::bail!("Ollama embed failed: HTTP {status} -- body: {body}"); }
        let parsed: EmbedResponse = resp.json().await.with_context(|| "decoding /api/embeddings")?;
        if parsed.embedding.is_empty() { anyhow::bail!("Ollama empty embedding for model {}", self.embed_model); }
        Ok(parsed.embedding)
    }
    pub async fn extract_facts(&self, _observation: &str) -> Result<Vec<String>> { Ok(vec![]) }
    pub async fn generate(&self, system: Option<&str>, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        let resp = self.http.post(&url).json(&GenerateRequest { model: &self.llm_model, prompt, system, stream: false })
            .send().await.with_context(|| format!("POST {url}"))?;
        let status = resp.status();
        if !status.is_success() { let body = resp.text().await.unwrap_or_default(); anyhow::bail!("Ollama generate failed: HTTP {status} -- body: {body}"); }
        let parsed: GenerateResponse = resp.json().await.with_context(|| "decoding /api/generate")?;
        Ok(parsed.response)
    }
    pub fn base_url(&self) -> &str { &self.base_url }
    pub fn embed_model(&self) -> &str { &self.embed_model }
    pub fn llm_model(&self) -> &str { &self.llm_model }
}
impl Default for OllamaClient { fn default() -> Self { Self::new(DEFAULT_BASE_URL) } }
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn new_trims_trailing_slash() { let c = OllamaClient::new("http://127.0.0.1:11434/"); assert_eq!(c.base_url(), "http://127.0.0.1:11434"); }
    #[test] fn new_empty_falls_back_to_default() { let c = OllamaClient::new(""); assert_eq!(c.base_url(), DEFAULT_BASE_URL); }
    #[test] fn defaults_set_expected_models() { let c = OllamaClient::default(); assert_eq!(c.embed_model(), DEFAULT_EMBED_MODEL); assert_eq!(c.llm_model(), DEFAULT_LLM_MODEL); }
    #[test] fn with_models_overrides_both() { let c = OllamaClient::new(DEFAULT_BASE_URL).with_models("mxbai-embed-large", "llama3:8b"); assert_eq!(c.embed_model(), "mxbai-embed-large"); assert_eq!(c.llm_model(), "llama3:8b"); }
}