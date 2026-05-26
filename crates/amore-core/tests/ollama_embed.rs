//! Integration test for OllamaClient::embed against a live Ollama daemon.
//!
//! All tests in this file are `#[ignore]`d by default so `cargo test` without
//! `--ignored` passes on CI machines that don't have Ollama installed.
//!
//! Run with:
//!     AMORE_TEST_OLLAMA=1 cargo test -p amore-core --test ollama_embed -- --ignored
//!
//! Prerequisites:
//!     - Ollama daemon running at http://127.0.0.1:11434
//!     - `ollama pull nomic-embed-text`
//!
//! The env-var gate is belt-and-suspenders alongside `#[ignore]`: even if a
//! developer accidentally runs `cargo test -- --ignored`, the tests no-op
//! without AMORE_TEST_OLLAMA=1 set — preventing surprise network calls.

use amore_core::ollama::OllamaClient;

const TEST_BASE_URL: &str = "http://127.0.0.1:11434";
const EXPECTED_EMBED_DIM: usize = 768; // nomic-embed-text

fn enabled() -> bool {
    std::env::var("AMORE_TEST_OLLAMA").ok().as_deref() == Some("1")
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_OLLAMA=1 + Ollama daemon + nomic-embed-text model"]
async fn embed_returns_expected_dimension_vector_for_short_prompt() {
    if !enabled() {
        eprintln!("AMORE_TEST_OLLAMA not set to 1 — skipping");
        return;
    }
    let client = OllamaClient::new(TEST_BASE_URL);
    let vec = client
        .embed("hello world")
        .await
        .expect("embed must succeed against live Ollama");
    assert!(!vec.is_empty(), "embedding must be non-empty");
    assert_eq!(
        vec.len(),
        EXPECTED_EMBED_DIM,
        "nomic-embed-text returns {EXPECTED_EMBED_DIM}-dim vectors"
    );
    // Sanity: at least one non-zero entry (real model output, not all-zeros stub)
    assert!(
        vec.iter().any(|f| f.abs() > 1e-6),
        "embedding must contain non-trivial values"
    );
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_OLLAMA=1 + Ollama daemon"]
async fn embed_deterministic_for_same_input() {
    if !enabled() {
        return;
    }
    let client = OllamaClient::new(TEST_BASE_URL);
    let prompt = "the quick brown fox jumps over the lazy dog";
    let a = client.embed(prompt).await.expect("first embed");
    let b = client.embed(prompt).await.expect("second embed");
    assert_eq!(
        a, b,
        "same prompt must produce identical embeddings (Ollama is deterministic at temperature 0)"
    );
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_OLLAMA=1 + Ollama daemon"]
async fn embed_differentiates_unrelated_prompts() {
    if !enabled() {
        return;
    }
    let client = OllamaClient::new(TEST_BASE_URL);
    let a = client
        .embed("Rust async runtime tokio")
        .await
        .expect("embed a");
    let b = client
        .embed("recipe for chocolate chip cookies")
        .await
        .expect("embed b");
    assert_ne!(
        a, b,
        "semantically distinct prompts must produce distinct embeddings"
    );
    // Cosine similarity sanity: unrelated prompts should have similarity well under 1.0
    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cos = dot / (norm_a * norm_b);
    assert!(
        cos < 0.95,
        "unrelated prompts should have cosine similarity < 0.95, got {cos}"
    );
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_OLLAMA=1 + Ollama daemon"]
async fn embed_error_on_unreachable_base_url() {
    if !enabled() {
        return;
    }
    // Use a port nothing should be listening on.
    let client = OllamaClient::new("http://127.0.0.1:1");
    let result = client.embed("x").await;
    assert!(result.is_err(), "embed against dead URL must return Err");
}
