# Changelog

All notable changes per release. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) + [SemVer](https://semver.org/spec/v2.0.0.html).

## [0.5.0] — 2026-05-26

### Phase H — Scale-out architecture

- **H.0** BM25 FTS5 + RRF fusion characterization baselines (8+8 frozen-fixture tests) ([fa47091])
- **H.1** Tantivy BM25 lane — rank-identical parity vs FTS5, score delta ≤1e-3 ([e8cd003])
- **H.2** 3-node Qdrant docker-compose + smoke harness (Raft + RF=2 + 1-node-fail) ([ef438cf])
- **H.3** Cross-encoder reranker (BAAI/bge-reranker-base via ort 2.0 + tokenizers 0.20) ([bf36103])
- **H.4+H.5** bb8 gRPC pool + per-dep circuit breakers (Closed→Open→HalfOpen) ADR 0008 ([ef438cf])
- **H.6** tonic gRPC server skeleton + protobuf schema per ADR 0009 ([14de274])
- **H.7** Snapshot + restore CLI (tar.gz Qdrant + SQLite + sha256 sidecar) ([500149c])
- **H.8** sled-backed WAL + streaming ingest with backpressure (kill-mid-ingest = zero loss) ([f28cd05])
- **H.9** Compaction worker (sha256 dedup + age eviction + incremental_vacuum) ([b0b5a22])
- **H.10** Load-test harness skeleton + Rust corpus seeder (10M-corpus / 100 QPS gate) ([1fc56e7])
- **H.12** Toxiproxy chaos harness (30% loss + 200ms latency → CB trips/recovers) ([7b1c1a6])
- **H.13** Multi-level cache (moka L1 + sled L2) with Zipfian hit-ratio test ([db2fdf3])

### Phase I — Real-user readiness (Wave 3)

- **I.3+I.4** Homebrew formula + winget manifest descriptors ([21162bc])
- **I.5+I.6** AUR PKGBUILD + multi-stage multi-arch Dockerfile (amd64 + arm64) ([af25a08])
- **I.7** Helm chart (3-node Qdrant subchart + Amore deployment + ingress) ([435706a])
- **I.8** mdBook docs site (book.toml + SUMMARY; gh-pages ready) ([7abf25b])
- **I.9** OSSF Scorecard local baseline 4.1/10 + 3 hardening fixes (~7.1 lift) ([539912c], [5502241])

### Phase G — Production hygiene

- **G.4** proptest 10 properties × 256 cases (provenance/recall/canonical-doc) ([ef438cf])
- v0.4.0 no-unwrap policy: `clippy::unwrap_used = "deny"` workspace-wide ([34dcbf6])

### Release pipeline

- Local-only pipeline `scripts/release-local.ps1` (Sigstore + SBOM; no GHA minutes) ([b398ff7])

### Notes

- Workspace rename `25 - Obelion/` → `25 - Amore/` deferred (NTFS lock; next session)
- `OBELION_*` env-var aliases kept one more release; removal v0.6.0
- 7 IDE adapters: Claude Code, Cursor, Codex, Cline, opencode, Windsurf, Hermes

## [0.3.1-live-fire] — 2026-05-26

Security review NO-GO from the security-reviewer subagent at v0.3.0 closed
in one parallel-fixer sprint. v0.3.1 is the **post-security-review production
candidate** for the v0.3.x line. See full review: `docs/SECURITY-REVIEW-v0.3.0-live-fire.md`.

### Security (Critical)
- **Critical 10a — Ollama installer SHA-256 verification** (`crates/amore-gui/src/install.rs`): the first-run wizard downloaded `OllamaSetup.exe` over HTTPS and immediately executed it with no integrity check. Any `ollama.com` BGP/DNS/CDN compromise would have silently RCE'd every fresh Amore install. Fix: pinned `OLLAMA_INSTALLER_SHA256 = "38ef4715a31b6fede8f37be840c5e1e1524150d2c637d1acca94227980daf300"` constant; hash computed incrementally (single-pass over the 857 MB download stream); fail-closed on mismatch — `run_installer` is unreachable if the hash doesn't match.
- **Critical 10b — npm Sigstore verification mandatory + all-3-OS bundles** (`npm/postinstall.js` + `.github/workflows/release.yml`): bundle verification was previously OPT-IN ("if cosign is on PATH") and only Linux artifacts were signed in the first place. Fix: bundle verification is now mandatory (fail-closed); cosign installed on demand into `~/.amore-cache/`; release.yml emits Sigstore keyless bundles for all three OS matrix rows; `AMORE_NPM_SKIP_SIGSTORE=1` escape hatch emits a loud stderr warning every install.

### Security (Major)
- **Major 11a — absolute-path child spawn** (`crates/amore-gui/src/main.rs`): `Command::new("amore")` previously PATH-resolved; on Windows, CWD-first semantics could let a dropped `amore.exe` in the user's Downloads folder hijack the `amore init <ide>` calls with `AMORE_DATA_DIR` pointing at user-chosen memory. Fix: resolve via `current_exe().parent().join("amore.exe")` (one-line).
- **Major 6a — MCP `recall` tool input bounds** (`crates/amore-mcp/src/main.rs`): `top_k: usize` and `query: String` were unbounded; release-profile arithmetic on large `top_k` wrapped silently into junk Qdrant params. Fix: `MAX_TOP_K = 100` clamp + `MAX_QUERY_BYTES = 16 * 1024` reject with `McpError::invalid_params`. Two new regression tests in `crates/amore-mcp/tests/mcp_handshake.rs`.

### Security (Hygiene)
- **Local nightly cargo-audit + cargo-deny + cargo-geiger** via Windows Task Scheduler at 02:30 daily — NO GitHub Actions minutes consumed per user mandate "no git actions or credits". `scripts/security-baseline.ps1` runs the full audit; writes `%LOCALAPPDATA%\Amore\security-baselines\<date>.json`; gates on high-severity findings; optional ntfy notification on FAIL when Tailscale + ntfy.log are present.
- **`deny.toml`** allow-list of license SPDX IDs (Apache-2.0 / MIT / BSD / ISC / Unicode / MPL-2.0 / CDLA-Permissive-2.0 / OpenSSL / Zlib) + ban wildcards + restrict to crates.io registry — GPL/AGPL/SSPL implicitly denied.

### Documentation
- `SECURITY.md` disclosure policy + 5-day-response / 30-day-fix SLA for High/Critical.
- `docs/THREAT-MODEL.md` STRIDE-per-asset + DREAD top-3.
- `docs/SECURITY-REVIEW-v0.3.0-live-fire.md` formal review report (verdict, findings, what's-already-good).

### Workspace
- Version bump 0.2.1 → 0.3.1.
- New workspace dep usage: `sha2` + `hex` added to `amore-gui` for Critical 10a verification.

### Verdict
- 4 parallel executor subagents closed 2 Criticals + 3 Majors in ~5 wall-minutes.
- `cargo build --release --workspace` exit 0.
- `cargo test --release -p amore-integration-tests` 9 passed, 0 failed.
- All 4 release binaries pass `binary-contracts.json` post-fix (manual spawn capture).
- Phase G nightly cargo-audit now active (no GHA minutes consumed).
- Phase G remaining: no-unwrap policy + coverage gate + proptest/fuzz/mutants + remaining docs (ARCHITECTURE / RUNBOOK / SLO / SCALE-100M / CONTRIBUTING / CODE_OF_CONDUCT / 12 ADRs / 7 quickstarts) — see `~/.claude/plans/amore-v1-no-gha-roadmap-20260526.md`.

## [0.3.0-live-fire] — 2026-05-26

First milestone where binary contracts are PROVEN, not just compiled. See
release notes: https://github.com/antonio-amore-akiki/amore/releases/tag/v0.3.0-live-fire

### Added
- **DG.2 integration tests:** `crates/amore-integration-tests/` ships 9 cargo tests that spawn the release binaries via `std::process::Command` + `Stdio::piped()`. Coverage: `cli_help` (Usage + 5 subcommands), `mcp_handshake` (JSON-RPC initialize round-trip with stderr-error-leak check), `init_dry_run` × 7 (one per IDE adapter).
- **DG.3 acceptance-tests spec:** `docs/ACCEPTANCE-TESTS.md` formal release-gate spec for v0.3.0 / v0.5.0 / v1.0 with literal `cmd : expected_stdout_pattern : must_not_contain` rows. Source: `crates/amore-cli/src/main.rs` for dry-run output pattern.
- **DG.7 amore-gui CLI flags:** `--version`, `--help`, `--no-gui` exit cleanly without opening the egui window. `--no-gui` emits a config-summary JSON for CI smoke. `/SUBSYSTEM:WINDOWS` preserved for production; CLI emit goes to both stdout + stderr so headless validation sees output regardless of redirection.
- **F.installer-1+2+3 Windows installer + GUI wizard:** Inno Setup `.iss` builds `Amore-Setup-v0.3.0.exe` (7.6 MB). egui-based first-run wizard (`amore-gui.exe`) with 7 IDE checkboxes, memory-dir picker, local-vs-cloud-AI toggle, Ollama silent-install background pipeline.
- **SECURITY.md** disclosure policy + supported-versions matrix + posture checklist.
- **docs/THREAT-MODEL.md** STRIDE per asset with DREAD top-3 scoring. Inherits the "stolen-laptop only" threat model from CLAUDE.md.

### Fixed
- **DG-D anyhow Display leak:** `amore-mcp::main()` no longer prints `Error: connection closed: initialize request` (rmcp::ServerInitializeError + anyhow chain). New `MainError` enum (`IdeDisconnected`, `DepUnreachable`, `ConfigInvalid`, `Other`) with plain-English Display: "Waiting for your IDE — start the editor and connect via MCP." See commit `9db9d73`.
- **DG-E empty-stdin race:** `amore-mcp` previously exited non-zero on empty stdin (lost every IDE connection if the server started before the IDE wrote). Now matches `ServerInitializeError::ConnectionClosed` → logs `INFO: Waiting for your IDE` → exits 0. Commit `9db9d73`.
- **DG-F qdrant version skew:** Pinned `qdrant-client = "1.15"` (was `1.18`) to match the bundled local Qdrant server. Eliminates the `Client version 1.18.0 is not compatible with server version 1.15.4` warning on first launch. Commit `353aacd`.

### Changed (breaking for installations — one-minor-release transition window)
- **Product renamed:** `obelion` -> `Amore`. All crate names, binary names, env vars, data paths, and npm package updated.
- **Crates renamed:** `obelion-core` -> `amore-core`, `obelion-mcp` -> `amore-mcp`, `obelion-cli` -> `amore-cli`, `obelion-eval` -> `amore-eval`, `obelion-adapter-{claude,cursor,codex,cline,opencode,windsurf,hermes}` -> `amore-adapter-{...}`.
- **Binary renamed:** `obelion` -> `amore`, `obelion-mcp` -> `amore-mcp`.
- **Env vars renamed:** `OBELION_*` -> `AMORE_*`. Legacy `OBELION_*` env vars accepted with `tracing::warn!` deprecation messages through v0.3.x; removed in v0.4.0.
- **Data path renamed:** `<AppData>/obelion/obelion.db` -> `<AppData>/Amore/amore.db`. First `amore-mcp` start auto-migrates the SQLite file if the old path exists and the new one does not; writes `migrated-from-obelion.txt` marker.
- **IDE adapter entry name:** adapters now write `"amore"` (not `"obelion"`) as the MCP server key; existing `obelion` entries are atomically replaced on the next `amore init <ide>` run.
- **npm package renamed:** `@anto/obelion` -> `@anto/amore`. Release artifacts renamed `amore-v*-<target>.*`.
- **GitHub repo renamed:** `antonio-amore-akiki/obelion` -> `antonio-amore-akiki/amore` via `gh repo rename amore`. GitHub auto-redirects the old URL.

## [0.2.1] — 2026-05-26

### Fixed
- **Release pipeline:** macOS + Windows upload steps in `release.yml` previously required the Sigstore `.bundle` file as a strict-match pattern, which only gets produced on Linux runners. v0.2.0 published Linux signed artifact but failed to publish macOS + Windows. v0.2.1 splits the upload step: archive (every target, strict-match) + bundle (Linux only, strict-match). No application code changed from v0.2.0.

## [0.2.0] — 2026-05-26

### Added
- **S8 BM25 + RRF fusion:** `observations_fts` FTS5 virtual table with `porter unicode61` tokenizer; `bm25_search()` with alphanumeric sanitization; `rrf_fuse()` at k=60 in `HybridRecall::search()` over-fetching `top_k * 4` per lane. Vector-only path preserved when `with_sqlite()` is unset.
- **S8 Canonical-docs router:** `CanonicalDocsRouter::route()` walks `*.md` files for `stable: true` headers, scores by alphanumeric >=3-char token overlap against filename + title + `topic:` line. `canonical_doc_lookup` MCP tool added. `AMORE_DOCS_PATHS` env honours `:` / `;` separator (legacy `OBELION_DOCS_PATHS` accepted with deprecation warning).
- **S9 Five new IDE adapters:** codex (`~/.codex/config.toml` TOML), cline (VSCode globalStorage JSON), opencode (top-level `mcp` field with `type:"local"`), windsurf (shared `mcpServers` JSON at `~/.codeium/`), hermes (`~/.hermes/config.yaml` YAML). All seven adapters share the atomic-write + `.bak` sibling + idempotent NoChange contract.
- **S10a Sigstore-signed Linux release:** `release.yml` workflow on `push: tags: v*` builds 3-target matrix (`x86_64-{linux-gnu,apple-darwin,pc-windows-msvc}`) and signs Linux artifact via `sigstore/cosign-installer@v3.7.0` + GitHub OIDC keyless. Authenticode (Win) + notarytool (macOS) skeleton stanzas in-file, commented, pending user EV cert + Apple Developer ID.
- **S11a npm wrapper:** `@anto/amore` package with `postinstall.js` that downloads the version-matched GitHub Release artifact, optional `cosign verify-blob` integrity check when cosign is on PATH, atomic `tar -xzf` / `Expand-Archive` extract. `bin/amore.js` + `bin/amore-mcp.js` exec shims forward argv to the native binary.
- **S12 World-model substrate:** persistent typed graph in 4 SQLite tables (`wm_projects`, `wm_project_edges`, `wm_tool_reliability`, `wm_revealed_preferences`). Laplace-smoothed `(s+1)/(s+f+2)` tool success rate, log-odds Bayesian preference updates clamped to `[-3, 3]` step magnitude. `top_preferences(top_n)` ordered by probability DESC.
- **S14a Multi-agent ensemble (orchestrator + 2 roles):** `LlmClient` trait via Rust 2024 native async-fn-in-traits, six `AgentRole` variants with per-role system prompts + shared `VOTE_SCHEMA` const, generic `Orchestrator<L: LlmClient>` with sequential await fan-out, confidence-weighted `tally()` with tie->abstain. Prose-tolerant JSON vote extraction. Architect + Skeptic wired; Historian / Reviewer / Negotiator / Implementer + EIG + persistence + credit assignment land in S14b/c.
- **S15 Adversarial-test mining substrate:** pure-function `mining` module parses edit-rationale-ledger JSONL (malformed-line tolerant), filters `failure | corrected` entries, generates `AdversarialTest` stubs `{context, forbidden_output, desired_output, source_session_id, source_ts, kind}` with alternate field-name lookup tables.

### Changed
- **Workspace version:** bumped from 0.1.0 -> 0.2.0 across `Cargo.toml` + `npm/package.json`.
- **CI workflow:** `ci.yml` trigger narrowed to `workflow_dispatch` only — `release.yml` owns tag triggers, avoiding double matrix burn per release.
- **README:** rewritten for v0.2.0 reality — adds Tier-1 npm + Tier-2 curl install paths, Sigstore verify-blob command, architecture table, "What's in" / "On deck" sections.

### Notes
- **Total test coverage:** 85 unit tests green (59 amore-core + 26 adapter). Integration tests against live Ollama + Qdrant + MCP handshake pass under `AMORE_TEST_{OLLAMA,QDRANT,E2E,MCP}=1` env gating.

## [0.1.0] — 2026-05-25

Initial substrate — committed at SHA `b81f261`. (Note: originally released as `obelion` v0.1.0 — renamed to Amore in the Unreleased entry above.)

### Added
- **S1 F4 Cryptographic provenance:** `canonical_json` v0.7 + length-prefixed SHA-256 envelope chain. `verify_full_chain()` wired into `SqliteStore`. Tampered payloads break the chain.
- **S2 Real `OllamaClient::embed()`:** via `reqwest` 0.12 against live Ollama 0.23 at `127.0.0.1:11434` with `nomic-embed-text` (768-dim).
- **S3 Real `QdrantStore`:** via `qdrant-client` 1.18 gRPC port 6334 with idempotent `ensure_collection`.
- **S4 HybridRecall vector path:** end-to-end `index()` + `search()` against Qdrant + Ollama.
- **S5 Cross-OS CI matrix:** GitHub Actions on `ubuntu-latest` + `macos-latest` + `windows-latest` with `Swatinem/rust-cache@v2`.
- **S6 MCP server:** wires the `recall` tool via `rmcp` 1.7 over stdio with `#[tool_router]` + `#[tool_handler]` macros.
- **S7 CLI init:** `amore init {claude,cursor}` with `IdeAdapter` trait + atomic-write (`tmp + rename + .bak`) + `--dry-run` + idempotent `NoChange` on byte-identical content.
