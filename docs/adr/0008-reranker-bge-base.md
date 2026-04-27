# 8. Use BAAI/bge-reranker-base for cross-encoder reranking

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore's retrieval pipeline returns a fused candidate set (BM25 + vector
RRF, top-50). A cross-encoder reranker re-scores the candidates against
the original query and returns the final top-k. The reranker is the
highest-accuracy stage of the pipeline and dominates recall quality.

Which cross-encoder model should Amore ship for the reranking stage in
v0.7.0 (Phase H)?

## Decision Drivers

* MTEB Reranking leaderboard top-10 rank
* ONNX-quantized binary size ≤ 30 MB (installer size budget constraint)
* Permissive licence (MIT or Apache-2.0; commercial use permitted)
* Production adoption as a credibility signal (Datadog / Vespa / etc.)
* Inference via `ort` (ONNX Runtime Rust bindings) — no Python runtime
* int8 quantization acceptable if NDCG degradation < 1 point vs fp32

## Considered Options

* BAAI/bge-reranker-base (MTEB top-10; ONNX int8 ~28 MB; MIT licence)
* BAAI/bge-reranker-large (higher accuracy; ONNX int8 ~130 MB)
* Cohere Rerank v3 (cloud API; paid per call)
* ColBERT v2 (late-interaction; complex batching)

## Decision Outcome

Chosen option: **BAAI/bge-reranker-base, int8-quantized ONNX (~28 MB),
downloaded on first run**.

The model is NOT bundled in the installer. On first `amore recall` that
triggers the reranking stage, Amore downloads
`BAAI/bge-reranker-base/onnx/model_quantized.bin` from HuggingFace Hub
to `~/.amore/models/bge-reranker-base/`. Subsequent runs load from
disk. SHA-256 checksum is pinned in `models/manifest.lock`.

Runtime path via `ort`:

```rust
// crates/amore-rerank/src/lib.rs (v0.7.0)
let session = Session::builder()?
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .with_intra_threads(4)?
    .commit_from_file(&model_path)?;
```

The reranking stage is scheduled for v0.7.0 (Phase H). In v0.1-0.6,
the pipeline returns the RRF-fused candidate set without reranking.

### Consequences

* Good: 28 MB model keeps the installer under the 400 MB target
* Good: MTEB top-10 means quality bar is independently verified
* Good: MIT licence permits commercial redistribution without royalties
* Good: Datadog and Vespa production adoption provides a credible
  reference outside Amore
* Bad: first-run requires a one-time ~28 MB download; non-technical user
  needs a plain-English progress indicator
* Bad: int8 quantization degrades NDCG@10 by ~0.3 points vs fp32 on
  BEIR benchmarks — acceptable under the size cap
* Bad: bge-reranker-base lags bge-reranker-large by ~2 NDCG points;
  power users can override via `AMORE_RERANKER_MODEL`

## Pros and Cons of the Options

### bge-reranker-base int8 ONNX (CHOSEN)

* Good: 28 MB fits under the 30 MB cap
* Good: MTEB top-10 reranker as of 2026-05
* Good: MIT licence; Datadog/Vespa production reference
* Good: pure ONNX inference via `ort`; no Python, no separate process
* Bad: ~0.3 NDCG@10 degradation vs fp32 due to int8 quantization

### bge-reranker-large int8 ONNX

* Good: ~2 NDCG points higher than base on BEIR
* Bad: ~130 MB — exceeds the 30 MB model size budget
* Bad: 4× slower inference on CPU (cross-encoder scales with model size)

### Cohere Rerank v3

* Good: state-of-the-art quality; no local model download
* Bad: paid API; violates the "no paid cloud lock-in" mandate
* Bad: network round-trip adds latency and requires internet access
* Bad: privacy risk: query + candidate text leaves the user's machine

### ColBERT v2

* Good: late-interaction allows pre-indexing of document representations
* Bad: indexing pipeline complexity is significantly higher than a
  cross-encoder score call
* Bad: no production-tested ONNX path in the Rust ecosystem as of 2026
* Bad: batching logic differs substantially from the bi-encoder +
  cross-encoder pattern already designed for the pipeline

## More Information

* MTEB Reranking leaderboard: https://huggingface.co/spaces/mteb/leaderboard
* HuggingFace model card: https://huggingface.co/BAAI/bge-reranker-base
* `ort` crate: https://crates.io/crates/ort (ONNX Runtime Rust bindings)
* Model manifest: `models/manifest.lock` (SHA-256 pinned)
* Power-user override: set `AMORE_RERANKER_MODEL=BAAI/bge-reranker-large`
  to trade installer-size for quality (requires manual model download)
* Scheduled for v0.7.0 (Phase H) alongside Tantivy BM25 lane (ADR-0006)
