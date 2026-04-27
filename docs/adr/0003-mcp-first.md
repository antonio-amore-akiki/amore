# 3. MCP as the primary IDE integration surface

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore must integrate with multiple IDEs at once: Claude Code, Cursor,
Codex CLI, Cline, opencode, Windsurf, Hermes Agent. Each IDE has its
own plugin model. We need ONE protocol surface that every IDE can talk
to, so that we ship one server and seven thin adapters rather than
seven implementations of the same logic.

## Decision Drivers

* Industry-converging open standard (as of 2026)
* Typed JsonSchema tool surface — clients can introspect
* Stdio-default transport — no network listener attack surface
* Server-side implementation language flexibility (Rust)
* Existing official SDK (rmcp 1.7)
* IDE ecosystem adoption: at least 7 IDEs ship MCP support natively

## Considered Options

* Model Context Protocol (MCP) — Anthropic's open standard
* Custom REST API
* LSP-style extension protocol
* Per-IDE plugin (one implementation per IDE)

## Decision Outcome

Chosen option: **MCP via rmcp 1.7**.

`amore-mcp` is a stdio MCP server. Each IDE adapter (`amore-adapter-*`)
generates the per-IDE config file pointing at the server binary; the
IDE spawns it on demand. No always-on daemon, no listener, no port.

### Consequences

* Good: stdio-only by default = zero network attack surface
* Good: typed JsonSchema tool surface means client-side introspection
  works across all 7 IDEs without per-IDE schema glue
* Good: rmcp 1.7 with `server`, `macros`, `transport-io` features =
  ~200 LOC server impl for the recall + canonical-doc-lookup tools
* Good: 7 IDE adapters share the same server; bug-fixes land once
* Bad: MCP spec still maturing; semantics for streaming tool calls
  shifted between protocol versions 2024-11-05 and 2025-06-18
* Bad: gRPC fallback (Phase H, v0.7.0) needed for shared-multi-client
  scenarios; adds a second transport surface to maintain

## Pros and Cons of the Options

### MCP via rmcp 1.7

* Good: industry-converging standard, 7+ IDEs support it natively in 2026
* Good: stdio default = no listener; gRPC opt-in for power users
* Good: Anthropic-maintained official SDK
* Bad: protocol version churn — `amore-mcp` must speak `2024-11-05`
  while negotiating with clients on `2025-06-18`
* Bad: streaming tool calls (Phase H) require careful protocol version
  pinning

### Custom REST API

* Good: maximum control over the wire format
* Bad: every IDE adapter has to bridge to JSON over HTTP — duplication
* Bad: HTTP server = network listener = attack surface
* Bad: zero alignment with the IDE ecosystem direction in 2026

### LSP-style protocol

* Good: well-understood by IDE teams
* Bad: LSP is editor-feature-focused (completion, hover, refs), not
  agent-tool-focused; semantic mismatch
* Bad: less ecosystem alignment than MCP for agent tooling

### Per-IDE plugin

* Bad: 7 implementations of the same recall logic
* Bad: bug fixes need 7 PRs against 7 plugin stores
* Bad: completely fails the user mandate "ship once, run everywhere"

## More Information

* The 7 adapters live in `crates/amore-adapter-{claude,cursor,codex,
  cline,opencode,windsurf,hermes}/` — each is ~50-150 LOC.
* The shared IdeAdapter trait is in `crates/amore-core/src/ide_adapter.rs`.
* MCP tool surface inventory:
  - `recall(query, top_k)` — hybrid retrieval (vector + BM25 RRF)
  - `canonical_doc_lookup(query)` — topic-match over stable: true docs
  - `observe(...)` — provenance-stamped writes (Phase H)
  - `world_model_query(...)` — typed graph projections (Phase H)
  - `ensemble_decide(...)` — multi-agent vote orchestrator (v0.4.0)
  - `eig_question(...)` — clarification question selection (v0.4.0)
  - `provenance_verify(...)` — chain integrity check
