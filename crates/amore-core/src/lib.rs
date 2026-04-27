// amore-core
//
// Hybrid retrieval engine: BM25 (SQLite FTS5) + vector (Qdrant) + graph (SQLite).
// Plus: canonical-docs router, multi-agent ensemble orchestrator, EIG question selection,
// world-model namespace, adversarial-test mining, cryptographic provenance.
//
// H.4 + H.5: qdrant_pool + circuit_breaker modules added.
// H.8: wal + streaming_ingest added (sled-backed WAL, kill-mid-ingest zero loss).
// H.3: reranker added (BAAI/bge-reranker-base cross-encoder via ort + tokenizers).

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod cache;
pub mod circuit_breaker;
pub mod compaction;
#[cfg(feature = "tantivy-bm25")]
pub(crate) mod porter1;
pub(crate) mod sqlite_compaction;
pub mod docs;
pub mod ensemble;
pub mod flags;
pub mod ide_adapter;
pub mod mining;
pub mod ollama;
pub mod provenance;
pub mod qdrant_pool;
pub mod qdrant_store;
pub mod recall;
#[cfg(feature = "rerank-onnx")]
pub mod reranker;
pub mod sqlite_store;
pub mod streaming_ingest;
#[cfg(feature = "tantivy-bm25")]
pub mod tantivy_index;
pub mod timeout;
pub mod wal;
pub mod world_model;

pub use recall::HybridRecall;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");