#!/usr/bin/env node
// amore-mcp -- postinstall (scripts/install.js).
//
// Downloads the amore-mcp binary for the current platform from GitHub Releases,
// verifies Sigstore bundle (fail-closed), and extracts it to bin/.
//
// Prior-art: Adopt from npm/postinstall.js (@anto/amore, repo-local).
// The download/verify/extract logic is identical -- only the binary target
// name (amore-mcp vs amore + amore-mcp) differs.
//
// Platform support: linux/amd64, darwin/amd64, darwin/arm64, win32/amd64.
// Failure is LOUD -- no silent fail-open. Per CLAUDE.md never-fallback rule.

"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const https = require("node:https");
const { spawnSync } = require("node:child_process");
const { createHash } = require("node:crypto");

const PKG = require("../package.json");
const VERSION = PKG.version;
const REPO_OWNER = "antonio-amore-akiki";
const REPO_NAME = "amore";

// cosign pins -- same as npm/postinstall.js; keep in sync on cosign upgrades.
const COSIGN_VERSION = "2.4.3";
const COSIGN_SHA256 = {
  "linux-amd64":       "caaad125acef1cb81d58dcdc454a1e429d09a750d1e9e2b3ed1aed8964454708",
  "darwin-amd64":      "98a3bfd691f42c6a5b721880116f89210d8fdff61cc0224cd3ef2f8e55a466fb",
  "darwin-arm64":      "edfc761b27ced77f0f9ca288ff4fac7caa898e1e9db38f4dfdf72160cdf8e638",
  "windows-amd64.exe": "a2ac24e197111c9430cb2a98f10a641164381afb83df036504868e4ea5720800",
};
const COSIGN_URLS = {
  "linux:x64":    `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-linux-amd64`,
  "darwin:x64":   `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-darwin-amd64`,
  "darwin:arm64": `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-darwin-arm64`,
  "win32:x64":    `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-windows-amd64.exe`,
};

const PLATFORM_TARGETS = {
  "linux:x64":    { target: "x86_64-unknown-linux-gnu",   ext: "tar.gz" },
  "darwin:x64":   { target: "x86_64-apple-darwin",        ext: "tar.gz" },
  "darwin:arm64": { target: "aarch64-apple-darwin",        ext: "tar.gz" },
  "win32:x64":    { target: "x86_64-pc-windows-msvc",     ext: "zip"    },
};

function platformKey() { return `${process.platform}:${process.arch}`; }

function resolveTarget() {
  const key = platformKey();
  const mapping = PLATFORM_TARGETS[key];
  if (!mapping) {
    throw new Error(
      `Unsupported platform ${key}. amore-mcp currently ships ` +
      `${Object.keys(PLATFORM_TARGETS).join(", ")}. ` +
      `Open an issue: https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`,
    );
  }
  return mapping;
}

function resolveToken() {
  return (
    process.env.AMORE_GITHUB_TOKEN ||
    process.env.OBELION_GITHUB_TOKEN ||
    process.env.GITHUB_TOKEN ||
    process.env.GH_TOKEN ||
    ""
  );
}

function httpHead(url, { headers = {} } = {}, hops = 0) {
  return new Promise((resolve, reject) => {
    if (hops > 5) { reject(new Error(`Too many redirects probing ${url}`)); return; }
    const u = new URL(url);
    const req = https.request(u, {
      method: "HEAD",
      headers: { "User-Agent": "amore-mcp-install", ...headers },
    }, (res) => {
      res.resume();
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        httpHead(new URL(res.headers.location, u).toString(), { headers }, hops + 1)
          .then(resolve).catch(reject);
        return;
      }
      resolve(res.statusCode);
    });
    req.on("error", reject);
    req.end();
  });
}

