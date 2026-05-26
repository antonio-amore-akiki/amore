---
stable: true
type: security-review
target: amore v0.3.0-live-fire
commit: eb98034
date: 2026-05-26
---

# Amore v0.3.0-live-fire — Security Review

**Reviewer:** security-reviewer subagent (Claude Opus 4.7)
**Date:** 2026-05-26
**Commit reviewed:** `eb98034` (HEAD at audit time)
**Scope:** workspace `C:\Users\anto\Documents\02- Projects\25 - Obelion\`
**Mode:** read-only host audit; container_use NOT required (no exploit code executed —
all findings are static-code / supply-chain evidence)
**Tools run:** `cargo audit` (1098-advisory DB last-updated 2026-05-23), `cargo license`,
`grep`, source review of every public boundary

## Executive summary

This is a young (~weeks-old), single-maintainer Rust workspace shipping a one-click
installer to non-technical users. The codebase is **structurally sound** — the
provenance crypto uses standard library primitives correctly, the IDE-adapter file
writes are atomic-rename, there is no telemetry, no `0.0.0.0` bind anywhere, no TLS
override, no homebrew encryption, no shell injection, no hardcoded secrets, BM25
input is sanitized + parameterized, and `cargo audit` reports zero RUSTSEC
vulnerabilities. The MCP server is correctly stdio-only by design.

**The blocker for production ship is supply-chain integrity at the installer
boundary.** Both the npm postinstall and the in-app Ollama installer download
executables over HTTPS without **mandatory** checksum/signature verification.
HTTPS-transport-only is not sufficient defense against a GitHub-Release-asset
tamper or an `ollama.com` compromise — and "non-technical user" raises the bar:
the user cannot recover from a poisoned installer.

**Verdict: NO-GO at v0.3.0 for the stated user mandate ("industry grade, 100M
users scalable, highest security practices") UNTIL the two Critical findings land.**
Everything else is Major/Minor and acceptable to defer with explicit conditions
(see Verdict section).

---

## Findings by category

### 1. Secrets in repo

**Clean.** No hardcoded keys, tokens, or credentials. All `GITHUB_TOKEN` / `GH_TOKEN`
/ `AMORE_GITHUB_TOKEN` references are env-var **names** consumed at runtime
(`npm/postinstall.js:60-63`, `tests/qa/a4_npm_postinstall_smoke.sh:40-41`), never
literal values committed to git.

Severity: none.

---

### 2. Supply chain (`cargo audit`)

`cargo audit --json` against the workspace's `Cargo.lock` (582 deps):

- **Vulnerabilities: 0**
- **Unmaintained warnings: 2** (both informational, no exploit)

| severity | crate | advisory | fix |
|---|---|---|---|
| Minor | `paste 1.0.15` | RUSTSEC-2024-0436 (archived 2024-10-07) | Migrate to `pastey` post-v0.5; transitive via `qdrant-client → prost-derive`, no direct dep |
| Minor | `rustls-pemfile 2.2.0` | RUSTSEC-2025-0134 (archived 2025-11-28) | Migrate to `rustls-pki-types::PemObject`; transitive via `reqwest → rustls`, no direct dep |

**Major (defense-in-depth, not advisory-tracked):** CI does NOT run `cargo audit` or
`cargo deny` on push (`.github/workflows/ci.yml:30-62` has only fmt + clippy + test).
Supply-chain drift between releases is not gated.

- severity: Major
- file: `.github/workflows/ci.yml:30-62`
- description: No cargo-audit / cargo-deny job. New RUSTSEC advisories against
  vendored deps are not caught until manual audit.
- exploit path: a `tokio` / `rustls` / `reqwest` advisory could land between
  releases and ship to users.
- fix: add a `audit` job invoking `cargo install --locked cargo-audit && cargo audit`
  on push (free, no extra minutes vs. existing matrix).

---

### 3. License compliance

`cargo license` over the full transitive graph (~580 crates):

- **Apache-2.0 / MIT / BSD only:** 99%+ of crates.
- **MPL-2.0:** 1 crate — `option-ext` (weak copyleft, file-level only; **safe to
  link statically** without copyleft contamination of the binary).
- **MPL-2.0-or-LGPL:** `r-efi` (dual-licensed Apache-2.0 OR MIT OR LGPL-2.1) — we
  consume under Apache-2.0/MIT, no LGPL contamination.
- **`Custom License File`:** `ollama-rs 0.3.4` — manually inspected
  (`~/.cargo/registry/src/.../ollama-rs-0.3.4/LICENSE.md`): straight **MIT**.
  cargo-license heuristics couldn't parse the markdown variant.
- **CDLA-Permissive-2.0:** `webpki-roots` — permissive, fine.
- **No GPL / AGPL / SSPL / Commons-Clause / Elastic / BUSL contamination.**

Severity: none. Action: pin `webpki-roots` license fact in `NOTICE` if not
already there (verify post-review).

---

### 4. Unsafe Rust

4 occurrences total; 2 are in `#[cfg(test)]` modules, 2 are env-var mutation in
prod test path:

