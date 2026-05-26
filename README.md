# obelion

stable: true

Universal MCP agent memory backbone. Rust core, cross-platform, cross-IDE.

Compatible with **Claude Code, Cursor, Codex CLI, Cline, opencode, Windsurf, Hermes Agent** out of the box — and any other MCP-capable client via the raw stdio endpoint.

## Status

[![Release](https://img.shields.io/github/v/release/antonio-amore-akiki/obelion?include_prereleases)](https://github.com/antonio-amore-akiki/obelion/releases) [![License](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

**v0.2.0** — substrate full stack live + Sigstore-signed Linux deploy. Per-step audit trail in [`docs/results.tsv`](docs/results.tsv).

## Install

### Via npm (universal — Tier-1 path)

```bash
npm install -g @anto/obelion
obelion init claude       # or: cursor, codex, cline, opencode, windsurf, hermes
```

The npm `postinstall` step fetches the matching signed binary from the [GitHub Release](https://github.com/antonio-amore-akiki/obelion/releases). On Linux, if [`cosign`](https://docs.sigstore.dev/cosign/installation/) is on your PATH, the Sigstore bundle is verified before extraction.

### Manual — Tier-2 path

```bash
# Pick your tag + target, e.g. v0.2.0 / x86_64-unknown-linux-gnu
curl -L https://github.com/antonio-amore-akiki/obelion/releases/download/v0.2.0/obelion-v0.2.0-x86_64-unknown-linux-gnu.tar.gz | tar -xz
./obelion init claude
```

Verify the Sigstore signature before running:

```bash
cosign verify-blob --bundle obelion-v0.2.0-x86_64-unknown-linux-gnu.tar.gz.bundle \
  --certificate-identity-regexp 'https://github\.com/antonio-amore-akiki/obelion/' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  obelion-v0.2.0-x86_64-unknown-linux-gnu.tar.gz
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

- ✅ S1–S5: cryptographic provenance + real Ollama embed + Qdrant search + HybridRecall vector path + cross-OS CI matrix
- ✅ S6: MCP server wires `recall` tool via rmcp 1.7 over stdio
- ✅ S7: `obelion init claude/cursor` CLI + IdeAdapter trait + atomic-write contract
- ✅ S8: BM25 (FTS5) + RRF k=60 fusion + canonical-docs router + `canonical_doc_lookup` MCP tool
- ✅ S9: 5 more IDE adapters — codex (TOML) + cline + opencode + windsurf + hermes (YAML)
- ✅ S10a: Sigstore-signed Linux binaries via GitHub OIDC keyless
- ✅ S11a: `@anto/obelion` npm wrapper with optional cosign verify
- ✅ S12: World-model substrate (4 SQLite tables, Bayesian preference updates)
- ✅ S14a: Multi-agent ensemble orchestrator + 2 agent roles

## On deck for v0.3.0 → v0.5.0

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

OBELION_TEST_OLLAMA=1 cargo test -p obelion-core --test ollama_embed -- --ignored
OBELION_TEST_QDRANT=1 cargo test -p obelion-core --test qdrant_roundtrip -- --ignored
OBELION_TEST_E2E=1    cargo test -p obelion-core --test hybrid_e2e -- --ignored
OBELION_TEST_MCP=1    cargo test -p obelion-mcp  --test mcp_handshake -- --ignored
```

Integration tests gate on live Ollama (`127.0.0.1:11434`) + Qdrant (gRPC `127.0.0.1:6334`) daemons.

## License

[Apache-2.0](LICENSE). Direct-dep attribution in [NOTICE](NOTICE).

> The name `obelion` is reused from an archived JS megamerge experiment (`Gmail_Transformer/_archive/obelion-failed-experiment/`). This is a clean Rust rewrite — same name, completely different shape.
