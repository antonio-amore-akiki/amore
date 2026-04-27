---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---

# Amore Dependency Impact Analysis

topic: dependency-impact
purpose: blast radius + alternatives + rollback test plan for 6 critical deps
stable: true
owner: Antonio Amore AKIKI

Cargo.lock source of truth. Advisory status from docs/RUSTSEC-TRIAGE-v0.5.0.md +
deny.toml (cargo-deny supply-chain rules). cargo-audit verdict to be appended
to docs/results.tsv at W8 closure.

---

## qdrant-client v1.18.0

**Source:** Cargo.lock line: `name = "qdrant-client" version = "1.18.0"`
**Advisory status:** no RUSTSEC advisories as of RUSTSEC-TRIAGE-v0.5.0.md; deny.toml enforces

**Role in Amore:** gRPC client for the embedded Qdrant vector store. Handles all vector
upsert, query, and collection management calls. Used by `crates/amore-core/src/qdrant_store.rs`.

**Blast radius if yanked:**
- `amore-core` fails to compile → `amore`, `amore-mcp`, `amore-gui` all fail
- Runtime: hybrid recall (vector + BM25) unavailable; system falls back to BM25-only if
  circuit breaker is open — but BM25-only is only a degraded mode, not full recall
- FEATURE_FLAG: `AMORE_FLAG_VECTOR_RECALL=off` would isolate the impact to vector lane only

**Named alternative:** `qdrant-client` is the official Qdrant Rust SDK; no drop-in replacement.
Forced alternative: implement raw gRPC calls via `tonic` directly against Qdrant proto
definitions (vendored from qdrant/qdrant proto repo). Estimated effort: 2–4 days.

**Rollback test plan:**
1. Pin to previous patch version in Cargo.toml (`qdrant-client = "=1.17.x"`).
2. Run `cargo build -p amore-core` — must compile clean.
3. Run `cargo test -p amore-core` — full test suite must pass.
4. Run `amore-eval` baseline recall against canary corpus; compare NDCG with current baseline.
5. If NDCG degrades > 2%: revert pin, file upstream issue.

---

## ollama-rs v0.3.4

**Source:** Cargo.lock: `name = "ollama-rs" version = "0.3.4"`
**Advisory status:** no RUSTSEC advisories; deny.toml enforces

**Role in Amore:** HTTP client for Ollama embedding and inference. Used in
`crates/amore-core/src/ollama.rs` to call `nomic-embed-text` for document ingestion
and query embedding.

**Blast radius if yanked:**
- `amore-core` fails to compile → full binary build fails
- Runtime: embedding unavailable → vector recall fails; BM25-only fallback activates
- `AMORE_FLAG_EMBED=off` would disable embed path and run BM25-only at startup

**Named alternative:** replace with direct `reqwest` HTTP calls to Ollama REST API
(`POST /api/embeddings`). The API surface is simple (model + prompt → float array).
Estimated effort: 1 day. Existing `reqwest` workspace dep means zero new dependencies.

**Rollback test plan:**
1. Vendor minimal Ollama HTTP client using existing `reqwest` dep (ADOPT path).
2. Run `OBELION_TEST_OLLAMA=1 cargo test -p obelion-core --test ollama_embed -- --ignored`.
3. All 4 integration tests must pass: 768-dim, determinism, distinctness, error-on-unreachable.
4. Record result in docs/results.tsv.

---

## ort v2.0.0-rc.12

**Source:** Cargo.lock: `name = "ort" version = "2.0.0-rc.12"`
**Advisory status:** RC status — no stable release yet; no RUSTSEC advisories; deny.toml tracks

**Role in Amore:** ONNX Runtime bindings for on-device cross-encoder reranking
(`bge-reranker-v2-m3.onnx`). Used by `crates/amore-core/src/reranker.rs` to score
candidate passages in the H3 reranking pipeline.

**Blast radius if yanked:**
- `amore-core` fails to compile → full binary build fails
- Runtime: cross-encoder reranking unavailable; system falls back to vector-similarity
  ranking only → NDCG degrades (quantified in docs/LONGMEMEVAL-CAPABILITY-REPORT-v0.5.0.md)
- `AMORE_FLAG_RERANKER=off` disables reranker lane; recall continues without cross-encoder

**Named alternative:** `tract` (sonos-ort/tract) — pure-Rust ONNX runtime, no C++ dep.
Trade-off: slower inference; model compatibility subset of ORT. Estimated effort: 3–5 days
(model format verification required). Alternative 2: candle (HuggingFace) — more active
but significantly larger API surface change.

**Rollback test plan:**
1. Set `AMORE_FLAG_RERANKER=off`.
2. Run full recall eval: `cargo run -p amore-eval -- --corpus canary`.
3. Capture NDCG@10; confirm degrades within documented degraded-mode range
   (docs/BENCHMARKS.md §reranker-ablation).
4. If ORT yanked entirely: evaluate tract ADOPT; run README baseline in fork before integration.

---

## rusqlite v0.39.0

