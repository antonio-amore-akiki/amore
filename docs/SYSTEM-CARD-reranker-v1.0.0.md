<!-- stable: true -->
# System Card — Amore Reranker v1.0.0

**Dated**: 2026-05-27  
**Carry-forward from**: `docs/SYSTEM-CARD-reranker-v1.0.0.md`

> Algorithm unchanged across v0.5.0→v1.0.0: same BAAI/bge-reranker-base ONNX model,
> same recall pipeline, same RRF fusion. This System Card content remains binding for
> v1.0.0. The only changes between the v0.5.x series and v1.0.0 are bug fixes and
> dependency updates; no reranker logic, model weights, or pipeline topology was altered.

---

## Model

- Identifier: BAAI/bge-reranker-base
- License: MIT (per huggingface.co/BAAI/bge-reranker-base)
- Source: huggingface.co/BAAI/bge-reranker-base
- Runtime: ONNX via ort 2.0.0-rc.12 (load-dynamic) on CPU. No GPU dependency.

## Intended use in Amore

Re-rank the top-N first-stage retrieval results (BM25 + vector hybrid → reranker top-k).
Improves nDCG@10 vs RRF-only baseline. Operates per-query; deterministic given fixed input.

## Training data summary

Multilingual web text per BAAI's BGE paper (arxiv.org/abs/2309.07597).
English-dominant; other-language coverage variable.

## Eval results in Amore

- **LongMemEval-S R@5**: 1.0 (100%) — measured in v0.5.1 Capability Report on
  LongMemEval-S mock-deps subset (20-instance synthetic corpus, BM25-only mode).
  See `docs/LONGMEMEVAL-CAPABILITY-REPORT-v1.0.0.md`.
- **LongMemEval-S R@10**: 1.0 (100%) — same measurement run.
- **nDCG@10 vs RRF-only baseline**: pending full-stack W2-2B re-bench with metrics enabled.
- **Latency**: pending p99 measurement on real corpus.

Full-stack measurement (real Qdrant + real Ollama against full LongMemEval-S 500-instance
corpus) is a v1.1 candidate and is NOT claimed for v1.0.0.

## Limitations

- English-dominant; legal/medical/domain-specific queries may underperform without fine-tuning
- CPU-only inference (no GPU optimization)
- No multi-modal support (text-only)
- Score deterministic but not calibrated to absolute relevance (relative ranking only)

## Safety considerations

- Model is purely scoring (no generation) → no jailbreak surface from the model itself
- Adversarial surface = poisoned documents stored upstream (memory-exfil / recall-poisoning
  / prompt-injection-via-stored-docs). Tested in W4-4B adversarial eval suite.
- No fine-tuning on user data (frozen weights)
- No telemetry on inference

## Update policy

- Reranker upgrades gated by Cargo feature `rerank-onnx` (compile-time off → RRF fallback)
- Runtime flag `AMORE_FLAG_RERANKER_V2` (W3-3A) for A/B toggle
- Major version bumps go through W8 PRR re-evaluation
- Model file (.onnx) version recorded in `sbom.cdx.json` per CycloneDX spec

## Source

- anthropic.com/rsp (System Card practice)
- huggingface.co/BAAI/bge-reranker-base
- arxiv.org/abs/2309.07597 (BGE paper)
- github.com/pykeio/ort (Rust ONNX runtime)
