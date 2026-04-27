# Upgrading Amore

stable: true
purpose: every breaking change between minor versions plus its migration path
update_cadence: per release tag

For every `[minor]` bump that breaks an installed deployment, this file
documents what changed and how to recover.

## v0.3.0-live-fire → v0.3.1-live-fire (security patch)

**No breaking changes.** Re-install replaces the binaries in-place; the
SQLite + Qdrant data on disk is untouched.

Re-install paths:
- Windows: double-click `Amore-Setup-v0.3.1.exe`; the installer detects
  the existing install and overwrites.
- macOS / Linux: extract the new `.tar.gz` over `~/.local/bin/amore`
  (or wherever you placed the v0.3.0 binaries).
- npm: `npm install -g @anto/amore@0.3.1` — the postinstall now
  enforces Sigstore bundle verification before extracting the new
  binaries. If verification fails, the upgrade aborts and the
  previously-installed v0.3.0 binaries remain in place.

Security implication: v0.3.1 closes 2 Critical + 3 Major findings.
Continuing to run v0.3.0 on a non-technical-user machine leaves it
exposed to the silent Ollama-installer RCE (Critical 10a) and the
unsigned npm artifact path (Critical 10b). **Upgrade is strongly
recommended.**

## v0.2.x → v0.3.x (rename obelion -> Amore)

Breaking changes:
- Crate names, binary names, env vars, data paths, npm package all
  renamed `obelion-*` -> `amore-*` / `OBELION_*` -> `AMORE_*`.
- Default data path moves from `%APPDATA%\obelion\obelion.db` to
  `%APPDATA%\Amore\amore.db`. First `amore-mcp` start auto-migrates the
  SQLite file if the old path exists and the new one does not — a
  `migrated-from-obelion.txt` marker is left in `%APPDATA%\Amore\`.
- Legacy `OBELION_*` env vars continue to work through v0.3.x with a
  `tracing::warn!` deprecation message. They are **removed in v0.4.0**.
- IDE adapter configs are atomically replaced on the next `amore init
  <ide>` run — the previous `obelion` MCP server entry is renamed to
  `amore` in-place. `.bak` siblings preserve the v0.2.x state for a
  one-revision rollback window.
- npm package: `@anto/obelion` is deprecated. Install `@anto/amore`
  instead. The old package's `unpublish` is left in place for one
  release cycle so existing installs continue to function but emit a
  deprecation banner on every postinstall.

Recovery / rollback:
- The SQLite migration is one-way. If you need to roll back to v0.2.x,
  restore from the `obelion.db.bak` sibling left in the old data path,
  or from your Kopia backup.

## Planned v0.3.1 → v0.4.0 (Phase G hygiene; not yet shipped)

Anticipated breaking changes:
- `OBELION_*` env vars removed (warned-since v0.3.0).
- `clippy::unwrap_used = "deny"` may surface latent panics that v0.3.x
  silently swallowed. Behaviour-compatible but error-message format
  changes for the affected paths.
- `npm/postinstall.js` pins cosign to a specific version (C-1 fix);
  existing installs that cached cosign at `~/.amore-cache/cosign` will
  re-download on upgrade to align with the pinned SHA-256.
- macOS `current_exe()` fallback removed (C-2 fix). If a future macOS
  release returns Err for `current_exe()` (extremely rare), the GUI
  surfaces a plain-English error instead of falling back to a
  PATH-injectable bare name.

Migration path:
- Run `amore init <ide>` once per IDE you use to refresh the MCP
  config (idempotent; safe to run on every upgrade).
- No data path changes from v0.3.x to v0.4.0.

## Format

Every section below this point follows:
```
## vA.B.C -> vA.B.D (one-line summary)

Breaking changes:
- ...

Recovery:
- ...
```
