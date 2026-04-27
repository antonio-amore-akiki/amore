# Amore Acceptance Tests

stable: true
topic: amore acceptance tests release gates v0.3.0 v0.5.0 v1.0 live-fire binary contracts
purpose: machine-runnable spec for "when can we tag <version>". Every row is a literal command + expected stdout pattern. The ~/.claude/runtime/guard-hooks/live-fire-verify.mjs hook auto-enforces the v0.3.0 binary contracts on every Stop event; the CI release workflow runs the full table for the matching gate.

## v0.3.0 — production-ready single-node Windows installer + 7-IDE MCP client

| # | binary | command | expected_stdout_pattern (regex) | must_not_contain (any of) |
|---|---|---|---|---|
| 1 | amore.exe | `amore --version` | `^amore \d+\.\d+\.\d+` | `^Error\|panic\|rmcp::\|anyhow::` |
| 2 | amore.exe | `amore --help` | `Usage:.*amore.*(init\|serve\|recall\|status\|doctor)` | `^Error` |
| 3 | amore-mcp.exe | `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' \| amore-mcp` | `"protocolVersion"` | `^Error\|panic\|ConnectionClosed\|not compatible` |
| 4 | amore-gui.exe | `amore-gui --version` | `^amore-gui \d+\.\d+\.\d+` | `^Error\|panic` |
| 5 | Amore-Setup-v0.3.0.exe | `Amore-Setup-v0.3.0.exe /HELP` | `Inno Setup` | (none) |
| 6 | amore.exe | `amore init claude --dry-run` | `^# dry-run: claude ->` | `^Error` |
| 7 | amore.exe | `amore init cursor --dry-run` | `^# dry-run: cursor ->` | `^Error` |
| 8 | amore.exe | `amore init codex --dry-run` | `^# dry-run: codex ->` | `^Error` |
| 9 | amore.exe | `amore init cline --dry-run` | `^# dry-run: cline ->` | `^Error` |
| 10 | amore.exe | `amore init opencode --dry-run` | `^# dry-run: opencode ->` | `^Error` |
| 11 | amore.exe | `amore init windsurf --dry-run` | `^# dry-run: windsurf ->` | `^Error` |
| 12 | amore.exe | `amore init hermes --dry-run` | `^# dry-run: hermes ->` | `^Error` |
| 13 | cargo | `cargo test --workspace --test '*'` | `\d+ passed, 0 failed` | `FAILED` |
| 14 | iscc | `iscc installer/windows/amore.iss` | `Successful compile` | `Error on line` |

Notes on dry-run pattern: `amore init <ide> --dry-run` prints `# dry-run: <ide> -> <config_path>` then the merged config JSON. Pattern `^# dry-run: <ide> ->` matches line 1. Source: `crates/amore-cli/src/main.rs:93`.

## v0.5.0 — adds cross-OS signed binaries + cluster mode + cross-encoder reranker

(placeholder table — rows for macOS .dmg notarized, Linux .AppImage cosign-verified, Qdrant cluster mode docker-compose smoke, reranker ONNX inference latency benchmark, all 7 IDE adapters E2E handshake test)

## v1.0 — adds SLSA L3 + SBOM + Scorecard >=8.0 + 100M corpus load test

(placeholder table — rows for SLSA L3 attestation verify, CycloneDX SBOM exists + non-empty, Scorecard score >=8.0, 100 QPS sustained for 1h @ 10M corpus with cluster, no Hard NO-GO leak in 1h chaos test)

## How the gates fire

- Locally on every Stop event: live-fire-verify.mjs hook reads `~/.claude/policy/binary-contracts.json` and asserts the v0.3.0 row contracts (rows 1-5 above are the binary-contracts.json subset).
- On every PR: CI runs `cargo test --workspace --test '*'` (row 13).
- On every release tag: CI runs the FULL table for the version being tagged.

## Updating the spec

- Add a new binary contract: append to `~/.claude/policy/binary-contracts.json` + add a row here in the matching version section.
- Promote a row from one version to the next: copy the row and adjust `must_not_contain`.
- NEVER weaken `must_not_contain` assertions without a security-reviewer subagent verdict.
- Dry-run pattern source of truth: `crates/amore-cli/src/main.rs` `cmd_init` function — update row patterns if output format changes.
