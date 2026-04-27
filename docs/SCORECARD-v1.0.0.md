<!-- stable: true -->
# OSSF Scorecard — Amore v1.0.0 Re-measure

**Date:** 2026-05-27
**Scorecard version:** v5.5.0 (commit c395761)
**Baseline reference:** 4.1/10 at v0.5.0
**Repo commit scored:** 5ed5ed2a6cdc88a0751e439da039d92f5d3fd5e9

---

## Aggregate Score

**3.0 / 10** (target >= 7.5; baseline 4.1 at v0.5.0; delta **-1.1**)

The score decreased from baseline. Root cause: W4–W6 wave deliverables are present in the
repository but scorecard cannot detect them because all CI workflows are set to
`workflow_dispatch` only (no `on: push` triggers). The scorecard checks for SAST, Token-Permissions,
Dangerous-Workflow, CI-Tests, Signed-Releases, and Packaging all require GitHub Actions to have
run on push events. Additionally 15 OSV advisories are open in Cargo.lock.

---

## Per-Check Score Table

| Check | Score | Reason | Amore artifact |
|---|---|---|---|
| Binary-Artifacts | 10/10 | No binaries in repo | — (pass by absence) |
| Security-Policy | 10/10 | SECURITY.md detected with disclosure + timeline | [SECURITY.md](../SECURITY.md) |
| Fuzzing | 10/10 | cargo-fuzz targets detected | [crates/amore-core/fuzz/fuzz_targets/](../crates/amore-core/fuzz/fuzz_targets/) |
| License | 10/10 | Apache-2.0 license detected (FSF/OSI) | [LICENSE](../LICENSE) |
| Contributors | 3/10 | 1 org contributor (tprc) | — |
| Pinned-Dependencies | 2/10 | 2 of 3 container images unpinned; 1 npm cmd unpinned | [Dockerfile.multiarch](../Dockerfile.multiarch) |
| Vulnerabilities | 0/10 | 15 OSV advisories open in Cargo.lock | [Cargo.lock](../Cargo.lock) |
| Maintained | 0/10 | Repo < 90 days old | — (temporal, auto-resolves) |
| Branch-Protection | 0/10 | Branch protection not enabled on main | GitHub repo settings |
| CII-Best-Practices | 0/10 | No OpenSSF Best Practices badge | — |
| Code-Review | 0/10 | 0/30 approved changesets (solo-author repo) | — |
| Dependency-Update-Tool | 0/10 | Dependabot config not detected by scorecard* | [.github/dependabot.yml](../.github/dependabot.yml) |
| SAST | 0/10 | No PRs merged; CodeQL only on workflow_dispatch | [.github/workflows/codeql.yml](../.github/workflows/codeql.yml) |
| CI-Tests | -1 (N/A) | No pull requests found | [.github/workflows/ci.yml](../.github/workflows/ci.yml) |
| Dangerous-Workflow | -1 (N/A) | No workflows detected (all workflow_dispatch) | .github/workflows/ |
| Packaging | -1 (N/A) | No GitHub/GitLab publishing workflow detected | [.github/workflows/release.yml](../.github/workflows/release.yml) |
| Signed-Releases | -1 (N/A) | No GitHub releases found | — |
| Token-Permissions | -1 (N/A) | No tokens found (no push-triggered workflows) | `permissions: read-all` in ci.yml |

*Scorecard v5.5.0 requires dependabot to have open PRs visible via GitHub API; file presence alone
is insufficient for a non-zero score.

---

## W4–W6 Deliverable Gap Analysis

| Wave | Claimed | Scorecard verdict | Gap |
|---|---|---|---|
| W4 Fuzzing | cargo-fuzz canonical_json + mcp_protocol | 10/10 CONFIRMED | None |
| W4 SAST | gitleaks + semgrep + CodeQL | 0/10 | All workflows are workflow_dispatch; no push triggers |
| W4 Pinned-Dependencies | Actions hash-pinned audit | 2/10 | Dockerfile.multiarch + tests/qa/ npm cmd unpinned |
| W5 Signed-Releases | cosign keyless on releases | -1 | No GitHub releases exist on remote |
| W5 Dependency-Update-Tool | dependabot.yml | 0/10 | Scorecard requires API-visible PRs, not just file presence |
| W6 SAST | CodeQL workflow_dispatch | 0/10 | workflow_dispatch not scored; needs push trigger |
| W6 Code-Review | CODEOWNERS | 0/10 | CODEOWNERS present but 0 approved PRs |
| W8 Vulnerabilities | cargo-audit 0 vulns | 0/10 | 15 OSV advisories open per live scan |

