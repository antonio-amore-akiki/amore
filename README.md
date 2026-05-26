# Amore

stable: true

Universal MCP agent memory backbone. Rust core, cross-platform, cross-IDE.

Compatible with **Claude Code, Cursor, Codex CLI, Cline, opencode, Windsurf, Hermes Agent** out of the box — and any other MCP-capable client via the raw stdio endpoint.

## Status

[![Release](https://img.shields.io/github/v/release/antonio-amore-akiki/amore?include_prereleases)](https://github.com/antonio-amore-akiki/amore/releases) [![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**v0.2.0** — substrate full stack live + Sigstore-signed Linux deploy. Per-step audit trail in [`docs/results.tsv`](docs/results.tsv).

## Install

### Via npm (universal — Tier-1 path)

```bash
npm install -g @anto/amore
amore init claude       # or: cursor, codex, cline, opencode, windsurf, hermes
```

The npm `postinstall` step fetches the matching signed binary from the [GitHub Release](https://github.com/antonio-amore-akiki/amore/releases). On Linux, if [`cosign`](https://docs.sigstore.dev/cosign/installation/) is on your PATH, the Sigstore bundle is verified before extraction.

### Manual — Tier-2 path

```bash
# Pick your tag + target, e.g. v0.2.0 / x86_64-unknown-linux-gnu
curl -L https://github.com/antonio-amore-akiki/amore/releases/download/v0.2.0/amore-v0.2.0-x86_64-unknown-linux-gnu.tar.gz | tar -xz
./amore init claude
```

Verify the Sigstore signature before running:

```bash
cosign verify-blob --bundle amore-v0.2.0-x86_64-unknown-linux-gnu.tar.gz.bundle \
  --certificate-identity-regexp 'https://github\.com/antonio-amore-akiki/amore/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  amore-v0.2.0-x86_64-unknown-linux-gnu.tar.gz
```

## Architecture

| Layer       | Choice                                                                 |
|-------------|------------------------------------------------------------------------|
| Storage     | SQLite + FTS5 (BM25) + Qdrant gRPC vector store                        |
| Embeddings  | Ollama + `nomic-embed-text` (768-dim, free local)                      |
| Retrieval   | BM25 + vector + canonical-docs router, fused via RRF k=60              |
| Provenance  | canonical_json + length-prefixed SHA-256 envelope chain                |
| MCP server  | `rmcp` 1.7 over stdio — exposes `recall` + `canonical_doc_lookup`      |
| World model | Persistent typed graph: projects + tool_reliability + revealed_prefs   |
| Ensemble    | Architect + Skeptic (S14a). Historian/Reviewer/Negotiator/Implementer next. |

## What's in v0.2.0

- S1-S5: cryptographic provenance + real Ollama embed + Qdrant search + HybridRecall vector path + cross-OS CI matrix
- S6: MCP server wires `recall` tool via rmcp 1.7 over stdio
- S7: `amore init claude/cursor` CLI + IdeAdapter trait + atomic-write contract
- S8: BM25 (FTS5) + RRF k=60 fusion + canonical-docs router + `canonical_doc_lookup` MCP tool
- S9: 5 more IDE adapters — codex (TOML) + cline + opencode + windsurf + hermes (YAML)
- S10a: Sigstore-signed Linux binaries via GitHub OIDC keyless
- S11a: `@anto/amore` npm wrapper with optional cosign verify
- S12: World-model substrate (4 SQLite tables, Bayesian preference updates)
- S14a: Multi-agent ensemble orchestrator + 2 agent roles

## On deck for v0.3.0 -> v0.5.0

- S10b: Windows Authenticode (pending user EV cert)
- S10c: macOS notarization (pending user Apple Dev ID)
- S13: Token-reduction eval harness with frozen fixture set
- S14b/c: Remaining 4 agent roles + EIG question selection + SQLite vote persistence + credit assignment
- S15: Adversarial-test mining from edit-rationale ledger
- S11b/c: Homebrew tap + winget manifest
- S17: Docs site (mdBook on GitHub Pages) + Anthropic plugin marketplace listing

## Test posture

```
cargo fmt --check                                  # workspace fmt
cargo clippy --workspace --all-targets -- -D warnings   # zero warnings
cargo test --workspace                             # 85 unit tests green at v0.2.0

AMORE_TEST_OLLAMA=1 cargo test -p amore-core --test ollama_embed -- --ignored
AMORE_TEST_QDRANT=1 cargo test -p amore-core --test qdrant_roundtrip -- --ignored
AMORE_TEST_E2E=1    cargo test -p amore-core --test hybrid_e2e -- --ignored
AMORE_TEST_MCP=1    cargo test -p amore-mcp  --test mcp_handshake -- --ignored
```

Integration tests gate on live Ollama (`127.0.0.1:11434`) + Qdrant (gRPC `127.0.0.1:6334`) daemons.

## Upgrade from obelion

If you previously installed `obelion`, the first `amore-mcp` start automatically migrates your data:
- `%APPDATA%/obelion/obelion.db` (Windows) is copied to `%APPDATA%/Amore/amore.db`
- A `migrated-from-obelion.txt` marker is written so the migration runs only once
- Legacy `OBELION_*` env vars are still accepted with a deprecation warning (removed in v0.4.0)

## License

[Apache-2.0](LICENSE). Direct-dep attribution in [NOTICE](NOTICE).

> Previously named `obelion` (renamed to Amore in v0.2.1 for the v1.0 product launch).
> The original `obelion` was an archived JS megamerge experiment — this Rust rewrite is a clean slate.
