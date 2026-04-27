// crates/amore-eval/src/bin/seed_load_test_corpus.rs — H.10 corpus seeder.
//
// Seeds N synthetic observations into Qdrant (vector lane) and SQLite (metadata lane)
// to produce a realistic 10M-entry corpus for the H.10 sustained load test.
//
// Embedding: deterministic std-LCG (no `rand` dep) seeded by obs index; 768-dim f32;
// unit-normalized so cosine distance is well-defined.
//
// Usage:
//   cargo run --release -p amore-eval --bin seed_load_test_corpus -- \
//     --count 10000000 --endpoint http://127.0.0.1:6333 --batch-size 256

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

use anyhow::{Context, Result};
use clap::Parser;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, ScalarQuantizationBuilder, VectorParamsBuilder,
};
use qdrant_client::qdrant::{PointId, PointStruct, UpsertPointsBuilder, VectorsConfig};
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::PathBuf;

// 50 sentence templates for synthetic body text
const TEMPLATES: &[&str] = &[
    "This entry covers memory management in Rust async runtimes.",
    "Performance profiling reveals bottlenecks in the I/O layer.",
    "The architecture review identified coupling in the service boundary.",
    "Database migration succeeded without downtime using shadow writes.",
    "Error handling paths have been hardened against partial failures.",
    "The CI pipeline now includes property-based tests for parsers.",
    "API versioning strategy aligns with semantic-versioning principles.",
    "Observability spans expose latency distribution across system tiers.",
    "The BM25 recall lane achieves sub-100ms at 100K corpus scale.",
    "Vector quantization reduces memory by 4x with acceptable recall loss.",
    "Cluster replication factor 2 tolerates single-node failure cleanly.",
    "Cold-start latency meets the 500ms SLO on standard dev hardware.",
    "Snapshot restore validates byte-identical recovery from backup.",
    "Integration tests cover all 7 IDE adapter initialisation paths.",
    "Security review found no critical findings in the MCP surface.",
    "The token-reduction harness averages 96.6% across 10 fixtures.",
    "Graceful degradation returns BM25 hits when Qdrant is unreachable.",
    "WAL mode prevents chain-fork under concurrent multi-writer load.",
    "The installer packages Qdrant and bge-small.onnx in one 8 MB exe.",
    "Cross-encoder reranking adds 50ms p95 with measurable quality lift.",
    "Rate limiting protects the embed endpoint under burst traffic.",
    "Tantivy shard fan-out latency is 100ms p95 across 16 shards.",
    "Provenance chain sha-256 detects tampered payloads reliably.",
    "The ensemble orchestrator achieves consensus on 80% of test prompts.",
    "Adversarial test mining extracts failure cases from session ledgers.",
    "The doctor subcommand reports all health checks in machine-readable JSON.",
    "Binary size is 9.84 MB for amore-mcp after release optimisations.",
    "Sigstore keyless signing attests the Linux release artifact cleanly.",
    "The npm postinstall wrapper supports private-repo GH token auth.",
    "World model Bayesian updates track tool reliability over sessions.",
    "Rust edition 2024 async-fn-in-traits simplifies the LLM client trait.",
    "gRPC transport reduces serialisation overhead vs REST for bulk ops.",
    "The backup harness verifies cosign verify-blob in a clean container.",
    "Lazy embedder timeout flips the ollama_unavailable degraded flag.",
    "Property tests validate RRF fusion is monotone, bounded, deterministic.",
    "The canonical-docs router achieves 91.9% token reduction on research queries.",
    "Multi-writer SQLite WAL test catches the chain-fork concurrency bug.",
    "Release workflow splits bundle upload from archive for cross-OS compat.",
    "Inno Setup compiles a 7.6 MB installer with silent Ollama install.",
    "The amore-gui eframe window opens within 1.5s cold-start budget.",
    "HybridRecall indexes 5-doc corpus and returns top hit by cosine score.",
    "BM25 FTS5 baseline characterization locks query score ordering.",
    "The MCP handshake test asserts both recall and canonical_doc_lookup tools.",
    "IDE adapter dry-run preserves all existing mcpServers keys atomically.",
    "The perf-baseline.tsv appends one row per metric on each release tag.",
    "Stack backtrace on SIGSEGV is captured via signal handler for debugging.",
    "Tantivy ADR 0011 documents the migration path from SQLite FTS5.",
    "Cluster opt-in ADR 0007 defines the single-node to cluster upgrade path.",
    "The threat model covers stolen-laptop as the primary attack vector.",
    "Amore targets 99.99% availability with RF=2 and 3-node cluster topology.",
];