function httpGet(url, { headers = {}, expectJson = false } = {}) {
  return new Promise((resolve, reject) => {
    const u = new URL(url);
    const req = https.get(u, {
      headers: { "User-Agent": "amore-mcp-install", ...headers },
    }, (res) => {
      const chunks = [];
      res.on("data", (c) => chunks.push(c));
      res.on("end", () => {
        const body = Buffer.concat(chunks);
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          resolve({ redirect: new URL(res.headers.location, u).toString(), status: res.statusCode });
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} fetching ${url}: ${body.toString("utf8").slice(0, 200)}`));
          return;
        }
        resolve({ body: expectJson ? JSON.parse(body.toString("utf8")) : body, status: res.statusCode });
      });
    });
    req.on("error", reject);
  });
}

async function lookupAssetId(owner, repo, tag, assetName, token) {
  const apiUrl = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${tag}`;
  const headers = { Accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28" };
  if (token) headers.Authorization = `Bearer ${token}`;
  const { body } = await httpGet(apiUrl, { headers, expectJson: true });
  const asset = (body.assets || []).find((a) => a.name === assetName);
  if (!asset) {
    throw new Error(
      `Asset ${assetName} not found on release ${tag} ` +
      `(available: ${(body.assets || []).map((a) => a.name).join(", ") || "<none>"})`,
    );
  }
  return asset.id;
}

function downloadToFile(url, destPath, headers, maxRedirects = 10) {
  return new Promise((resolve, reject) => {
    const visit = (currentUrl, hops) => {
      if (hops > maxRedirects) { reject(new Error(`Too many redirects fetching ${url}`)); return; }
      const u = new URL(currentUrl);
      const sendAuth = u.hostname === "api.github.com" || u.hostname === "github.com";
      const reqHeaders = sendAuth ? headers : { "User-Agent": "amore-mcp-install" };
      const req = https.get(currentUrl, { headers: reqHeaders }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          visit(new URL(res.headers.location, currentUrl).toString(), hops + 1);
          return;
        }
        if (res.statusCode !== 200) {
          const hint = res.statusCode === 404 && !headers.Authorization
            ? " (set GITHUB_TOKEN if the repo is private)"
            : "";
          reject(new Error(`HTTP ${res.statusCode} fetching ${currentUrl}${hint}`));
          res.resume();
          return;
        }
        const tmp = `${destPath}.tmp`;
        const file = fs.createWriteStream(tmp);
        res.pipe(file);
        file.on("finish", () => file.close(() => { fs.renameSync(tmp, destPath); resolve(); }));
        file.on("error", reject);
      });
      req.on("error", reject);
    };
    visit(url, 0);
  });
}

async function fetchReleaseAsset(owner, repo, tag, assetName, destPath) {
  const token = resolveToken();
  if (token) {
    const id = await lookupAssetId(owner, repo, tag, assetName, token);
    const apiUrl = `https://api.github.com/repos/${owner}/${repo}/releases/assets/${id}`;
    await downloadToFile(apiUrl, destPath, {
      Authorization: `Bearer ${token}`,
      Accept: "application/octet-stream",
      "User-Agent": "amore-mcp-install",
    });
  } else {
    const url = `https://github.com/${owner}/${repo}/releases/download/${tag}/${assetName}`;
    await downloadToFile(url, destPath, { "User-Agent": "amore-mcp-install" });
  }
}

function extract(archivePath, ext, outDir) {
  fs.mkdirSync(outDir, { recursive: true });
  if (ext === "tar.gz") {
    const r = spawnSync("tar", ["-xzf", archivePath, "-C", outDir], { stdio: "inherit" });
    if (r.status !== 0) throw new Error(`tar -xzf failed (status ${r.status})`);
  } else if (ext === "zip") {
    if (process.platform === "win32") {
      const r = spawnSync("powershell.exe",
        ["-NoProfile", "-Command",
          `Expand-Archive -LiteralPath "${archivePath}" -DestinationPath "${outDir}" -Force`],
        { stdio: "inherit" });
      if (r.status !== 0) throw new Error(`Expand-Archive failed (status ${r.status})`);
    } else {
      const r = spawnSync("unzip", ["-o", archivePath, "-d", outDir], { stdio: "inherit" });
      if (r.status !== 0) throw new Error(`unzip failed (status ${r.status})`);
    }
  } else {
    throw new Error(`Unsupported archive extension: ${ext}`);
  }
  const targetDir = path.join(outDir, "target");
  if (fs.existsSync(targetDir)) {
    const moveFrom = (dir) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) { moveFrom(full); }
        else if (entry.isFile()) {
          const dest = path.join(outDir, entry.name);
          if (fs.existsSync(dest)) fs.unlinkSync(dest);
          fs.renameSync(full, dest);
        }
      }
    };
    moveFrom(targetDir);
    fs.rmSync(targetDir, { recursive: true, force: true });
  }
}

function chmodExec(outDir) {
  if (process.platform === "win32") return;
  for (const f of fs.readdirSync(outDir)) {
    const p = path.join(outDir, f);
    if (fs.statSync(p).isFile()) fs.chmodSync(p, 0o755);
  }
}

async function resolveCosign() {
  if (spawnSync("cosign", ["version"], { stdio: "ignore" }).status === 0) return "cosign";
  const key = platformKey();
  const isWin = process.platform === "win32";
  const cacheDir = path.join(os.homedir(), ".amore-cache");
  const cacheBin = path.join(cacheDir, `cosign-${key.replace(":", "-")}${isWin ? ".exe" : ""}`);
  if (fs.existsSync(cacheBin) && spawnSync(cacheBin, ["version"], { stdio: "ignore" }).status === 0) return cacheBin;
  if (fs.existsSync(cacheBin)) fs.unlinkSync(cacheBin);
  const cosignUrl = COSIGN_URLS[key];
  if (!cosignUrl) throw new Error(`No cosign URL for platform ${key}.`);
  const shaKey = isWin ? "windows-amd64.exe"
    : process.platform === "darwin"
      ? (process.arch === "arm64" ? "darwin-arm64" : "darwin-amd64")
      : "linux-amd64";
  const expectedSha = COSIGN_SHA256[shaKey];
  if (!expectedSha) throw new Error(`No SHA-256 pin for cosign key "${shaKey}".`);
  console.log(`[amore-mcp] downloading cosign v${COSIGN_VERSION} to ${cacheBin}...`);
  fs.mkdirSync(cacheDir, { recursive: true });
  await downloadToFile(cosignUrl, cacheBin, { "User-Agent": "amore-mcp-install" });
  const actualSha = createHash("sha256").update(fs.readFileSync(cacheBin)).digest("hex");
  if (actualSha !== expectedSha) {
    fs.unlinkSync(cacheBin);
    throw new Error(
      `cosign SHA-256 mismatch. Expected: ${expectedSha}. Got: ${actualSha}.\n` +
      `Possible tampering. Install cosign manually: https://docs.sigstore.dev/cosign/system_config/installation/`,
    );
  }
  if (!isWin) fs.chmodSync(cacheBin, 0o755);
  return cacheBin;
}

async function verifySigstore(archivePath, bundlePath, bundleStatus) {
  if (process.env.AMORE_NPM_SKIP_SIGSTORE === "1") {
    process.stderr.write(
      "\n[amore-mcp] WARNING: AMORE_NPM_SKIP_SIGSTORE=1 -- Sigstore verification SKIPPED.\n" +
      "  You are installing an UNVERIFIED binary.\n\n",
    );
    return;
  }
  if (bundleStatus === 404) throw new Error(
    `No Sigstore bundle for ${path.basename(archivePath)} (HTTP 404). ` +
    `Platform artifact was not signed. Refusing unverified install.\n` +
    `Set AMORE_NPM_SKIP_SIGSTORE=1 to override.`,
  );
  if (bundleStatus !== 200) throw new Error(
    `Unexpected HTTP ${bundleStatus} fetching Sigstore bundle. Aborting (fail-closed).`,
  );
  const cosignBin = await resolveCosign();
  console.log("[amore-mcp] verifying Sigstore bundle...");
  const r = spawnSync(cosignBin, [
    "verify-blob", "--bundle", bundlePath,
    "--certificate-identity-regexp", `https://github\\.com/${REPO_OWNER}/${REPO_NAME}/`,
    "--certificate-oidc-issuer", "https://token.actions.githubusercontent.com",
    archivePath,
  ], { stdio: "inherit" });
  if (r.status !== 0) throw new Error(
    `Sigstore verification FAILED for ${path.basename(archivePath)}. Possible tampering.`,
  );
  console.log("[amore-mcp] Sigstore verification OK.");
}

async function main() {
  const { target, ext } = resolveTarget();
  const archiveName = `amore-v${VERSION}-${target}.${ext}`;
  const binDir = path.join(__dirname, "..", "bin");
  const archivePath = path.join(__dirname, "..", archiveName);
  const bundleName = `${archiveName}.bundle`;
  const bundlePath = `${archivePath}.bundle`;
  const tag = `v${VERSION}`;

  console.log(`[amore-mcp] downloading ${archiveName} from GitHub Release ${tag}...`);
  await fetchReleaseAsset(REPO_OWNER, REPO_NAME, tag, archiveName, archivePath);

  const bundleUrl = `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${tag}/${bundleName}`;
  const bundleStatus = await httpHead(bundleUrl);
  if (bundleStatus === 200) {
    await fetchReleaseAsset(REPO_OWNER, REPO_NAME, tag, bundleName, bundlePath);
  }
  await verifySigstore(archivePath, bundlePath, bundleStatus);

  console.log(`[amore-mcp] extracting to ${binDir}`);
  extract(archivePath, ext, binDir);
  chmodExec(binDir);

  try { fs.unlinkSync(archivePath); } catch (_) {}
  try { fs.unlinkSync(bundlePath); } catch (_) {}

  console.log(`[amore-mcp] installed v${VERSION} (${target}). Run: amore-mcp --version`);
}

main().catch((err) => {
  console.error(`[amore-mcp] postinstall FAILED: ${err.message}`);
  console.error(`  Open issue: https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`);
  process.exit(1);
});
