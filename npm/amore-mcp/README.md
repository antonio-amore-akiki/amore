# amore-mcp

Amore MCP server npm wrapper — agent memory backbone for Claude Code, Cursor, Cline, Windsurf, and any MCP-compatible IDE.

## Install

```bash
npm install -g amore-mcp
```

The postinstall script downloads the signed Rust binary for your platform from GitHub Releases and verifies it with Sigstore.

## Usage

After install, `amore-mcp` is on your PATH. IDEs pick it up via their MCP config.

### Claude Code

Add to `.claude/settings.json`:

```json
{
  "mcpServers": {
    "amore": {
      "command": "amore-mcp",
      "args": []
    }
  }
}
```

### Cursor

Add to `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "amore": {
      "command": "amore-mcp",
      "args": []
    }
  }
}
```

### Cline / Windsurf / OpenCode

Same pattern -- `command: "amore-mcp"`, no args needed for default local Qdrant.

## Manual binary path

```json
{
  "mcpServers": {
    "amore": {
      "command": "/usr/local/bin/amore-mcp",
      "args": []
    }
  }
}
```

## Python client (mem0 migration)

```bash
pip install amore
```

```python
from amore import Memory
m = Memory()
m.add("Alice prefers dark mode", user_id="alice")
results = m.search("UI preferences", user_id="alice")
```

## License

Apache-2.0