**Source:** Cargo.lock: `name = "rusqlite" version = "0.39.0"`
**Advisory status:** no RUSTSEC advisories; deny.toml enforces; SQLite bundled version
audited via `rusqlite/bundled` feature in Cargo.toml

**Role in Amore:** SQLite bindings for provenance chain storage, conversation history,
and metadata indexing. Used by `crates/amore-core/src/sqlite_store.rs` (provenance) and
conversation storage layer.

**Blast radius if yanked:**
- `amore-core` fails to compile → full binary build fails
- Runtime: provenance chain verification fails on startup; conversation history unavailable
- No feature-flag bypass for SQLite (it is the provenance store); data on disk remains intact

**Named alternative:** `rusqlite` has no drop-in Rust replacement.
Forced alternatives:
- (a) `sqlx` with SQLite backend — async, different API surface; estimated 2–3 days
- (b) Direct SQLite C FFI via `libsqlite3-sys` — low-level; estimated 5+ days
Preference: (a) sqlx if forced; API migration is mechanical (same SQL, different binding layer).

**Rollback test plan:**
1. Pin to `rusqlite = "=0.38.x"` (previous minor).
2. `cargo test -p obelion-core` — provenance chain tests must pass (14/14 per results.tsv S1).
3. Verify `tampered payload via UPDATE breaks chain` test still passes (regression guard).
4. Run `amore doctor --json`; confirm `{"status":"ok"}`.

---

## sled v0.34.7

**Source:** Cargo.lock: `name = "sled" version = "0.34.7"`
**Advisory status:** sled is in maintenance mode (no new releases since 2022); no RUSTSEC
advisories but upstream activity is low. deny.toml has explicit allow for sled in deny.toml.

**Role in Amore:** embedded key-value store used as L2 cache layer for recall results
and intermediate embedding cache. Used by `crates/amore-core/src/sled_cache.rs`.

**Blast radius if yanked:**
- `amore-core` fails to compile → full binary build fails
- Runtime: L2 cache unavailable; recall falls back to L1 (in-memory) only — latency increases
  for cold queries beyond L1 capacity
- `AMORE_FLAG_L2_CACHE=off` disables sled layer; system runs L1-only

**Named alternative:** `redb` (Vincent Lorentz) — actively maintained embedded key-value store,
pure Rust, ACID-compliant. API is similar but not drop-in (different transaction model).
Estimated migration: 2–3 days. `redb` is the primary ADOPT candidate for sled replacement.

**Rollback test plan:**
1. Set `AMORE_FLAG_L2_CACHE=off`.
2. Run `cargo test -p amore-core` — cache tests should pass with L1-only path.
3. Run recall latency benchmark: confirm p99 within degraded-mode SLO (docs/SLO.md).
4. If sled yanked: evaluate redb ADOPT; run redb README baseline before integration.

---

## tantivy v0.22.1

**Source:** Cargo.lock: `name = "tantivy" version = "0.22.1"`
**Advisory status:** no RUSTSEC advisories; deny.toml enforces; actively maintained (quickwit-oss)

**Role in Amore:** full-text BM25 search engine for the keyword recall lane. Used by
`crates/amore-core/src/bm25_store.rs`. Forms the fallback lane when Qdrant is unavailable
(circuit breaker open) and is the primary lane in degraded mode.

**Blast radius if yanked:**
- `amore-core` fails to compile → full binary build fails
- Runtime: BM25 recall lane unavailable; only vector recall remains — degraded mode fails
  completely (no fallback lane). This is the highest-impact single-dep failure scenario.
- No feature flag isolates tantivy (it is the fallback lane itself)

**Named alternative:**
- (a) `meilisearch` embedded — actively maintained, higher abstraction; estimated 5–7 days
- (b) `sonic` — simpler, fewer features; estimated 3–4 days
- (c) reimplement BM25 scoring over the SQLite FTS5 extension (already present via rusqlite)
  — no new dep; estimated 2–3 days; lower fidelity than tantivy but zero new dep risk.
Preference: (c) SQLite FTS5 ADAPT path if tantivy is forced out — minimum dependency surface.

**Rollback test plan:**
1. Pin to `tantivy = "=0.21.x"` (previous minor).
2. `cargo test -p amore-core` — BM25 tests must pass.
3. Run `amore recall "test query"` against canary corpus; verify non-empty results.
4. Confirm circuit-breaker-open path (disable Qdrant, fire recall) still returns BM25 results.
5. If tantivy yanked: evaluate SQLite FTS5 ADAPT; run BM25 parity test before swap.

---

## cargo-audit status

Current RUSTSEC triage: docs/RUSTSEC-TRIAGE-v0.5.0.md (v0.5.0 baseline).
W8 closure action: run `cargo audit` against current Cargo.lock; append verdict row to
docs/results.tsv as `W8\tcargo-audit\t<verdict>\t<advisory-count>`.

---

Source: Cargo.lock (deps + versions); docs/RUSTSEC-TRIAGE-v0.5.0.md; deny.toml
