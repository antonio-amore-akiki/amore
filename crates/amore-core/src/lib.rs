// amore-core
//
// Hybrid retrieval engine: BM25 (SQLite FTS5) + vector (Qdrant) + graph (SQLite).
// Plus: canonical-docs router, multi-agent ensemble orchestrator, EIG question selection,
// world-model namespace, adversarial-test mining, cryptographic provenance.
//
// Status: skeleton, v0.1.0-pre-alpha.

// ADR 0010: no-unwrap policy enforced via clippy lints in production paths.
// Test modules are exempted via cfg_attr so the harness can use .unwrap() freely.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

pub mod docs;
pub mod ensemble;
pub mod ide_adapter;
pub mod mining;
pub mod ollama;
pub mod provenance;
pub mod qdrant_store;
pub mod recall;
pub mod sqlite_store;
pub mod timeout;
pub mod world_model;

pub use recall::HybridRecall;

/// Library version (compile-time)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
