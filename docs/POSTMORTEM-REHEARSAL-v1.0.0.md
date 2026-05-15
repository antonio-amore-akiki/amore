---
stable: true
topic: postmortem-rehearsal w10 paper-exercise
---

# Incident Postmortem: Qdrant Pool Exhaustion — 17-Min Silent Recall Blackout (Hypothetical)

**Date**: 2026-06-15
**Authors**: Antonio Amore AKIKI
**Status**: Final

---

## Summary

During an extended self-dogfood session on 2026-06-15, sustained ~55 QPS recall traffic
against an 80k-observation corpus exhausted the default qdrant client pool (max=16);
the circuit breaker opened and recall queries silently returned empty results for
17 minutes before the operator noticed via anomalous Claude Code behavior.

## Impact

- **Availability breach**: 17 min (14:23–14:40 UTC) — Service Class A
- **Queries dropped**: ~920 (55 QPS/min × 17 min; pool saturation = 100% recall failure)
- **Error budget consumed**: ~39% of 30-day Class-A budget (17 min of 43.2 min monthly
  allowance per docs/SLO.md availability target of 99.9%)
- **Paths affected**: recall only; ingest, WAL persistence, and storage unaffected
- **Users affected**: 1 — Antonio (single-user dev-host; no external sessions active)
- **Data integrity**: none compromised; WAL intact throughout

## Root Causes (5-Whys)

1. **Why did recall return empty results?** — circuit breaker transitioned to open state
   because the qdrant pool was fully exhausted (all 16 connections held).
2. **Why were all 16 connections held?** — sustained ~55 QPS self-dogfood traffic
   exceeded the per-connection throughput ceiling for the default pool size of 16.
3. **Why was the pool sized at 16 under 80k corpus load?** — `AMORE_POOL_MAX_SIZE=16`
   default was tuned for the stage-1-canary baseline (≤10k corpus); the 80k corpus
   requirement was not in the  sizing matrix.
4. **Why was 80k corpus not in the sizing matrix?** — the sizing matrix was authored at
    before extended self-dogfood at scale; no Prometheus-driven pool-auto-tune runbook
   existed to update the default.
5. **Why was there no operational gap detected earlier?** — this is the first W10
   rehearsal exercise; the pool-sizing gap had never been exercised at this corpus scale
   before, and no alert was wired to qdrant pool pressure on the dev machine.

**Root cause (single sentence):** The default pool ceiling (max=16) was sized for
≤10k corpus and was never updated when corpus grew to 80k, with no alert to detect
pool pressure before circuit breaker activation.

## Trigger

Antonio ran an extended self-dogfood session with active recall queries against an ~80k
corpus. The sustained QPS of ~55 exceeded the throughput capacity of the 16-slot pool,
saturating all connections and triggering circuit breaker open state.

## Detection

- **Detection method**: operator-observed via Claude Code silent-failure behavior
  (recall returning empty results without error messages visible to user)
- **Time-to-detect**: 17 minutes (14:23–14:40 UTC)
- **Alerts that fired**: none — Prometheus scraper (`localhost:9090`) was not running on
  the dev machine; no scraping agent active to evaluate alert rules from
  `docs/MONITORING-ALERTS.md`
- **Alerts that should have fired**: `AmoreCircuitBreakerOpen` (per
  `docs/RUNBOOK.md#circuit-breaker-triage`) and a pool-pressure alert rule
  `amore_qdrant_pool_checkout_duration_seconds_p99 > 1s sustained 5 min` (not yet
  defined)
- **Detection gap**: silent-fail-open in recall path (empty results rather than surfaced
  error) masked the fault from the operator; no tray-icon state change on pool exhaustion

## Resolution

- **14:40:00 UTC** — Antonio noticed anomalous silent-results pattern in Claude Code
- **14:40:30 UTC** — opened amore-mcp JSON logs; identified pool exhaustion via log
  lines showing `bb8::Pool::state() connections=16 idle=0` (per  OTel observability)
- **14:40:45 UTC** — restarted amore-mcp (`systemctl --user restart amore-mcp.service`
  on Linux equivalent; `amore status --stop && amore status --start` on Windows)
- **14:41:00 UTC** — pool re-initialized; subsequent recall queries returned results

Time-to-resolve once detected: **<2 min**. Correct mitigation per
`docs/RUNBOOK.md#circuit-breaker-triage` (steps 3–4: identify pool exhaustion from
log, restart to re-initialize pool).

