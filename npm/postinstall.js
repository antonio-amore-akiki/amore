#!/usr/bin/env node
// @anto/amore — npm postinstall.
//
// Fetches the matching signed binary tarball from the Amore GitHub Release
// for the current `package.json:version` and the host OS/arch, verifies the
// Sigstore bundle (MANDATORY when bundle exists — fail-closed), extracts the
// binaries into ./bin/, and exits.
//
// Sigstore verification policy (S10b fix — security finding 10b):
//   Bundle present (HTTP 200): verification MANDATORY via cosign (PATH or
//   ~/.amore-cache/). Bundle absent (HTTP 404): ABORT — platform not signed.
//   Escape hatch: AMORE_NPM_SKIP_SIGSTORE=1 emits a LOUD warning and bypasses.
//   "HTTPS proves transport only" — content integrity is via Sigstore attestation.
//
// Adapted from the esbuild + ripgrep-prebuilt + @vscode/ripgrep npm patterns
// (cited in prior-art-verdict.json). All three are Apache-2.0/MIT compatible
// pattern adaptations, none of their source is vendored.
//
// Failure mode is LOUD: any network/extract error throws and npm install
// fails visibly — silent fail-open would ship an unusable bin/.
//
// Per CLAUDE.md never-fallback rule, this script does NOT degrade to "best
// effort". Either we install the real signed binary or we error out.

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const https = require("node:https");
const { spawnSync } = require("node:child_process");
const { createHash } = require("node:crypto");

// Cosign v2.4.3 SHA-256 pins (verified 2026-05-26 against
// https://github.com/sigstore/cosign/releases/download/v2.4.3/cosign_checksums.txt).
// To re-verify: download cosign_checksums.txt from the release page, grep the binary name.
const COSIGN_VERSION = "2.4.3";
const COSIGN_SHA256 = {
  "linux-amd64":       "caaad125acef1cb81d58dcdc454a1e429d09a750d1e9e2b3ed1aed8964454708",
  "darwin-amd64":      "98a3bfd691f42c6a5b721880116f89210d8fdff61cc0224cd3ef2f8e55a466fb",
  "darwin-arm64":      "edfc761b27ced77f0f9ca288ff4fac7caa898e1e9db38f4dfdf72160cdf8e638",
  "windows-amd64.exe": "a2ac24e197111c9430cb2a98f10a641164381afb83df036504868e4ea5720800",
};

// Cosign download URLs — pinned to COSIGN_VERSION (never "latest"); one entry per COSIGN_SHA256 key.
const COSIGN_URLS = {
  "linux:x64":  `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-linux-amd64`,
  "darwin:x64": `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-darwin-amd64`,
  "win32:x64":  `https://github.com/sigstore/cosign/releases/download/v${COSIGN_VERSION}/cosign-windows-amd64.exe`,
};

const PKG = require("./package.json");
const VERSION = PKG.version;
const REPO_OWNER = "antonio-amore-akiki";
const REPO_NAME = "amore";

const PLATFORM_TARGETS = {
  "linux:x64":  { target: "x86_64-unknown-linux-gnu", ext: "tar.gz" },
  "darwin:x64": { target: "x86_64-apple-darwin",      ext: "tar.gz" },
  "win32:x64":  { target: "x86_64-pc-windows-msvc",   ext: "zip"    },
};

function platformKey() {
  return `${process.platform}:${process.arch}`;
}

function resolveTarget() {
  const key = platformKey();
  const mapping = PLATFORM_TARGETS[key];
  if (!mapping) {
    throw new Error(
      `Unsupported platform ${key}. @anto/amore currently ships ` +
      `${Object.keys(PLATFORM_TARGETS).join(", ")}. ARM lanes (aarch64) ` +
      `land in v0.5.0 — track https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`,
    );
  }
  return mapping;
}

function resolveToken() {
  // AMORE_GITHUB_TOKEN > OBELION_GITHUB_TOKEN > GITHUB_TOKEN > GH_TOKEN (Bearer); empty = public-repo.
  return (
    process.env.AMORE_GITHUB_TOKEN ||
    process.env.OBELION_GITHUB_TOKEN ||
    process.env.GITHUB_TOKEN ||
    process.env.GH_TOKEN ||
    ""
  );
}

