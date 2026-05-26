# Obelion concurrency model

stable: true

## SQLite (BM25 + provenance + world-model)

- **WAL journal mode**: enabled at every `SqliteStore::open` via
  `PRAGMA journal_mode=WAL`. Allows concurrent readers + a single writer
  without `SQLITE_BUSY` panics under contention.
- **`synchronous = NORMAL`**: paired with WAL — durable across application
  crashes, only fsyncs at checkpoint. Standard recommendation from the
  SQLite docs.
- **`busy_timeout = 5s`**: if a writer holds the file lock, callers wait
  up to 5 s before returning `SQLITE_BUSY`. Real obelion writes complete
  in microseconds, so this is generous; it just prevents fast-failing
  under transient contention.
- **`BEGIN IMMEDIATE` for chain writes**: `insert_observation` reads the
  current chain head AND writes the new row inside one
  `TransactionBehavior::Immediate` transaction. That acquires the SQLite
  reserved + pending lock at `BEGIN`, blocking other writers until commit
  — without it, two concurrent writers see the same head and the chain
  forks. Cross-process safety on top of the WAL layer.
- **`Mutex<Connection>` field**: intra-process serialization. Combined with
  WAL + IMMEDIATE, multiple processes on the same machine sharing one
  data dir behave correctly (4-writer × 250-row stress test verified —
  see `crates/core/tests/concurrency_writes.rs`).

## Qdrant (vector lane)

Qdrant's gRPC server handles its own concurrency internally — clients
(including obelion's `QdrantStore`) just issue concurrent `upsert` /
`search` RPCs. The qdrant-client crate uses tonic / hyper under the hood;
each call is independent and parallel-safe.

## Ollama (embeddings)

Ollama's HTTP API on `:11434` handles concurrent embed requests serially
on the model worker. Obelion's `OllamaClient` uses `reqwest`, which is
fully async + parallel-safe. No locking on the obelion side.

## MCP server (one server, many IDE clients)

`obelion-mcp` holds one `Arc<HybridRecall>` shared across all client
sessions. The struct's internal `Mutex<Connection>` (SQLite) +
gRPC client (Qdrant) + reqwest client (Ollama) are all
parallel-safe. The MCP recall tool is intentionally stateless — each
`tools/call recall` performs one search and returns the envelope; no
session affinity required.

## Multi-process posture

Recommended deployment: one `obelion-mcp` per machine, serving all the
machine's IDEs. The CLI + MCP-server processes can coexist on the same
data dir (e.g. user runs `obelion recall "x"` while their IDE has the
MCP server attached). The SQLite WAL layer plus the IMMEDIATE-tx chain
write keeps that safe.

Heavy multi-machine fan-out (one obelion-mcp per replica writing the
same data dir over NFS) is **NOT** supported in v0.x — file-locking
semantics over NFS are too unreliable. v0.5+ ships qdrant cluster mode
for cross-machine vector replication; the BM25 lane stays local-only
per machine.
