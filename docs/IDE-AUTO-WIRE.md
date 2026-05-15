---
stable: true
topic: ide-auto-wire
---
# IDE Auto-Wire

How Amore automatically wires itself into each supported AI tool's MCP config.

## Supported Tools

| Tool | Config format | mcpServers shape | Source |
|------|--------------|------------------|--------|
| Claude Desktop | JSON | Object | modelcontextprotocol.io/quickstart/user |
| Claude Code | JSON | Object | code.claude.com/docs/en/mcp |
| Cursor | JSON | Object | forum.cursor.com/t/what-are-the-capabilities-of-mcp-json/63130 |
| Cline | JSON | Object | docs.cline.bot/mcp/configuring-mcp-servers |
| Continue | YAML | **Array** | docs.continue.dev/customize/deep-dives/mcp |

> Critical: Continue uses an **array** for `mcpServers`, not an object. Amore generates
> client-specific config blobs — not a single shared JSON.

---

## Per-IDE Config Schemas

### Claude Desktop / Claude Code / Cursor / Cline (JSON object)

```json
{
  "mcpServers": {
    "amore": {
      "command": "amore-mcp",
      "args": ["--stdio"],
      "env": {}
    }
  }
}
```

### Continue (YAML array)

```yaml
mcpServers:
  - name: amore
    command: amore-mcp
    args:
      - --stdio
    env: {}
```

---

## Wire-Up Steps (per IDE)

1. Read existing config file
2. Parse (JSON or YAML)
3. Save backup: `<config>.bak-<YYYYMMDDTHHMMSSz>`
4. Merge `amore` entry (warns + overwrites if `amore` already exists)
5. Write to tmp file in the same directory
6. Atomic `rename` tmp → original (same-filesystem, POSIX-safe)
7. Verify by re-parsing the written file

Implementation: `crates/amore-gui/src/ide_wire.rs`

---

## Backup and Rollback

Each wire-up creates a backup before mutating the config. To restore:

```
# JSON tools (Claude Desktop, Claude Code, Cursor, Cline)
copy claude_desktop_config.json.bak-20260527T004400Z claude_desktop_config.json

# Continue (YAML)
copy config.yaml.bak-20260527T004400Z config.yaml
```

Backup files are named `<original>.<ext>.bak-<ISO-timestamp>`. They are not auto-deleted.

---

## Manual Override (Tools Not Auto-Detected)

If a tool's config file is not at the default location, add the amore entry manually.

For **JSON tools** (Claude Desktop / Claude Code / Cursor / Cline): add under `mcpServers`:

```json
"amore": {
  "command": "amore-mcp",
  "args": ["--stdio"],
  "env": {}
}
```

For **Continue** (`~/.continue/config.yaml`): append to `mcpServers` array:

```yaml
- name: amore
  command: amore-mcp
  args: ["--stdio"]
  env: {}
```

---

## Tests

- `crates/amore-gui/tests/ide_detect_tests.rs` — 5 tests (one per IDE, tempfile fixtures)
- `crates/amore-gui/tests/ide_wire_tests.rs` — 5 tests (backup + merge + preservation)
