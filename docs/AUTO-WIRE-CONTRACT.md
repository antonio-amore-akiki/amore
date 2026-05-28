<!-- topic: auto-wire headless JSON contract schema for IDE MCP wiring -->
<!-- stable: true -->
# Auto-Wire JSON Contract

Machine-readable schema: [`schema/auto-wire-contract.schema.json`](../schema/auto-wire-contract.schema.json)

## `amore-mcp --register-claude-code --self-contained`

Registers the MCP server with Claude Code non-interactively and emits a single JSON
object on stdout conforming to `schema/auto-wire-contract.schema.json` (JSON Schema draft-07).

```json
{
  "detected": [{"id": "claude-code", "config_path": "/home/user/.claude.json"}],
  "wired":    [{"id": "claude-code", "method": "atomic-write"}],
  "skipped":  [],
  "errors":   []
}
```

| Field      | Type   | Meaning |
|------------|--------|---------|
| `detected` | array  | IDEs found on disk; each `{id, config_path}` |
| `wired`    | array  | IDEs successfully registered; each `{id, method}` |
| `skipped`  | array  | IDEs already containing an identical entry; each `{id, reason}` |
| `errors`   | array  | IDEs where wiring failed; each `{id, error}` |

Exit 0 when `errors == []`; exit 1 otherwise.

## `amore-gui --auto-wire` (legacy)

`amore-gui --auto-wire` runs headless IDE detection and MCP config wiring, then
exits without launching any GUI or display system.

## JSON schema

Emitted as a single JSON object on stdout (last line), UTF-8:

```json
{
  "detected": ["<ide-name>", ...],
  "wired":    ["<ide-name>", ...],
  "skipped":  ["<ide-name>", ...],
  "errors":   [{"ide": "<ide-name>", "error": "<message>"}, ...]
}
```

| Field      | Type            | Meaning |
|------------|-----------------|---------|
| `detected` | array of string | IDE names whose config file was found on disk |
| `wired`    | array of string | IDEs successfully updated (MCP entry inserted/overwritten) |
| `skipped`  | array of string | IDEs whose config already contained an identical entry |
| `errors`   | array of object | IDEs where wiring failed; each object has `ide` and `error` keys |

Every detected IDE appears in exactly one of `wired`, `skipped`, or `errors`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0    | `errors == []` — all detected IDEs wired or skipped |
| 1    | `errors` is non-empty — at least one IDE failed to wire |

## Example: clean machine with Claude Code installed

```json
{"detected":["Claude Code"],"wired":["Claude Code"],"skipped":[],"errors":[]}
```

Exit code: `0`

## Example: no IDEs detected

```json
{"detected":[],"wired":[],"skipped":[],"errors":[]}
```

Exit code: `0`

## Example: wiring failed for one IDE

```json
{"detected":["Claude Code","Cursor"],"wired":["Cursor"],"skipped":[],"errors":[{"ide":"Claude Code","error":"sibling amore-mcp not found: expected at /usr/local/bin/amore-mcp"}]}
```

Exit code: `1`

## Headless safety

`--auto-wire` is handled BEFORE `eframe::run_native(...)` in `main.rs`. No winit,
X11, or Wayland surface is initialized. Safe to invoke with `DISPLAY` unset on Linux
(e.g., from an installer postinst script or a CI container without a display server).
