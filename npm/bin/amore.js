#!/usr/bin/env node
// @anto/amore — `amore` CLI shim.
// Execs the OS-native binary installed by postinstall.js into ./bin/.

const { spawnSync } = require("node:child_process");
const path = require("node:path");
const fs = require("node:fs");

const isWin = process.platform === "win32";
const binName = isWin ? "amore.exe" : "amore";
const binPath = path.join(__dirname, binName);

if (!fs.existsSync(binPath)) {
  console.error(
    `[@anto/amore] native binary missing at ${binPath}.\n` +
      `  Re-run \`npm rebuild @anto/amore\` to retry postinstall, or open an issue.`,
  );
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error(`[@anto/amore] exec failed: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