## Action Items

| Description | Type | Owner | Bug Tracker | Status |
|---|---|---|---|---|
| Auto-tune `AMORE_POOL_MAX_SIZE` to ≥32 + idle ≥8 for corpus >50k observations | prevent | Antonio | issue#TBD | open |
| Add Prometheus alert rule `amore_qdrant_pool_checkout_duration_seconds_p99 > 1s` sustained 5 min | mitigate | Antonio | issue#TBD | open |
| Surface pool exhaustion state in tray icon via Recent Activity ( delivery) | mitigate | Antonio | issue#TBD | open |
| Run quarterly self-dogfood stress test at 100k+ corpus to catch sizing drift | process | Antonio | issue#TBD | open |

## Lessons Learned

- **What went well**: circuit breaker correctly opened on pool exhaustion (did not cascade
  to other dependencies — ingest and WAL paths were unaffected); WAL persistence remained
  intact throughout the 17-min window; the operator noticed within 17 min, not days;
  restart resolved the fault in under 2 min, confirming the restart-as-mitigation path in
  `docs/RUNBOOK.md#circuit-breaker-triage` is viable.
- **What went wrong**: no Prometheus scraper was running on the dev machine, so no alert
  fired — the 17-min detection window was entirely operator-observation; pool default
  (max=16) was not updated when corpus exceeded the stage-1 sizing baseline; recall
  silently returned empty results rather than surfacing an error, masking the fault
  completely from the operator via Claude Code.
- **Where we got lucky**: the incident occurred during solo self-dogfood (single operator,
  no concurrent user sessions); in a multi-user configuration the same pool exhaustion
  would have affected all sessions simultaneously and consumed ~39% of the 30-day Class-A
  error budget in 17 minutes.

## Timeline (UTC)

- **14:23:00** — OUTAGE BEGINS — qdrant pool exhausts; first query blocked past timeout;
  circuit breaker `amore_circuit_breaker_state{dependency="qdrant"}` transitions to open
- **14:23:01** — circuit breaker open; all subsequent recall requests return empty results
  (silent fail; no HTTP error code surfaced to Claude Code MCP caller)
- **14:23–14:40** — Antonio continues self-dogfood session; interprets empty recall
  results as "nothing relevant in corpus" — silent failure masks fault entirely
- **14:40:00** — Antonio notices the pattern is anomalous: recent observations that should
  match recall queries are returning no results; opens amore-mcp logs
- **14:40:30** — log inspection confirms pool exhaustion:
  `WARN amore_qdrant pool_state={"connections":16,"idle":0} checkout_wait_ms=5003`
- **14:40:45** — initiates amore-mcp restart per `docs/RUNBOOK.md#circuit-breaker-triage`
- **14:41:00** — OUTAGE ENDS — pool re-initialized; first recall query post-restart
  returns expected results

## Supporting Information

- Circuit breaker triage runbook: `docs/RUNBOOK.md#circuit-breaker-triage` — steps 3–4
  are the correct path (pool exhaustion from log, restart to recover)
- Alert rules reference: `docs/MONITORING-ALERTS.md` — `AmoreCircuitBreakerOpen` alert
  would have fired under this scenario if a Prometheus scraper were active; this incident
  motivates the pool-pressure alert addition (Action Item 2 above)
- SLO error budget reference: `docs/SLO.md` Service Class A — 99.9% availability target,
  30-day rolling window, 43.2 min monthly budget; 17-min outage = ~39% budget consumed
- Log excerpt (representative — from  OTel structured JSON output):
  ```
  {"timestamp":"2026-06-15T14:40:30Z","level":"WARN","target":"amore_mcp::qdrant",
   "message":"pool_exhausted","connections":16,"idle":0,"checkout_wait_ms":5003,
   "circuit_breaker":"open"}
  ```
- Prometheus dashboard: `http://localhost:9090` (not scraped during incident; placeholder
  for future persistent Grafana instance when provisioned)

---

> **REHEARSAL NOTICE**: This is a paper-exercise rehearsal postmortem — no real incident
> occurred on 2026-06-15. This exercise validates that `docs/POSTMORTEM-TEMPLATE.md` is
> fully operable end-to-end: every template field accepted a concrete value from the
> hypothetical scenario without ambiguity. Conducted per W10 deliverable scope.
>
> Source: sre.google/sre-book/postmortem-culture