| file | lines | context | verdict |
|---|---|---|---|
| `crates/amore-adapter-codex/src/lib.rs:143,153` | `unsafe { std::env::set_var/remove_var }` | Inside `#[test] mod tests {}` — the `CODEX_HOME` test | OK — has SAFETY comment lines 140-142 explaining single-thread invariant |
| `crates/amore-core/tests/recall_degraded.rs:189,202` | `unsafe { std::env::set_var/remove_var("AMORE_TIMEOUT_MS") }` | Inside integration test file | OK — same Rust 2024 env-mutation pattern |

**Production code has zero `unsafe` blocks.** This is excellent for a Rust workspace
of this size.

Severity: none.

---

### 5. `unwrap` / `expect` / `panic!` in production paths

Per-file counts on hot prod modules (excluding `/tests/` + `#[cfg(test)]`):

| file | count | verdict |
|---|---|---|
| `crates/amore-mcp/src/main.rs` | 0 | excellent |
| `crates/amore-cli/src/main.rs` | 1 | acceptable |
| `crates/amore-core/src/ollama.rs` | 1 (line 69: `"reqwest client build (infallible defaults)"`) | acceptable — documented infallible |
| `crates/amore-core/src/provenance.rs` | 8 — but all in `#[cfg(test)] mod tests` after line 111 | none in prod |
| `crates/amore-core/src/sqlite_store.rs` | 7 in prod (`conn.lock().unwrap()` only) | acceptable — `Mutex` poison panic is idiomatic |
| `crates/amore-gui/src/main.rs` | 8 (all `state.lock().unwrap()`) | acceptable — same poison pattern |
| `crates/amore-gui/src/install.rs` | 3 (all `status.lock().unwrap()`) | acceptable |

`panic!()` / `unreachable!()` / `todo!()` in **production** code: zero (all matches
are in `tests/` files).

**Minor:** `crates/amore-core/src/provenance.rs:99-108` — `gen_id()` uses
`SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()`. If clock is
before epoch (impossible on Win/Linux post-1970), `nanos = 0` and successive
observations get the SAME id `"obs-0"`. SQLite PRIMARY KEY would reject the
second, surfacing as `Err` — non-fatal but worth a note. Hypothesis, not exploit.

Severity: Minor only.

---

### 6. Input validation at public boundaries

#### 6a. MCP `recall` tool — unbounded `top_k` and `query`

- severity: **Major**
- file: `crates/amore-mcp/src/main.rs:88-100`, `crates/amore-core/src/recall.rs:117-118`
- description: `RecallParams.top_k: usize` has no upper bound. `recall.search()`
  immediately does `let fetch = (top_k * 4).max(top_k) as u64`. In release profile
  there are no overflow checks (`Cargo.toml:75-79` does not enable
  `overflow-checks`), so a malicious caller passing `top_k = usize::MAX / 3` would
  wrap to a small number (benign) but `top_k = usize::MAX / 4` would wrap on the
  `* 4` and propagate a junk `u64` to Qdrant — non-deterministic resource cost
  on Qdrant. Similarly, `query: String` is unbounded — a multi-megabyte query
  embeds via Ollama (no caller-side length cap), causing memory pressure +
  long-running embed.
