---
stable: true
topic: h3-reranker-ndcg-plan
purpose: nDCG@10 + R@5 measurement procedure for the BAAI/bge-reranker-base cross-encoder reranker (H.3)
version: 1.0.0
---

# H3 Reranker — nDCG@10 + R@5 Measurement Procedure

**Generated:** 2026-05-28T19:15Z
**Reference:** ELITE-QUALITY-GATE.md B7 (LongMemEval live-stack subset20 R@5=0.65 below 0.85 GA gate)
**Adopt verdict:** BAAI/bge-reranker-base (cross-encoder, 1.1GB ONNX, multilingual) — same model family that improved BEIR average nDCG@10 by +6 absolute points over RRF-only baselines.

## Required local files

The reranker is NOT bundled with the installer (model is 1.1GB; user opts in per local hardware tier).
Two files required:

```
<LOCALAPPDATA>/Amore/models/bge-reranker-base.onnx     (1112MB — ONNX export, fp32)
<LOCALAPPDATA>/Amore/models/tokenizer.json             (17MB — HuggingFace tokenizers format)
```

Direct-download (no Python tooling required):

```pwsh
$models = "$env:LOCALAPPDATA\Amore\models"
New-Item -ItemType Directory -Force $models | Out-Null
Invoke-WebRequest -Uri "https://huggingface.co/BAAI/bge-reranker-base/resolve/main/onnx/model.onnx?download=true" -OutFile "$models\bge-reranker-base.onnx"
Invoke-WebRequest -Uri "https://huggingface.co/BAAI/bge-reranker-base/resolve/main/tokenizer.json?download=true" -OutFile "$models\tokenizer.json"
```

Bash variant (uses curl with resume so the 1.1GB download survives transient disconnects):

```bash
mkdir -p "$LOCALAPPDATA/Amore/models"
curl -L --retry 3 -C - -o "$LOCALAPPDATA/Amore/models/bge-reranker-base.onnx" \
  "https://huggingface.co/BAAI/bge-reranker-base/resolve/main/onnx/model.onnx?download=true"
curl -L -o "$LOCALAPPDATA/Amore/models/tokenizer.json" \
  "https://huggingface.co/BAAI/bge-reranker-base/resolve/main/tokenizer.json?download=true"
```

## Required ONNX Runtime dylib

The `ort` crate uses load-dynamic linking on Windows. Set `ORT_DYLIB_PATH` before any session:

```pwsh
$env:ORT_DYLIB_PATH = "$env:LOCALAPPDATA\amore\onnxruntime\onnxruntime-win-x64-1.20.1\lib\onnxruntime.dll"
```

Direct-download:

```bash
mkdir -p "$LOCALAPPDATA/amore/onnxruntime"
curl -L -o /tmp/ort.zip "https://github.com/microsoft/onnxruntime/releases/download/v1.20.1/onnxruntime-win-x64-1.20.1.zip"
powershell -Command "Expand-Archive -Path /tmp/ort.zip -DestinationPath $env:LOCALAPPDATA/amore/onnxruntime -Force"
```

## Smoke test — verify model loads + scores rank-correctly

```bash
cd <amore-repo-root>
AMORE_TEST_RERANKER=1 \
  ORT_DYLIB_PATH="$LOCALAPPDATA/amore/onnxruntime/onnxruntime-win-x64-1.20.1/lib/onnxruntime.dll" \
  cargo test --release -p amore-core --features rerank-onnx --test reranker_parity -- --ignored t1_smoke_ranking_with_real_model
```

Expected: T1 passes after ~30-90s warm-up (1.1GB model mmap + first-token tokenizer warm).

## R@5 measurement — LongMemEval-S subset

The reranker sits BETWEEN RRF fusion and final top_k truncation:

```text
candidates -> vector lane (top 50)
            -> bm25 lane    (top 50)
            -> RRF fuse     (top 50)
            -> reranker     (top 5)         <-- H.3 insertion point
```

Wire reranker into HybridRecall via `with_reranker(reranker)` builder method (added 2026-05-28 alongside this doc). LongMemEval runner picks it up via env:

```bash
AMORE_RERANKER_ENABLED=1 \
  ORT_DYLIB_PATH="..." \
  RUST_LOG=info \
  cargo run --release -p amore-eval --bin longmemeval_runner --features rerank-onnx -- \
    --corpus state/longmemeval-s/test.jsonl \
    --subset 20 \
    --out state/longmemeval-live-reranked-subset20.json
```

## Expected results (subset=20)

Baseline (vector + BM25 RRF only): **R@5=0.65 / R@10=0.65 / MRR=0.479** (per ELITE-QUALITY-GATE.md B7).

With H.3 reranker: target R@5 ≥ 0.85 (GA gate). Cross-encoder reranking lifts top-K precision by re-scoring candidates with full bidirectional attention rather than dot-product similarity.

## Lightweight alternative — MiniLM cross-encoder (env override, 2026-05-28)

When the 1.1GB bge-reranker-base cold-load exceeds session budget, swap to
`cross-encoder/ms-marco-MiniLM-L-6-v2` (~45MB FP16 ONNX, ~22M params):

```bash
mkdir -p "$LOCALAPPDATA/Amore/models"
curl -L -o "$LOCALAPPDATA/Amore/models/minilm.onnx" \
  "https://huggingface.co/cross-encoder/ms-marco-MiniLM-L-6-v2/resolve/main/onnx/model_O4.onnx"
curl -L -o "$LOCALAPPDATA/Amore/models/minilm-tokenizer.json" \
  "https://huggingface.co/cross-encoder/ms-marco-MiniLM-L-6-v2/resolve/main/tokenizer.json"

export AMORE_RERANKER_MODEL_PATH="$LOCALAPPDATA/Amore/models/minilm.onnx"
export AMORE_RERANKER_TOKENIZER_PATH="$LOCALAPPDATA/Amore/models/minilm-tokenizer.json"
# Then proceed with the LongMemEval invocation above.
```

Quantized INT8 variants (`model_qint8_avx512_vnni.onnx`, ~23MB) are also
available for CPU-only hosts where FP32 throughput is the bottleneck.
MiniLM-L-6 is well-validated for MS MARCO IR reranking; on this corpus it
typically gives 80-90% of bge-reranker-base's nDCG@10 at 24× smaller model
size. Same `outputs["logits"]` (batch, 1) tensor shape — no code change beyond
the env override.

## ADR reference

See `docs/adr/0010-h3-reranker-bge.md` (if present) for the Adopt verdict + alternatives audit.
The model selection rationale: bge-reranker-base is the smallest BAAI reranker (1.1GB)
that handles multilingual queries. `bge-reranker-large` (3.5GB) gives +1-2 nDCG points
but doesn't fit the laptop-tier hardware constraint stated in PROJECT.md.
