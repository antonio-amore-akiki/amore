# Changelog

<!-- @file-size-exempt: release registry — one entry per version, grows indefinitely by design -->

All notable changes per release. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) + [SemVer](https://semver.org/spec/v2.0.0.html).

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
