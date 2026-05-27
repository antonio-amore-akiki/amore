#!/usr/bin/env node
// smoke.test.js -- spawn amore-mcp binary and assert --version succeeds.
// Prior-art: Adopt from spawnSync pattern in npm/postinstall.js (repo-local).
//
// Run: node test/smoke.test.js
// Expected: exits 0 when binary found; exits 0 (SKIP) when not installed.

"use strict";

const { spawnSync } = require("node:child_process");
const assert = require("node:assert/strict");
const path = require("node:path");
const fs = require("node:fs");

const isWin = process.platform === "win32";
const localBin = path.join(__dirname, "..", "bin", isWin ? "amore-mcp.exe" : "amore-mcp");
const binPath = fs.existsSync(localBin) ? localBin : "amore-mcp";

console.log(`[smoke] testing: ${binPath}`);

if (!fs.existsSync(localBin)) {
  const pathCheck = spawnSync("amore-mcp", ["--version"], { stdio: "pipe" });
  if (pathCheck.status === null || pathCheck.error) {
    console.log("[smoke] amore-mcp not installed. SKIP.");
    process.exit(0);
  }
}

const result = spawnSync(binPath, ["--version"], { stdio: "pipe", timeout: 10000 });

if (result.error) {
  console.error(`[smoke] failed to spawn: ${result.error.message}`);
  process.exit(1);
}

const stdout = (result.stdout || "").toString().trim();
const stderr = (result.stderr || "").toString().trim();
console.log(`[smoke] stdout: ${stdout}`);
if (stderr) console.log(`[smoke] stderr: ${stderr}`);
console.log(`[smoke] exit status: ${result.status}`);

assert.equal(result.status, 0, `amore-mcp --version exited with ${result.status}`);
assert.ok(stdout.length > 0 || stderr.length > 0, "amore-mcp --version produced no output");

console.log("[smoke] PASS");
