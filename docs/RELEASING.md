# Releasing Amore — Local Pipeline
<!-- stable: true -->

## Why local?

GHA paid minutes ran out on this private repo (run #26464161984 billing error).
User mandate: "only completely unlimited free features or other public products".
Flipping the repo to public is system-prohibited (access-control invariant).
Result: release pipeline moved 100% local. macOS dropped — no Apple hardware
available; no paid macOS CI minutes permitted.

## Prerequisites

Install once; all free:

| Tool | Install |
|---|---|
| Docker Desktop | https://www.docker.com/products/docker-desktop — must be **running** |
| Rust toolchain | `winget install Rustlang.Rustup` then `rustup toolchain install stable` |
| gh CLI | `winget install GitHub.cli` then `gh auth login` |
| cosign | `winget install sigstore.cosign` |
| cargo-cyclonedx | `cargo install cargo-cyclonedx` |

Verify all are on `PATH` before the first run — step 1 of `release-local.ps1`
checks them and exits with code 1 + an actionable message if any are missing.

## Quick release

```powershell
# 1. Commit all changes, push, then create the tag
gh release create v0.5.0 --draft --repo antonio-amore-akiki/amore

# 2. Run the local pipeline (opens browser for OIDC sign — click the URL)
pwsh ./scripts/release-local.ps1 -Version 0.5.0
```

Expected wall-clock: Windows build ~3-8 min + Linux Docker build ~10-20 min
(first run pulls `rust:1.95-bookworm` image ~1 GB) + sign/upload ~2 min.
Full run: **15-30 min**. With `-SkipLinux`: ~5-10 min.

## Per-step explanation

| Step | What happens |
|---|---|
| 1 Deps check | Probes docker, cargo, gh, cosign, cargo-cyclonedx; exits 1 if any missing |
| 2 Git guard | Asserts clean worktree + tag exists; exits 2 otherwise |
| 3 Windows build | `cargo build --release -p amore-cli -p amore-mcp`; packages `amore-vX-x86_64-pc-windows-msvc.zip` |
| 4 Linux build | Docker `rust:1.95-bookworm`; same crates; packages `.tar.gz` |
| 5 SBOM | `cargo cyclonedx --workspace --format json`; writes `dist/sbom.cdx.json` |
| 6 Sign | `cosign sign-blob --yes --bundle` per artifact; browser OIDC flow |
| 7 Attest | `cosign attest-blob --type slsaprovenance` with local SLSA predicate JSON |
| 8 Verify | `cosign verify-blob --bundle` round-trip on each artifact; exits 8 on fail |
| 9 Upload | `gh release upload vX.Y.Z <artifact> <sbom> <bundle>` per file |
| 10 Proof | Writes `%LOCALAPPDATA%\Amore\release-local-vX-<ts>.json` |
| 11 TSV | Appends one row to `docs/results.tsv` |

## Verification — end-user cosign verify

Users who download a release artifact can verify the signature:

```bash
cosign verify-blob \
  --bundle amore-v0.5.0-x86_64-pc-windows-msvc.zip.sigstore \
  --certificate-identity-regexp antonioakiki15@gmail.com \
  --certificate-oidc-issuer https://accounts.google.com \
  amore-v0.5.0-x86_64-pc-windows-msvc.zip
```

## Troubleshooting

**Docker daemon down** — step 4 will hang or error. Start Docker Desktop
and wait for the whale icon to show "running".

**cosign OIDC browser flow** — step 6 prints a URL to stdout. Open it in a
browser, authenticate with Google, and signing completes automatically.
If no browser opens, copy-paste the URL manually.

**gh auth expired** — run `gh auth login --web` before starting the script.

**Version mismatch with installer** — if `installer/windows/amore.iss` has a
hardcoded version, update it before tagging.

**Linux binary missing after Docker build** — Docker writes into the repo-root
`target/release/` via the `-v` bind-mount. Verify Docker Desktop bind-mount
permissions are enabled in Settings > Resources > File Sharing.

## Dev / dry-run flags

```powershell
# Windows only, no sign, no upload (fast iteration ~5 min)
pwsh ./scripts/release-local.ps1 -Version 0.4.1 -SkipLinux -SkipSign -SkipUpload
```

## Future work

- **macOS**: if Apple hardware becomes available, add a `-IncludeMac` branch
  that runs `cargo build --release` natively and signs with `codesign` +
  `xcrun notarytool`. No cross-compilation path exists for macOS targets.
- **GHA minutes**: if the repo ever gains unlimited minutes (e.g. open-source
  plan), revive `.github/workflows/release.yml` from git history and re-enable
  the matrix.
- **Windows Authenticode**: EV cert path is already stubbed in the old
  release.yml comments. When cert is available, call AzureSignTool in step 3
  of this script before the zip step.
