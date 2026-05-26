#!/usr/bin/env node
// @anto/amore — npm postinstall.
//
// Fetches the matching signed binary tarball from the Amore GitHub Release
// for the current `package.json:version` and the host OS/arch, verifies the
// Sigstore bundle (MANDATORY when bundle exists — fail-closed), extracts the
// binaries into ./bin/, and exits.
//
// Sigstore verification policy (S10b fix — security finding 10b):
//   - If a .bundle file is present on the release (HTTP 200): verification is
//     MANDATORY. cosign is located on PATH, or downloaded on-demand into
//     ~/.amore-cache/cosign-<platform>. If neither succeeds, install ABORTS.
//   - If the .bundle is absent (HTTP 404): the release was not signed for this
//     platform. Install ABORTS with a clear error + manual-verify instructions.
//   - Escape hatch: AMORE_NPM_SKIP_SIGSTORE=1 bypasses, but emits a LOUD
//     multi-line stderr warning on every use.
//   - "HTTPS + GitHub URL proves transport only, not content integrity" — this
//     script enforces content integrity via Sigstore keyless attestation.
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

const PKG = require("./package.json");
const VERSION = PKG.version;
const REPO_OWNER = "antonio-amore-akiki";
const REPO_NAME = "amore";

const PLATFORM_TARGETS = {
  "linux:x64":  { target: "x86_64-unknown-linux-gnu", ext: "tar.gz" },
  "darwin:x64": { target: "x86_64-apple-darwin",      ext: "tar.gz" },
  "win32:x64":  { target: "x86_64-pc-windows-msvc",   ext: "zip"    },
};

// cosign download URLs used when cosign is absent from PATH.
const COSIGN_URLS = {
  "linux:x64":  "https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64",
  "darwin:x64": "https://github.com/sigstore/cosign/releases/latest/download/cosign-darwin-amd64",
  "win32:x64":  "https://github.com/sigstore/cosign/releases/latest/download/cosign-windows-amd64.exe",
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
  // Allow private-repo installs during the MVP window: any of
  // AMORE_GITHUB_TOKEN > GITHUB_TOKEN > GH_TOKEN is consumed as a Bearer
  // credential to the GitHub REST API. Once the Amore repo flips to public
  // (post-v1.0), no token is needed and the browser-style
  // releases/download/<tag>/<asset> URL serves the asset directly.
  // Legacy OBELION_GITHUB_TOKEN is also accepted for backward compat.
  return (
    process.env.AMORE_GITHUB_TOKEN ||
    process.env.OBELION_GITHUB_TOKEN ||
    process.env.GITHUB_TOKEN ||
    process.env.GH_TOKEN ||
    ""
  );
}

// Probe whether a URL exists without downloading its body.
// Returns the HTTP status code (follows redirects up to 5 hops).
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
  // Resolve the asset's numeric id via the API, so we can download via the
  // private-repo-capable asset endpoint instead of the browser-only
  // releases/download path.
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
      // GitHub's API responds with a 302 to a signed CDN URL (objects.githubusercontent.com)
      // that REJECTS the Authorization header. Send creds only to github.com hosts.
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
  // Defense against zip archives that preserve the build-output relative path
  // (target/<triple>/release/amore[.exe]) instead of putting binaries at the
  // root. v0.2.1 windows-msvc.zip had this shape; release.yml is patched for
  // v0.2.2+ but this keeps existing tagged artifacts installable.
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

// Locate cosign: PATH first, then ~/.amore-cache/, then download on-demand.
// fail-closed: throws if cosign cannot be obtained. Never silent fail-open.
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
  console.log(`[@anto/amore] downloading cosign to ${cacheBin} …`);
  fs.mkdirSync(cacheDir, { recursive: true });
  await downloadToFile(cosignUrl, cacheBin, { "User-Agent": "anto-amore-postinstall" });
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
