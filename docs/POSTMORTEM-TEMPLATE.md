# Postmortem Template

topic: postmortem-template
purpose: blameless incident postmortem template per Google SRE format
stable: true

Use this template for every incident that causes SLO breach or data loss event.
Fill all sections; do not skip Root Causes or Action Items.

---

```
# Incident Postmortem: <Title>

**Date**: <YYYY-MM-DD>
**Authors**: <names>
**Status**: <Draft | Final | Action-Items-In-Progress>

## Summary
<1-2 sentences>

## Impact
<Quantified: users affected, queries dropped, downtime minutes, data integrity>

## Root Causes (5-Whys)
1. Why did X happen? Because...
2. Why...? Because...
...

## Trigger
<What initiated the incident>

## Detection
<Time-to-detect, how detected, alerts that fired or should have>

## Resolution
<Steps taken, time-to-resolve>

## Action Items
| Description | Type (mitigate/prevent/process) | Owner | Bug ID | Status |
|---|---|---|---|---|

## Lessons Learned
- **What went well**: ...
- **What went wrong**: ...
- **Where we got lucky**: ...

## Timeline (UTC)
- HH:MM — OUTAGE BEGINS — <event>
- HH:MM — <event>
- HH:MM — OUTAGE ENDS — <event>

## Supporting Information
- Dashboard: <URL>
- Logs: <path>
- Related incidents: <links>
```

---

## Filing a Postmortem

1. Copy template above into `docs/postmortems/YYYY-MM-DD-<slug>.md`.
2. Fill all sections within 24 h of incident resolution.
3. Root cause must be evidenced (log line / metric spike / stack trace) — not hypothesised.
4. Action items must have an owner and a target wave or date.
5. Mark `Status: Final` only after all P0/P1 action items are in a tracking issue.

## Blameless Policy

Postmortems identify systemic causes, not individuals. Action items target
processes, tooling, and runbook gaps — not human error in isolation.

---

Source: sre.google/sre-book/example-postmortem + sre.google/sre-book/postmortem-culture