---

## Per-Below-Target Remediations

**Vulnerabilities (0/10) — immediate:**
Run `cargo update` for the 15 open advisories and commit the updated Cargo.lock:
RUSTSEC-2024-0370/0384/0412–0413/0415–0416/0418–0420/0429/0436,
RUSTSEC-2025-0057/0119/0134, RUSTSEC-2026-0002.

**Pinned-Dependencies (2/10) — immediate:**
Pin two Docker images in `Dockerfile.multiarch:5` and `Dockerfile.multiarch:11` using the
sha256 digests scorecard reported, and pin the npm command in
`tests/qa/a4_npm_postinstall_smoke.sh:49`.

**Branch-Protection (0/10) — immediate:**
Enable branch protection on `main` via GitHub repo settings or:
`gh api repos/antonio-amore-akiki/amore/branches/main/protection --method PUT`.

**SAST (0/10) — v-next:**
Change `codeql.yml` `on:` from `workflow_dispatch` to `push: branches: [main]`.
Requires accepting GHA minute cost or flipping repo to public.

**Token-Permissions (-1) — v-next:**
ci.yml already has `permissions: read-all`. Score appears once workflows run on push events
(same prerequisite as SAST).

**Signed-Releases (-1) — v-next:**
`gh workflow run release.yml -f version=1.0.0` to create a GitHub release; cosign signing
is wired in release.yml.

**Dependency-Update-Tool (0/10) — v-next:**
Merge at least one dependabot PR to make the tool visible to scorecard's API scan.

**CII-Best-Practices (0/10) — v-next:**
Apply at bestpractices.coreinfrastructure.org (self-assessment, ~1–2 weeks).

**Maintained (0/10) — temporal:**
Auto-resolves 90 days after repo creation. No action needed.

**Code-Review (0/10) — structural:**
Open PRs rather than direct-push to main, even for solo work.

---

## Score Projection (after immediate remediations)

Fixing Vulnerabilities (→7–10) + Pinned-Dependencies (→8) + Branch-Protection (→8) +
SAST push trigger (→7) + Signed-Releases (→7): projected aggregate ~5.5–6.5/10.
Reaching >=7.5 additionally requires: Token-Permissions detection + CII badge +
dependabot PRs + Code-Review PRs.

---

## Methodology

Score computed by running OSSF Scorecard v5.5.0 against the remote GitHub repository:

```
scorecard --repo=github.com/antonio-amore-akiki/amore --format=json --show-details
```

Raw output: `state/w10-scorecard-local.json`

The `--local` flag was attempted first but is incompatible with checks requiring GitHub API
access. The remote run covers all 18 checks. Checks returning -1 are N/A (not scored),
not zero.

To re-run:
```
export GITHUB_TOKEN=$(gh auth token)
scorecard --repo=github.com/antonio-amore-akiki/amore --format=json --show-details \
  > state/w10-scorecard-rerun-$(date +%Y%m%dT%H%MZ).json
```

Scorecard install: `C:/Users/anto/go/bin/scorecard.exe`
(github.com/ossf/scorecard/releases/download/v5.5.0/scorecard_5.5.0_windows_amd64.tar.gz)
Install log: `state/w10-scorecard-install.log`

## W10 cargo update — 2026-05-27

no-op: Cargo.lock unchanged. Vulnerabilities 0/10 → 0/10.
`cargo update` locks 0 of 28 updates (all held by semver constraints).
Clearing needs Cargo.toml major bumps: eframe 0.29→0.30+ (8 advisories),
tantivy 0.22→0.26+ (4), reqwest 0.12→0.13+ (1), tonic 0.12→0.14+ (2).
Audit: vulns=0, unmaintained=13, unsound=2 (unchanged).
State: `state/w10-osv-clear-{pre,post}-cargo-update.json`.
