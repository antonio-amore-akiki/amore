# obelion

stable: true

Universal agent memory backbone. MCP server + per-IDE adapters. Cross-platform, cross-IDE, production-grade.

> **Note**: The name `obelion` is reused from an earlier failed experiment (a JS megamerge of 5 forks, now archived at `Gmail_Transformer/_archive/obelion-failed-experiment/`). This is a clean Rust rewrite — same name, completely different shape.

Compatible with Claude Code, Cursor, Codex CLI, Cline, Continue, Roo, Zed — anything that speaks MCP (Model Context Protocol).

## Status

Pre-alpha (v0.1.0). Skeleton committed 2026-05-25. See plan: `~/.claude/plans/first-make-an-audit-transient-wind.md`.

## Architecture

- **Language**: Rust (single small binary per OS, no GC, fast)
- **Vector store**: Qdrant (self-hosted, production scale)
- **LLM + embeddings**: Ollama (free unlimited local; `qwen2.5:7b` + `nomic-embed-text`)
- **Metadata + BM25**: SQLite with bundled FTS5
- **MCP SDK**: `rmcp` 1.7 (official Anthropic Rust SDK)

## Differentiator layer (beyond mem0 / Letta / Zep)

- Canonical-docs router (`docs/<topic>.md` with `stable: true` headers — deterministic before probabilistic recall)
- Multi-agent ensemble (Architect / Skeptic / Historian / Reviewer / Negotiator / Implementer) with credit assignment
- Expected Information Gain (EIG) question selection
- Cryptographic provenance (sha256 envelope chains, portable-agent-memory paper spec)
- Adversarial-test mining (failures auto-become regression tests)
- World-model namespace (projects + revealed preferences + tool reliability + threat model)

## Distribution (when ready)

- npm `@anto/obelion` (universal wrapper)
- GitHub Releases (signed binaries per OS)
- Homebrew, winget, AppImage
- Marketplace listings: Claude Code, Cursor, Codex

## License

Apache-2.0