- exploit path: hypothesis. Local-only MCP via stdio means the threat model is
  "a malicious IDE extension OR a poisoned IDE adapter config". Not a remote
  RCE, but a non-technical-user can be DoS'd into "Amore freezes my machine"
  by a malicious MCP config that hits the tool with a huge top_k.
- fix: clamp `top_k` to a documented max (e.g. 100) in the MCP handler;
  reject queries longer than e.g. 16 KiB with a typed `McpError::invalid_params`.

#### 6b. `AMORE_DATA_DIR` env var — no path normalization

- severity: Minor
- file: `crates/amore-mcp/src/main.rs:286-295`
- description: `resolve_sqlite_path()` accepts any path from the env var verbatim
  (no `canonicalize()`, no `Path::starts_with` against allowed roots). A user can
  set `AMORE_DATA_DIR=C:\Windows\System32` and Amore will try to write `amore.db`
  there. On Windows non-admin install this fails-closed (no write perms), so
  the issue is user-self-inflicted, not adversarial. Same hazard in
  `resolve_docs_paths()` (line 350-367) — but those paths are only **read**, so
  worst case is reading a file the user could already read.
- exploit path: none in the stolen-laptop threat model.
- fix: optional — log the resolved canonical path and refuse non-existent
  parents that ascend above $HOME. Not blocking.

#### 6c. npm postinstall — controlled inputs, but URL composition trusts env

- file: `npm/postinstall.js:60-66, 161-176`
- description: `resolveToken()` consumes env vars without length/format check.
  `Authorization: Bearer ${token}` is sent to `api.github.com`. A malicious env
  with embedded `\r\n` could attempt header injection — but Node's `https.request`
  rejects CRLF in header values since Node 10, so this is **mitigated by
  runtime**. Worth noting for the threat model record.
- severity: Minor (mitigated by runtime).

---

### 7. Output sanitization / log injection

`grep` for `tracing::*!(` patterns logging `query`, `payload`, or `body`:
**zero matches.** Logs only emit URLs, paths, env-var names, and bounded enum
strings. No PII in logs. No newline-injection vector found.

Severity: none.

---

### 8. Authentication / authorization (MCP transport)

- severity: none
- evidence: `crates/amore-mcp/src/main.rs:259` — `server.serve(stdio()).await`.
  No TCP listener. Grep for `bind(` / `0.0.0.0` / `TcpListener` workspace-wide:
  **zero matches**.
- the only HTTP servers in the data plane are external (Ollama on `127.0.0.1:11434`,
  Qdrant on `127.0.0.1:6334/6333`) — clients only, no listeners.

The stated planned gRPC mode (per scope note "should be Unix-socket default") is
**not yet implemented** so no listener exists to bind incorrectly. Good defensive
posture: keep MCP stdio-only through v1.0; if/when gRPC mode lands, it MUST
default to Unix socket / Windows named pipe and refuse `--listen 0.0.0.0` without
an explicit `--allow-network` flag.

---

### 9. Network operations / TLS

- severity: none on the Rust side.
- evidence: `Cargo.toml:61` configures
  `reqwest = { default-features = false, features = ["json", "rustls-tls"] }`
  workspace-wide — **rustls backend, no native-tls bypass risk on Windows
  schannel quirks**. Grep for `accept_invalid_certs` / `danger_accept` /
  `allow_http`: zero matches.
- every URL constructed in code is `https://` (`crates/amore-gui/src/install.rs:18`
  hardcodes `https://ollama.com/...`, `npm/postinstall.js:99-104` hits
  `https://api.github.com`, `npm/postinstall.js:174` hits
  `https://github.com/.../releases/download/...`).