const COLLECTION_NAME: &str = "amore-h10-load-corpus";
const VECTOR_DIM: usize = 768;

#[derive(Parser)]
#[clap(
    name = "seed_load_test_corpus",
    about = "H.10 corpus seeder: populates Qdrant + SQLite with N synthetic observations for the 10M load test."
)]
struct Args {
    /// Number of observations to seed.
    #[clap(long)]
    count: u64,

    /// Qdrant HTTP/gRPC endpoint.
    #[clap(long, default_value = "http://127.0.0.1:6333")]
    endpoint: String,

    /// Upsert batch size.
    #[clap(long, default_value = "256")]
    batch_size: usize,

    /// SQLite metadata file path (default: system temp dir).
    #[clap(long)]
    sqlite_path: Option<PathBuf>,
}

/// LCG random number generator seeded by observation index.
/// Parameters: Knuth multiplicative; full-period for u64.
/// No `rand` crate needed — avoids a new workspace dependency.
fn lcg_u64(seed: u64) -> u64 {
    seed.wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

/// Generate a deterministic 768-dim unit-normalised f32 embedding from obs index.
fn synthetic_embedding(idx: u64) -> Vec<f32> {
    let mut state = idx.wrapping_add(1); // avoid zero seed
    let mut raw = Vec::with_capacity(VECTOR_DIM);
    for _ in 0..VECTOR_DIM {
        state = lcg_u64(state);
        // Map u64 to [-1, 1]
        let val = (state as f64 / u64::MAX as f64) * 2.0 - 1.0;
        raw.push(val as f32);
    }
    // Unit-normalise so cosine distance is meaningful
    let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-9 {
        raw.iter().map(|x| x / norm).collect()
    } else {
        raw
    }
}

/// Build a synthetic body text for observation idx.
fn synthetic_body(idx: u64) -> String {
    let template = TEMPLATES[(idx as usize) % TEMPLATES.len()];
    format!("Observation #{idx} — {template}")
}

fn sqlite_path_default() -> PathBuf {
    std::env::temp_dir().join("amore-h10-seed-meta.db")
}

fn init_sqlite(path: &std::path::Path) -> Result<Connection> {
    let conn =
        Connection::open(path).with_context(|| format!("open SQLite at {}", path.display()))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         CREATE TABLE IF NOT EXISTS observations (
             id INTEGER PRIMARY KEY,
             body TEXT NOT NULL,
             created_at TEXT NOT NULL
         );",
    )
    .context("init SQLite schema")?;
    Ok(conn)
}

