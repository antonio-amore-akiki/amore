// amore-core
//
// Hybrid retrieval engine: BM25 (SQLite FTS5) + vector (Qdrant) + graph (SQLite).
// Plus: canonical-docs router, multi-agent ensemble orchestrator, EIG question selection,
// world-model namespace, adversarial-test mining, cryptographic provenance.
//
// H.4 + H.5: qdrant_pool + circuit_breaker modules added.
// H.8: wal + streaming_ingest added (sled-backed WAL, kill-mid-ingest zero loss).

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod circuit_breaker;
pub mod docs;
pub mod ensemble;
pub mod ide_adapter;
pub mod mining;
pub mod ollama;
pub mod provenance;
pub mod qdrant_pool;
pub mod qdrant_store;
pub mod recall;
pub mod sqlite_store;
pub mod streaming_ingest;
pub mod tantivy_index;
pub mod timeout;
pub mod wal;
pub mod world_model;

pub use recall::HybridRecall;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");