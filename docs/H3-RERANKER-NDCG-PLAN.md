---
stable: true
---
# H.3 — Reranker nDCG@10 Comparison Plan

## Purpose

Measure whether the BAAI/bge-reranker-base cross-encoder reranker improves
retrieval quality over the RRF-only baseline by ≥5% nDCG@10 on a frozen
100-query golden set sourced from the real Amore corpus.

## Golden set

100 queries drawn from two sources:

- **Base** (50 queries): real queries recorded in `crates/amore-core/tests/fixtures/bm25_baseline.json`
  (the Q field of each fixture entry). These represent the existing
  BM25 regression set and cover diverse topic areas already in the corpus.

- **Expanded** (50 queries): curated set covering additional retrieval
  scenarios — navigational queries, multi-hop reasoning queries, paraphrase
  pairs, and adversarial edge cases (stop-word-heavy, rare terms).

Queries frozen at evaluation time; never modified after ground truth is labelled.

## Ground truth

For each query in the golden set, label the top-10 documents returned by the
RRF recall stage as relevant (1) or irrelevant (0). Labelling approach:

1. Run `amore recall --query "<q>" --limit 20` to collect candidate pool.
2. Human-label top-10 per query (binary relevance). Label once; freeze labels.
3. Store labels in `crates/amore-core/tests/fixtures/ndcg_golden.json` with schema:
   ```json
   { "query": "rust async runtime", "relevant_ids": ["doc_id_1", "doc_id_3"] }
   ```

## Procedure

```bash
# Step 1: collect RRF-only results
cargo run -p amore-eval --bin ndcg_compare -- \
    --mode rrf_only \
    --golden crates/amore-core/tests/fixtures/ndcg_golden.json \
    --output docs/rrf_ndcg_results.json

# Step 2: collect reranker results
cargo run -p amore-eval --bin ndcg_compare -- \
    --mode reranker \
    --model-path "$LOCALAPPDATA/Amore/models/bge-reranker-base.onnx" \
    --tokenizer-path "$LOCALAPPDATA/Amore/models/tokenizer.json" \
    --golden crates/amore-core/tests/fixtures/ndcg_golden.json \
    --output docs/reranker_ndcg_results.json

# Step 3: compare
cargo run -p amore-eval --bin ndcg_compare -- \
    --mode compare \
    --rrf docs/rrf_ndcg_results.json \
    --reranker docs/reranker_ndcg_results.json
```

## Tools

`crates/amore-eval/src/bin/ndcg_compare.rs` (deferred — Wave 3/J).

nDCG@10 formula (standard):

```
nDCG@k = DCG@k / IDCG@k
DCG@k  = sum_{i=1}^{k} rel_i / log2(i + 1)
IDCG@k = DCG@k for ideal ranking (all relevant docs first)
```

## Pass gate

**Reranker nDCG@10 ≥ RRF nDCG@10 + 0.05** on the full 100-query golden set.

Result appended as a row in `docs/results.tsv` with:
- `step=H.3-ndcg`, `metric=ndcg10_delta`, `value=<delta>`

The test stub is in `crates/amore-core/tests/reranker_parity.rs` as
`t4_ndcg10_reranker_beats_rrf_baseline` (`#[ignore]`).
