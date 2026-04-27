---
stable: true
---

# ADR-0015: tonic + reqwest major-version bump blocked by qdrant-client

**Date**: 2026-05-27
**Status**: Accepted (deferred to qdrant-client 1.19+)
**Scope**: v-next #34 (4 major-version migrations: eframe / tantivy / reqwest / tonic)

## Context

Task #34 required bumping 4 major-version deps to clear OSV Scorecard gaps:

| Dep | From | Latest | Status |
|---|---|---|---|
| `eframe` | 0.29 | **0.34.3** | ✅ Migrated |
| `tantivy` | 0.22 | **0.26.1** | ✅ Migrated |
| `qdrant-client` | 1.15 | **1.18.0** | ✅ Bumped (transitive) |
| `reqwest` | 0.12 | 0.13.4 | ❌ BLOCKED |
| `tonic` | 0.12 | 0.14.6 | ❌ BLOCKED |
| `prost` | 0.13 | 0.14.3 | ❌ BLOCKED (transitive of tonic) |

## Blocker

`qdrant-client` 1.18.0 (latest as of 2026-05-27) pins transitive deps:
- `tonic = "^0.12.3"` (per https://crates.io/api/v1/crates/qdrant-client/1.18.0/dependencies)
- `prost = "^0.13.3"`
- `reqwest = "^0.12.8"`

Bumping `reqwest` to `0.13` OR `tonic` to `0.14` would cause cargo resolver to either fail (semver incompatible) or pull both versions (dual-version compile errors).

## Decision

**Defer reqwest 0.12 → 0.13 and tonic 0.12 → 0.14 until qdrant-client 1.19+ upgrades its transitive constraints.**

The 3 successful migrations (eframe + tantivy + qdrant-client minor) close the majority of #34's value; the remaining 2 wait on upstream.

## Tracking

- Upstream: https://github.com/qdrant/rust-client (file v-next #36-followup to track qdrant 1.19 release ETA)
- Re-attempt trigger: `cargo search qdrant-client --limit 1` returns `>= 1.19.0` AND `cargo info qdrant-client@1.19.0` shows tonic/reqwest constraints loosened
- OSV impact: 2 of N advisories remain warned (reqwest 0.12 + tonic 0.12 transitive chain); current `cargo audit` exits 0 with warnings only — not blocking GA

## Rejected alternatives

- **Vendor qdrant-client locally + patch its Cargo.toml**: Build verdict per Karpathy — duplicates upstream maintenance burden; first upstream bump becomes a merge chore.
- **Fork qdrant-client + ship our own crate**: Same Build cost; abandons the parity guarantee.
- **Stay on reqwest 0.12 + tonic 0.12 + bump only eframe + tantivy + qdrant-client**: ✅ Chosen — minimum-change Karpathy bias, parity preserved, 3/5 deps modernized today.
