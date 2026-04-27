# Amore Roadmap

Single-author local-first project — public roadmap is intentionally short and honest.

## v1.0 — shipped

- Windows MSI (cargo-wix; bundles Ollama + Qdrant; ~60 MB)
- Linux AppImage + `.deb` + `.rpm`
- **macOS Homebrew tap LIVE** — `brew install antonio-amore-akiki/tap/amore` works for Apple Silicon + Intel via GHA macos-latest builds (free public-repo runners)
- 6-screen first-run wizard (≤2 min target); 5-IDE auto-detect + auto-wire (Claude Desktop / Claude Code / Cursor / Cline / Continue.dev)
- System-tray icon (cross-platform); HKCU Run-key autostart on Windows
- SSH-signed `sha256sums.txt`; CycloneDX SBOM `composition.aggregate = complete`; SLSA L3 attestation
- LongMemEval R@5 = 1.0 / R@10 = 1.0 on mock corpus (live-stack run scheduled v1.1)
- Adversarial-eval 0 failures across 3 attack classes (prompt-injection / memory-exfil / recall-poisoning)
- 14/14 PRR PASS; SLO + error-budget policy live
- **Repo public-flipped 2026-05-27** — source visible at https://github.com/antonio-amore-akiki/amore; free GHA macOS runners unlocked; OSSF Scorecard public scoring activated; Discussions discoverable
- **v-next #34: eframe 0.29→0.34.3 + tantivy 0.22→0.26.1 + qdrant-client 1.15→1.18 migrations landed** (60 test binaries / 0 failures; cargo clippy -D warnings clean); reqwest + tonic deferred to qdrant 1.19+ per `docs/adr/0015`

## v1.1 — next (no committed timeline; single-author throughput)

- **Windows ARM64 binaries** — Surface Pro X + ARM laptops. Currently no Windows ARM SDK on dev host
- **LongMemEval live-stack run** — full Qdrant + Ollama stack instead of mock corpus. Currently mock-only
- **Animated demo GIF in README** — captured during clean-Windows-VM smoke. Currently static mockup PNGs
- **`crates.io` publication** — enables `cargo install amore-mcp` + rustdoc on docs.rs. Currently local-binary distribution only. Public-flip unblocks this
- **Hosted docs site at `docs.amore.dev`** — mdBook → GitHub Pages. Currently `docs/` in-repo only

## v-next — sustaining (no version commitment)

- **Apple Developer ID notarization** — ships signed `.pkg` / `.dmg` removing macOS Gatekeeper warning. Excluded under "only unlimited free options" constraint (Apple Dev ID is $99/yr). Re-evaluate if constraint changes
- **reqwest 0.13 + tonic 0.14 + prost 0.14** — bump when qdrant-client 1.19+ relaxes transitive pins (currently blocked per `docs/adr/0015`)
- **OSSF Scorecard structural ceiling raise** — current score is structural ceiling for workflow_dispatch-only workflows. Re-measure after public-flip + relaxed constraints
- **GitHub Discussions categories + community curation** — enabled at flip; activate categories + curate after first external users land
- **Brand landing page at `amore.ai`** — separate marketing site. Sustained marketing copy + hosting; not a one-day item
- **Additional CLI IDE adapters** — current 9 total (5 wizard + 4 CLI-only). Add on user demand only

## Out of scope (explicit non-goals)

- **SaaS hosting / cloud edition** — Amore is local-first by design; data never leaves the user's machine.
- **Telemetry / analytics** — no collection, no opt-in, no opt-out (per `docs/GDPR-SCOPING.md`).
- **Multi-tenant / enterprise SLA tier** — single-author scope; not a vendor product.
- **Closed-source pivot** — Apache-2.0 OSS is the chosen distribution model.
