# Amore Error Budget Policy

topic: sre-error-budget-policy
purpose: error budget allocations and freeze triggers per Google SRE Ch.3
stable: true

Per Google SRE Ch.3: error budget = `1 − SLO_target` over a 30-day rolling window.
The budget quantifies acceptable unreliability; when exhausted, reliability work
takes precedence over all feature work.

## Budget Allocations (30-day rolling window)

| Class | SLO | Budget | Calendar equivalent |
|---|---|---|---|
| A availability | 99.9% | 0.1% × 30d × 86400s = **2592 s/month** | 43.2 min/month |
| A latency | 99.0% | 1% × 30d = **7.2 h/month** of breached requests | 7.2 h total |
| B WAL fsync | 99.9% | 0.1% per write call | per-operation |
| C gRPC health | 99.95% | 0.05% × 30d × 86400s = **1296 s/month** | 21.6 min/month |

## Freeze Trigger

When 50% of any 30-day budget is consumed within that window:

1. All non-reliability PRs are **blocked** until budget is restored.
2. A postmortem (per `docs/POSTMORTEM-TEMPLATE.md`) is opened within 24 h.
3. On-call owner notified via Tailscale ntfy alert.

**Recovery**: 7 consecutive days with error rate below the SLI threshold
restores the budget and lifts the freeze. The unfreeze must be explicit
(manual confirmation in postmortem action-items, not automatic).

## Burn-Rate Tracking

Weekly Prometheus query against `amore_db_operation_total{status="error"}`:

```promql
# 1-hour burn rate vs 30-day budget
sum(rate(amore_db_operation_total{status="error"}[1h]))
  /
sum(rate(amore_db_operation_total[1h]))
```

**Alert threshold**: 5× burn rate → will exhaust budget in ~6 days at
current trajectory. Alert fires via Prometheus alertmanager rule:

```yaml
- alert: ErrorBudgetBurnRateHigh
  expr: |
    (
      sum(rate(amore_db_operation_total{status="error"}[1h]))
      / sum(rate(amore_db_operation_total[1h]))
    ) > (5 * 0.001)
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Error budget burning at 5x rate — exhausts in ~6d"
```

## Policy Enforcement

| Condition | Action |
|---|---|
| Budget > 50% remaining | All PRs allowed |
| Budget 25–50% remaining | Reliability items prioritised in sprint |
| Budget < 25% remaining | Freeze — reliability-only PRs |
| Budget exhausted | Hard freeze; escalate to incident |

---

Source: sre.google/sre-book/embracing-risk
