<!-- stable: true -->
# Amore Benchmark Results

State-of-the-art proof for v1.0 marketing claims.
Every cell must be backed by a runnable command or marked PENDING.

---

## Token reduction — canonical-docs router (43 fixtures, measured 2026-05-27)

| Metric | Result | Target | Verdict |
|---|---|---|---|
| Average token reduction | **21.1%** | ≥ 85% (stretch) | PARTIAL |
| Worst-case reduction | -144.6% (one outlier: `timeout-amore-ms-env-cap`) | ≥ 75% | FAIL |
| Sample size | 43 fixtures | ≥ 30 | PASS |

**What this measures**: token count of Amore's routed-context recall (top-k canonical-docs snippets) vs raw-context baseline (full doc dump) for 43 real questions about Amore itself (install, config, troubleshooting). Lower is better; positive % = Amore used fewer tokens.

**Reproduction**:
```
amore-eval-benchmark --release token-reduction \
  --fixtures crates/amore-eval/fixtures/ \
  --results-tsv state/results.tsv
```

**Honest read**: 21% per-query savings is modest. The community reference of "~70× per session" from `lucasrosati/claude-code-memory-setup` measures a different thing — session-level context-window utilization across many turns. v1.1 plan: real Claude Code session-level measurement to close the gap.

---

## Datasets

| Dataset | License | Purpose | Status |
|---|---|---|---|
| LongMemEval (xiaowu0162/LongMemEval) | MIT | Agent-memory recall R@1/5/10 | Harness shipped; eval pending Wave 3 recall wiring |
| HotpotQA | CC BY-SA 4.0 | Multi-hop QA, complementary | Deferred Wave 4 |
| 2WikiMultihopQA | Apache-2.0 | Multi-hop QA, complementary | Deferred Wave 4 |

---

## Methodology

- Hardware recorded per run (see report JSON).
- All commands below are exact reproduction steps.
- Cells marked **PENDING** require live Qdrant + Ollama; see Reproduction section.
- Competitor numbers cite source URL + retrieval date — never estimated.

---

## Binary Sizes (measured 2026-05-26, Windows 11 x86_64, release LTO)

```
amore-eval-benchmark      0.691 MB
amore-eval-longmemeval    0.718 MB
token-reduction           4.385 MB
seed_load_test_corpus     5.342 MB
amore-mcp                10.338 MB
```

Command: `amore-eval-benchmark binary-size`
Report: `%LOCALAPPDATA%\Amore\benchmarks\20260526T191955Z-binary-size.json`

---

## Cache Hit Ratio (measured 2026-05-26, Zipfian s=1.0, corpus 10k, 1000 queries)

| Metric | Value |
|---|---|
| L1 hit ratio (cap=256) | **46.2%** |
| L2 hit ratio (cap=4096) | **11.6%** |
| Combined L1+L2 | **57.8%** |
| Miss rate | 42.2% |
| Warmup queries | 500 |
| Measurement queries | 500 |

In-process LRU simulation over Zipfian access distribution — no daemon required.
Command: `amore-eval-benchmark --queries 1000 cache-hit-ratio`

---

## Latency Percentiles — MEASUREMENT PENDING

Requires live Qdrant (port 6333) + Ollama (port 11434) + seeded corpus.

Reproduction:
```sh
pwsh ./tests/qa/lib/ensure_daemons.ps1
cargo run --release --bin seed_load_test_corpus -- --count 100000
amore-eval-benchmark --corpus-size 100000 --queries 1000 latency
```

Target: p99 < 200 ms at 100k corpus (per SCALE-100M.md SLO spec).

---

## Throughput — MEASUREMENT PENDING

```sh
amore-eval-benchmark --corpus-size 100000 throughput
```

Target: ≥ 50 QPS sustained at 100k corpus.

---

## Cold-Start Latency — MEASUREMENT PENDING

```sh
amore-eval-benchmark cold-start
```

Target: < 500 ms (per SLO.md).

---

## LongMemEval — MEASUREMENT PENDING

