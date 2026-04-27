---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---

# Post-Release Monitoring — v1.0.0

topic: post-release-monitoring
release: v1.0.0
window: 48h daily checklist + Day 7 summary
initialized: 2026-05-27T00:00Z

---

## Purpose

48h close-watch after the v1.0.0 release, escalating from daily manual checks (Day 0–2)
to a weekly summary (Day 7). Every day MUST record a result for each checklist item
before it can be marked complete.

---

## Daily Checklist Items (all six required each day)

1. **Download count** — record new downloads since previous check.
   ```powershell
   gh release view v1.0.0 --json assets --jq '.assets[].downloadCount'
   ```
2. **Issue triage** — count new issues labeled `v1.0.0`; P0 must be triaged within 4h.
   ```powershell
   gh issue list --label v1.0.0
   ```
3. **ntfy queue** — check Tailscale ntfy for critical alerts; ack or escalate.
4. **Error rate** — check Prometheus dashboard for
   `amore_db_operation_total{status="error"}` over the last 24h; flag any rate > 0.1%.
   ```
   http://localhost:9090/graph?g0.expr=rate(amore_db_operation_total{status="error"}[24h])
   ```
5. **Error budget delta** — append one row to `docs/ERROR-BUDGET-TRACKER-v1.0.0.md`
   with the incremental burn for each service class since the last entry.
6. **Disk usage** — flag if sled + qdrant volumes exceed 80% on dev host.
   ```powershell
   Get-PSDrive C | Select-Object Used, Free
   ```

---

## Day 0 (release day)

**Date**: ______ **Completed by**: ______

| Item | Result | Notes |
|---|---|---|
| Download count | | |
| Issue triage | | |
| ntfy queue | | |
| Error rate (24h) | | |
| Error budget delta | | |
| Disk usage | | |

**Day 0 verdict**: PASS / FLAG / ESCALATE

---

## Day 1 (T+24h)

**Date**: ______ **Completed by**: ______

| Item | Result | Notes |
|---|---|---|
| Download count | | |
| Issue triage | | |
| ntfy queue | | |
| Error rate (24h) | | |
| Error budget delta | | |
| Disk usage | | |

**Day 1 verdict**: PASS / FLAG / ESCALATE

---

## Day 2 (T+48h — close of monitoring window)

**Date**: ______ **Completed by**: ______

| Item | Result | Notes |
|---|---|---|
| Download count | | |
| Issue triage | | |
| ntfy queue | | |
| Error rate (24h) | | |
| Error budget delta | | |
| Disk usage | | |

**Day 2 verdict**: PASS / FLAG / ESCALATE

**48h monitoring closed**: YES / NO (close only if all three days PASS)

---

## Day 7 (T+7d — weekly summary)

**Date**: ______ **Completed by**: ______

| Metric | Value | Trend |
|---|---|---|
| Total downloads | | |
| Open issues (v1.0.0) | | |
| P0/P1 issues resolved | | |
| Cumulative error budget burned (Class A) | | |
| Cumulative error budget burned (Class B) | | |
| Cumulative error budget burned (Class C) | | |
| Any freeze trigger crossed? | | |

**Day 7 verdict**: PASS / FLAG / ESCALATE

**Transition**: after Day 7 PASS, monitoring cadence shifts to weekly via
`scripts/error-budget-update.ps1` (Mondays).

---

## Escalation Criteria

| Condition | Action |
|---|---|
| P0 issue filed | Triage within 4h; hotfix branch if confirmed regression |
| Error rate > 1% over 1h | Page immediately; consider rollback via `AMORE_FLAG_*` toggles |
| Any class budget > 50% burned | Trigger release freeze per ERROR-BUDGET-TRACKER policy |
| Disk > 80% | Prune old compaction snapshots; alert if still > 80% after prune |
