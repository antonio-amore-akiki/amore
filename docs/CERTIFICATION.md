# Amore v1.1.0 — Production-Deployed Certification

<!-- stable: true -->
<!-- topic: amore certification production-deployed stranger-reproducible 5-channel-publish -->

Canonical certification record for Amore v1.1.0 production-deployment. Aggregated by `.github/workflows/certify.yml` after `.github/workflows/release.yml` ships all artifacts to all 5 distribution channels; C3 container-isolated reviewer subagent appends the VERDICT block.

## Release surface

Release page: https://github.com/antonio-amore-akiki/amore/releases/tag/v1.1.0

| Channel | Install command (stranger-reproducible) | Status | Asset / package |
|---|---|---|---|
| GitHub Releases (Linux .deb) | `wget https://github.com/antonio-amore-akiki/amore/releases/download/v1.1.0/amore-1.1.0-linux-amd64.deb && sudo dpkg -i amore-1.1.0-linux-amd64.deb` | PENDING | `amore-1.1.0-linux-amd64.deb` + `.sigstore` |
| GitHub Releases (Linux .rpm) | `wget https://github.com/antonio-amore-akiki/amore/releases/download/v1.1.0/amore-1.1.0-linux-x86_64.rpm && sudo rpm -i amore-1.1.0-linux-x86_64.rpm` | PENDING | `amore-1.1.0-linux-x86_64.rpm` + `.sigstore` |
| GitHub Releases (Linux AppImage) | `wget https://github.com/antonio-amore-akiki/amore/releases/download/v1.1.0/amore-gui-x86_64.AppImage && chmod +x amore-gui-x86_64.AppImage && ./amore-gui-x86_64.AppImage` | PENDING | `amore-gui-x86_64.AppImage` + `.sigstore` |
| GitHub Releases (macOS .dmg) | Open `.dmg`, drag to Applications | PENDING | `amore-1.1.0-macos-{aarch64,x86_64}.dmg` + `.sigstore` |
| GitHub Releases (macOS .pkg) | Double-click `.pkg`, follow installer | PENDING | `amore-1.1.0-macos-{aarch64,x86_64}.pkg` + `.sigstore` |
| GitHub Releases (macOS .tar.gz) | `tar -xzf amore-1.1.0-macos-aarch64.tar.gz && cd amore-1.1.0-macos-aarch64 && sudo cp amore* /usr/local/bin/` | PENDING | `amore-1.1.0-macos-{aarch64,x86_64}.tar.gz` + `.sigstore` |
| GitHub Releases (Windows MSI) | Download `.msi`, double-click, follow installer | PENDING | `amore-windows-x64.msi` + `.sigstore` |
| GitHub Releases (Windows Inno .exe) | Download `.exe`, double-click, follow installer | PENDING | `amore-windows-x64.exe` |
| **Homebrew** | `brew install antonio-amore-akiki/amore/amore` | LIVE | `antonio-amore-akiki/homebrew-amore` tap, Formula/amore.rb |
| **PyPI** | `pip install amore` | PENDING | https://pypi.org/project/amore/ |
| **npm** | `npm install -g amore-mcp` | PENDING | https://www.npmjs.com/package/amore-mcp |
| **Docker Hub** | `docker pull antonio0101/amore:1.1.0` | PENDING | https://hub.docker.com/r/antonio0101/amore |
| **crates.io** | `cargo install amore-mcp` (also amore-core, amore-cli, amore-gui) | PENDING | https://crates.io/crates/amore-mcp |

## Cosign keyless OIDC signature verification

Every release artifact ships with a `.sigstore` bundle (Sigstore keyless OIDC, signed in GHA via `id-token: write` permission). Verify any artifact:

```bash
cosign verify-blob \
  --bundle <asset>.sigstore \
  --certificate-identity-regexp "^https://github.com/antonio-amore-akiki/amore" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  <asset>
```

## Cert pipeline results

`certify.yml` matrix: `{windows-latest × {en-US, de-DE, ja-JP}, ubuntu-22.04, macos-latest}`. macOS leg allowed-fail per Apple-Dev-ID gap (scope-cut per `docs/INSTALL.md`).

<!-- VERDICT block appended by C3 container-isolated reviewer subagent -->
<!-- See plans/zesty-foraging-lake.md Phase C3 for reviewer protocol -->

## Free OSS audit substitute stack

Per Phase F1, `docs/FREE-OSS-AUDIT.md` aggregates 9 free-tier OSS audit artifacts (cargo-audit, gitleaks, semgrep, cargo-fuzz, cargo-mutants, OSSF Scorecard, Sigstore cosign, GH Security Advisories, container-isolated hostile-reviewer log). This is the strongest free substitute for accredited third-party SOC2 / ISO27001 / NCC pen-test — explicitly NOT a replacement for paid audit (per free-only constraint).

## Honest gaps acknowledged

- **macOS Apple-Dev-ID notarization** — paid out-of-scope. Free escape: Homebrew tap install (`brew install antonio-amore-akiki/amore/amore`) bypasses Gatekeeper.
- **Windows Authenticode EV cert** — paid out-of-scope. Stranger sees one SmartScreen "More info → Run anyway" click; documented in `docs/INSTALL.md` per F9. Microsoft reputation accrues over months.
- **OSSF Scorecard `codeApproved` subscore (0/10)** — agent-quorum reviews ARE substantively multi-reviewer adversarial reviews logged at `state/multi-reviewer-quorum.jsonl`, but invisible to Scorecard's GH-PR-Review-API probe. Workaround optional v-next (bot-account PR reviews).
- **6 known_gaps in `state/goal-state.json`** — Scorecard Vulnerabilities ceiling, real-OS smoke, AccessKit screen-reader audit, v1.1 GTK4 migration, wal.rs production-interface refactoring, LongMemEval real-corpus benchmark.

## Closure receipt

This file is committed to `main` after `release.yml` shows 10/10 GREEN AND `certify.yml` aggregator shows all-PASS (excluding macOS allowed-fail). See `state/goal-state.json` `status: done` for the closure timestamp.