async fn ensure_qdrant_collection(client: &Qdrant) -> Result<()> {
    // Delete if exists so re-runs start clean
    let collections = client
        .list_collections()
        .await
        .context("list Qdrant collections")?;
    let exists = collections
        .collections
        .iter()
        .any(|c| c.name == COLLECTION_NAME);
    if exists {
        client
            .delete_collection(COLLECTION_NAME)
            .await
            .context("delete existing h10 collection")?;
        eprintln!("[seed] deleted existing collection '{COLLECTION_NAME}'");
    }

    client
        .create_collection(
            CreateCollectionBuilder::new(COLLECTION_NAME)
                .vectors_config(VectorsConfig::from(VectorParamsBuilder::new(
                    VECTOR_DIM as u64,
                    Distance::Cosine,
                )))
                .quantization_config(ScalarQuantizationBuilder::default()),
        )
        .await
        .context("create Qdrant collection")?;
    eprintln!(
        "[seed] created collection '{COLLECTION_NAME}' (dim={VECTOR_DIM}, Cosine, int8 quant)"
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.count == 0 {
        anyhow::bail!("--count must be > 0");
    }
    if args.batch_size == 0 {
        anyhow::bail!("--batch-size must be > 0");
    }

    let sqlite_path = args.sqlite_path.clone().unwrap_or_else(sqlite_path_default);
    eprintln!("[seed] SQLite metadata -> {}", sqlite_path.display());
    eprintln!("[seed] Qdrant endpoint -> {}", args.endpoint);
    eprintln!("[seed] count={} batch_size={}", args.count, args.batch_size);

    let client = Qdrant::from_url(&args.endpoint)
        .build()
        .context("build Qdrant client")?;

    ensure_qdrant_collection(&client).await?;

    let mut conn = init_sqlite(&sqlite_path)?;

    let total = args.count;
    let batch_size = args.batch_size as u64;
    let mut idx: u64 = 0;

    while idx < total {
        let batch_end = (idx + batch_size).min(total);
        let batch_len = (batch_end - idx) as usize;

        // Build Qdrant points batch
        let mut points: Vec<PointStruct> = Vec::with_capacity(batch_len);
        for i in idx..batch_end {
            let vec = synthetic_embedding(i);
            let body = synthetic_body(i);
            let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();
            payload.insert(
                "body".to_string(),
                qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::StringValue(body)),
                },
            );
            payload.insert(
                "idx".to_string(),
                qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(i as i64)),
                },
            );
            #[allow(deprecated)] // qdrant-client Vector.data field deprecated; load-test seed only
            points.push(PointStruct {
                id: Some(PointId {
                    point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(i)),
                }),
                payload,
                vectors: Some(qdrant_client::qdrant::Vectors {
                    vectors_options: Some(qdrant_client::qdrant::vectors::VectorsOptions::Vector(
                        qdrant_client::qdrant::Vector {
                            data: vec,
                            ..Default::default()
                        },
                    )),
                }),
            });
        }

        // Upsert to Qdrant
        client
            .upsert_points(UpsertPointsBuilder::new(COLLECTION_NAME, points).wait(false))
            .await
            .with_context(|| format!("upsert Qdrant batch idx={idx}..{batch_end}"))?;

        // Insert metadata to SQLite
        {
            let tx = conn.transaction().context("begin SQLite transaction")?;
            for i in idx..batch_end {
                let body = synthetic_body(i);
                tx.execute(
                    "INSERT OR REPLACE INTO observations (id, body, created_at) VALUES (?1, ?2, datetime('now'))",
                    params![i as i64, body],
                )
                .with_context(|| format!("sqlite insert idx={i}"))?;
            }
            tx.commit().context("commit SQLite transaction")?;
        }

        idx = batch_end;

        // Progress every 10k observations
        if idx.is_multiple_of(10_000) || idx == total {
            eprintln!(
                "[seed] progress: {idx}/{total} ({:.1}%)",
                idx as f64 / total as f64 * 100.0
            );
        }
    }

    eprintln!(
        "[seed] done: {total} observations seeded to Qdrant collection '{COLLECTION_NAME}' + SQLite {}",
        sqlite_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcg_is_deterministic() {
        assert_eq!(lcg_u64(1), lcg_u64(1));
        assert_ne!(lcg_u64(1), lcg_u64(2));
    }

    #[test]
    fn embedding_is_unit_normalised() {
        let v = synthetic_embedding(0);
        assert_eq!(v.len(), VECTOR_DIM);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm={norm}");
    }

    #[test]
    fn embedding_distinct_for_different_indices() {
        let v0 = synthetic_embedding(0);
        let v1 = synthetic_embedding(1);
        let dot: f32 = v0.iter().zip(v1.iter()).map(|(a, b)| a * b).sum();
        // Two distinct random unit vectors should not be identical
        assert!(dot.abs() < 0.99, "vectors too similar: dot={dot}");
    }

    #[test]
    fn body_contains_index() {
        let body = synthetic_body(42);
        assert!(body.contains("42"), "body missing index: {body}");
    }

    #[test]
    fn body_uses_template_rotation() {
        // Template cycles through 50 entries
        let b0 = synthetic_body(0);
        let b50 = synthetic_body(50);
        // Both hit the same template (offset 0 mod 50)
        assert_eq!(
            b0.split(" — ").nth(1),
            b50.split(" — ").nth(1),
            "template rotation broken"
        );
    }
}