- `127.0.0.1:11434` (Ollama) and `127.0.0.1:6334` (Qdrant) are http:// — this is
  **correct** for localhost daemons over loopback; mandating TLS for loopback
  IPC adds attack surface (cert management) without benefit.

---

### 10. Critical / installer findings

#### 10a. **CRITICAL — Ollama installer downloaded over HTTPS with NO checksum verification before execution**

- severity: **Critical**
- file: `crates/amore-gui/src/install.rs:18, 77-138, 145-148`
- description: The first-run wizard downloads `https://ollama.com/download/OllamaSetup.exe`
  into `%TEMP%/OllamaSetup.exe` and then immediately executes it via
  `std::process::Command::new(temp_path).args(["/SILENT", "/SUPPRESSMSGBOXES"]).status()`.
  There is **no SHA-256 / signature / Authenticode check** between the HTTPS
  download and the execution.
- exploit path: any compromise of `ollama.com` (BGP hijack, DNS poisoning of a
  user's resolver, MITM at a coffee-shop wifi if the user's system has a
  malicious root CA installed, supply-chain compromise of ollama.com's CDN)
  results in a **silent, automatic, install-as-user execution of an arbitrary
  attacker payload on every fresh Amore install on Windows**. Combined with the
  Inno Setup `[Run]` block (`installer/windows/amore.iss:93`) that launches
  `amore-gui.exe --first-run` immediately after the Amore installer exits, the
  exploit window is **automatic on first launch — no user click required beyond
  the initial "Install Ollama automatically" button**.
- this is a textbook supply-chain CRITICAL for a non-technical-user installer.
- mitigations partially in place:
  - HTTPS-only ✓
  - Timeouts ✓ (600s download, 30 probe attempts)
- mitigations MISSING:
  - **SHA-256 (or, better, an Authenticode signature check via WinTrust API)
    of the downloaded `OllamaSetup.exe` against a hash committed to the Amore
    binary at build time.**
  - **Verification that Ollama's publisher Authenticode chain validates** before
    execution (Windows: `WinVerifyTrust`; PowerShell `Get-AuthenticodeSignature`).
- fix:
  1. Vendor the current Ollama installer's SHA-256 into `src/install.rs` as a
     constant (rotated per Amore release that bumps Ollama version).
  2. After download, compute SHA-256 of the temp file, compare to the constant,
     **fail closed** on mismatch.
  3. Stretch: also call `WinVerifyTrust` (Windows) / `codesign --verify` (macOS)
     on the downloaded binary's signature chain.
  4. Document in README that **Amore pins a specific Ollama installer hash** and
     users get notified at first-run if a newer Ollama version is available
     (rather than silently auto-bumping).

#### 10b. **CRITICAL — npm postinstall: Sigstore verification is OPTIONAL ("best-effort"), not required**

- severity: **Critical**
- file: `npm/postinstall.js:237-259, 274-280`
- description: `verifySigstoreIfAvailable()` only verifies the Sigstore bundle
  **if `cosign` is on PATH** (line 243-244: `if (probe.status !== 0) return;`).
  On a fresh non-technical-user laptop, cosign is **never** on PATH (cosign is
  a developer tool, not bundled with Node or Windows). The function comments
  acknowledge this explicitly:
  > `On a fresh user machine without cosign, fall through; the GitHub Releases
  > URL itself plus HTTPS already provides transport integrity.`
  This is **false defense**: HTTPS + GitHub URL only proves the URL was served
  by github.com at request time. It does **not** verify the artifact's integrity
  against a known-good signature.
- exploit path: a GitHub account compromise (`antonio-amore-akiki` PAT theft,
  token reuse), a release-workflow compromise (a malicious PR that modifies
  `release.yml` and gets merged), or a maintainer-laptop compromise (Antonio's
  stolen laptop in the stated threat model) lets an attacker upload a malicious
  `amore-vX.Y.Z-x86_64-pc-windows-msvc.zip` to a Release.  Every user who runs
  `npm install -g @anto/amore` then auto-executes the malicious binaries via
  `amore status` or any MCP client invocation. Sigstore bundle absence on
  non-Linux platforms (acknowledged in `release.yml:177-183` — bundles only
  generated for Linux) means the Windows / macOS lanes have **no integrity
  verification path at all**.
- this is a textbook supply-chain CRITICAL for a young npm package targeting
  a wide audience.
- fix (smallest sufficient):
  1. Generate Sigstore bundles for **all three OSes** (cosign keyless works on
     macos-latest + windows-latest runners — only the OIDC token is needed,
     which all three runners get).
  2. Make Sigstore verification **mandatory** when a `.bundle` is present and
     `cosign` is missing — bundle `cosign-installer` to `~/.amore-cache/cosign`
     on demand (or shell out to a tiny Rust `verify-blob` reimplementation
     compiled into a `amore-verify` mini-binary, ~2 MB).
  3. Until that lands, ship a **manual** verification recipe in `npm/README.md`
     ("how to verify your install"), and emit a LOUD warning on every install
     that cosign was skipped.

#### 10c. **Major — Inno Setup HTTPS download of OllamaSetup.exe deferred to amore-gui post-install means SmartScreen warning is the only barrier**

- severity: Major
- file: `installer/windows/amore.iss:11-15, 89-93`, `crates/amore-gui/src/install.rs`
- description: The Inno Setup installer itself runs `PrivilegesRequired=lowest`
  to `%LOCALAPPDATA%` (good — line 32, 39). However, Windows binaries are
  **unsigned** for v0.3.0 (`release.yml:14-17` — Authenticode skeleton blocked
  on EV cert). The first-run flow is therefore: **unsigned `Amore-Setup-v0.3.0.exe`
  triggers SmartScreen → user clicks "More info" → "Run anyway" → installer
  runs as user → installer launches `amore-gui.exe --first-run` → user clicks
  "Install Ollama automatically" → unverified OllamaSetup.exe downloads + runs.**
  Each click trains the user to bypass Windows defenses.
- exploit path: hypothesis — increases the social-engineering attack surface
  but no direct CVE-class exploit beyond what 10a/10b already cover.
- fix:
  - v0.3.0: document the SmartScreen click-path in `README.md` and the GitHub
    Releases page so users have a sanity check.
  - v1.0: pay for the EV cert (already on the roadmap per `release.yml:14`).

#### 10d. Minor — `installer/windows/amore.iss:46` SetupIconFile is commented out

- severity: Minor (UX, not security per se)
- file: `installer/windows/amore.iss:46`
- description: `SetupIconFile=..\..\branding\amore.ico  ; v0.3.0: branding ico
  not yet shipped; default Inno icon used` — ships with the **generic Inno
  Setup icon**, which is widely used by malware authors. SmartScreen + AV
  heuristics flag generic-icon unsigned installers more aggressively. Adds
  to the "users learn to dismiss warnings" risk.
- fix: ship a branded `amore.ico` before public release.

#### 10e. Minor — Embedded bge-small.onnx model has no integrity check

- severity: Minor
- file: `installer/windows/amore.iss:66-70`
- description: `staging\models\bge-small-en-v1.5.onnx` is embedded by CI's
  "separate fetch step" with no checksum committed to the installer config.
  An ONNX model is sandboxed (ONNX Runtime parses it; not direct code
  execution), but malformed ONNX has historically had CVEs
  (ONNX Runtime parser bugs surface every few months). Worth a SHA-256
  committed to `installer/windows/amore.iss` alongside the version.
- fix: commit `ONNX_BGE_SMALL_SHA256` and verify in the CI staging step that
  copies the model.

---

### 11. Process spawning

| file:line | command | input source | verdict |
|---|---|---|---|
| `crates/amore-gui/src/install.rs:145` | `Command::new(temp_path).args(["/SILENT","/SUPPRESSMSGBOXES"])` | downloaded executable | no shell injection (no shell), but covered by 10a |
| `crates/amore-gui/src/main.rs:231` | `Command::new("amore").args(["init", ide]).env("AMORE_DATA_DIR", &memory_dir)` | **PATH lookup**, attacker-controllable | **Major** — see 11a |
| `npm/postinstall.js:182, 187, 195` | `tar`, `powershell.exe`, `unzip` invoked via `spawnSync` with arg arrays | local archive path | no shell-string injection (arg array, not shell-cmd) — OK |

#### 11a. **Major — `amore-gui` shells out to `amore` via PATH, not absolute path**

- severity: Major
- file: `crates/amore-gui/src/main.rs:231-235`
- description: `Command::new("amore")` resolves via `PATH`. On Windows, this
  also checks the **current working directory first** (Windows `CreateProcess`
  semantics — even with `UseSearchPath=true`). A user who launches the
  Amore-Setup-v0.3.0.exe from their Downloads folder containing a malicious
  `amore.exe` would have **the malicious binary invoked with `AMORE_DATA_DIR`
  pointing at their chosen memory location**. Combined with the auto-start
  registry entry (`amore.iss:83`), persistence is trivial once the user runs
  the Inno installer from a poisoned working directory.
- exploit path: hypothesis. Requires the attacker to drop `amore.exe` in the
  user's CWD; mitigated when the Inno installer is launched from a click in
  Edge/Chrome (Downloads folder, but no malicious `amore.exe` typically there).
  Not zero-effort, but the fix is one-line.
- fix: use the absolute path of the installed CLI:
  ```rust
  let cli_path = std::env::current_exe()
      .ok()
      .and_then(|p| p.parent().map(|d| d.join("amore.exe")))
      .unwrap_or_else(|| PathBuf::from("amore"));
  Command::new(&cli_path)...
  ```
  The Inno installer ships `amore.exe` to `{app}` alongside `amore-gui.exe`
  (`amore.iss:62`), so `current_exe().parent().join("amore.exe")` is correct.

#### 11b. Minor — child processes spawned without `CREATE_NO_WINDOW`

- severity: Minor (UX, not security)
- file: `crates/amore-gui/src/install.rs:145-148`, `crates/amore-gui/src/main.rs:231-235`
- description: The GUI is `/SUBSYSTEM:WINDOWS`, but its child processes
  (`OllamaSetup.exe`, `amore init <ide>`) are spawned without setting
  `CREATE_NO_WINDOW` (Windows-specific creation flag via `CommandExt::creation_flags(0x08000000)`).
  Result: a console window flashes briefly when the GUI shells out. The
  `installer/windows/amore.iss:11` claim "No console window flashes" is
  therefore **not actually enforced** for child processes.
- exploit path: none. Polish issue.
- fix: `use std::os::windows::process::CommandExt;` + `.creation_flags(0x08000000)` on
  every `Command::new(...)` in the GUI.

---

### 12. Crypto (`provenance.rs`)

**Solid.** Length-prefixed SHA-256 over `id || prev_hash || canonical_json` with
canonical-JSON serialization. No custom crypto. Uses `sha2` (industry-standard,
RustCrypto) + `canonical_json` (Mozilla, MIT). Tests cover tamper-detection on
each field + chain-link breakage. `GENESIS_PREV_HASH` is a sentinel that cannot
collide with a real SHA-256 output (64 zero hex chars = preimage exhaustion).

One **Minor** note:

- `gen_id()` (`provenance.rs:99-108`) uses
  `SystemTime::now()...as_nanos()` formatted as `obs-{nanos:x}`. Not
  cryptographically random — predictable by an attacker who knows wall-clock
  drift. Since IDs are content-addressed by the hash, predictability of the
  human-readable ID is not a security property — but consider documenting
  this in the rustdoc so future code never *assumes* `id` carries entropy.

`verify_chain` correctly checks both per-link hash AND inter-link linkage AND
genesis prev_hash. No constant-time comparison needed (these are stored hashes,
not secrets).

---

### 13. Atomic file write semantics

`crates/amore-core/src/ide_adapter.rs:60-87` implements correct atomic-rename:
1. Write `<path>.tmp`
2. Rename existing `<path>` → `<path>.bak` (one revision rollback)
3. Rename `<path>.tmp` → `<path>`

Rust 1.66+ `fs::rename` is atomic on both POSIX and Windows. **No `fsync` is
called between write and rename** — on a power loss, the file system journal
covers the rename but the file *contents* could be empty. For an installer
config file this is acceptable (worst case: user re-runs `amore init`).

Severity: Minor — consider `file.sync_all()` between `fs::write` and the rename
for the IDE adapter config writes (low cost, eliminates corruption window).

---

### 14. Default-on telemetry / phone-home

`grep -i 'telemetry|phone_home|analytics|reporting'` workspace-wide:
**zero matches** outside of the `tracing` infrastructure (which logs to stderr
only, no network). No analytics SDK linked. No `AMORE_TELEMETRY` /
`OBELION_TELEMETRY` env var consulted. No default outbound network call beyond
what the user invokes (Ollama API, Qdrant API).

Severity: none. Matches the stated "no telemetry by default" mandate.

---

### 15. Default open ports

| port | binder | owner | exposed? |
|---|---|---|---|
| `127.0.0.1:11434` | Ollama (external) | the user's local Ollama install | localhost only |
| `127.0.0.1:6333/6334` | Qdrant (external) | the user's local Qdrant binary | localhost only |
| stdio | amore-mcp | Amore (this code) | not a port |

**Zero ports** opened by code in this workspace. The two external deps Ollama
and Qdrant default to localhost-only binds in their own configs. No
`0.0.0.0` exposure.

Severity: none.

---

### 16. DoS / resource exhaustion

#### 16a. Unbounded query string (covered in 6a)

#### 16b. Unbounded top_k (covered in 6a) — overflow check absent in release profile

#### 16c. Recursive `flattenNestedTargetDir` in npm postinstall

- severity: Minor
- file: `npm/postinstall.js:204-225`
- description: `moveFromDir` recurses into every directory under
  `outDir/target/`. The function reads `target/` from the extracted archive
  contents. **An attacker who controls the released zip** can craft a
  zip-bomb of deeply nested directories that exhausts Node's call stack
  before recursion depth limits trip. Mitigated by 10b (signed releases) —
  i.e. once you trust the archive, the recursion is safe; if you don't trust
  the archive, this is a tertiary issue.
- exploit path: only realizable if 10b is unfixed.
- fix: cap recursion depth (e.g. 8) or use an iterative `readdirSync({ withFileTypes: true })`
  worklist.

#### 16d. SQLite + FTS5 unsanitized-bound DoS

- severity: Minor
- file: `crates/amore-core/src/sqlite_store.rs:138-176`
- description: BM25 query sanitizer reduces to alphanumeric tokens before
  MATCH. An attacker passing a huge query (millions of bytes) would force
  the sanitizer to allocate a String of the same order. Mitigated by 6a once
  query length is bounded at the MCP boundary.

---

## What's already good

A young codebase rarely earns this many positives; flagging them so the
review isn't lopsided:

1. **Workspace-wide `rustls-tls` (no native-tls)** in `Cargo.toml:61` —
   eliminates schannel-related TLS bypass classes on Windows.
2. **Zero unsafe in production code** — only test-fixture env-var mutation uses unsafe.
3. **Provenance crypto is textbook-correct**: length-prefixed SHA-256, canonical JSON,
   genesis sentinel, chain + per-link tamper tests in `provenance.rs:111-194`.
4. **Atomic-rename file writes** in `ide_adapter.rs:60-87` with `.bak` rollback siblings.
5. **MCP is stdio-only.** No accidental TCP listener.
6. **No telemetry, no analytics, no phone-home.** Verified by grep.
7. **No hardcoded secrets / tokens / keys.** Verified by grep.
8. **No shell-string command construction.** Every `Command::new` uses arg
   arrays. No `sh -c` / `cmd /c` injection vectors.
9. **`MainError` plain-English Display impl in `amore-mcp/src/main.rs:61-86`** —
   no rust internal types leak to user-facing stderr.
10. **BM25 FTS5 query is sanitized + parameterized** (`sqlite_store.rs:138-162`).
11. **0 RUSTSEC vulnerabilities; 2 unmaintained informational only.**
12. **`cargo license` shows zero GPL/AGPL/SSPL contamination.**
13. **Sigstore keyless OIDC signing scaffolded** in `release.yml:119-130`
    (Linux). The skeleton exists; flipping it to mandatory + extending to
    mac+win is incremental.
14. **`PrivilegesRequired=lowest` in `amore.iss:39`** — no admin elevation needed.
15. **`HKCU\...\Run` auto-start** (line 83) instead of `HKLM\...\Run` —
    no privilege escalation requirement, easily user-removable.

---

## Verdict

**NO-GO at v0.3.0-live-fire for the stated mandate** ("industry grade, 100M
users scalable, highest security practices").

Two **Critical** findings block the live-fire ship:

1. **10a Ollama installer integrity** (`amore-gui/src/install.rs`) — fix with
   a hash constant + verify before exec. ~30 LOC change.
2. **10b npm postinstall Sigstore mandatory** (`npm/postinstall.js`) — fix by
   (a) emitting bundles for all 3 OS in `release.yml` and (b) installing
   cosign on-demand from postinstall (or shipping `amore-verify` mini-binary).
   ~50 LOC change in npm + ~20 lines in `release.yml`.

These are the **only** findings classed Critical. Everything else is Major
(deferrable with explicit conditions) or Minor (defense-in-depth hygiene).

### GO-WITH-CONDITIONS path to v0.3.1 / v0.4.0

Acceptable to ship v0.3.0 *as a friends/family preview* (NOT a public 100M-user
launch) once the two Criticals are fixed AND the following Majors land:

- **11a** absolute-path child spawn for `amore` (one-line fix; ship immediately).
- **6a** `top_k` clamp and `query` length cap at the MCP boundary (~10 LOC).
- **CI cargo-audit gate** (one new job in `ci.yml`).

The remaining Majors / Minors are acceptable defer-to-v0.5 / v1.0 with rationale:
- **10c** EV cert: blocked_on:user $300-500/yr — defer-to-v1.0 (`release.yml:14`).
- **11b** child-process `CREATE_NO_WINDOW`: cosmetic; defer-to-v0.5.
- **13** `file.sync_all()` between write+rename: theoretical corruption window;
  defer-to-v0.5.

---

## Reproduction notes

- All findings derived from static-code review on host (read-only). Per the
  scope brief, container_use is required only for exploit reproduction;
  **no exploit code was executed** during this audit. The findings rest on
  primary-source citations (file:line) and a fresh `cargo audit` run against
  the workspace `Cargo.lock`.
- `cargo audit` advisory DB: `last-updated 2026-05-23T18:31:49-04:00`,
  `advisory-count 1098`.
- `cargo license` output captured at audit time over 580+ resolved deps.

---

## Re-review at v0.3.1

**Re-review date:** 2026-05-26
**Re-review commit:** `7f4594a` (tag `v0.3.1-live-fire`)
**Re-review file:** `docs/SECURITY-REVIEW-v0.3.1-live-fire.md`

The v0.3.0 verdict above (NO-GO) **stands as a historical record** and is NOT
modified. The post-fix state has been re-reviewed at `7f4594a` and the verdict
has moved to **GO-WITH-CONDITIONS** for the v0.3.1-live-fire friends/family
preview. All 2 Critical + 3 Major findings named above are closed; one new
Major (cosign-download TOFU in `npm/postinstall.js`) is introduced by the fix
sprint and is tracked as condition C-1 against the v0.4.0 public ship.

See `docs/SECURITY-REVIEW-v0.3.1-live-fire.md` for per-finding verdicts and the
full list of conditions (C-1 through C-5).
