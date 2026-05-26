---
stable: true
type: security-review-re-review
target: amore v0.3.1-live-fire
commit: 7f4594a
prior_review: docs/SECURITY-REVIEW-v0.3.0-live-fire.md
date: 2026-05-26
---

# Amore v0.3.1-live-fire — Security Re-Review

**Reviewer:** security-reviewer subagent (Claude Opus 4.7)
**Date:** 2026-05-26
**Commit reviewed:** `7f4594a` (HEAD of `master`, tag `v0.3.1-live-fire`)
**Prior verdict:** NO-GO at `eb98034` / v0.3.0-live-fire
**Mode:** read-only host audit; static diff `eb98034..7f4594a`; `cargo audit` against the
new `Cargo.lock`; live `Get-ScheduledTask` query for nightly registration
**Question:** Are the 2 Criticals + 3 Majors closed cleanly enough to move from NO-GO to
GO-WITH-CONDITIONS?

## Executive summary

All 5 prior-blocking findings are closed cleanly. The fix discipline is high: each
fix lands at the exact boundary the prior report named (download stream, MCP tool
boundary, child-spawn site, release workflow, postinstall). `cargo audit` against
the post-fix `Cargo.lock` is identical to v0.3.0 (0 vulns + 2 unmaintained
informational warnings). One **Major** is introduced by the fix sprint:
the cosign on-demand download in `postinstall.js` is TOFU (no hash/sig pinning of
the cosign binary itself), reproducing finding 10a one layer down. This is
acceptable for v0.3.1 friends/family preview but must be closed before the
public 100M-user ship (condition C-1 below).

**Verdict: GO-WITH-CONDITIONS for v0.3.1-live-fire friends/family preview.**

---

## Critical 10a — Ollama installer SHA-256 verify-before-exec

`verdict: CLOSED` · `file: crates/amore-gui/src/install.rs:30-31, 113-158, 161` ·
`residual-risk: none for v0.3.1; hash-pin maintenance burden each Ollama bump (operational, not security)`

**Pinned constant** (`install.rs:30-31`):
```
const OLLAMA_INSTALLER_SHA256: &str =
    "38ef4715a31b6fede8f37be840c5e1e1524150d2c637d1acca94227980daf300";
```
Matches the value named in the agent prompt. Comment block (`install.rs:25-29`)
documents source URL + verification command + bump procedure.

**Hasher is incremental** (`install.rs:116, 136`): `let mut hasher = Sha256::new()`
allocated before the read loop; `hasher.update(&buf[..n])` inside the same loop that
writes to disk. Single-pass over the download stream — no second open(), no
re-read.

**Fail-closed before exec** (`install.rs:148-157`): the mismatch path returns `None`
from `download()`. The caller in `run(...)` at `install.rs:60-75` matches:
```rust
let temp_path = download(&client, status.clone(), &ctx)?;  // ? on None → early return
set(DepStatus::Installing);
if !run_installer(&temp_path, status.clone(), &ctx) { return; }
```
On hash mismatch `download()` returns `None`, the `?` propagates, and `run_installer`
is **never called**. Verified by reading lines 161-186 (`run_installer` definition)
and confirming no other call site exists in the file. Control flow is provably
fail-closed.

**Residual:** the operator must update `OLLAMA_INSTALLER_SHA256` whenever Ollama
ships a new installer. Doc-comment at `install.rs:26` names the procedure. Not a
security risk — a CD task.

---

## Critical 10b — npm Sigstore mandatory + all-3-OS bundles

`verdict: CLOSED with one Major regression (C-1)` ·
`files: npm/postinstall.js:279-353, .github/workflows/release.yml:116-126, 169-172` ·
`residual-risk: cosign download TOFU — see Regression risk section`

**a) Old `verifySigstoreIfAvailable` gone.** `git diff` shows the entire
opt-in/silent-skip helper deleted (`postinstall.js:237-258` in `eb98034`); no
references remain in the file. Replaced by `verifySigstore()` at lines 310-353.

**b) New `verifySigstore` is fail-closed.** Logic at `postinstall.js:310-352`:
- `bundleStatus === 404` → `throw Error("Refusing to install an unverified binary")`.
- `bundleStatus !== 200` → `throw Error("Cannot verify integrity — aborting (fail-closed)")`.
- `bundleStatus === 200` → `await resolveCosign()` (throws if unobtainable) →
  `cosign verify-blob`; non-zero status → `throw Error("Sigstore verification FAILED")`.
