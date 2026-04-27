# 6. Migrate BM25 lane from SQLite FTS5 to Tantivy at 100M-corpus scale

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore's recall pipeline fuses BM25 (keyword) + vector (semantic) results
via Reciprocal Rank Fusion. In v0.1-0.6 the BM25 lane is implemented
with SQLite FTS5, which is embedded in Rust via `rusqlite` and requires
zero external dependencies.

As the corpus grows toward 100M documents (Phase H target), SQLite FTS5
hits a practical single-table ceiling around 10M rows: vacuum becomes
multi-minute, the single-WAL checkpoint blocks BM25 queries during
ingestion, and per-shard partitioning is not natively supported.

Which full-text search engine should drive the BM25 lane at v0.7.0?

## Decision Drivers

* 100M-document corpus target (Phase H, v0.7.0)
* Shardable by doc-hash range (must scale horizontally)
* Rust-native: no JVM, no Python runtime, no CGO
* BM25 scoring must match or exceed SQLite FTS5 quality
* Permissive licence (Apache-2.0 preferred)
* Production-tested at billion-scale by a credible reference deployment

## Considered Options

* Keep SQLite FTS5 (rusqlite + FTS5 virtual table)
* Tantivy (Rust-native full-text search library)
* Bleve (Go; CGO bridge required from Rust)
* Lucene via JNI (Java; JVM required at runtime)

## Decision Outcome

Chosen option: **Tantivy**.

Migration is **scheduled for v0.7.0 (Phase H)**. Until then, SQLite FTS5
remains the production BM25 lane.

The Tantivy index will be sharded by doc-hash modulo N (configurable;
default 8 shards for single-node, 96 shards for cluster mode). Each
shard is an independent Tantivy `Index` on disk, searched in parallel
via Rayon. Results are merged by BM25 score before RRF fusion.

Schema:

```rust
// crates/amore-fts/src/schema.rs (v0.7.0)
let schema = SchemaBuilder::new()
    .add_text_field("body", TEXT | STORED)
    .add_u64_field("doc_id", INDEXED | STORED | FAST)
    .add_u64_field("shard", INDEXED | STORED | FAST)
    .build();
```

Migration path: `amore migrate fts5-to-tantivy` reads all SQLite FTS5
rows in batches of 10k and writes to Tantivy shards. Estimated duration
for 10M rows: ~4 minutes on NVMe.

### Consequences

* Good: Quickwit (Tantivy-based) is production-proven at billion-scale;
  same underlying library
* Good: Rust-native — no FFI, no external process, no JVM
* Good: horizontal sharding by doc-hash range removes the single-table
  ceiling entirely
* Good: Apache-2.0 licence; already used by Quickwit, Meilisearch
* Bad: migration step required for existing installations
* Bad: Tantivy on-disk format is not SQL-queryable (admin tooling must
  use the Tantivy API)
* Bad: adds ~4 MB to the compiled binary vs SQLite-only

## Pros and Cons of the Options

### SQLite FTS5 (current, to be retired at v0.7.0)

* Good: zero extra dependency, embedded in rusqlite
* Good: admin can inspect the index with any SQLite browser
* Bad: single-table ceiling ~10M rows before vacuum/checkpoint pain
* Bad: no native sharding; horizontal scale requires external routing
* Bad: BM25 implementation is simpler than Tantivy's (no field boosts,
  no custom tokenizers)

### Tantivy (CHOSEN for v0.7.0)

* Good: Rust-native, no FFI overhead
* Good: shardable; Rayon parallel search across shards
* Good: BM25+ with configurable field boosts; custom tokenizer support
* Good: Quickwit billion-scale production reference
* Good: Apache-2.0 licence
* Bad: migration step for existing installs
* Bad: on-disk format opaque to SQL tools

### Bleve

* Good: capable full-text search
* Bad: Go library; requires CGO bridge from Rust (added complexity, cross-
  compile pain on Windows)
* Bad: smaller production reference set than Tantivy

### Lucene via JNI

* Good: gold standard for full-text search
* Bad: JVM required at runtime; violates the no-runtime-install mandate
* Bad: JNI from Rust is fragile, especially on Windows/macOS arm64
* Bad: fails the single-binary mandate

## More Information

* Tantivy crate: `tantivy = "0.22"` (target for v0.7.0; pin will be
  confirmed in Phase H planning)
* Quickwit production reference: https://quickwit.io/docs/get-started/
  (billion-scale Tantivy user)
* Migration script: `scripts/migrate-fts5-to-tantivy.sh` (Phase H)
* SQLite FTS5 will be kept as a dependency for metadata queries that are
  not in the BM25 hot path; it is NOT removed entirely in v0.7.0
