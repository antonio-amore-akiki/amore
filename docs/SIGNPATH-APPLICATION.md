---
stable: true
---

# SignPath OSS Authenticode application — Amore pre-fill

**Goal**: eliminate the Windows SmartScreen "amore-windows-x64.msi isn't commonly downloaded — Make sure you trust amore-windows-x64.msi before you open it" warning.

**Path**: SignPath Foundation provides **free Authenticode code-signing certificates** to open-source projects (https://signpath.io/foundation — verified 2026-05-27: "SignPath DevSec360 — The free Code Signing & Software Integrity solution for Open Source Projects"). Once issued + integrated, every Amore Windows `.exe` and `.msi` gets a real Authenticode signature → SmartScreen recognizes it → warning disappears.

**Cost**: zero. **Eligibility**: open-source project on GitHub with Apache-2.0 or other OSI license (Amore qualifies).

## Application steps (user-action, ~10 min)

1. Go to https://signpath.io/foundation/apply
2. Sign in with the GitHub account that owns `antonio-amore-akiki/amore`
3. Fill the form with the answers below (copy-paste-able)
4. Submit; SignPath reviews + issues cert within ~3-5 business days

## Pre-filled answers

| Field | Answer |
|---|---|
| Project name | Amore |
| Project URL | https://github.com/antonio-amore-akiki/amore |
| License (SPDX) | Apache-2.0 |
| Primary maintainer | Antonio (single-author project) |
| Maintainer email | antonioakiki15@gmail.com |
| Repository visibility | Public |
| Project description (≤300 chars) | Local-first AI memory backbone for IDE/agent assistants — Claude Desktop / Claude Code / Cursor / Cline / Continue. One-click install on Windows / macOS / Linux. Privacy-by-design (all data stays on user device). Apache-2.0 licensed; SLSA L3 attested; CycloneDX SBOM included. |
| Why open-source code-signing | Amore is a free, local-first privacy-focused tool. Without Authenticode signing, every Windows download triggers SmartScreen "uncommonly downloaded" warnings, blocking non-technical users from installing. SignPath OSS enables consumer-grade install UX without the $300/yr cost barrier. |
| Build pipeline | GitHub Actions (release.yml workflow) — already produces .exe (Inno Setup wrapper) and .msi (cargo-wix) artifacts at every tag. SignPath integration goal: signpath.io GitHub Action signs both .exe and .msi during release pipeline; signed artifacts uploaded to GH releases. |
| Number of artifacts to sign per release | 2 (one `.exe` Inno Setup wrapper + one `.msi` MSI from cargo-wix) |
| Other relevant projects you maintain | None (single-author solo project) |

## After cert issued

SignPath sends two pieces of integration:
1. A GitHub Actions workflow snippet (extends `.github/workflows/release.yml`)
2. Project-side secret config (SIGNPATH_API_TOKEN as repo secret)

Integration is one workflow-step addition to the existing `windows-build` job. Once integrated:
- `cargo-wix` produces unsigned MSI → SignPath signs → signed MSI uploaded
- Inno Setup produces unsigned .exe → SignPath signs → signed .exe uploaded
- README first-launch-help section drops the "click 'More info → Run anyway'" instruction
- SmartScreen reputation accumulates over downloads; warning disappears entirely after ~50-100 downloads on signed artifacts

## What it does NOT solve

- **macOS Gatekeeper warning**: SignPath is Windows-only. macOS code-signing requires Apple Developer ID ($99/yr) for Notarization. Currently Amore ships unsigned macOS .pkg/.dmg with one-time right-click → Open friction.
- **Linux**: no equivalent friction (binary execution + .deb/.rpm don't need code-signing).

## Status

**Pending user submission**. This doc is the pre-fill; Antonio clicks Apply → pastes answers → submits → waits 3-5 business days for cert.
