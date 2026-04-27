---
stable: true
---
# Amore — Linux Install Guide

Three install paths for Linux x86_64: AppImage (no sudo, runs anywhere), `.deb`
(Debian/Ubuntu/Mint), and `.rpm` (Fedora/RHEL/openSUSE). All artifacts are Sigstore-signed.

Download from the [GitHub releases page](https://github.com/antonio-amore-akiki/amore/releases).

## AppImage (Recommended — no sudo, all distros)

```bash
# Download
wget https://github.com/antonio-amore-akiki/amore/releases/latest/download/amore-x86_64.AppImage

# Make executable
chmod +x amore-x86_64.AppImage

# Run (double-click in file manager, or from terminal)
./amore-x86_64.AppImage
```

The AppImage bundles all runtime libraries — no system packages required.

## .deb (Debian, Ubuntu 22.04+, Linux Mint)

```bash
sudo apt install ./amore_<version>_amd64.deb
```

This installs:
- `/usr/bin/amore` (CLI)
- `/usr/bin/amore-gui` (GUI / tray)
- `/usr/share/applications/amore.desktop` (launcher entry)
- `/usr/lib/systemd/user/amore-tray.service` (optional tray auto-start)

Runtime deps installed automatically: `libgtk-3-0`, `libayatana-appindicator3-1`.

Uninstall:
```bash
sudo apt remove amore
```

Data directory `~/.local/share/amore/` is preserved on uninstall.

## .rpm (Fedora 39+, RHEL 9+, openSUSE)

```bash
sudo dnf install amore-<version>-1.x86_64.rpm
```

Same files installed as the .deb path. Runtime deps: `gtk3`, `libayatana-appindicator`.

Uninstall:
```bash
sudo dnf remove amore
```

## Sigstore Verification

Each artifact ships with a `.sigstore` bundle. Verify before running:

```bash
# Install cosign once
curl -Lo cosign https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64
chmod +x cosign && sudo mv cosign /usr/local/bin/

# Verify AppImage
cosign verify-blob --bundle amore-x86_64.AppImage.sigstore amore-x86_64.AppImage

# Verify .deb
cosign verify-blob --bundle amore_<version>_amd64.deb.sigstore amore_<version>_amd64.deb

# Verify .rpm
cosign verify-blob --bundle amore-<version>-1.x86_64.rpm.sigstore amore-<version>-1.x86_64.rpm
```

A clean `Verified OK` response confirms the artifact was signed by the Amore release key.

## Tray Auto-Start (Optional)

The `.deb` and `.rpm` installers place a systemd user unit at
`/usr/lib/systemd/user/amore-tray.service`. It is **not enabled by default**.

Enable after install (Amore will appear in the system tray on next login):

```bash
systemctl --user enable --now amore-tray.service
```

Disable:

```bash
systemctl --user disable --now amore-tray.service
```

The tray service requires a display server (`DISPLAY=:0`). On headless systems do not
enable it. The installer wizard (screen 5) offers to enable it during setup.

## Data Preservation

Uninstalling via `apt remove amore` or `dnf remove amore` preserves your memory data in
`~/.local/share/amore/`. To also remove data:

```bash
rm -rf ~/.local/share/amore/
```
