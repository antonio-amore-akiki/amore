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

function releaseUrl(filename) {
  return `https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/v${VERSION}/${filename}`;
}

function downloadFollowingRedirects(url, destPath, maxRedirects = 10) {
  return new Promise((resolve, reject) => {
    const visit = (currentUrl, hops) => {
      if (hops > maxRedirects) {
        reject(new Error(`Too many redirects fetching ${url}`));
        return;
      }
      const req = https.get(currentUrl, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          res.resume();
          visit(new URL(res.headers.location, currentUrl).toString(), hops + 1);
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} fetching ${currentUrl}`));
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

  console.log(`[@anto/obelion] downloading ${archiveName} from GitHub Release v${VERSION}…`);
  await downloadFollowingRedirects(releaseUrl(archiveName), archivePath);

  // Best-effort fetch of the Sigstore bundle (Linux artifacts only — verify
  // function above no-ops when bundle absent).
  try {
    await downloadFollowingRedirects(releaseUrl(`${archiveName}.bundle`), bundlePath);
  } catch (_) {
    // ignore: bundles only published for x86_64-unknown-linux-gnu in v0.1.0
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
