---
stable: true
---
# Amore — Windows Installer Guide

## Download

Download `amore-windows-x64.msi` from the [releases page](https://github.com/antonio-amore-akiki/amore/releases).

## Verify integrity before installing

### SHA256 checksum

```powershell
Get-FileHash amore-windows-x64.msi -Algorithm SHA256
```

Compare the output hash against the `sha256sums.txt` published alongside the release asset.

### Sigstore verification (cosign)

Install cosign: https://docs.sigstore.dev/cosign/system_config/installation/

```powershell
cosign verify-blob `
    --bundle amore-windows-x64.msi.sigstore `
    amore-windows-x64.msi
```

Expected output: `Verified OK`. The signature is keyless OIDC (Sigstore transparency log)
— no private key to manage or rotate.

## Run the installer

Double-click `amore-windows-x64.msi`, or run from PowerShell:

```powershell
msiexec /i amore-windows-x64.msi
```

The installer requires no admin rights for a per-user install. A UAC elevation dialog
appears only if you choose a system-wide destination.

## What gets installed

| File | Location | Purpose |
|---|---|---|
| `amore-gui.exe` | `%ProgramFiles%\Amore\` | First-run wizard + system tray |
| `amore-mcp.exe` | `%ProgramFiles%\Amore\` | MCP server (ports 9090/9091) |
| `amore.exe` | `%ProgramFiles%\Amore\` | CLI for advanced ops |
| `ollama.exe` | `%ProgramFiles%\Amore\` | Bundled local LLM runtime (v0.24.0) |
| `qdrant.exe` | `%ProgramFiles%\Amore\` | Bundled vector store (v1.18.1) |
| Start Menu shortcut | `%AppData%\Microsoft\Windows\Start Menu\Programs\Amore\` | Launches `amore-gui.exe` |
| Run registry key | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\Amore` | Tray auto-start on login |

## First-run wizard

On first launch `amore-gui.exe` detects whether to open the setup wizard or go straight
to the system tray:

1. **Welcome** — version info, bundled runtime versions.
2. **Memory directory** — choose where Amore stores your data (default `%LOCALAPPDATA%\amore\`).
3. **Ollama** — auto-detected from PATH or bundled binary; model download initiated.
4. **IDE detection** — scans for Claude Desktop, Claude Code, Cursor, Cline, Continue.
5. **IDE auto-wire** — writes MCP server config into each detected IDE's config file.
6. **Done** — wizard exits; tray icon appears in the system notification area.

Total first-run time: under 2 minutes on a fresh install.

## Tray icon

After setup, `amore-gui.exe --tray` runs automatically on login (via the Run registry key).
Right-click the tray icon to access:

- **Open wizard** — rerun any setup step.
- **Status** — MCP server and Ollama health at a glance.
- **Quit** — stops the MCP server and exits.

## Uninstalling

Open **Add or Remove Programs** (Settings → Apps) and select **Amore**, or run:

```powershell
msiexec /x amore-windows-x64.msi
```

### Data directory preservation

The uninstaller removes all installed files from `%ProgramFiles%\Amore\` and the Start
Menu entry. It does **not** delete your data directory (`%LOCALAPPDATA%\amore\`). Your
stored memories are preserved after uninstall.

To delete the data directory manually:

```powershell
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\amore"
```

## SmartScreen warning

Because the MSI is Sigstore-signed (keyless OIDC) rather than Authenticode-signed with
a paid EV certificate, Windows SmartScreen may show a "Windows protected your PC" dialog:

1. Click **More info**.
2. Click **Run anyway**.

The Sigstore bundle (`amore-windows-x64.msi.sigstore`) lets you verify the binary
against a public transparency log independently of Windows trust stores.

## Bundled runtime versions

| Runtime | Version | Source | SHA256 (zip) |
|---|---|---|---|
| ollama | v0.24.0 | github.com/ollama/ollama | `40c523d3eeba6f4647c5ca58fe47f15b8dee79f7675ebf573458890064f424c7` |
| qdrant | v1.18.1 | github.com/qdrant/qdrant | `fe1eab78c24157b21988b3480ce75709e76ca0168ba644fc5a49017bacfec1c6` |

Bundled binaries are extracted during the MSI build; verify with cosign before
running to confirm supply-chain integrity.

## MSI artifact (current build)

| Field | Value |
|---|---|
| Filename | `amore-windows-x64.msi` |
| SHA256 | `b60dd6faef32efa9b39146ddabf2a289e6ad0fc8f4386f9f90234baffc5d8d36` |
| ollama.exe inside | 40.59 MB (real binary, SHA-verified zip) |
| qdrant.exe inside | 82.81 MB (real binary, SHA-verified zip) |
| Sigstore | pending W9 stable-tag (see `state/w8.5a-sign-pending-user.md`) |
| Binary changes | 6-screen wizard (AmoreWizardApp); `--tray` handler wired; 5-IDE auto-detect/wire (Claude Desktop, Claude Code, Cursor, Cline, Continue); OllamaBin Vital=yes |
| Supersedes | `0d7b53a53a88341ec3f0fb81e3045320b8e588acf7ee40abbb33d8be18f9c9f9` |
