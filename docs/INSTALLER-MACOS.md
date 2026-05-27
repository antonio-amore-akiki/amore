---
stable: true
---
# Installing Amore on macOS

> v1.0.0-rc1 — scope-cut: Homebrew tap + Sigstore-signed raw binary.
> Full .pkg/.dmg with Apple Notarization is deferred to v-next (see documented gap below).

---

## Recommended: Homebrew

The fastest path for users comfortable with a terminal.

```sh
brew tap antonio-amore-akiki/amore
brew install amore
```

After install, `amore`, `amore-mcp`, and `amore-gui` are on your PATH.

To upgrade later:

```sh
brew update && brew upgrade amore
```

---

## Alternative: Raw Binary (manual install)

Use this path if you prefer not to use Homebrew, or to verify the binary before running it.

### 1. Download

Choose the archive that matches your Mac:

| Mac type | Archive |
|---|---|
| Apple Silicon (M1/M2/M3/M4) | `amore-v1.0.0-rc1-aarch64-apple-darwin.tar.gz` |
| Intel | `amore-v1.0.0-rc1-x86_64-apple-darwin.tar.gz` |

Download from:
`https://github.com/antonio-amore-akiki/amore/releases/tag/v1.0.0-rc1`

Extract:

```sh
tar -xzf amore-v1.0.0-rc1-<arch>-apple-darwin.tar.gz
```

### 2. Verify with Sigstore (cosign)

Each binary ships with a `.sigstore` bundle. Verify before running:

```sh
cosign verify-blob \
  --bundle amore-cli.sigstore \
  amore-cli
```

Exit 0 = verified. If cosign is not installed: `brew install cosign`.

### 3. Install

```sh
chmod +x amore amore-mcp amore-gui
sudo mv amore amore-mcp amore-gui /usr/local/bin/
```

---

## Tray + GUI (amore-gui) on macOS

The `amore-gui` binary includes the system tray icon. Run it once to confirm it works:

```sh
amore-gui
```

To launch it automatically at login:

1. Open **System Settings → General → Login Items**.
2. Click **+** under "Open at Login".
3. Select `amore-gui` from `/usr/local/bin/amore-gui`.

> macOS 13 (Ventura) and later enforce sandbox policy that blocks self-installing
> LaunchAgents. The one-time manual Login Items step is the supported path.

---

## Gatekeeper / "Apple cannot verify this app" warning

Because v1.0.0-rc1 is not yet Notarized (see gap below), macOS Gatekeeper will show a
warning on first launch if you double-click the binary in Finder.

**Workaround**: open from a terminal (`/usr/local/bin/amore`), or right-click → Open in
Finder and confirm once. After the one-time confirmation, Gatekeeper will not block it again.

### Documented gap: .pkg / .dmg + Apple Notarization deferred to v-next

Notarization requires an Apple Developer ID membership ($99 USD/year). Without it:

- `.pkg` and `.dmg` installers cannot be created in a way that passes Gatekeeper by default.
- Double-clicking a non-Notarized binary triggers the "Apple cannot verify" dialog; users
  must right-click → Open for a one-time override.
- Notarized binaries open without any dialog.

Mitigation in v-next: enroll in Apple Developer Program → sign with `codesign --options runtime`
→ notarize via `xcrun notarytool submit` → staple → wrap in `.dmg` with `hdiutil`.

Until then, users on macOS without CLI fluency will experience Gatekeeper friction. The
right-click → Open workaround is documented above.

---

## Uninstall

Via Homebrew:

```sh
brew uninstall amore
```

Manual install:

```sh
sudo rm /usr/local/bin/amore /usr/local/bin/amore-mcp /usr/local/bin/amore-gui
```

---

## Verify your install

```sh
amore --version
# amore 1.0.0-rc1
```
