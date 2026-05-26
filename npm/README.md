# @anto/amore

Universal MCP agent memory backbone — Rust core, distributed as signed cross-OS binaries.

## Install

```bash
npm install -g @anto/amore
```

The postinstall step fetches the matching native binary for your OS from the
[Amore GitHub Release](https://github.com/antonio-amore-akiki/amore/releases)
matching this package's `version`. On Linux, if [cosign](https://docs.sigstore.dev/cosign/installation/)
is on your PATH, the Sigstore signature bundle is verified before extraction.

Supported targets in v0.2.0: `linux-x64`, `darwin-x64`, `win32-x64`. ARM lanes
(`aarch64-apple-darwin`, `aarch64-unknown-linux-gnu`) land in v0.5.0.

## Wire Amore into your IDE

```bash
amore init claude       # patches ~/.claude.json
amore init cursor       # patches ~/.cursor/mcp.json
amore init codex        # patches ~/.codex/config.toml
amore init cline        # patches the Cline VSCode extension globalStorage
amore init opencode     # patches ~/.config/opencode/opencode.json
amore init windsurf     # patches ~/.codeium/windsurf/mcp_config.json
amore init hermes       # patches ~/.hermes/config.yaml
```

Atomic-write contract: tmp+rename+`.bak` sibling, idempotent NoChange on
matching entries, `--dry-run` prints the merged config without touching disk.

## Status / version

```bash
amore status            # resolved daemon URLs + amore version
amore --version         # version only
```

## Run the MCP server directly

```bash
amore-mcp               # speaks Model Context Protocol over stdio
```

After `amore init <ide>`, your IDE launches `amore-mcp` automatically.

## Upgrade from obelion

If you previously installed `@anto/obelion`, uninstall it first:

```bash
npm uninstall -g @anto/obelion
npm install -g @anto/amore
amore init claude   # replaces the obelion MCP entry in your IDE config
```

Your existing data is migrated automatically on first `amore-mcp` start.

## License

Apache-2.0
