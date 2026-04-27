---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---

# Error Budget Tracker — v1.0.0

topic: error-budget-tracker
initialized: 2026-05-27T00:00Z (post-release initialization)
update-cadence: weekly (every Monday via scripts/error-budget-update.ps1)
source: sre.google/sre-book/embracing-risk Ch.3

---

## Service Class A — User-Facing Recall (availability SLO: 99.9% rolling 30d)

| Field | Value |
|---|---|
| SLO target | 99.9% |
| Budget ceiling (30d) | 0.1% × 30d × 86 400 s = **2 592 s** |
| Burned this period | 0 s |
| Remaining | **2 592 s (100%)** |
| Burn-rate (last 7d) | 0% / day |
| Freeze trigger | 50% burned → **1 296 s** |
| Current state | **SAFE** |

## Service Class B — Storage / Ingest (availability SLO: 99.99% rolling 30d)

| Field | Value |
|---|---|
| SLO target | 99.99% |
| Budget ceiling (30d) | 0.01% × 30d × 86 400 s = **259 s** |
| Burned this period | 0 s |
| Remaining | **259 s (100%)** |
| Burn-rate (last 7d) | 0% / day |
| Freeze trigger | 50% burned → **130 s** |
| Current state | **SAFE** |

## Service Class C — gRPC Health Endpoint (availability SLO: 99.95% rolling 30d)

| Field | Value |
|---|---|
| SLO target | 99.95% |
| Budget ceiling (30d) | 0.05% × 30d × 86 400 s = **1 296 s** |
| Burned this period | 0 s |
| Remaining | **1 296 s (100%)** |
| Burn-rate (last 7d) | 0% / day |
| Freeze trigger | 50% burned → **648 s** |
| Current state | **SAFE** |

---

## Freeze Trigger Policy (Google SRE Ch.3)

If **50% of any class budget** is burned within a 30-day rolling window:

1. Halt feature releases for that service class immediately.
2. All engineering capacity redirects to reliability work.
3. Freeze lifts after **7 consecutive days** with availability above SLO threshold.
4. `scripts/error-budget-update.ps1` exits non-zero (code 1) when this threshold is
   crossed — wire to CI to block release pipeline.

---

## Weekly Burn Log

Append one row per Monday after running `scripts/error-budget-update.ps1`.

| Date (UTC) | Class A burned (s) | Class B burned (s) | Class C burned (s) | Notes |
|---|---|---|---|---|
| 2026-05-27 | 0 | 0 | 0 | post-release initialization |

---

## Update Instructions

```powershell
# Default: queries localhost:9090 over the last 7 days
.\scripts\error-budget-update.ps1

# Custom endpoint + window
.\scripts\error-budget-update.ps1 -Endpoint http://prometheus.local:9090 -Window 14
```

Append the output row to the Weekly Burn Log table above. If the script exits 1,
trigger release freeze per policy above.