There is no silent-skip path. Bundle existence is the gate; `httpHead` probe at
`postinstall.js:367` determines it before any decision.

**c) cosign auto-install path** (`postinstall.js:281-308`):
- Path concat uses `path.join(os.homedir(), ".amore-cache", ...)` — no user-input
  on the path; `key.replace(":", "-")` is the only dynamic segment and `key` is a
  closed enum from `PLATFORM_TARGETS`. No traversal.
- Self-test (`spawnSync(cacheBin, ["version"])` at line 303) confirms the
  downloaded binary actually runs before it's returned.
- `fs.unlinkSync(cacheBin)` at line 292 cleans corrupted prior caches.
**But:** the binary is downloaded with no SHA / cosign-of-cosign verification. See
Regression risk C-1.

**d) Escape hatch is loud** (`postinstall.js:312-320`): when `AMORE_NPM_SKIP_SIGSTORE=1`
is set, a multi-line ASCII-bordered warning is written to **stderr** with "Sigstore
verification SKIPPED" and "UNVERIFIED binary". Emitted on every install (no
suppression flag). Stderr satisfies the "loud" criterion.

**e) `release.yml` matrix guard removed.** Diff shows lines 121 (`if: matrix.target
== 'x86_64-unknown-linux-gnu'`) and 178 deleted from both the `Install cosign` and
`Sigstore sign artifact` steps. The new `Install cosign` and `Sigstore sign
artifact` blocks (`release.yml:116-126`) have no `if:` guard.

**f) Bundle upload applies to all 3 OS.** Diff at `release.yml:169-172` deletes the
Linux-only guard from the bundle upload step. Combined with `permissions:
id-token: write` at the workflow level (line 40) and `cosign-installer@v3.7.0`
working on all three runners, this produces `.bundle` artifacts for
`x86_64-unknown-linux-gnu`, `x86_64-apple-darwin`, and `x86_64-pc-windows-msvc`.

---

## Major 11a — absolute-path child spawn

`verdict: CLOSED` · `file: crates/amore-gui/src/main.rs:227-238, 245` ·
`residual-risk: very minor — `current_exe()` failure path falls back to bare "amore"`

