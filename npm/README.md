# @anto/obelion

Universal MCP agent memory backbone — Rust core, distributed as signed cross-OS binaries.

## Install

```bash
npm install -g @anto/obelion
```

The postinstall step fetches the matching native binary for your OS from the
[obelion GitHub Release](https://github.com/antonio-amore-akiki/obelion/releases)
matching this package's `version`. On Linux, if [cosign](https://docs.sigstore.dev/cosign/installation/)
is on your PATH, the Sigstore signature bundle is verified before extraction.

Supported targets in v0.1.0: `linux-x64`, `darwin-x64`, `win32-x64`. ARM lanes
(`aarch64-apple-darwin`, `aarch64-unknown-linux-gnu`) land in v0.5.0.

## Wire obelion into your IDE

```bash
obelion init claude       # patches ~/.claude.json
obelion init cursor       # patches ~/.cursor/mcp.json
obelion init codex        # patches ~/.codex/config.toml
obelion init cline        # patches the Cline VSCode extension globalStorage
obelion init opencode     # patches ~/.config/opencode/opencode.json
obelion init windsurf     # patches ~/.codeium/windsurf/mcp_config.json
obelion init hermes       # patches ~/.hermes/config.yaml
```

Atomic-write contract: tmp+rename+`.bak` sibling, idempotent NoChange on
matching entries, `--dry-run` prints the merged config without touching disk.

## Status / version

```bash
obelion status            # resolved daemon URLs + obelion version
obelion --version         # version only
```

## Run the MCP server directly

```bash
obelion-mcp               # speaks Model Context Protocol over stdio
```

After `obelion init <ide>`, your IDE launches `obelion-mcp` automatically.

## License

Apache-2.0
