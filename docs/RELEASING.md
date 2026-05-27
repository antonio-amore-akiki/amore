# Releasing Amore — Local Pipeline
<!-- stable: true -->

## Release pipeline

Release pipeline runs on free public-repo GHA + Sigstore keyless OIDC.
Local pipeline retained as offline fallback only (no GHA access / air-gap).
macOS targets deferred — no Apple hardware on dev host (see Cross-compilation targets below).

## Builder images (one-time pre-build — W8-8C M2)

Before the first release run, build the pre-baked x86_64 Linux builder image locally:

```powershell
pwsh ./scripts/build-builder-images.ps1
```

This builds `amore-builder-linux-x86_64:latest` from `Dockerfile.builder-linux-x86_64` (digest-pinned
`rust@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1` + `protobuf-compiler`
pre-installed). Re-run only when `Dockerfile.builder-linux-x86_64` changes. The image is LOCAL ONLY —
not pushed to any registry.

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
| 4 Linux build | Docker `amore-builder-linux-x86_64:latest` (pre-baked from `rust@sha256:6258907...` via `Dockerfile.builder-linux-x86_64`; W8-8C M2 fix); same crates; packages `.tar.gz` |
| 5 SBOM | `cargo cyclonedx --workspace --format json`; writes `dist/sbom.cdx.json` |
| 6 Sign | `cosign sign-blob --yes --bundle` per artifact; browser OIDC flow |
| 7 Attest | `cosign attest-blob --type slsaprovenance` with local SLSA predicate JSON |
| 8 Verify | `cosign verify-blob --bundle` round-trip on each artifact; exits 8 on fail |
| 9 Upload | `gh release upload vX.Y.Z <artifact> <sbom> <bundle>` per file |
| 10 Proof | Writes `%LOCALAPPDATA%\Amore\release-local-vX-<ts>.json` |
| 11 TSV | Appends one row to `test logs` |

## SLSA L3 Verification

Each release ships: `<artifact>.sigstore` (keyless sig), `<artifact>.attest.bundle` (SLSA
provenance), `sha256sums.txt` (digest manifest), `sha256sums.txt.sigstore`, and
`sha256sums.txt.attest.bundle`. Verify with cosign:

```bash
# Verify artifact signature
cosign verify-blob \
  --bundle amore-v0.9.0-x86_64-unknown-linux-gnu.tar.gz.sigstore \
  --certificate-identity-regexp antonioakiki15@gmail.com \
  --certificate-oidc-issuer https://accounts.google.com \
  amore-v0.9.0-x86_64-unknown-linux-gnu.tar.gz

# Verify SLSA provenance predicate for artifact
cosign verify-blob-attestation \
  --bundle amore-v0.9.0-x86_64-unknown-linux-gnu.tar.gz.attest.bundle \
  --certificate-identity-regexp antonioakiki15@gmail.com \
  --certificate-oidc-issuer https://accounts.google.com \
  --type slsaprovenance \
  amore-v0.9.0-x86_64-unknown-linux-gnu.tar.gz

# Verify sha256sums.txt manifest signature
cosign verify-blob \
  --bundle sha256sums.txt.sigstore \
  --certificate-identity-regexp antonioakiki15@gmail.com \
  --certificate-oidc-issuer https://accounts.google.com \
  sha256sums.txt

# Verify bundle-level SLSA provenance
cosign verify-blob-attestation \
  --bundle sha256sums.txt.attest.bundle \
  --certificate-identity-regexp antonioakiki15@gmail.com \
  --certificate-oidc-issuer https://accounts.google.com \
  --type slsaprovenance \
  sha256sums.txt
```

Full requirement walk-through and status table: `docs/SLSA-L3-ATTESTATION.md`.

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

## Reproducible builds (Buck2-pattern hermeticity)

`SOURCE_DATE_EPOCH=git log -1 --format=%ct` + `RUSTFLAGS --remap-path-prefix` are set
before each `cargo build`. Two builds of the same commit produce byte-identical SHA256
hashes. Hermeticity details and SLSA gap status: `docs/SLSA-L3-ATTESTATION.md`.

Sources: reproducible-builds.org/docs/source-date-epoch + doc.rust-lang.org/cargo.

## Cross-compilation targets

| Target | Tool | Status |
|---|---|---|
| x86_64-pc-windows-msvc | native cargo | active |
| x86_64-unknown-linux-gnu | Docker `amore-builder-linux-x86_64` (pre-baked, `rust@sha256:6258907...` digest-pinned W8-8C M2A) | active |
| aarch64-unknown-linux-gnu | cross-rs/cross (ghcr.io/cross-rs/aarch64-unknown-linux-gnu@sha256:7f8308... — digest-pinned W8-8C M2C) | active (W5-5A) |
| aarch64-pc-windows-msvc | DEFERRED v1.1 | requires Windows ARM SDK (no dev host) |
| *-apple-darwin | DEFERRED v1.x | no Apple hardware on dev host |

Sources: github.com/cross-rs/cross | doc.rust-lang.org/nightly/rustc/platform-support.html

## Crates.io secrets

Crates.io publishing requires the `CARGO_REGISTRY_TOKEN` secret in repo settings. Set this once:

1. Run `cargo login` on your dev machine — this writes the token to `~/.cargo/credentials.toml`.
2. Copy the token value (or generate a new one at https://crates.io/settings/tokens).
3. In the GitHub repo: **Settings → Secrets and variables → Actions → New repository secret**.
   Name: `CARGO_REGISTRY_TOKEN`, value: the token from step 2.

The `crates-publish` job in `.github/workflows/release.yml` publishes `amore-core`, `amore-mcp`,
`amore-cli`, and `amore-gui` in that order (amore-core first because it is a direct dependency
of the others). Each crate must have `publish = true` (the default) in its `Cargo.toml`.

## Docker Hub secrets

Docker Hub multi-arch publishing requires two secrets in repo settings:

1. Log in to Docker Hub at https://hub.docker.com → **Account Settings → Personal access tokens → Generate new token**.
   Scope: **Read, Write, Delete** (needed to push new tags and overwrite `latest`).
2. In the GitHub repo: **Settings → Secrets and variables → Actions**:
   - `DOCKER_USERNAME`: your Docker Hub username (e.g. `antonioamoreakiki`)
   - `DOCKER_HUB_TOKEN`: the access token generated in step 1.

The `docker-publish` job in `.github/workflows/release.yml` builds and pushes a
`linux/amd64` + `linux/arm64` image from `Dockerfile.multiarch` to
`antonioamoreakiki/amore:<release_tag>` and `antonioamoreakiki/amore:latest`.

## Future work

- **macOS**: add `-IncludeMac` branch if Apple hardware becomes available; sign with `codesign` + `xcrun notarytool`.
- **GHA minutes**: `.github/workflows/release.yml` runs on free public-repo runners; extend to matrix builds as needed.
- **Windows Authenticode**: call AzureSignTool in step 3 when EV cert is available.
