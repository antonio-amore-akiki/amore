---
stable: true
topic: lancedb-rust-embedded-vector-store-adoption-replaces-qdrant-daemon
---

# ADR-0016: Adopt LanceDB as embedded vector-store backend (Qdrant migration)

**Date**: 2026-05-27
**Status**: Accepted (parallel implementation; full Qdrant replacement v1.2)
**Decision driver**: deep-research pass verdict 2026-05-27 (sources cited inline)

## Context

Amore's current vector lane is `qdrant-client` 1.18.0 → external Qdrant daemon on `localhost:6334`. Daemon dependency:
- Forces users to install + run Qdrant separately (or use bundled-installer Windows MSI path)
- Adds first-run wizard step (qdrant health-probe in `crates/amore-gui/src/wizard/screens.rs`)
- Conflicts with "truly local-first single-process" elite-engineering claim

## Alternatives evaluated

Per deep-research 2026-05-27 (research pass ID `a68d94c257db462f7`):

| Candidate | License | Native Rust | Embedded | Verdict |
|---|---|---|---|---|
| **LanceDB** ([docs.rs/lancedb](https://docs.rs/lancedb/latest/lancedb/)) | Apache-2.0 | ✅ `lancedb` crate | ✅ in-process | **Adopt** |
| Qdrant (status quo) | Apache-2.0 | client only | ❌ daemon | retain as adapter parallel |
| ChromaDB | Apache-2.0 | ❌ Python | ✅ | reject (Python sidecar) |
| Marqo | Apache-2.0 | ❌ Python | ❌ daemon | reject |
| LightRAG | MIT | ❌ Python | ❌ | reject (orthogonal — graph+RAG, not vector store) |

## Decision

**Adopt LanceDB as the primary embedded vector-store backend**; keep Qdrant adapter behind a feature flag for users who already run Qdrant at scale.

### Why LanceDB beats alternatives for Amore specifically

1. **Karpathy subtraction** — every other candidate ADDS a sidecar; LanceDB REMOVES the Qdrant daemon. Narrows architecture rather than expanding.
2. **Native Rust** — `cargo add lancedb` is the entire install story. No Python runtime, no Docker, no port conflicts.
3. **Apache-2.0** — license-compatible with Amore's own Apache-2.0 stance.
4. **Columnar Lance format** — supports vector + full-text + SQL hybrid search in-process; complements (does not replace) tantivy BM25 lane.
5. **Performance** — Lance team benchmarks 1.5M IOPS (2026); production deployments at Geneva-region + ByteDance per [github.com/lancedb/lancedb](https://github.com/lancedb/lancedb).

## Risks

- **Rust API marked "not yet stable, breaking changes expected"** as of v0.29 (early 2026). Mitigation: pin minor version + keep Qdrant adapter as fallback until LanceDB hits 1.0.
- **Schema-migration path** — sled WAL contains qdrant point IDs; one-shot migration tool needed for existing user data. Mitigation: bump to v1.2.0 (semver minor → schema migration documented in `docs/MIGRATION-v1.1-to-v1.2.md`).

## Implementation plan (executor-ready)

1. Add `lancedb = "0.29"` to `[workspace.dependencies]` in `Cargo.toml`
2. Define `pub trait VectorStore` in `crates/amore-core/src/vector_store/mod.rs` with current `qdrant_pool.rs` behavior as default impl
3. Create `crates/amore-core/src/vector_store/lancedb_backend.rs` implementing the trait
4. Add feature flags: `vector-qdrant` (default) | `vector-lancedb` | `vector-both`
5. Wizard step: detect Qdrant daemon → fall through to LanceDB if absent (no daemon required)
6. Migration tool: `amore migrate-vector --from qdrant --to lancedb` reads from running Qdrant + writes Lance dataset

Estimated wall-clock: ~1-2 weeks single-author including regression tests against `docs/perf-baseline-v0.6.0.tsv` + LongMemEval re-measure.

## Tracking

- Upstream: [github.com/lancedb/lancedb](https://github.com/lancedb/lancedb) — watch for 1.0 stable
- Re-attempt full Qdrant removal when LanceDB Rust API marked stable
- Bench parity gate: LongMemEval R@5 ≥ 1.0 (matches v0.5.1 mock-stack measurement) AND p99 ≤ Qdrant baseline + 10%
