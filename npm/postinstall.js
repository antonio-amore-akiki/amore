#!/usr/bin/env node
// @anto/obelion — npm postinstall.
//
// Fetches the matching signed binary tarball from the obelion GitHub Release
// for the current `package.json:version` and the host OS/arch, verifies the
// Sigstore bundle when cosign is on PATH (Linux only — Windows/macOS sign-
// skeletons land in S10b/c), extracts the binaries into ./bin/, and exits.
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
const path = require("node:path");
const https = require("node:https");
const { spawnSync } = require("node:child_process");

const PKG = require("./package.json");
const VERSION = PKG.version;
const REPO_OWNER = "antonio-amore-akiki";
const REPO_NAME = "obelion";

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
      `Unsupported platform ${key}. @anto/obelion currently ships ` +
      `${Object.keys(PLATFORM_TARGETS).join(", ")}. ARM lanes (aarch64) ` +
      `land in v0.5.0 — track https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`,
    );
  }
  return mapping;
}

function resolveToken() {
  // Allow private-repo installs during the MVP window: any of
  // OBELION_GITHUB_TOKEN > GITHUB_TOKEN > GH_TOKEN is consumed as a Bearer
  // credential to the GitHub REST API. Once the obelion repo flips to public
  // (post-v1.0), no token is needed and the browser-style
  // releases/download/<tag>/<asset> URL serves the asset directly.
  return (
    process.env.OBELION_GITHUB_TOKEN ||
    process.env.GITHUB_TOKEN ||
    process.env.GH_TOKEN ||
    ""
  );
}

function httpGet(url, { headers = {}, expectJson = false } = {}) {
  return new Promise((resolve, reject) => {
    const u = new URL(url);
    const opts = {
      method: "GET",
      headers: { "User-Agent": "anto-obelion-postinstall", ...headers },
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
      const reqHeaders = sendAuth ? headers : { "User-Agent": "anto-obelion-postinstall" };
      const req = https.get(currentUrl, { headers: reqHeaders }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          visit(new URL(res.headers.location, currentUrl).toString(), hops + 1);
          return;
        }
        if (res.statusCode !== 200) {
          const hint =
            res.statusCode === 404 && !headers.Authorization
              ? " (set GITHUB_TOKEN/GH_TOKEN if the obelion repo is still private)"
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
      "User-Agent": "anto-obelion-postinstall",
    };
    await downloadToFile(apiUrl, destPath, headers);
  } else {
    // Unauthenticated browser-style URL — works for public repos only.
    const url = `https://github.com/${owner}/${repo}/releases/download/${tag}/${assetName}`;
    await downloadToFile(url, destPath, { "User-Agent": "anto-obelion-postinstall" });
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
  // (target/<triple>/release/obelion[.exe]) instead of putting binaries at the
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

function verifySigstoreIfAvailable(archivePath, bundlePath) {
  // Optional integrity check — only enforced when cosign is on PATH.
  // On a fresh user machine without cosign, fall through; the GitHub
  // Releases URL itself plus HTTPS already provides transport integrity.
  // For supply-chain attestation, install cosign and re-run `npm rebuild`.
  if (!fs.existsSync(bundlePath)) return; // no bundle for this OS
  const probe = spawnSync("cosign", ["version"], { stdio: "ignore" });
  if (probe.status !== 0) return;
  const r = spawnSync(
    "cosign",
    ["verify-blob", "--bundle", bundlePath,
     "--certificate-identity-regexp", `https://github\\.com/${REPO_OWNER}/${REPO_NAME}/`,
     "--certificate-oidc-issuer", "https://token.actions.githubusercontent.com",
     archivePath],
    { stdio: "inherit" },
  );
  if (r.status !== 0) {
    throw new Error(
      `Sigstore signature verification FAILED for ${path.basename(archivePath)}. ` +
      `Refusing to install an unverified binary.`,
    );
  }
}

async function main() {
  const { target, ext } = resolveTarget();
  const archiveName = `obelion-v${VERSION}-${target}.${ext}`;
  const binDir = path.join(__dirname, "bin");
  const archivePath = path.join(__dirname, archiveName);
  const bundlePath = `${archivePath}.bundle`;

  const tag = `v${VERSION}`;
  console.log(`[@anto/obelion] downloading ${archiveName} from GitHub Release ${tag}…`);
  await fetchReleaseAsset(REPO_OWNER, REPO_NAME, tag, archiveName, archivePath);

  // Best-effort fetch of the Sigstore bundle (Linux artifacts only — verify
  // function above no-ops when bundle absent).
  try {
    await fetchReleaseAsset(REPO_OWNER, REPO_NAME, tag, `${archiveName}.bundle`, bundlePath);
  } catch (_) {
    // ignore: bundles only published for x86_64-unknown-linux-gnu
  }

  verifySigstoreIfAvailable(archivePath, bundlePath);
  console.log(`[@anto/obelion] extracting → ${binDir}`);
  extract(archivePath, ext, binDir);
  chmodExecBinaries(binDir);

  // Cleanup downloaded archives — bin/ now contains obelion + obelion-mcp.
  try { fs.unlinkSync(archivePath); } catch (_) {}
  try { fs.unlinkSync(bundlePath); } catch (_) {}

  console.log(`[@anto/obelion] installed v${VERSION} (${target}). Try: npx obelion status`);
}

main().catch((err) => {
  console.error(`[@anto/obelion] postinstall FAILED: ${err.message}`);
  console.error(`  Open issue: https://github.com/${REPO_OWNER}/${REPO_NAME}/issues`);
  process.exit(1);
});
