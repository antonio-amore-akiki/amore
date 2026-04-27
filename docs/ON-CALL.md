---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---

# Amore On-Call Memo

topic: on-call
purpose: solo on-call rotation definition + escalation path + coverage gaps
stable: true
owner: Antonio Amore AKIKI

---

## Scope

Amore is a single-author open-source project. There is one on-call operator:
**Antonio Amore AKIKI** (antonioakiki15@gmail.com).

This memo documents the on-call commitment, escalation path, coverage gaps, and postmortem
trigger, per Google SRE workbook §Engagement Model (on-call rotation staffed category).

---

## Rotation

**Frequency:** weekly self-rotation — every Monday, the operator reviews:
1. Error budget consumption (docs/ERROR-BUDGET-POLICY.md § burn-rate thresholds)
2. Open incident queue (GitHub Issues tagged `incident`)
3. RUSTSEC advisory feed for new advisories on Cargo.lock deps

**Rotation handoff:** N/A — single-author; no secondary on-call.

---

## Reach

| Channel | Address | SLA |
|---|---|---|
| Email | antonioakiki15@gmail.com | 24h non-business; 4h business-hours (best-effort) |
| Tailscale ntfy | whispergate (internal; see docs/SECRETS.md) | immediate for critical alerts |

No PagerDuty or third-party paging. Critical alert → Tailscale ntfy fires immediately per
docs/MONITORING-ALERTS.md alertmanager route.

---

## Acknowledge SLA

- **Business hours (09:00–18:00 local, Mon–Fri):** acknowledge within 4 hours
- **Non-business hours / weekends:** acknowledge within 24 hours
- **Extended absence (vacation, illness):** see Coverage Gap below

These are single-author best-effort targets. No SLA penalty applies; documented for
transparency to users and for the PRR record.

---

## Escalation path

**None.** Single-author project; there is no secondary escalation path.

If the operator is unreachable:
- Users file GitHub Issues; queue is reviewed on return.
- No live coverage during extended absence.
- This gap is formally documented in docs/SUPPORT.md.

---

## Postmortem trigger

Any SLO breach (docs/SLO.md Class A/B/C) triggers a mandatory postmortem within 5 business days.

Postmortem format: docs/POSTMORTEM-TEMPLATE.md (Google SRE blameless format; action-item tracking).

Severity thresholds:
- **P0 (critical):** any availability breach > 1h OR any data loss → postmortem within 48h
- **P1 (high):** latency SLO breach > 30min OR error budget > 50% consumed in a day → postmortem within 5 business days
- **P2 (medium):** warning-level alert > 2h → incident note (GitHub Issue); no formal postmortem required

---

## Coverage gap — extended absence

During extended absence (vacation, illness, or other), Amore provides no live on-call coverage:
- Monitoring alerts continue to fire to whispergate (unattended).
- Users are directed to GitHub Issues for support.
- Incident queue is reviewed on the operator's return.
- This gap is acknowledged and documented; users are informed in docs/SUPPORT.md.

No workaround exists for this gap in a single-author project. The gap is accepted risk.

---

Source: sre.google/workbook/engagement-model §on-call; docs/POSTMORTEM-TEMPLATE.md
