<!---
stable: true
type: security-baseline
target: amore v0.5.0
tool: OSSF Scorecard
mode: --local
-->
# OSSF Scorecard baseline — v0.5.0

## Purpose

Document the baseline OSS supply-chain hygiene score for the v0.5.0 release.
This artifact establishes the starting score and tracks remediation toward the
target `Scorecard >= 8.0` set in `SECURITY.md` (v0.9.0 gate).

## Tool

- **OSSF Scorecard** — `gcr.io/openssf/scorecard:stable`
- Version measured: `v5.1.1-45-g40bbc9c9` (commit `40bbc9c9`)
- Source: <https://github.com/ossf/scorecard> (Apache-2.0)

## Mode

`--local=.` (file-system mode) — the repo is private per user mandate; some
Scorecard checks (Branch-Protection, Code-Review, Contributors, Webhooks)
require public-repo API access and report N/A in local mode.

Automated weekly runs activate via `.github/workflows/scorecard.yml` when
the repo flips public on the free GHA public-repo tier.

## Date

2026-05-26T18:29 UTC

## Run command (proof-isolated via Docker)

```
docker run --rm -v "<clean-repo-copy>:/repo" \
  gcr.io/openssf/scorecard:stable \
  --local=/repo --show-details --format=json
```

A clean copy excluding `target/` (Rust build dir, gitignored) is used to avoid
transient build-artifact races during the file walk.

## Score table

| Check | Score | Status | Note |
|---|---:|---|---|
| Binary-Artifacts | 7 | WARN | 3 .exe in `installer/windows/staging/` |
| Dangerous-Workflow | 10 | PASS | no dangerous patterns |
| Dependency-Update-Tool | 0 | FAIL | no `dependabot.yml` (Wave 3 G owner) |
| Fuzzing | 0 | FAIL | no fuzzer integrations |
| License | 9 | PASS | LICENSE present (Apache-2.0); FSF/OSI matcher mis-flag |
| Packaging | N/A | INFO | no publishing workflow detected (-1 score) |
| Pinned-Dependencies | 0 | FAIL | unpinned GH actions + container images |
| SAST | 0 | FAIL | no SARIF SAST tool wired |
| Security-Policy | 10 | PASS | `SECURITY.md` detected |
| Token-Permissions | 0 | FAIL | workflows lack top-level `permissions:` |
| Vulnerabilities | 4 | WARN | 6 RUSTSEC advisories open in Cargo.lock |
| Branch-Protection | N/A | INFO | requires public repo |
| Code-Review | N/A | INFO | requires public repo |
| Contributors | N/A | INFO | requires public repo |
| Webhooks | N/A | INFO | requires public repo |

## Overall score

**4.1 / 10** (average of non-N/A, non-`-1` checks: 7+10+0+0+9+0+0+10+0+4)

## Pass gate

**Pass gate: >=7.0**
**Actual: 4.1**
**Verdict: PARTIAL — below gate; remediation plan documented below.**

The gap is concentrated in five low-hanging metrics (Pinned-Dependencies,
Token-Permissions, SAST, Dependency-Update-Tool, Vulnerabilities). Closing
these would lift the score to ~8.5 without architectural change.

## Remediation plan (per <10 metric)

- **Binary-Artifacts (7 → 10)** — remove 3 `.exe` in `installer/windows/staging/`
  (Wave-2 build outputs, should be gitignored); add `installer/windows/staging/*.exe`
  to `.gitignore`. Target: v0.5.1.
- **Dependency-Update-Tool (0 → 10)** — land `.github/dependabot.yml` for
  Cargo + GH Actions + Docker ecosystems (Wave 3 G owner, in-flight). Target: v0.5.0.
- **Fuzzing (0 → ≤5)** — wire `cargo-fuzz` against canonical-JSON + chain-verify
  codepaths in `crates/amore-core/fuzz/`. Target: v0.7.0 (per THREAT-MODEL milestone).
- **License (9 → 10)** — Apache-2.0 text present and valid; Scorecard's content
  matcher false-negatives a known issue (ossf/scorecard#3567). No remediation.
- **Pinned-Dependencies (0 → 10)** — pin every `uses:` action reference and every
  `FROM` in `Dockerfile.multiarch` by `@sha256:` hash (with version-tag comment).
  `scorecard.yml` already demonstrates the pattern. Adopt dependabot to keep them
  current. Owners: `.github/workflows/*.yml`, `Dockerfile.multiarch`,
  `tests/qa/a4_npm_postinstall_smoke.sh`. Target: v0.5.0.
- **SAST (0 → 10)** — wire `cargo clippy --message-format=json` → `clippy-sarif`
  → `github/codeql-action/upload-sarif`. CodeQL covers the JS portions (npm-package
  + Electron GUI). Owner: `.github/workflows/codeql.yml`. Target: v0.6.0.
- **Token-Permissions (0 → 10)** — add top-level `permissions: read-all` to
  `ci.yml` + `release.yml` (mirror `scorecard.yml`); override per-job for write
  scope only where genuinely needed. Target: v0.5.0 (single-line edit).
- **Vulnerabilities (4 → 10)** — 6 open RUSTSEC advisories at HEAD: RUSTSEC-2025-0057,
  RUSTSEC-2024-0384, RUSTSEC-2026-0002 / GHSA-rhfx-m35p-ff5j, RUSTSEC-2025-0119,
  RUSTSEC-2024-0436, RUSTSEC-2025-0134. Resolve via `cargo update` + `cargo audit`
  re-run. Wave 3 D / E siblings may have addressed some. Target: v0.5.0.

## Public-repo follow-up

When the user flips the repo public:

1. The committed `.github/workflows/scorecard.yml` activates automatically.
2. Weekly cron @ Mon 05:00 UTC re-measures.
3. Results upload to GitHub Security tab as SARIF.
4. Badge at `https://api.scorecard.dev/projects/github.com/<owner>/<repo>/badge`.
5. Re-measure baseline with full-network checks (Branch-Protection,
   Code-Review, Contributors, Webhooks). Expect the overall score to dip
   transiently until branch-protection rules are applied, then rise above
   the `>=8.0` target by v0.9.0.

## Re-run instructions

For the local audit at any future tag:

```powershell
$src  = "<repo-root>"
$dst  = "$env:TEMP\amore-scorecard-clean"
robocopy $src $dst /MIR /XD target node_modules /XF *.lock | Out-Null
robocopy "$src\.git" "$dst\.git" /MIR | Out-Null
Copy-Item -Path "$src\Cargo.lock" -Destination "$dst\Cargo.lock"
docker run --rm -v "${dst}:/repo" gcr.io/openssf/scorecard:stable `
  --local=/repo --show-details --format=json
```
