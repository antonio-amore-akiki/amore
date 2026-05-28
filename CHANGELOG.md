# Changelog

<!-- @file-size-exempt: release registry — one entry per version, grows indefinitely by design -->

All notable changes per release. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) + [SemVer](https://semver.org/spec/v2.0.0.html).

## [1.1.0] — 2026-05-28

Stranger-reproducible cert + i18n + library-first parity + free OSS audit stack. Closes 26 reviewer findings cumulatively (F1-F26) plus reviewer-3/4/5/6 follow-ups, per the closure plan in `plans/zesty-foraging-lake.md`.

### Added

- **MCP `observe` tool** — forwards to `SqliteStore::insert_observation` + `HybridRecall::ingest`; returns `{id, persisted_bm25, persisted_vector, degraded}`; 16 KiB cap matches WAL envelope limit (A1 / F4).
- **`amore-gui --auto-wire` headless arm** — runs IDE detect+wire BEFORE `eframe::run_native`, emits JSON contract `{detected, wired, skipped, errors}`, exit 0 iff `errors == []` (A2 / F3 + F24).
- **Qdrant per-user storage path** via bundled `qdrant.exe --config-path` reading `%LOCALAPPDATA%\Amore\qdrant\config.yaml` (A3 / F7 + reviewer-3 F-30).
- **`amore-mcp --register-claude-code` + `--self-contained`** — CLI path uses `claude mcp add`; self-contained path direct-writes `~/.claude.json` with atomic-rename + `.bak` + ACL safeguards (A5 + A6 / F11 + F12).
- **Corporate-proxy support** across every HTTP client via `amore_core::http::build_client()` honoring `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` (A7 / F14).
- **`amore data erase --confirm`** — GDPR data-erasure CLI; wipes SQLite + qdrant storage + WAL + keyring + crash dumps + registry keys; auto-invoked by uninstaller (A8 / F22).
- **Inno `[Run]` auto-wire entries** on Windows install — silent IDE detection + writer invocation; fail-loud on any non-zero exit (B1 / F2).
- **Linux `.deb` postinst + `.AppImage` first-launch wrapper** — resolves real user via `SUDO_USER`/`PKEXEC_UID`/`logname` fallback chain; runs writer as that user (B2 / F26 + reviewer-3 F-36).
- **`--bundle-deps` fat-installer variant** — ships `ollama.exe` + `qdrant.exe` + `nomic-embed-text` model preloaded; ~535 MB; default release recommendation for first-time users (B3 / F20).
- **Bundled `cosign-verify-mini` pre-extract verification** — Inno `[Code] InitializeSetup` + `.deb preinst` verify SHA256 + Sigstore signature BEFORE extraction (B4 / F21).
- **Stranger-cert pipeline**: `scripts/certify-user-flow.{ps1,sh}` + `schema/cert-result.schema.json` (C1 / F8 + F23 + F24 + F25), `.github/workflows/certify.yml` matrix `{windows-latest, ubuntu-22.04, macos-latest}` with JSON-Schema validation + aggregate-and-commit `docs/CERTIFICATION.md` (C2), locale axis `{en-US, de-DE, ja-JP}` Windows-only via `Set-WinUILanguageOverride` + static `.isl` render verification (G1 / F15 + reviewer-4 F-45).
- **i18n with 5 user-required languages**: English, French, Dutch, German, Arabic. Inno installer MUI pack for 4 + vendored Arabic.isl; runtime `amore-i18n` crate using Mozilla Fluent; Arabic partial-RTL (installer + CLI full RTL; GUI strings render correctly, layout stays LTR pending egui Issue #1016 — documented honestly in INSTALL.md) (D1 + D2 / F18 + reviewer-4 F-43/F-44).
- **Auto-update via signed appcast** — `cargo-dist` + `self_update` crate polls GH releases, verifies Sigstore signature, prompts user before applying; opt-out via `AMORE_NO_AUTOUPDATE=1` (E1 / F19).
- **Opt-in local-only crash diagnostics** — `minidumper` writes to `%LOCALAPPDATA%\Amore\crashes\`; `amore diag bundle` packages dumps + sanitized logs into a zip the user can manually share via GH issue. Zero backend, zero PII auto-leak (E2 / F17 partial closure; CONSTRAINT-BOUND per reviewer-3 F-37).
- **Free OSS audit stack in CI** — cargo-audit + gitleaks + semgrep + cargo-fuzz + cargo-mutants + OSSF Scorecard + Sigstore cosign + GH Security Advisories + container-isolated hostile-reviewer log; aggregated into `docs/FREE-OSS-AUDIT.md` with honest framing per reviewer-3 F-39 (F1 / F16).
- **Library-first parity** — `cargo install amore-{mcp,cli,gui}` via L1 crates.io publish on every tag; `pip install amore` via L2 PyPI OIDC Trusted Publishing; `npm install -g amore-mcp` via L3 npm OIDC Trusted Publishing; `docker run antonioamoreakiki/amore:latest` via L4 multi-arch Docker Hub publish; `brew install antonio-amore-akiki/amore/amore` via L5 Homebrew tap with explicit `needs: [release-build, sigstore-sign]` race fix (reviewer-4 F-47).
- **Universal Continual-Harness** (`~/.claude/`) — Pre-Plan H lands the failure-detector + class-classifier + self-updater + verification-loop + multi-agent quorum + cross-session transcript replay + persistence + safe-rollback bootstrap. Multi-trigger autonomous firing via PostToolUse(Edit|Write|Bash) + SubagentStop + PreCompact + Stop + 75% context-window consolidation flag. Opus 4.7 standard window (550K) for all harness subagents. Adapted from arxiv:2605.09998 (cross-domain Build verdict; coding-agent has no upstream parity).

### Changed

- **Repo public flip** verified live (no-op; was already public). `docs/RELEASING.md` drift cleaned to reflect public-repo GHA + Sigstore keyless OIDC pipeline; legacy "GHA paid minutes ran out" framing removed (Phase 0.3 / reviewer-3 F-29).
- **Homebrew tap renamed** `homebrew-tap` → `homebrew-amore`; install command becomes `brew install antonio-amore-akiki/amore/amore`. GitHub 301-redirects old URL for ~30 days (Phase 0.4 / reviewer-4 F-41).
- **Agent-side `AskUserQuestion` denied** via `settings.json` `permissions.deny` so every `##decision` block auto-resolves to option (a) per CLAUDE.md DECIDE-and-ACT (Phase 0.1).
- **ntfy hook v3 goal-state-discipline gate** replaces v1 (`inflight-agents.json` dead-read) + v2 (bg-agents `.output` filesystem over-suppress). v3 reads `state/goal-state.json` status+blocked_on+mtime; fail-loud suppress with reason logged when stale or open. Adapted from Erlang let-it-crash supervisor pattern. Lives in `~/.claude/runtime/guard-hooks/anthropic-ntfy.mjs` (commit `5451dbf` in the user's global Claude config repo; out-of-band from this Amore release but documented for traceability).

### Fixed

- **15 RUSTSEC advisories cleared** where transitive-fixable; remainder documented in `deny.toml` ignore-with-sunset (A4 / F6).
- **`actions/cache@v4.2.0` SHA typo** in `.github/workflows/free-oss-audit.yml` — last 8 chars were corrupted, causing every push to fail at "Unable to resolve action" step. Pin corrected to canonical SHA `1bd1e32a3bdc45362d1e726936510720a7c30a57`.

### Security

- **Pre-extract Sigstore verification** of every release artifact via bundled `cosign-verify-mini` (B4 / F21).
- **Per-user qdrant storage** prevents `Program Files` write-permission errors and reduces blast radius (A3 / F7).
- **Atomic IDE config writes** with `.bak` backup + ACL preservation + retry-on-sharing-violation prevent corruption of `~/.claude.json` / `claude_desktop_config.json` (A5+A6 / F11).
- **Free OSS audit stack** runs on every push (cargo-audit + gitleaks + semgrep + cargo-fuzz + cargo-mutants + OSSF Scorecard). Honest framing: this is the strongest free-tier substitute for paid SOC2 / pen-test / NCC audit, NOT a replacement. Users with regulated procurement must engage their own auditor per `docs/FREE-OSS-AUDIT.md` header (F1 / F16).

### Known limitations (carved out from free-only constraint or different product class)

- macOS `.dmg` / `.pkg` shows Gatekeeper warning — paid Apple Dev ID required. Free escape: Homebrew tap.
- Windows `.exe` shows one-time SmartScreen prompt — paid Authenticode EV required to remove. Free escape: documented click-through per `docs/INSTALL.md`.
- No telemetry / silent crash detection — by free + zero-backend constraint. Users opt-in to `amore diag bundle` for manual share.
- No accredited SOC2 / ISO27001 / NCC pen-test — paid; free OSS substitute stack is the elite-bar replacement.
- Arabic GUI layout LTR (not full RTL) pending egui Issue #1016; strings render correctly in BiDi.

### Verification

```bash
# Stranger-reproducible cert on hosted runners (free for public repos)
gh workflow run certify.yml -f release_tag=v1.1.0
gh run watch
# expected docs/CERTIFICATION.md: Win × {en-US, de-DE, ja-JP} + Linux PASS; macOS allowed-fail with annotation

# Free OSS audit stack
gh workflow run free-oss-audit.yml
# expected docs/FREE-OSS-AUDIT.md: 9/9 substitute audits PASS with thresholds met

# Install paths (5 channels)
cargo install amore-mcp
pip install amore
npm install -g amore-mcp
docker run --rm -it antonioamoreakiki/amore:latest
brew install antonio-amore-akiki/amore/amore
```

## [1.0.0] — 2026-05-27

First public release.

### Added

- **Local-first persistent memory** for AI assistants. All user data stays on the user's machine; no telemetry, no cloud sync, no account.
- **One-click installers** for Windows (`.msi`), macOS (`.pkg` + `.dmg`, Apple Silicon + Intel), and Linux (`.AppImage`, `.deb`, `.rpm`). Bundled runtime (ollama + qdrant) on Windows — no separate installs.
- **First-run wizard** (6 screens, < 2 min): license → memory location → IDE auto-detect → wire → smoke test → done.
- **IDE auto-detect + wire** for Claude Desktop, Claude Code, Cursor, Cline, and Continue.dev. Configs written atomically with backup-before-edit.
- **Tray icon** (Windows / macOS / Linux): Open dashboard, Pause/Resume, Recent activity, Check for updates, Quit. Auto-starts on login.
- **MCP server** over stdio + gRPC for IDE integration. Native `rmcp` protocol.
- **Hybrid recall**: BM25 (tantivy) + vector search (qdrant) + cross-encoder reranker (`BAAI/bge-reranker-base` via ort).
- **Power-user CLI**: `amore`, `amore-mcp`, `amore-gui`. `amore doctor` for diagnostics; `amore --version` for build info.
- **Homebrew tap** at `antonio-amore-akiki/homebrew-amore` — `brew install antonio-amore-akiki/amore/amore`.
- **Observability**: Prometheus `/metrics`, OpenTelemetry 3-signal (traces + metrics + structured logs), HTTP `/healthz` + `/readyz`.
- **Feature flags** at compile time (`qdrant`, `reranker`, `wm`, `grpc`, `fts`) and runtime (`AMORE_FEATURES`).
- **Graceful shutdown** on SIGTERM: WAL flush + in-flight drain + clean exit.
- **Rate limiting** via `governor` (default 500 RPM; configurable).
- **Bundled documentation**: install guide, threat model, accessibility statement, system card, SLO, error budget policy, postmortem template, canary runbook, PRR checklist, SLSA L3 attestation, GDPR scoping memo, RUSTSEC triage.

### Security

- **SLSA L3** cosign keyless attestation on every binary in the release.
- **CycloneDX SBOM** (`sbom.cdx.json`) with `composition.aggregate = complete`.
- **Cryptographically signed** `sha256sums.txt` (SSH-signed) — verify any download against the recorded hash.
- **Adversarial-eval** suite: 0 failures on prompt-injection-via-memory, memory-exfil, recall-poisoning.
- **Fuzz harnesses** for ingest, MCP parser, snapshot restore (`cargo fuzz`).
- **Mutation testing** baseline ≥ 60% for `amore-core` + `amore-mcp` (`cargo mutants`).
- **SAST**: gitleaks + semgrep + cargo-audit nightly.
- **Secrets**: `keyring` crate; no plaintext credential files.
- **Reproducible builds**: `SOURCE_DATE_EPOCH` locked to tag commit timestamp.
- **Pinned dependencies**: `Cargo.lock` committed; `cargo-deny` bans wildcard specs; all GitHub Action `uses:` refs pinned to commit SHAs.

### Performance

- **Cold-start latency** 12 milliseconds (`amore --version`).
- **Resident memory** 22 megabytes at idle for the MCP server.
- **Recall p95** ≤ 80 milliseconds per query (target SLO).
- **Long-context recall**: synthetic LongMemEval-style benchmark passes R@5 / R@10 / MRR = 1.0 (mock corpus; real-corpus measurement v1.1).

### Privacy

- **0 bytes leave the user's computer** in default configuration. No telemetry, no analytics, no crash reporting.
- **GDPR Art. 25** scoping memo (`docs/GDPR-SCOPING.md`).
- **Threat model** (`docs/THREAT-MODEL.md`): stolen-laptop scope, disk-encryption recommendation.

### Accessibility

- **WCAG 2.2 AA** target for the GUI wizard + tray. MSAA/UIA exposure on Windows. Statement: `docs/ACCESSIBILITY-STATEMENT.md`.
