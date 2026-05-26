#!/usr/bin/env node
// @anto/obelion — `obelion-mcp` MCP server shim.
// Execs the OS-native MCP server binary installed by postinstall.js.

const { spawnSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const isWin = process.platform === "win32";
const binName = isWin ? "obelion-mcp.exe" : "obelion-mcp";
const binPath = path.join(__dirname, binName);

if (!fs.existsSync(binPath)) {
  console.error(
    `[@anto/obelion] native MCP binary missing at ${binPath}.\n` +
      `  Re-run \`npm rebuild @anto/obelion\` to retry postinstall, or open an issue.`,
  );
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`[@anto/obelion] exec failed: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
