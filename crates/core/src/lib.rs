// obelion-core
//
// Hybrid retrieval engine: BM25 (SQLite FTS5) + vector (Qdrant) + graph (SQLite).
// Plus: canonical-docs router, multi-agent ensemble orchestrator, EIG question selection,
// world-model namespace, adversarial-test mining, cryptographic provenance.
//
// Status: skeleton, v0.1.0-pre-alpha.

pub mod recall;
pub mod docs;
pub mod ensemble;
pub mod world_model;
pub mod provenance;
pub mod ollama;
pub mod qdrant_store;
pub mod sqlite_store;

pub use recall::HybridRecall;

/// Library version (compile-time)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
