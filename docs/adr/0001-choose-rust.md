# 1. Choose Rust as the implementation language

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore is a non-technical-user MCP agent memory backbone shipping a
one-click installer per OS (Windows .exe, macOS .dmg, Linux .AppImage).
The user mandate is "industry grade, 100M users scalable, production
grade, small clean single binary per OS".

Which implementation language gives us that distribution shape without
forcing a runtime install on every user machine?

## Decision Drivers

* Single static binary per OS, target size 5-50 MB
* No GC pause for hot retrieval paths
* Memory safety without runtime overhead
* Strong async / concurrent runtime (Tokio)
* Cross-platform via cargo + cross + matrix builds
* Growing AI ecosystem (Candle, ort, qdrant-client-rs, rmcp)
* Apache-2.0 / MIT crate availability for every layer

## Considered Options

* Rust + Cargo
* Python with PyInstaller / Nuitka
* Go
* Node.js + pkg / Single Executable Applications

## Decision Outcome

Chosen option: **Rust + Cargo**.

### Consequences

* Good: smallest single-static-binary (~5-50 MB) vs PyInstaller bundles
  at 300-500 MB and Node SEAs at ~80-100 MB
* Good: no GC pause; predictable p99 latency for retrieval pipeline
* Good: memory safety + ownership prevents whole classes of vulns
* Good: rmcp 1.7 (official Anthropic Rust MCP SDK) ships server + macros
  + transport-io features
* Good: qdrant-client + ollama-rs + rusqlite all first-class Rust crates
* Bad: Rust learning curve is real; contributor bus-factor risk amplified
* Bad: compile times slower than Go or Node
* Bad: smaller pool of contributors familiar with the language

## Pros and Cons of the Options

### Rust + Cargo

* Good: smallest binary footprint, no GC, memory-safe
* Good: AI ecosystem maturing fast (Candle for LLM, ort for ONNX,
  tantivy for full-text search, qdrant-client for vectors)
* Good: cargo + cross + GitHub Actions matrix gives clean cross-OS CI
* Bad: longer compile times than Go
* Bad: steeper learning curve than Node / Python

### Python with PyInstaller / Nuitka

* Good: largest AI ecosystem
* Good: fastest to prototype
* Bad: PyInstaller bundles 300-500 MB; fails the small-binary mandate
* Bad: slow startup; non-technical-user "Amore takes forever to open"
* Bad: GIL prevents true parallel retrieval; latency unpredictable
* Bad: mem0 (the closest prior-art base) is Python — adopting it forces
  this whole class of problems back in. Rejected per user mandate

### Go

* Good: fast compile times; clean cross-compilation
* Good: small binaries (~10-20 MB)
* Bad: GC pause undermines p99 retrieval latency at scale
* Bad: smaller AI ecosystem than Rust as of 2026
* Bad: rmcp official MCP SDK not available for Go

### Node.js + pkg / Single Executable Applications

* Good: largest contributor pool
* Good: official MCP SDK first-class
* Bad: SEA bundles ~80-100 MB
* Bad: V8 GC pause similar to Go
* Bad: native-module rebuild pain on cross-OS deployment

## More Information

* mem0 (Python) was rejected as a runtime dep per the same mandate
* The Cargo workspace structure will be documented in
  `docs/ARCHITECTURE.md` (landing in v0.4.0)
* Compile-time concern is mitigated by `[profile.release]` with
  `lto = true` + `codegen-units = 1` accepted as a one-time cost
