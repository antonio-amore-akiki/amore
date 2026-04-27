# Introduction

Amore is a local-first MCP memory backbone for AI coding assistants.

## What problem it solves

AI coding assistants start every session cold — no memory of the codebase decisions you explained
last week, the debugging session that took three hours, or the style preferences you established
over months. Amore gives your assistant persistent, searchable memory that stays on your machine.

## What makes it different

- **Local-first.** All data stays on disk in a Qdrant + SQLite store under `~/.local/share/amore/`.
  No cloud account, no API key, no data leaving your machine.
- **Zero-daemon.** The MCP server starts on demand and exits when your IDE closes. No background
  process, no startup item.
- **MCP-first.** Every IDE-facing operation goes through the [Model Context Protocol](https://modelcontextprotocol.io/)
  stdio transport. Claude Code, Cursor, Codex, Cline, opencode, Windsurf, and Hermes all
  connect via the same interface.
- **Hybrid recall.** Observations are indexed in both a dense vector store (semantic search via
  Qdrant + an embedding model) and a BM25 full-text index (Tantivy). Recall merges both lanes,
  so exact-match and fuzzy-semantic queries both work.
- **Tamper-evident.** Every observation carries a SHA-256 provenance chain so you can verify the
  audit trail has not been modified.

## What it is not

Amore does not run inference. It stores and retrieves observations; your IDE's AI model reads them
and decides what to do with them. Amore is infrastructure, not intelligence.

## Where to start

- [Install in 60 seconds](./quickstart/install.md) — pick your package manager.
- IDE-specific setup: [Claude Code](../quickstart/claude.md) · [Cursor](../quickstart/cursor.md)
  · [Codex](../quickstart/codex.md) · [Cline](../quickstart/cline.md)
  · [opencode](../quickstart/opencode.md) · [Windsurf](../quickstart/windsurf.md)
  · [Hermes](../quickstart/hermes.md)
- [Architecture overview](../ARCHITECTURE.md) — internal structure and data flow.
- [Threat model](../THREAT-MODEL.md) — what Amore protects and what it explicitly does not.
