# Amore Service Level Objectives

stable: true
purpose: published latency + availability targets per corpus size
tag_baseline: v0.3.1-live-fire

## Latency tiers

p95 recall latency, measured end-to-end (client send → response receive)
on dev-laptop class hardware (16 GB RAM, NVMe SSD, modern x86_64 CPU):

| Corpus size | p95 | p99 | Mode |
|---|---|---|---|
| ≤ 10K observations | 200 ms | 400 ms | single-node default |
| 10K–100K | 500 ms | 1.0 s | single-node default |
| 100K–1M | 1.5 s | 3.0 s | single-node default |
| 1M–10M | 5 s | 10 s | cluster mode (Phase H, opt-in) |
| 10M–100M | 10 s | 20 s | cluster mode + dedicated peers |

## Cold-start

- `amore --version` ≤ 100 ms cold
- `amore-mcp` ready-to-serve ≤ 500 ms after stdio connect
- GUI first paint ≤ 1.5 s on cold launch

## Resource footprint

- Idle RSS: ≤ 80 MB
- 10K-corpus RSS: ≤ 200 MB
- 100K-corpus RSS: ≤ 500 MB
- Binary size on disk: ≤ 80 MB combined (`amore.exe` + `amore-mcp.exe`
  + `amore-gui.exe`)
- Installer .exe: ≤ 150 MB (includes bundled Qdrant + bge-small.onnx)

## Availability

- Single-node: 99.9% over the user's typical session window
- Cluster (3-node, RF=2): 99.99% — 1 node loss tolerated without
  recall interruption
- Error budget: 0.1% / 0.01% respectively

## Throughput

- Sustained recall QPS @ p95 < 300 ms: ≥ 100 QPS on 10K corpus
- Ingest throughput: ≥ 50 docs/sec on 10K corpus

## What "p95" means here

We use the rolling 1-hour p95 measured at the MCP server boundary
(spans `recall_request_received` → `recall_response_sent` in the
tracing spans). A query that hits the cross-encoder reranker plus
both vector + BM25 lanes counts as one observation.

## Degraded mode SLO

When one lane (Qdrant or Ollama) is unreachable, recall degrades to
BM25-only. The degraded SLO is:

| Corpus | p95 (degraded) |
|---|---|
| ≤ 10K | 100 ms |
| 10K–100K | 300 ms |
| 100K–1M | 1 s |

BM25-only is faster than full hybrid because there's no embed step.

## Out-of-SLO triage

If sustained p95 exceeds the documented target by ≥ 20% for 30 min:
1. `amore doctor --slo` (v0.7.0+) prints lane-by-lane timings.
2. Check `%APPDATA%\Amore\amore.log` for `WARN slow_lane=...` lines.
3. Inspect Qdrant collection size; if exceeded corpus tier, consider
   cluster opt-in.
4. If on cluster, run `curl http://localhost:6333/cluster/info` to
   verify all peers are reachable.

## How SLO ties to releases

Every release tag includes a `state/perf-baseline-vA.B.C.json` produced
by the local Criterion bench harness (`cargo bench`). The release is
**blocked** from publish if p95 regresses against the previous tag's
baseline by > 20% on any corpus tier.

The bench harness lives in `crates/amore-eval/benches/` (Phase G +
Phase H builds).

## Reference

- Architecture: `docs/ARCHITECTURE.md`
- Capacity math: `docs/SCALE-100M.md`
- Runbook out-of-SLO triage: `docs/RUNBOOK.md` "Performance triage"
