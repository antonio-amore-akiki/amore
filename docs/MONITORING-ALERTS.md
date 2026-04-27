---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---

# Amore Monitoring Alerts

topic: monitoring-alerts
purpose: per-SLO alert rules with Prometheus queries and thresholds
stable: true
owner: Antonio Amore AKIKI (solo on-call — see docs/ON-CALL.md)

These alert rules are wired to the SLO targets in docs/SLO.md (Service Class A/B/C).
Full Prometheus scrape config ships in W2-2B. Runbook anchors reference docs/RUNBOOK.md.

---

## AmoreAvailabilityBudgetBurn

**SLO:** Availability 99.9% rolling 30d (Service Class A)
**Severity:** critical
**For:** 5m

```promql
rate(amore_db_operation_total{status="error"}[5m])
/
rate(amore_db_operation_total[5m])
> 0.001
```

**Meaning:** Error rate exceeds 0.1% over 5-minute window — burns availability budget faster than 1x.
**Threshold:** 0.001 (0.1% error rate)
**Runbook:** docs/RUNBOOK.md#availability-triage
**Owner:** antonioakiki15@gmail.com

---

## AmoreLatencyP99Breach

**SLO:** Latency p99 ≤ 500ms @ ≤ 10K corpus (Service Class A)
**Severity:** critical
**For:** 5m

```promql
histogram_quantile(
  0.99,
  rate(amore_db_operation_duration_seconds_bucket[5m])
) > 0.5
```

**Meaning:** p99 latency exceeds 500ms for 5 minutes — SLO breach in progress.
**Threshold:** 0.5 seconds
**Runbook:** docs/RUNBOOK.md#performance-triage
**Owner:** antonioakiki15@gmail.com

---

## AmoreThroughputDrop

**SLO:** Throughput ≥ 50 QPS sustained (Service Class A)
**Severity:** warning
**For:** 10m

```promql
rate(amore_db_operation_total[5m]) < 50
```

**Meaning:** Operation rate below 50 QPS for 10 minutes. Correlate with request volume — low traffic is also a valid trigger; investigate process crash, OOM, or upstream dep failure.
**Threshold:** 50 ops/second
**Runbook:** docs/RUNBOOK.md#health-check
**Owner:** antonioakiki15@gmail.com

---

## AmoreWALFsyncFailure

**SLO:** Durability 99.99% WAL fsync success (Service Class B)
**Severity:** critical
**For:** 1m

```promql
rate(amore_wal_writes_total{status="error"}[5m]) > 0
```

**Meaning:** Any WAL fsync error is a durability risk — data loss possible. Immediate triage required.
**Threshold:** any error rate > 0
**Runbook:** docs/RUNBOOK.md#storage-failure
**Owner:** antonioakiki15@gmail.com

---

## AmoreCircuitBreakerOpen

**SLO:** Dependency availability (Qdrant + Ollama) — supports Service Class A
**Severity:** warning
**For:** 1m

```promql
amore_circuit_breaker_state > 1
```

**Meaning:** Circuit breaker open/half-open. Recall degraded to BM25-only. Investigate Qdrant/Ollama reachability.
**Metric values:** 0 = closed (healthy), 1 = half-open, 2 = open (degraded)
**Threshold:** state > 1 for 1 minute
**Runbook:** docs/RUNBOOK.md#circuit-breaker-triage
**Owner:** antonioakiki15@gmail.com

---

## AmoreCacheHitRatioLow

**SLO:** L1 cache effectiveness — latency proxy for Service Class A
**Severity:** warning
**For:** 15m

```promql
amore_cache_hit_ratio{level="l1"} < 0.5
```

**Meaning:** L1 cache hit ratio below 50% for 15 minutes — recall latency degrades toward raw Qdrant + embed costs. Investigate cache eviction or cold corpus.
**Threshold:** 0.5 (50% hit ratio)
**Runbook:** docs/RUNBOOK.md#cache-triage
**Owner:** antonioakiki15@gmail.com

---

## Deployment notes

- Alert rules complete; Prometheus scrape config + alertmanager wiring ships in W2-2B.
- Metric names defined in docs/SLO.md §Service Class A SLI measurement and in the Prometheus
  exporter at `crates/amore-mcp/src/metrics.rs` (W2-2B target).
- Alertmanager route: critical → immediate Tailscale ntfy (whispergate); warning → daily digest.
- No PagerDuty (single-author scope; see docs/ON-CALL.md).

---

Source: docs/SLO.md; Prometheus alerting best practices (prometheus.io/docs/practices/alerting)