**Resolve via current_exe().** Code at `main.rs:227-238`:
```rust
let cli_path = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|d| d.join(if cfg!(windows) { "amore.exe" } else { "amore" })))
    .unwrap_or_else(|| std::path::PathBuf::from("amore"));
```
Then `Command::new(&cli_path)` at line 245 replaces the old `Command::new("amore")`.
The Windows CWD-first `CreateProcess` path-search vector named in finding 11a is
eliminated when `current_exe()` succeeds (the normal path under both `amore.exe`
installed in `Program Files\Amore\` and a `cargo run` lane).

**Fallback is the residual.** When `current_exe()` returns `Err` (rare — Windows-API
failure, e.g. extremely long path on a system without long-path support), the
fallback `PathBuf::from("amore")` re-introduces the bare-name PATH lookup.
However: (i) `current_exe()` failure is itself an anomalous condition; (ii) by that
point the user has launched the GUI installer, so they're sitting at a desktop, not
SSHd in; (iii) the threat-model attacker still needs `amore.exe` write access in
the CWD. The fallback is graceful (doesn't panic) and is documented by the
comment block at `main.rs:227-231`. Acceptable for v0.3.1; condition C-2 names a
hardening for v0.4.0.

---

## Major 6a — MCP recall input bounds

`verdict: CLOSED` · `files: crates/amore-mcp/src/main.rs:88-101, 159-172; crates/amore-mcp/tests/mcp_handshake.rs:155-376` ·
`residual-risk: none for v0.3.1; regression tests `#[ignore]` until live-daemon CI lane lands`

**Constants** (`main.rs:99-100`):
```rust
const MAX_TOP_K: usize = 100;
const MAX_QUERY_BYTES: usize = 16 * 1024; // 16 KiB
```

**Validation at the MCP tool boundary** (`main.rs:159-172`). The bounds check is
the **first statement** in the `recall(...)` handler body, before `self.recall.search`
is reached:
```rust
async fn recall(&self, Parameters(params): Parameters<RecallParams>) -> ... {
    if params.query.len() > MAX_QUERY_BYTES {
        return Err(McpError::invalid_params(format!("query exceeds {MAX_QUERY_BYTES} bytes (got {})", params.query.len()), None));
    }
    let top_k = params.top_k.clamp(1, MAX_TOP_K);
    let envelope = self.recall.search(&params.query, top_k).await...
```
The rejection fires **before** `params.query` is borrowed for the Ollama embed
call. `.len()` on a `&String` is O(1), not consuming, so the borrow check is
trivial. `top_k.clamp(1, MAX_TOP_K)` defangs both `usize::MAX` overflow and
`0` corner-cases (replaces with 100 or 1 respectively).

**Regression tests are well-formed** (`mcp_handshake.rs:178-376`):
- `recall_rejects_oversized_top_k` (line 218-291): sends `usize::MAX`; asserts
  process is still alive and response is a valid JSON-RPC object (allows either
  result-after-clamp or error, but forbids crash).
- `recall_rejects_oversized_query` (line 296-376): sends 17 KiB; asserts an
  `"error"` key is present and the message mentions the byte limit.
- Both `#[ignore = "requires AMORE_TEST_MCP=1 + live Qdrant + Ollama"]` — same
  ignore policy as the existing `handshake_lists_recall_tool` integration test
  (`mcp_handshake.rs:152`). Consistent with the v0.3.0 baseline that has no live-
  daemon CI lane.

---

## Major hygiene — nightly cargo-audit baseline

`verdict: CLOSED` · `files: scripts/security-baseline.ps1, deny.toml` ·
`residual-risk: script is not self-registering; user-owned schtasks step required (verified present)`

**`scripts/security-baseline.ps1` exists** (135 LOC) and runs `cargo audit --json`
(phase 1, lines 53-76), `cargo deny check` (phase 2, lines 79-83), and `cargo
geiger` (phase 3, lines 86-99). Writes `%LOCALAPPDATA%\Amore\security-baselines\<date>.json`
with `{date, workspace, audit_exit, deny_exit, geiger_exit, vulnerabilities,
vuln_count, deny_errors, unsafe_exprs, gate_fail}`. Sends `ntfy` alert on
gate-fail (lines 130-132) — reuses the existing `~/.claude/state/ntfy.log` URL,
silent-skips if absent (acceptable; this is a private maintainer signal, not a
user-visible gate).

**`deny.toml` exists** with the documented allow-list at root: `Apache-2.0`, `MIT`,
`BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Unicode-DFS-2016`, `Unicode-3.0`, `CC0-1.0`,
`MPL-2.0`, `CDLA-Permissive-2.0`, `OpenSSL`, `Zlib` (lines 14-26). `yanked = "deny"`,
`unknown-registry = "deny"`, `unknown-git = "deny"`, `wildcards = "deny"`. The
allow-list matches the agent-prompt list; `confidence-threshold = 0.93` is correct
cargo-deny default for license detection.

**Task Scheduler registration verified.** `Get-ScheduledTask -TaskName
'Amore-Security-Baseline-Nightly'` returned `State: Ready, TaskPath: \`. The
script does NOT register itself (no `Register-ScheduledTask` block in the file —
this is **inconsistent with the commit message** which claims `Task
"Amore-Security-Baseline-Nightly" registered`; the registration was done out-of-
band by the user). Functionally fine because the task IS live, but a future
maintainer cloning the repo onto a fresh machine will have to register manually.
Condition C-3 names this as a v0.4.0 follow-up.

**`cargo audit` against the new `Cargo.lock`** (run from host, 2026-05-26): 0
vulnerabilities, 2 unmaintained warnings (`paste 1.0.15` RUSTSEC-2024-0436,
`rustls-pemfile 2.2.0` RUSTSEC-2025-0134). Identical to the v0.3.0 baseline — no
new transitive RUSTSEC ID introduced by the `sha2 + hex` additions to `amore-gui`.

---

## Regression risk — new concerns introduced by the fix sprint

**C-1 (Major, condition for v0.4.0):** `postinstall.js:301` downloads cosign from
`github.com/sigstore/cosign/releases/latest/download/cosign-<platform>` over HTTPS
**without verifying cosign's own signature or hash**. This is TOFU on the cosign
binary — and `latest` (not a pinned version), so a tag-move by sigstore is
silently honored. Effectively, the trust root for `verifySigstore` is "GitHub
TLS to sigstore/cosign". Mitigation per the analogous finding-10a model:
**pin `COSIGN_VERSION` + a SHA-256 per platform**, fetch from `releases/download/v<ver>/`,
verify the hash, then exec. ~10 LOC, no new deps (Node's `crypto.createHash` is
core). Should land before v0.4.0; non-blocking for v0.3.1 friends/family because
the realistic attack requires either compromising `github.com/sigstore` or
mounting an active GitHub-MITM, both far outside the threat model named in the
prior report (single stolen laptop). For 100M-user public ship, it must close.

**Not a regression — `current_exe()` fallback to bare "amore"** (`main.rs:238`):
the `unwrap_or_else(|| PathBuf::from("amore"))` path is only reachable when the
Windows API itself can't tell us our own exe path. In that anomalous case the
old PATH-injection vector returns. But this is a graceful default, not a typical
runtime path, and panicking would be a worse UX for the user-facing GUI
installer. Documented by the inline comment. **Accepted**, but flagged as C-2
to track for a `.expect("current_exe must succeed for security-sensitive
spawn")` panic upgrade in v0.4.0 once telemetry confirms zero real users hit the
fallback.

**Not a regression — `sha2 + hex` additions** (`amore-gui/Cargo.toml:28-30`): both
are already in `Cargo.toml [workspace.dependencies]` (lines 51, 52), used by
`amore-core` for the provenance crypto. Adding them to `amore-gui` resolves to
the same crate-graph node — zero new transitive deps. `cargo audit` against the
new lockfile confirms this: dep tree count unchanged at 582-equivalent class.
Both crates are top-tier RustCrypto (audited, widely used). **No new attack
surface.**

**Not a regression — bounds check ordering** (`amore-mcp/src/main.rs:160`): the
rejection branch fires on `params.query.len() > MAX_QUERY_BYTES`. Since `len()`
is `O(1)` on `String` (cached len field) and doesn't consume the borrow,
`params.query` is fully available at the subsequent `self.recall.search(&params.query, ...)`
call. **No use-after-move, no late-rejection issue.** The check is applied
BEFORE `params.query` is consumed by anything but the early-return path.

---

## Verdict

**GO-WITH-CONDITIONS for v0.3.1-live-fire friends/family preview.**

All 5 prior blocking findings (2 Critical + 3 Major) are closed at the exact
boundary the prior report named. One new Major (C-1, cosign TOFU) is introduced;
it is acceptable for the friends/family preview but blocks the public 100M-user
ship.

### Explicit conditions for the GO

- **C-1 (must close before v0.4.0 public ship):** pin a cosign version + SHA-256
  per platform in `postinstall.js`; verify before exec. Eliminates the TOFU
  trust-root for the supply-chain verifier itself. Source: this report
  Regression risk section.
- **C-2 (track for v0.4.0):** harden `current_exe()` fallback in
  `crates/amore-gui/src/main.rs:238` — either panic (security-critical spawn) or
  log + abort. Acceptable to leave for v0.4.0 because failure is an OS-level
  anomaly, not a typical install path.
- **C-3 (track for v0.4.0):** add a self-registering block to
  `scripts/security-baseline.ps1` (or a sibling `install-task.ps1`) so a fresh
  clone gets the nightly task without manual `Register-ScheduledTask`. Commit
  message claims the registration is automatic; today it is not.
- **C-4 (deferred to v1.0, blocked_on:user):** EV cert for Windows
  Authenticode + Apple Developer ID for macOS notarization. `$300-500/yr + $99/yr`.
  Documented at `release.yml:13-16` (S10c). Until certs land, SmartScreen
  warning on Windows + Gatekeeper warning on macOS are expected on first run.
- **C-5 (deferred to v0.4.0):** the two new MCP regression tests in
  `mcp_handshake.rs:218-376` are `#[ignore]` until CI gains an Ollama + Qdrant
  docker harness. This matches the existing handshake test's `#[ignore]` policy
  so it is not a new gap — but the harness must land before v1.0 for any kind
  of public-launch confidence.

### What remains exactly as v0.3.0

- Provenance crypto (textbook-correct).
- BM25 sanitization + parameterization.
- Atomic-rename file writes.
- Zero unsafe in production code.
- Zero telemetry / 0.0.0.0 bind / hardcoded secrets.
- `cargo audit`: 0 vulnerabilities + 2 unmaintained-informational. Unchanged.

---

## Reproduction notes

- All verdicts derived from static-diff review of `eb98034..7f4594a` and full-file
  reads of the post-fix versions. No exploit code executed; no `container_use`
  needed per the scope brief.
- `cargo audit` run from host against `Cargo.lock` at `7f4594a`: 0 vulns, 2
  unmaintained-warnings (identical to v0.3.0). Advisory DB unchanged from prior
  review.
- `Get-ScheduledTask -TaskName 'Amore-Security-Baseline-Nightly'` returned
  `State: Ready` — confirms the nightly task is live on the maintainer machine
  even though the registration is out-of-band.