SOTA target: mem0 R@5 = 95.2%
Source: https://github.com/mem0ai/mem0 (retrieved 2026-05-27)
Paper: https://arxiv.org/abs/2504.19413 (Mem0, Chhikara et al., 2025)

### Download dataset (one-time, ~100 MB, MIT license)

```sh
pip install datasets
python -c "from datasets import load_dataset; \
  load_dataset('xiaowu0162/LongMemEval', split='test') \
  .to_json('~/.local/share/Amore/datasets/longmemeval/test.jsonl')"
```

### Run eval

```sh
pwsh ./tests/qa/lib/ensure_daemons.ps1
amore-eval-longmemeval \
  --dataset ~/.local/share/Amore/datasets/longmemeval/test.jsonl
```

### Results table

| Category | R@1 | R@5 | R@10 |
|---|---|---|---|
| single-session | PENDING | PENDING | PENDING |
| multi-session | PENDING | PENDING | PENDING |
| knowledge-update | PENDING | PENDING | PENDING |
| temporal-reasoning | PENDING | PENDING | PENDING |
| **OVERALL** | **PENDING** | **PENDING** | **PENDING** |
| mem0 SOTA (cited) | — | **95.2%** | — |

---

## Vs Competitor Table

| System | R@5 (LongMemEval) | Source | Retrieved |
|---|---|---|---|
| **Amore** | PENDING | this repo | — |
| mem0 | 95.2% | https://arxiv.org/abs/2504.19413 | 2026-05-27 |
| Zep | no public number | https://github.com/getzep/zep | 2026-05-27 |
| Letta (MemGPT) | no public number | https://github.com/letta-ai/letta | 2026-05-27 |

---

## Test Suite Results (measured in earlier waves — see test logs)

| Wave | Component | Result |
|---|---|---|
| G.4 | proptest (provenance + recall + canonical-docs) | 10/10 × 256 cases |
| H.0 | BM25 FTS5 + RRF frozen-fixture tests | 8/8 + 8/8 |
| H.1 | Tantivy parity vs FTS5 baseline | 20/20 rank-identical |
| H.3 | Cross-encoder reranker (ort) | 4/4 default-on pass |
| H.5 | Circuit-breaker state machine | 6/6 pass |
| H.8 | WAL replay | 2/2 unconditional pass |
| H.9 | Compaction worker | 2/3 default-on; 1 env-gated |
| H.13 | Multi-level cache (moka L1 + sled L2) | 4/5 default-on; 1 env-gated |
| H.12 | Chaos (toxiproxy) | dry-run pass; full run deferred Phase J |

---

## Hardware

- OS: Windows 11 Pro 10.0.26200 (x86_64)
- CPU: see `PROCESSOR_IDENTIFIER` in report JSON
- Disk: NVMe (OS drive)

---

## Reproduction

```sh
# 1. Ensure daemons
pwsh ./tests/qa/lib/ensure_daemons.ps1

# 2. Build release binaries
cargo build --release --bin amore-eval-benchmark --bin amore-eval-longmemeval

# 3. Binary sizes (no daemon)
./target/release/amore-eval-benchmark binary-size

# 4. Cache hit ratio (no daemon)
./target/release/amore-eval-benchmark --queries 1000 cache-hit-ratio

# 5. Latency / throughput / cold-start (daemon required)
./target/release/amore-eval-benchmark --corpus-size 100000 --queries 1000 latency
./target/release/amore-eval-benchmark --corpus-size 100000 throughput
./target/release/amore-eval-benchmark cold-start

# 6. LongMemEval (dataset + daemon required)
./target/release/amore-eval-longmemeval \
  --dataset ~/.local/share/Amore/datasets/longmemeval/test.jsonl
```

---

## Status

| Benchmark | Status |
|---|---|
| Binary sizes | MEASURED 2026-05-26 |
| Cache hit ratio (Zipfian) | MEASURED 2026-05-26 |
| Latency p50/p95/p99/p99.9 | PENDING Wave 3 |
| Throughput QPS | PENDING Wave 3 |
| Cold-start | PENDING Wave 3 |
| LongMemEval R@1/5/10 | PENDING Wave 3 + dataset download |
