<!-- stable: true -->
# OSSF Scorecard — Amore v1.0.2 Re-measure

**Date:** 2026-05-27
**Scorecard version:** v5.5.0 (C:/Users/anto/go/bin/scorecard.exe)
**Baseline reference:** 3.0/10 at v1.0.0 (docs/SCORECARD-v1.0.0.md)
**Repo commit scored:** 61b992bfd9aaaf47fff977c933e2fef7c051390a
**Bigtech-grade threshold:** ≥ 7.5/10

---

## Aggregate Score

**3.8 / 10** (threshold ≥ 7.5; prior baseline 3.0 at v1.0.0; delta **+0.8**)

Threshold status: **BELOW** (−3.7 points to threshold).

Delta vs v1.0.0: CII-Best-Practices moved from 0 to 2 (InProgress badge added);
Pinned-Dependencies moved from 2 to 7 (Docker image pinning partially applied);
Signed-Releases moved from -1 to 8 (first GitHub release with cosign artifact exists);
Contributors moved from 3 to 3 (unchanged). Vulnerabilities held at 0/10 (14 OSV advisories
open; Cargo.toml major-version bumps required, blocked — see SCORECARD-v1.0.0.md §W10).

---

## Per-Check Score Table

| Check | Score | Delta vs v1.0.0 | Reason | Amore artifact |
|---|---|---|---|---|
| Binary-Artifacts | 10/10 | 0 | No binaries in repo | — (pass by absence) |
| Security-Policy | 10/10 | 0 | SECURITY.md detected | [SECURITY.md](../SECURITY.md) |
| Fuzzing | 10/10 | 0 | cargo-fuzz targets detected | [crates/amore-core/fuzz/](../crates/amore-core/fuzz/) |
| License | 10/10 | 0 | Apache-2.0 detected (FSF/OSI) | [LICENSE](../LICENSE) |
| Signed-Releases | 8/10 | +9 | 1/1 releases have 1 signed artifact | release.yml + cosign |
| Pinned-Dependencies | 7/10 | +5 | Partial pinning applied | [Dockerfile.multiarch](../Dockerfile.multiarch) |
| CII-Best-Practices | 2/10 | +2 | Badge InProgress detected | bestpractices.coreinfrastructure.org |
| Contributors | 3/10 | 0 | 1 contributing org | — |
| Code-Review | 0/10 | 0 | 0/25 approved changesets (solo-author) | — |
| Branch-Protection | 0/10 | 0 | Branch protection not enabled on main | GitHub repo settings |
| Dependency-Update-Tool | 0/10 | 0 | No update tool detected (PRs required) | [.github/dependabot.yml](../.github/dependabot.yml) |
| SAST | 0/10 | 0 | No SAST tool detected (push trigger needed) | [.github/workflows/codeql.yml](../.github/workflows/codeql.yml) |
| Maintained | 0/10 | 0 | Repo < 90 days old (temporal, auto-resolves) | — |
| Vulnerabilities | 0/10 | 0 | 14 OSV advisories open in Cargo.lock | [Cargo.lock](../Cargo.lock) |
| CI-Tests | -1 (N/A) | 0 | No pull requests found | [.github/workflows/ci.yml](../.github/workflows/ci.yml) |
| Dangerous-Workflow | -1 (N/A) | 0 | No push-triggered workflows | .github/workflows/ |
| Packaging | -1 (N/A) | 0 | No GitHub/GitLab publishing workflow detected | [.github/workflows/release.yml](../.github/workflows/release.yml) |
| Token-Permissions | -1 (N/A) | 0 | No tokens found (no push-triggered workflows) | `permissions: read-all` in ci.yml |

Checks returning -1 are N/A (not scored, not penalizing the aggregate).

---

## Progress Since v1.0.0

| Improvement | Old | New | How |
|---|---|---|---|
| Signed-Releases | -1 (N/A) | 8/10 | First GitHub release created with cosign-signed artifact |
| Pinned-Dependencies | 2/10 | 7/10 | Docker image SHA pins applied in Dockerfile.multiarch |
| CII-Best-Practices | 0/10 | 2/10 | OpenSSF Best Practices application submitted (InProgress) |

---

## Remaining Gap to ≥ 7.5 Threshold

Current: 3.8. Gap: 3.7 points. Achievable remediations (estimated contribution):

| Remediation | Est. impact | Effort |
|---|---|---|
| Fix 14 OSV vulnerabilities (Cargo.toml major bumps) | +2–3 pts | High — requires eframe/tantivy/reqwest/tonic upgrades |
| Enable branch protection on main | +1 pt | Low — GitHub settings, 1 click |
| Add push trigger to CodeQL + CI workflows | +1 pt | Low — 2-line yml change |
| Merge ≥1 dependabot PR | +0.5 pt | Medium |
| Complete CII-Best-Practices badge | +0.5 pt | Medium — self-assessment |
| Use PRs instead of direct push (Code-Review) | +0.5 pt | Process change |

Fixing Vulnerabilities + Branch-Protection + SAST push trigger alone projects to ~5.8–6.5/10.
Reaching ≥ 7.5 additionally requires: CII badge complete + dependabot PRs + Code-Review.

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
