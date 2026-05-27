#!/usr/bin/env node
// amore-mcp — bin shim.
// Locates the OS-native amore-mcp binary installed by scripts/install.js and execs it.
// Adapted from npm/bin/amore-mcp.js (repo-local @anto/amore) — same pattern, standalone package.

"use strict";

const { spawnSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const isWin = process.platform === "win32";
const binName = isWin ? "amore-mcp.exe" : "amore-mcp";
// binaries land in bin/ after postinstall
const binPath = path.join(__dirname, binName);

if (!fs.existsSync(binPath)) {
  process.stderr.write(
    `[amore-mcp] native binary missing at ${binPath}.\n` +
    `  Re-run \`npm rebuild amore-mcp\` to retry postinstall, or open an issue:\n` +
    `  https://github.com/antonio-amore-akiki/amore/issues\n`,
  );
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  process.stderr.write(`[amore-mcp] exec failed: ${result.error.message}\n`);
  process.exit(1);
}
process.exit(result.status ?? 1);
