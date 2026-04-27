# 5. Bundle Qdrant and Ollama inside the installer

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore targets non-technical users: product managers, writers, analysts.
The user mandate is "one-click install — user should not need to open a
terminal". Amore depends on two heavyweight subprocesses: Qdrant (vector
store) and Ollama (local embedding model). Both have their own
installation flows that require a terminal, admin rights, or package
managers.

How should these dependencies reach the end-user machine?

## Decision Drivers

* Non-technical user cannot be asked to install deps manually
* Single .exe per OS (Windows), .dmg (macOS), .AppImage (Linux)
* Installer must be reproducible and signable
* Silent install path for enterprise managed machines
* Qdrant ~70 MB binary; Ollama ~200 MB binary — size is acceptable
* No Docker requirement (Docker Desktop is a $21/seat enterprise product
  and requires manual install on non-technical user machines)

## Considered Options

* Bundled subprocess via OS-native installer (Inno Setup / pkgbuild)
* Docker-required: ship a docker-compose.yml, require Docker Desktop
* User-installs-manually: link to Qdrant + Ollama download pages

## Decision Outcome

Chosen option: **bundled subprocess via Inno Setup `[Files]` block
(Qdrant) + Ollama silent install (Windows)**.

The Windows installer (`installer/windows/amore.iss`) stages
`qdrant.exe` from GitHub Releases into `{app}\bin\` and runs the Ollama
installer silently via `[Run]` with `/S` flag. The Amore daemon detects
both binaries at `{app}\bin\` and spawns them as managed subprocesses on
startup.

macOS ships a `.dmg` with a bundled `qdrant` universal binary and
invokes the Ollama `.pkg` silently via `pkgbuild`. Linux AppImage
embeds both via `linuxdeploy --plugin qdrant`.

First-run sequence:
1. Amore daemon starts.
2. Checks `bin/qdrant.exe` present → spawns on `127.0.0.1:6334`.
3. Checks `bin/ollama.exe` present → spawns on `127.0.0.1:11434`.
4. Pulls embedding model (`nomic-embed-text:latest`) on first run only.
5. Health-check loop with 30s timeout; surfaces plain-English error
   if either subprocess fails to respond.

Reference: v0.3.1 Inno installer pattern, `installer/windows/amore.iss`.

### Consequences

* Good: zero terminal interaction required; one double-click completes
  the full setup including dependencies
* Good: reproducible installer artifact; version-pinned binaries checked
  into `installer/manifest.lock`
* Good: no Docker Desktop dependency — works on Home edition Windows
* Good: silent-install flag (`/S`) supports enterprise managed deployment
* Bad: installer grows to ~300-400 MB compressed (Qdrant + Ollama + Amore)
* Bad: Amore owns the subprocess lifecycle; crash loops require
  watchdog logic
* Bad: Ollama silent install on macOS requires Gatekeeper notarisation
  of the outer .dmg bundle

## Pros and Cons of the Options

### Bundled subprocess (CHOSEN)

* Good: one-click install, no manual steps
* Good: version-pinned dependencies, reproducible build
* Good: supports enterprise silent-deploy via `/S` flag
* Bad: larger installer size (~300-400 MB)
* Bad: subprocess watchdog logic required in Amore daemon

### Docker-required

* Good: clean process isolation
* Good: easy to update dependencies independently
* Bad: Docker Desktop is not free for enterprise (>$21/seat)
* Bad: non-technical user cannot install Docker unassisted
* Bad: docker-compose.yml launch adds friction (requires terminal)
* Bad: completely fails the one-click install mandate

### User-installs-manually

* Good: smallest Amore installer footprint
* Bad: violates the one-click mandate outright
* Bad: version skew: user installs mismatched Qdrant / Ollama versions
* Bad: non-technical user calls support at first step

## More Information

* `installer/windows/amore.iss` — the Inno Setup script (v0.3.1)
* `installer/manifest.lock` — pinned Qdrant + Ollama binary checksums
* Subprocess watchdog lives in `crates/amore-daemon/src/watchdog.rs`
* macOS notarisation workflow: `ci/notarize.sh` (Phase G)
* Linux AppImage build: `ci/appimage.sh` (Phase G)
