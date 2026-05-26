# Changelog

All notable changes per release. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) + [SemVer](https://semver.org/spec/v2.0.0.html).

## [0.2.1] — 2026-05-26

### Fixed
- **Release pipeline:** macOS + Windows upload steps in `release.yml` previously required the Sigstore `.bundle` file as a strict-match pattern, which only gets produced on Linux runners. v0.2.0 published Linux signed artifact but failed to publish macOS + Windows. v0.2.1 splits the upload step: archive (every target, strict-match) + bundle (Linux only, strict-match). No application code changed from v0.2.0.

## [0.2.0] — 2026-05-26

### Added
- **S8 BM25 + RRF fusion:** `observations_fts` FTS5 virtual table with `porter unicode61` tokenizer; `bm25_search()` with alphanumeric sanitization; `rrf_fuse()` at k=60 in `HybridRecall::search()` over-fetching `top_k * 4` per lane. Vector-only path preserved when `with_sqlite()` is unset.
- **S8 Canonical-docs router:** `CanonicalDocsRouter::route()` walks `*.md` files for `stable: true` headers, scores by alphanumeric ≥3-char token overlap against filename + title + `topic:` line. `canonical_doc_lookup` MCP tool added. `OBELION_DOCS_PATHS` env honours `:` / `;` separator.
- **S9 Five new IDE adapters:** codex (`~/.codex/config.toml` TOML), cline (VSCode globalStorage JSON), opencode (top-level `mcp` field with `type:"local"`), windsurf (shared `mcpServers` JSON at `~/.codeium/`), hermes (`~/.hermes/config.yaml` YAML). All seven adapters share the atomic-write + `.bak` sibling + idempotent NoChange contract.
- **S10a Sigstore-signed Linux release:** `release.yml` workflow on `push: tags: v*` builds 3-target matrix (`x86_64-{linux-gnu,apple-darwin,pc-windows-msvc}`) and signs Linux artifact via `sigstore/cosign-installer@v3.7.0` + GitHub OIDC keyless. Authenticode (Win) + notarytool (macOS) skeleton stanzas in-file, commented, pending user EV cert + Apple Developer ID.
- **S11a npm wrapper:** `@anto/obelion` package with `postinstall.js` that downloads the version-matched GitHub Release artifact, optional `cosign verify-blob` integrity check when cosign is on PATH, atomic `tar -xzf` / `Expand-Archive` extract. `bin/obelion.js` + `bin/obelion-mcp.js` exec shims forward argv to the native binary.
- **S12 World-model substrate:** persistent typed graph in 4 SQLite tables (`wm_projects`, `wm_project_edges`, `wm_tool_reliability`, `wm_revealed_preferences`). Laplace-smoothed `(s+1)/(s+f+2)` tool success rate, log-odds Bayesian preference updates clamped to `[-3, 3]` step magnitude. `top_preferences(top_n)` ordered by probability DESC.
- **S14a Multi-agent ensemble (orchestrator + 2 roles):** `LlmClient` trait via Rust 2024 native async-fn-in-traits, six `AgentRole` variants with per-role system prompts + shared `VOTE_SCHEMA` const, generic `Orchestrator<L: LlmClient>` with sequential await fan-out, confidence-weighted `tally()` with tie→abstain. Prose-tolerant JSON vote extraction. Architect + Skeptic wired; Historian / Reviewer / Negotiator / Implementer + EIG + persistence + credit assignment land in S14b/c.
- **S15 Adversarial-test mining substrate:** pure-function `mining` module parses edit-rationale-ledger JSONL (malformed-line tolerant), filters `failure | corrected` entries, generates `AdversarialTest` stubs `{context, forbidden_output, desired_output, source_session_id, source_ts, kind}` with alternate field-name lookup tables.

### Changed
- **Workspace version:** bumped from 0.1.0 → 0.2.0 across `Cargo.toml` + `npm/package.json`.
- **CI workflow:** `ci.yml` trigger narrowed to `workflow_dispatch` only — `release.yml` owns tag triggers, avoiding double matrix burn per release.
- **README:** rewritten for v0.2.0 reality — adds Tier-1 npm + Tier-2 curl install paths, Sigstore verify-blob command, architecture table, "What's in" / "On deck" sections.

### Notes
- **Total test coverage:** 85 unit tests green (59 obelion-core + 26 adapter). Integration tests against live Ollama + Qdrant + MCP handshake pass under `OBELION_TEST_{OLLAMA,QDRANT,E2E,MCP}=1` env gating.

## [0.1.0] — 2026-05-25

Initial substrate — committed at SHA `b81f261`.

### Added
- **S1 F4 Cryptographic provenance:** `canonical_json` v0.7 + length-prefixed SHA-256 envelope chain. `verify_full_chain()` wired into `SqliteStore`. Tampered payloads break the chain.
- **S2 Real `OllamaClient::embed()`:** via `reqwest` 0.12 against live Ollama 0.23 at `127.0.0.1:11434` with `nomic-embed-text` (768-dim).
- **S3 Real `QdrantStore`:** via `qdrant-client` 1.18 gRPC port 6334 with idempotent `ensure_collection`.
- **S4 HybridRecall vector path:** end-to-end `index()` + `search()` against Qdrant + Ollama.
- **S5 Cross-OS CI matrix:** GitHub Actions on `ubuntu-latest` + `macos-latest` + `windows-latest` with `Swatinem/rust-cache@v2`.
- **S6 MCP server:** wires the `recall` tool via `rmcp` 1.7 over stdio with `#[tool_router]` + `#[tool_handler]` macros.
- **S7 CLI init:** `obelion init {claude,cursor}` with `IdeAdapter` trait + atomic-write (`tmp + rename + .bak`) + `--dry-run` + idempotent `NoChange` on byte-identical content.
