<!-- stable: true -->
# Feature Flags (Meta Gatekeeper pattern)

Amore separates code deployment from capability activation per
engineering.fb.com/2017/08/31/web/rapid-release-at-massive-scale.
Rollback is a flag toggle, not a code revert.

## Layers

**Compile-time (Cargo features)** — gates major risk surfaces. Default-on:
- `rerank-onnx` — ort reranker; off disables `reranker` module
- `tantivy-bm25` — Tantivy index; off disables `tantivy_index` + `porter1` modules
- `compaction-worker` — background compaction; off means manual only
- `wal-sync` — fsync per WAL write; off means batched
- `metrics-exporter` — Prometheus :9090; off means no export

Build with all defaults: `cargo build --bin amore`

Build with specific features: `cargo build --bin amore --features rerank-onnx,tantivy-bm25`

Build with nothing: `cargo build --bin amore --no-default-features`

**Runtime (env / file)** — gates capability rollouts. Env overrides file.
- Env: `AMORE_FLAG_<NAME>=on|off` (e.g., `AMORE_FLAG_RERANKER_V2=on`)
- File: `$AMORE_FLAGS_FILE` JSON `{"reranker_v2": "on", "new_ranking": "off"}`

## Inspect

```
amore flags           # human-readable
amore flags --json    # machine-readable
```

## Usage in Rust

```rust
if amore_core::flags::Flags::is_enabled("reranker_v2") {
    // new path
} else {
    // legacy path
}
```

## ADR

`docs/adr/0014-feature-flags.md` — Build decision; SaaS clients overkill for
single-author scope; std-only 80 LoC resolver preferred.