// HEAD probe for URL existence; follows up to 5 redirects; returns HTTP status code.
function httpHead(url, { headers = {} } = {}, hops = 0) {
  return new Promise((resolve, reject) => {
    if (hops > 5) { reject(new Error(`Too many redirects probing ${url}`)); return; }
    const u = new URL(url);
    const req = https.request(u, {
      method: "HEAD",
      headers: { "User-Agent": "anto-amore-postinstall", ...headers },
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
    const opts = {
      method: "GET",
      headers: { "User-Agent": "anto-amore-postinstall", ...headers },
    };
    const req = https.get(u, opts, (res) => {
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

async function lookupAssetIdByName(owner, repo, tag, assetName, token) {
  // Resolve asset numeric id via API for private-repo-capable download endpoint.
  const apiUrl = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${tag}`;
  const headers = {
    Accept: "application/vnd.github+json",
    "X-GitHub-Api-Version": "2022-11-28",
  };
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
      if (hops > maxRedirects) {
        reject(new Error(`Too many redirects fetching ${url}`));
        return;
      }
      const u = new URL(currentUrl);
      // GitHub CDN redirects reject the Authorization header — send creds only to github.com.
      const sendAuth = u.hostname === "api.github.com" || u.hostname === "github.com";
      const reqHeaders = sendAuth ? headers : { "User-Agent": "anto-amore-postinstall" };
      const req = https.get(currentUrl, { headers: reqHeaders }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          visit(new URL(res.headers.location, currentUrl).toString(), hops + 1);
          return;
        }
        if (res.statusCode !== 200) {
          const hint =
            res.statusCode === 404 && !headers.Authorization
              ? " (set GITHUB_TOKEN/GH_TOKEN if the Amore repo is still private)"
              : "";
          reject(new Error(`HTTP ${res.statusCode} fetching ${currentUrl}${hint}`));
          res.resume();
          return;
        }
        const tmp = `${destPath}.tmp`;
        const file = fs.createWriteStream(tmp);
        res.pipe(file);
        file.on("finish", () => {
          file.close(() => {
            fs.renameSync(tmp, destPath);
            resolve();
          });
        });
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
    // Authenticated path — works for private repos.
    const id = await lookupAssetIdByName(owner, repo, tag, assetName, token);
    const apiUrl = `https://api.github.com/repos/${owner}/${repo}/releases/assets/${id}`;
    const headers = {
      Authorization: `Bearer ${token}`,
      Accept: "application/octet-stream",
      "User-Agent": "anto-amore-postinstall",
    };
    await downloadToFile(apiUrl, destPath, headers);
  } else {
    // Unauthenticated browser-style URL — works for public repos only.
    const url = `https://github.com/${owner}/${repo}/releases/download/${tag}/${assetName}`;
    await downloadToFile(url, destPath, { "User-Agent": "anto-amore-postinstall" });
  }
}

function extract(archivePath, ext, outDir) {
  fs.mkdirSync(outDir, { recursive: true });
  if (ext === "tar.gz") {
    const r = spawnSync("tar", ["-xzf", archivePath, "-C", outDir], { stdio: "inherit" });
    if (r.status !== 0) throw new Error(`tar -xzf failed (status ${r.status})`);
  } else if (ext === "zip") {
    // Prefer PowerShell Expand-Archive on Windows (built-in); fall back to unzip elsewhere.
    if (process.platform === "win32") {
      const r = spawnSync(
        "powershell.exe",
        ["-NoProfile", "-Command",
         `Expand-Archive -LiteralPath "${archivePath}" -DestinationPath "${outDir}" -Force`],
        { stdio: "inherit" },
      );
      if (r.status !== 0) throw new Error(`Expand-Archive failed (status ${r.status})`);
    } else {
      const r = spawnSync("unzip", ["-o", archivePath, "-d", outDir], { stdio: "inherit" });
      if (r.status !== 0) throw new Error(`unzip failed (status ${r.status})`);
    }
  } else {
    throw new Error(`Unsupported archive extension: ${ext}`);
  }
  flattenNestedTargetDir(outDir);
}

function flattenNestedTargetDir(outDir) {
  // Flatten target/<triple>/release/ layout from v0.2.1 windows zip (patched v0.2.2+).
  const targetDir = path.join(outDir, "target");
  if (!fs.existsSync(targetDir) || !fs.statSync(targetDir).isDirectory()) return;
  const moveFromDir = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        moveFromDir(full);
      } else if (entry.isFile()) {
        const dest = path.join(outDir, entry.name);
        if (fs.existsSync(dest)) fs.unlinkSync(dest);
        fs.renameSync(full, dest);
      }
    }
  };
  moveFromDir(targetDir);
  fs.rmSync(targetDir, { recursive: true, force: true });
}

function chmodExecBinaries(outDir) {
  if (process.platform === "win32") return;
  for (const f of fs.readdirSync(outDir)) {
    const p = path.join(outDir, f);
    if (fs.statSync(p).isFile()) {
      fs.chmodSync(p, 0o755);
    }
  }
}

// Locate cosign: PATH first, then ~/.amore-cache/ (verified by SHA-256 pin), then download.
async function resolveCosign() {
  if (spawnSync("cosign", ["version"], { stdio: "ignore" }).status === 0) return "cosign";

  const key = platformKey();
  const isWin = process.platform === "win32";
  const cacheDir = path.join(os.homedir(), ".amore-cache");
  const cacheBin = path.join(cacheDir, `cosign-${key.replace(":", "-")}${isWin ? ".exe" : ""}`);

  if (fs.existsSync(cacheBin)) {
    if (spawnSync(cacheBin, ["version"], { stdio: "ignore" }).status === 0) return cacheBin;
    fs.unlinkSync(cacheBin); // corrupted — re-download
  }

  const cosignUrl = COSIGN_URLS[key];
  if (!cosignUrl) throw new Error(
    `No cosign URL for platform ${key}. Install manually: ` +
    `https://docs.sigstore.dev/cosign/system_config/installation/`,
  );
  // Map platform key → COSIGN_SHA256 key.
  const shaKey = isWin ? "windows-amd64.exe"
    : process.platform === "darwin"
      ? (process.arch === "arm64" ? "darwin-arm64" : "darwin-amd64")
      : "linux-amd64";
  const expectedSha = COSIGN_SHA256[shaKey];
  if (!expectedSha) throw new Error(`No SHA-256 pin for cosign key "${shaKey}" — update COSIGN_SHA256.`);
  console.log(`[@anto/amore] downloading cosign v${COSIGN_VERSION} to ${cacheBin} …`);
  fs.mkdirSync(cacheDir, { recursive: true });
  await downloadToFile(cosignUrl, cacheBin, { "User-Agent": "anto-amore-postinstall" });
  // C-1 fix: verify SHA-256 before executing the downloaded binary (fail-closed).
  const actualSha = createHash("sha256").update(fs.readFileSync(cacheBin)).digest("hex");
  if (actualSha !== expectedSha) {
    fs.unlinkSync(cacheBin);
    throw new Error(
      `cosign SHA-256 mismatch — download may be tampered.\n` +
      `  Expected: ${expectedSha}\n  Got: ${actualSha}\n` +
      `Aborting. Install cosign manually: https://docs.sigstore.dev/cosign/system_config/installation/`,
    );
  }
  if (!isWin) fs.chmodSync(cacheBin, 0o755);
  if (spawnSync(cacheBin, ["version"], { stdio: "ignore" }).status !== 0) throw new Error(
    `cosign self-test failed at ${cacheBin}. ` +
    `Install manually: https://docs.sigstore.dev/cosign/system_config/installation/`,
  );
  return cacheBin;
}

// fail-closed Sigstore verification — mandatory when bundle exists (bundleStatus 200).
// 404 = platform not signed → abort. Non-200/404 → abort. Escape hatch: AMORE_NPM_SKIP_SIGSTORE=1.
async function verifySigstore(archivePath, bundlePath, bundleStatus) {
  if (process.env.AMORE_NPM_SKIP_SIGSTORE === "1") {
    process.stderr.write(
      "\n╔══════════════════════════════════════════════════════════════════════╗\n" +
      "║  WARNING: AMORE_NPM_SKIP_SIGSTORE=1 — Sigstore verification SKIPPED ║\n" +
      "║  You are installing an UNVERIFIED binary. Abort now if untrusted.    ║\n" +
      "║  See README.md § 'Verify manually' to check the artifact yourself.   ║\n" +
      "╚══════════════════════════════════════════════════════════════════════╝\n\n",
    );
    return;
  }

  if (bundleStatus === 404) throw new Error(
    `No Sigstore bundle for ${path.basename(archivePath)} (HTTP 404). ` +
    `This platform's artifact was not signed. Refusing to install an unverified binary.\n` +
    `Set AMORE_NPM_SKIP_SIGSTORE=1 to override, or verify manually:\n` +
    `  https://github.com/${REPO_OWNER}/${REPO_NAME}#verify-manually`,
  );

  if (bundleStatus !== 200) throw new Error(
    `Unexpected HTTP ${bundleStatus} fetching Sigstore bundle. ` +
    `Cannot verify integrity — aborting (fail-closed). Set AMORE_NPM_SKIP_SIGSTORE=1 to override.`,
  );

  const cosignBin = await resolveCosign();
  console.log(`[@anto/amore] verifying Sigstore bundle …`);
  const r = spawnSync(cosignBin, [
    "verify-blob", "--bundle", bundlePath,
    "--certificate-identity-regexp", `https://github\\.com/${REPO_OWNER}/${REPO_NAME}/`,
    "--certificate-oidc-issuer", "https://token.actions.githubusercontent.com",
    archivePath,
  ], { stdio: "inherit" });
  if (r.status !== 0) throw new Error(
    `Sigstore verification FAILED for ${path.basename(archivePath)}.\n` +
    `Refusing to install — possible tampering. See README.md § 'Verify manually' or\n` +
    `open an issue: https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`,
  );
  console.log(`[@anto/amore] Sigstore verification OK.`);
}

async function main() {
  const { target, ext } = resolveTarget();
  const archiveName = `amore-v${VERSION}-${target}.${ext}`;
  const binDir = path.join(__dirname, "bin");
  const archivePath = path.join(__dirname, archiveName);
  const bundleName = `${archiveName}.bundle`;
  const bundlePath = `${archivePath}.bundle`;

  const tag = `v${VERSION}`;
  console.log(`[@anto/amore] downloading ${archiveName} from GitHub Release ${tag}…`);
  await fetchReleaseAsset(REPO_OWNER, REPO_NAME, tag, archiveName, archivePath);

  // Probe bundle existence before downloading. fail-closed: 200 = must verify,
  // 404 = platform not signed → abort (unless AMORE_NPM_SKIP_SIGSTORE=1).
  const bundleUrl = `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${tag}/${bundleName}`;
  const bundleStatus = await httpHead(bundleUrl);
  if (bundleStatus === 200) {
    await fetchReleaseAsset(REPO_OWNER, REPO_NAME, tag, bundleName, bundlePath);
  }
  await verifySigstore(archivePath, bundlePath, bundleStatus);

  console.log(`[@anto/amore] extracting → ${binDir}`);
  extract(archivePath, ext, binDir);
  chmodExecBinaries(binDir);

  // Cleanup downloaded archives — bin/ now contains amore + amore-mcp.
  try { fs.unlinkSync(archivePath); } catch (_) {}
  try { fs.unlinkSync(bundlePath); } catch (_) {}

  console.log(`[@anto/amore] installed v${VERSION} (${target}). Try: npx amore status`);
}

main().catch((err) => {
  console.error(`[@anto/amore] postinstall FAILED: ${err.message}`);
  console.error(`  Open issue: https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`);
  process.exit(1);
});
