# Amore Architecture

stable: true
topic: amore architecture crate-map data-flow sequence-diagrams
tag_baseline: v0.3.1-live-fire (commit b0d8815)

## High-level

Amore is a local-first MCP agent memory backbone. The user runs a
single binary (`amore.exe` / `amore-mcp.exe` / `amore-gui.exe`) plus
two managed subprocess deps (Ollama + Qdrant). Multiple IDEs connect
to the same MCP server simultaneously via stdio.

```
+-------+   stdio MCP    +-----------+   embed via HTTP    +----------+
|  IDE  |--------------> | amore-mcp |-------------------> | Ollama   |
+-------+                +-----------+                     +----------+
                                |
                                | gRPC                     +----------+
                                +------------------------->| Qdrant   |
                                                           +----------+
                                |
                                | SQL                      +----------+
                                +------------------------->| SQLite   |
                                                           +----------+
```

## Crate map

| crate | purpose | binary? |
|---|---|---|
| `amore-core` | retrieval, provenance, world-model, mining, ensemble | library |
| `amore-mcp` | MCP stdio server (rmcp 1.7) | yes (`amore-mcp.exe`) |
| `amore-cli` | CLI: `init / serve / recall / status / doctor` | yes (`amore.exe`) |
| `amore-gui` | egui first-run wizard + system-tray icon | yes (`amore-gui.exe`) |
| `amore-adapter-claude` | Claude Code MCP config writer | library |
| `amore-adapter-cursor` | Cursor MCP config writer | library |
| `amore-adapter-codex` | Codex CLI MCP config writer | library |
| `amore-adapter-cline` | Cline (VSCode) MCP config writer | library |
| `amore-adapter-opencode` | opencode MCP config writer | library |
| `amore-adapter-windsurf` | Windsurf MCP config writer | library |
| `amore-adapter-hermes` | Hermes Agent MCP config writer | library |
| `amore-eval` | token-reduction + retrieval-quality eval harness | yes (`amore-eval.exe`) |
| `amore-integration-tests` | binary-spawn integration tests | tests only |

## Data flow: `recall(query, top_k)`

1. IDE sends MCP `tools/call` for `recall` over stdio.
2. `amore-mcp` accepts; bounds-checks `query` ≤ 16 KiB and `top_k` ≤ 100
   (Major 6a fix from v0.3.1).
3. Query is sanitized (alphanumeric tokens for BM25) and embedded via
   Ollama (`nomic-embed-text` 768-dim).
4. **Vector lane**: Qdrant `search(collection, vector, top_k * 4)` over
   the embed dim; returns top candidates by cosine.
5. **BM25 lane**: SQLite FTS5 `MATCH(...) ORDER BY bm25 LIMIT top_k*4`
   (v0.3.x) / Tantivy sharded scan (v0.7.0+).
6. **RRF fusion** at k=60 combines the two ranked lists.
7. **Cross-encoder rerank** (v0.7.0+): top-50 RRF candidates → ONNX
   `bge-reranker-base` → top-K by rerank score.
8. Response: JSON-RPC result with `hits: [{doc_id, score, excerpt}]`.

## Data flow: `canonical_doc_lookup(query)`

1. IDE sends MCP `tools/call` for `canonical_doc_lookup`.
2. `amore-mcp` walks `AMORE_DOCS_PATHS` (default: user-config docs dir)
   for `*.md` files with `stable: true` in first 10 lines.
3. For each candidate, score = token-overlap(query, filename + title +
   `topic:` line + body excerpt).
4. Return top 3 hits as inlined system-reminders.

This is the deterministic source-of-truth router for known domains
where probabilistic recall is the wrong tool.

## Data flow: `observe(observation)`

1. IDE sends `observation` payload.
2. `amore-core::provenance::link()` builds an envelope:
   `{id, prev_hash, canonical_json(payload), ts}`.
3. SHA-256 chain hash computed via length-prefixed
   `id || prev_hash || canonical_json`.
4. SQLite INSERT into `observations` table.
5. Vector embedding submitted to Qdrant in the background batch queue
   (v0.7.0 streaming ingest WAL).

## v0.3.x deployment topology (single-user)

```
            ~/.local/bin (or %LOCALAPPDATA%\Programs\Amore)
             |
             +-- amore.exe
             +-- amore-mcp.exe
             +-- amore-gui.exe
             +-- qdrant.exe (bundled)

           %APPDATA%\Amore  (or ~/.config/amore)
             |
             +-- amore.db                  (SQLite + FTS5)
             +-- qdrant-storage/           (vector index)
             +-- models/bge-small.onnx     (~120 MB)
             +-- security-baselines/<date>.json
```

## v0.7.0 deployment topology (power-user cluster mode)

```
              docker-compose
                |
                +-- qdrant-node-1 (RF=2, shards 0-3)
                +-- qdrant-node-2 (RF=2, shards 4-7)
                +-- qdrant-node-3 (RF=2, shards 8-11)
                +-- amore-mcp (single instance, gRPC server)
```

## Trust boundaries

Detailed in `docs/THREAT-MODEL.md`. Summary:
- IDE ↔ amore-mcp: same-process-tree stdio; no isolation
- amore-mcp ↔ Ollama / Qdrant: localhost only
- amore ↔ user data: user-level file perms; OS disk-encryption
- installer ↔ network: HTTPS + Sigstore (Linux) / self-signed
  (Win, macOS) + SHA-256 pinning for bundled deps

## Build + release

`cargo build --release --workspace` produces the 4 binaries (plus
adapter library crates, not distributed alone). `iscc` packs them
plus Qdrant + ONNX models into `Amore-Setup-vX.Y.Z.exe` (Windows).
macOS + Linux ship as `.tar.gz` + `.AppImage`.

## Out of scope here

- Phase H scale-out internals (Tantivy migration, cluster
  reconciliation): see `docs/SCALE-100M.md`.
- SLO + latency tiers: see `docs/SLO.md`.
- Security threat model: see `docs/THREAT-MODEL.md`.
