---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---
# Post-Release Integrity — v1.0.0

**Scope clarification (single-author local-first):** Amore is local-first; the author is both producer and sole user. Industry "post-release monitoring" assumes external signals — download counts from third parties, issue triage from external reporters, alerts firing on production traffic — that do not exist in this scope. The W9 v1 plan's monitoring framework (48h GH download-count tracking, issue triage, Tailscale ntfy queue review, Prometheus error rate) was theatre transplanted from Google-SRE-scale to a scope that does not contain it. This doc lists the two activities that DO matter for a single-author local-first product.

## 1. Signature integrity (the one thing that can go wrong without a second user)

Tampering at the upload boundary is the only externally-introduced risk that single-author scope still faces. Verify every uploaded artifact before any future install.

For each artifact on the GitHub release page:

```bash
# Linux / macOS
cosign verify-blob \
  --bundle <artifact>.sigstore \
  --certificate-identity-regexp 'antonioakiki15@gmail\.com' \
  --certificate-oidc-issuer https://github.com/login/oauth \
  <artifact>

sha256sum -c sha256sums.txt
```

```powershell
# Windows
cosign verify-blob `
  --bundle <artifact>.sigstore `
  --certificate-identity-regexp 'antonioakiki15@gmail\.com' `
  --certificate-oidc-issuer https://github.com/login/oauth `
  <artifact>

Get-FileHash <artifact> -Algorithm SHA256
# Compare against sha256sums.txt manually OR via:
# (Get-Content sha256sums.txt | Select-String <artifact>).Line
```

Pass criterion: every artifact returns `exit 0` from cosign-verify AND matches its SHA256 in `sha256sums.txt`.

Failure response: do NOT install the unverified artifact. Open an investigation-ledger row at `state/investigation-ledger.jsonl` with root cause = upload-boundary tampering OR signing-key compromise OR sha256sums.txt mismatch. File a postmortem per `docs/POSTMORTEM-TEMPLATE.md`.

## 2. Self-dogfood (the only "monitoring" signal that exists in this scope)

Run `amore-mcp` on your own machine during normal Claude Code / Claude Desktop / Cursor / Cline / Continue use. Any panic / SLO breach / crash you observe IS the monitoring signal — there is no second user to file a bug report.

Tracking template:

| Date | Workflow | Hours active | Anomalies observed | Action |
|---|---|---|---|---|
| `<ISO>` | _describe what you used Amore for_ | _N_ | _none / crash X / latency spike Y_ | _none / investigation-ledger row / postmortem_ |

Append rows to `state/v1.0.0-self-dogfood.jsonl` (gitignored).

If you observe a SLO breach (per `docs/SLO.md` thresholds):
1. Reproduce — confirm the breach is real and not a transient.
2. Investigation-ledger row at `state/investigation-ledger.jsonl` with root-cause analysis.
3. If SLO-breach class: fill `docs/POSTMORTEM-TEMPLATE.md` end-to-end (5-Whys, UTC timeline, action items).
4. Cut a fix; tag a v1.0.x patch.

## What this doc explicitly drops as theatre

The following items appeared in the W9 v1 plan and are explicitly dropped because they do not apply to single-author local-first scope:

- **48h GitHub download-count tracking** — zero external downloaders; the only downloader is you. Tracking your own download is not signal.
- **Issue triage from external reporters** — zero external reporters; the only issue-filer is you, and you do not file issues against yourself (you file investigation-ledger rows + postmortems instead, see §2).
- **Tailscale ntfy queue review** — ntfy only fires when YOU run the binary. Already captured under self-dogfood (§2); no separate monitoring step needed.
- **Prometheus error rate from "production traffic"** — there is no production traffic in this scope. Prometheus scrape happens against your own machine; meaningful only IF you run Amore enough to generate samples (see `docs/ERROR-BUDGET-TRACKER-v1.0.0.md` for the conditional-on-usage framing).

This is not a degraded copy of the Google-SRE pattern; it is the scope-honest substitute. If/when Amore acquires external users (public-flip, hosted offering), this doc updates to add the external-signal activities back.

## When this doc applies

- **Now and continuously**: §1 signature integrity — run before every install of every release.
- **During active use**: §2 self-dogfood — track ambient anomalies you observe during normal work.
- **On any SLO breach observed in §2**: full postmortem per `docs/POSTMORTEM-TEMPLATE.md`.

## Cross-references

- Two-layer quality bar status: `docs/ELITE-QUALITY-GATE.md`
- Per-release error budget: `docs/ERROR-BUDGET-TRACKER-v1.0.0.md`
- Operational runbook: `docs/RUNBOOK.md`
- Rollback procedure: `docs/ROLLBACK-RUNBOOK.md`
- Postmortem template: `docs/POSTMORTEM-TEMPLATE.md`
