# H.10 Load Test Procedure — 10M Corpus, 100 QPS, 1 Hour

stable: true
purpose: operator runbook for H.10 sustained load test

## Purpose

Prove the production scale claim in `docs/SCALE-100M.md`: Amore recall sustains
100 QPS at 10M observations with p95 <= 5 s per `docs/SLO.md` (1M–10M tier).

Pass gate: p95 <= 5000 ms AND error rate <= 0.1% AND achieved QPS >= 95 QPS
(95% of 100 QPS target).

## Prerequisites

All of the following must be satisfied before the full run:

1. Qdrant 3-node cluster up and green:
   `pwsh ./tests/qa/h2_qdrant_cluster_smoke.ps1` exits 0.
2. Ollama running with `nomic-embed-text` model loaded:
   `ollama pull nomic-embed-text && ollama serve`
3. `oha` installed (Rust HTTP load tester, single binary):
   `cargo install oha`
4. `amore` and `amore-mcp` built in release mode:
   `cargo build --release`
5. Disk: 64 GB free on the data volume (10M obs × 768-dim int8 + SQLite).
6. RAM: 16 GB free on the host (Qdrant in-memory HNSW index).

## Quick-run

### Verify harness (no actual load)

```powershell
pwsh ./tests/qa/h10_load_10m.ps1 -DryRun
```

Exits 0 when: oha present, amore present, Qdrant 3-node reachable,
seeder compiles and seeds 100 observations successfully.

### Full 1-hour load test

```powershell
pwsh ./tests/qa/h10_load_10m.ps1
```

Runs in four phases:
1. Deps check (oha, amore, qdrant 3 peers).
2. Corpus seed: 10M synthetic observations via `seed_load_test_corpus`.
3. Sustained load: 100 QPS POST /recall for 3600 s via `oha`.
4. Assert p95/error-rate/QPS gates; write proof JSON.

### Resource-constrained hosts (< 64 GB disk or < 16 GB free RAM)

Use a smaller corpus and apply the extrapolation factor from `docs/SCALE-100M.md`:

```powershell
pwsh ./tests/qa/h10_load_10m.ps1 -CorpusSize 1000000 -DurationSec 1800
```

At 1M corpus p95 target is 1.5 s per SLO.md; multiply by the H.8 measured
scaling factor to project 10M behaviour.

## Output location

All output files land in `%LOCALAPPDATA%\Amore\`:

- `h10-load-<ts>.json` — raw oha JSON output.
- `h10-load-result-<ts>.json` — structured proof JSON with all metrics + gate verdicts.

## Metrics captured

Per `docs/SLO.md` and `docs/SCALE-100M.md`:

| Metric | Source | Gate |
|---|---|---|
| p50 latency | oha latency_percentiles.p50 | informational |
| p95 latency | oha latency_percentiles.p95 | <= 5000 ms |
| p99 latency | oha latency_percentiles.p99 | informational |
| p99.9 latency | oha latency_percentiles.p99_9 | informational |
| sustained QPS | oha summary.requests_per_sec | >= 95 QPS |
| error count | oha summary.errors | <= 0.1% of total |
| error rate | computed | <= 0.1% |

RSS, disk I/O, and network bytes are not captured automatically; use
Task Manager or `Get-Process` / `Get-Counter` during the run if needed.

## Pass gate

```
p95_ms <= 5000
AND error_rate_pct <= 0.1
AND achieved_qps >= 95
```

Exit code 0 = PASS. Non-zero exit codes:
- 1: missing dep (oha / amore / qdrant cluster).
- 2: p95 over budget.
- 3: error rate over 0.1% or QPS below minimum.
- 4: corpus seeder failed.

## Disaster scenarios

**Qdrant cluster down mid-run**: oha errors cluster together in the histogram.
Recovery: re-run `h2_qdrant_cluster_smoke.ps1` to verify cluster health, then
restart the full load test.

**Ollama down**: embedding path fails; amore-mcp falls back to BM25-only
(degraded mode per B1/B2). Error rate may spike if vector lane is required.
Recovery: `ollama serve`, re-run.

**Disk full during seeding**: seeder exits non-zero with a SQLite or Qdrant
write error. Recovery: move the data dir to a larger volume:
`$env:AMORE_DATA_DIR = "D:\amore-data"`, then re-run.

**oha unavailable**: `cargo install oha` installs from crates.io (requires
internet + Rust toolchain). Estimated install time: 2–5 minutes.

## References

- SLO targets: `docs/SLO.md`
- Scale math: `docs/SCALE-100M.md`
- Cluster smoke: `tests/qa/h2_qdrant_cluster_smoke.ps1`
- Seeder binary: `crates/amore-eval/src/bin/seed_load_test_corpus.rs`
- oha tool: <https://github.com/hatoo/oha>
