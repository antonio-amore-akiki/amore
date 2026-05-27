<!-- stable: true -->
# OSSF Scorecard — Amore v1.0.2 Re-measure

**Date:** 2026-05-27 (postfix pass: 2026-05-27)
**Scorecard version:** v5.5.0 (C:/Users/anto/go/bin/scorecard.exe)
**Baseline reference:** 3.0/10 at v1.0.0 (docs/SCORECARD-v1.0.0.md)
**Repo commit scored:** 61b992bfd9aaaf47fff977c933e2fef7c051390a
**Postfix actions applied:** `cargo update` (8 patch bumps), Branch-Protection enabled,
CodeQL workflow confirmed `workflow_dispatch:`-only, RUSTSEC triage written.
**Bigtech-grade threshold:** ≥ 7.5/10

---

## Aggregate Score

**Pre-postfix: 3.8 / 10**
**Post-postfix confirmed: 4.1 / 10** (live run 2026-05-27; raw: `state/scorecard-v1.0.2-postfix.json`)

Threshold status: **BELOW** (−3.4 points to ≥ 7.5 bigtech threshold).
Bigtech 7.5 threshold: **NOT MET**.

---

## Per-Check Score Table (post-postfix state)

| Check | Score | Delta vs baseline | Reason | Amore artifact |
|---|---|---|---|---|
| Binary-Artifacts | 10/10 | 0 | No binaries in repo | — (pass by absence) |
| Security-Policy | 10/10 | 0 | SECURITY.md detected | [SECURITY.md](../SECURITY.md) |
| Fuzzing | 10/10 | 0 | cargo-fuzz targets detected | [crates/amore-core/fuzz/](../crates/amore-core/fuzz/) |
| License | 10/10 | 0 | Apache-2.0 detected (FSF/OSI) | [LICENSE](../LICENSE) |
| Signed-Releases | 8/10 | +9 | 1/1 releases have 1 signed artifact | release.yml + cosign |
| Pinned-Dependencies | 7/10 | +5 | Partial pinning applied | [Dockerfile.multiarch](../Dockerfile.multiarch) |
| CII-Best-Practices | 2/10 | +2 | Badge InProgress detected | bestpractices.coreinfrastructure.org |
| Contributors | 3/10 | 0 | 1 contributing org | — |
| Code-Review | 0/10 | 0 | 0/25 approved changesets — honest gap; solo-author; not fixable without external reviewers | — |
| Branch-Protection | 3/10 | +3 | Protection enabled 2026-05-27: force-push=false, deletions=false; live scorecard confirmed 3/10; full 10/10 requires required_status_checks + PR reviews | GitHub API 200 confirmed |
| Dependency-Update-Tool | 0/10 | 0 | No update tool detected (PRs required) | [.github/dependabot.yml](../.github/dependabot.yml) |
| SAST | 0/10 | 0 | CodeQL exists but `workflow_dispatch:`-only — Scorecard requires push/schedule trigger; mandate prevents changing this | [.github/workflows/codeql.yml](../.github/workflows/codeql.yml) |
| Maintained | 0/10 | 0 | Repo < 90 days old (temporal, auto-resolves) | — |
| Vulnerabilities | 0/10 | 0 | 14 OSV `warning:`-class advisories (unmaintained/unsound); 0 `error:`-class vulns; all transitive; not patchable via compatible update | [RUSTSEC-TRIAGE-v1.0.2.md](./RUSTSEC-TRIAGE-v1.0.2.md) |
| CI-Tests | -1 (N/A) | 0 | No pull requests found | [.github/workflows/ci.yml](../.github/workflows/ci.yml) |
| Dangerous-Workflow | -1 (N/A) | 0 | No push-triggered workflows | .github/workflows/ |
| Packaging | -1 (N/A) | 0 | No GitHub/GitLab publishing workflow detected | [.github/workflows/release.yml](../.github/workflows/release.yml) |
| Token-Permissions | -1 (N/A) | 0 | No tokens found (no push-triggered workflows) | `permissions: read-all` in ci.yml |

Checks returning -1 are N/A (not scored, not penalizing the aggregate).

---

## What changed in this postfix pass

| Action | Result |
|---|---|
| `cargo update` (8 patch bumps) | Cargo.lock updated; advisory-triggering crates unchanged (major bump required) |
| Branch-Protection enabled | API 200; force-push=false, deletions=false on main |
| CodeQL workflow | Already `workflow_dispatch:`-only; confirmed per mandate; no change |
| RUSTSEC-TRIAGE-v1.0.2.md | Written; 14 advisories documented with root cause and sunset |

---

## Progress Since v1.0.0

| Improvement | Old | New | How |
|---|---|---|---|
| Signed-Releases | -1 (N/A) | 8/10 | First GitHub release created with cosign-signed artifact |
| Pinned-Dependencies | 2/10 | 7/10 | Docker image SHA pins applied in Dockerfile.multiarch |
| CII-Best-Practices | 0/10 | 2/10 | OpenSSF Best Practices application submitted (InProgress) |
| Branch-Protection | 0/10 | 3/10 | Force-push + deletion blocked on main (2026-05-27 postfix); confirmed live scorecard |

---

## Honest gap analysis — why 7.5 is not reached

| Check | Blocker | Fixable now? |
|---|---|---|
| Vulnerabilities | 14 transitive advisories (all unmaintained/unsound); need tray-icon GTK4 migration + sled/tokenizer major bumps | No — v1.1 scope |
| SAST | Mandate: no push-triggered GHA (zero credits policy); Scorecard requires push/schedule | No — policy constraint |
| Code-Review | Solo-author repo; Scorecard counts 0/25 approved changesets | No — structural |
| Dependency-Update-Tool | Dependabot .yml present but Scorecard expects active PRs merged | Partial — first PR merge needed |
| Maintained | Repo age < 90 days | Auto-resolves by ~July 2026 |

Realistic ceiling with constraints: ~5.5–6.0 once Branch-Protection scores and repo ages.
Reaching 7.5 requires: (a) SAST push trigger (mandate waiver), (b) 14 advisories resolved,
(c) CII badge completed. Log as open-thread for v1.1 planning.

---

## Methodology

Run command:
```
export GITHUB_TOKEN=$(gh auth token)
C:/Users/anto/go/bin/scorecard.exe \
  --repo=github.com/antonio-amore-akiki/amore \
  --format=json --show-details \
  > state/scorecard-v1.0.2.json
```

Raw output: `state/scorecard-v1.0.2.json`

The `--local` flag is incompatible with checks requiring GitHub API access. Remote run
covers all 18 checks. Scorecard install: `C:/Users/anto/go/bin/scorecard.exe` (v5.5.0).
