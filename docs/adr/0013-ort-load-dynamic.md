---
stable: true
---
# ADR 0013 — ort load-dynamic feature for MSVC CRT mismatch

**Status**: accepted
**Date**: 2026-05-26

## Context

`ort 2.0.0-rc.12` is the only viable ort 2.x release on crates.io (ort 1.x is yanked as
unsafe; see github.com/pykeio/ort/discussions/501). When linking on Windows MSVC, the static
lib bundled with ort is compiled with `/MD` (dynamic CRT), but at least one transitive
dependency compiles with `/MT` (static CRT). The result is:

```
error LNK2038: mismatch detected for 'RuntimeLibrary': MT_StaticRelease != MD_DynamicRelease
```

Root cause tracked at github.com/pykeio/ort/issues/329 (closed 2024-12-02 as not-planned for
the static linking path). Downgrading to ort 1.x is not viable (yanked, security issues).

## Decision

Enable the `load-dynamic` feature on the ort dependency. This disables ort-sys static linkage
entirely (`disable-linking` flag in ort-sys) and instead loads `onnxruntime.dll` at runtime
via `libloading`. The CRT mismatch disappears because no ort objects are linked at compile
time.

Feature set chosen: `load-dynamic`, `std`, `ndarray`, `tracing`, `api-24`.
`api-24` is the highest API level in rc.12 (`api-26` from the docs does not exist in this
release). `download-binaries`, `copy-dylibs`, and `tls-native` are excluded to avoid
pulling in unnecessary network/crypto deps and to keep the binary self-contained.

## Runtime requirement

The binary requires `ORT_DYLIB_PATH` to point to a valid `onnxruntime.dll` at process
launch time. The reranker init (`ensure_ort_init()` in `amore-core/src/reranker.rs`) fails
fast with a clear diagnostic if the env var is missing.

For local development: `.cargo/config.toml` sets a default path under `vendor/onnxruntime/`
(gitignored). Developers must place `onnxruntime.dll` there (v1.20.1 from
github.com/microsoft/onnxruntime/releases).

For release builds: the installer or launch wrapper sets `ORT_DYLIB_PATH` to the bundled DLL
path. The release script copy step is a TODO tracked in the CHANGELOG.

## Alternatives considered

| Option | Reason rejected |
|--------|----------------|
| ort 1.16 | Yanked + has known safety issues (discussions/501) |
| Remove ONNX reranker | Drops H.3 cross-encoder reranking; nDCG@10 improvement is core value |
| Patch transitive dep CRT flags | No practical control over downstream `fastembed` or CUDA deps build flags |
| ort `alternative-backend` feature | Only relevant when supplying a fully custom session backend; not applicable here |

## Consequences

- `onnxruntime.dll` must be distributed alongside the binary. Not committed to the repo.
- `vendor/onnxruntime/` is gitignored.
- `ORT_DYLIB_PATH` must be set at runtime. Missing var = explicit error at Reranker init.
- No change to the model path or ONNX session API — only the linkage mechanism changes.
