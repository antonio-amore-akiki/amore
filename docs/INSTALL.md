stable: true

# Installation — Amore

## Installer (no terminal needed — recommended)

| OS | Download | What to do |
|---|---|---|
| Windows | [amore-windows-x64.msi](https://github.com/antonio-amore-akiki/amore/releases/latest/download/amore-windows-x64.msi) | Double-click. If SmartScreen warns, click "More info" → "Run anyway". |
| macOS | `brew install antonio-amore-akiki/amore/amore` (one Terminal command) OR [amore-1.0.0-macos-x86_64.tar.gz](https://github.com/antonio-amore-akiki/amore/releases/latest) | Run Homebrew command, or download + double-click the `.tar.gz`. |
| Linux | [amore-1.0.0-x86_64.AppImage](https://github.com/antonio-amore-akiki/amore/releases/latest/download/amore-1.0.0-x86_64.AppImage) | Make executable (`chmod +x`), then double-click. No sudo needed. |

After installing, the first-run wizard opens automatically. It takes about 2 minutes and guides you through connecting Amore to your AI tools (Claude Desktop, Cursor, Cline, Continue.dev).

## Developer install paths (terminal required)

5 paths, each yields the same v1.0.0 binaries.

## 1. Homebrew (macOS/Linux)

```bash
brew install antonio-amore-akiki/amore/amore
```

## 2. WinGet (Windows)

```powershell
winget install Antonio.Amore
```

## 3. AUR (Arch Linux)

```bash
yay -S amore-bin
```

## 4. Docker (any OS)

```bash
docker pull ghcr.io/antonio-amore-akiki/amore:1.0.0
docker run -p 7777:7777 ghcr.io/antonio-amore-akiki/amore:1.0.0
```

## 5. Binary download from GitHub

```bash
gh release download v1.0.0 --repo antonio-amore-akiki/amore --pattern 'amore-*-<OS>-<ARCH>.*'
```

Verify SHA256:

```bash
gh release download v1.0.0 --pattern 'sha256sums.txt'
sha256sum -c sha256sums.txt
```

Verify signature (Sigstore keyless):

```bash
cosign verify-blob \
  --bundle amore-v1.0.0-*.sigstore \
  --certificate-identity-regexp "antonio-amore-akiki" \
  --certificate-oidc-issuer "https://accounts.google.com" \
  amore-v1.0.0-*.tar.gz
```

## First run

```bash
amore init      # creates ~/.local/share/amore + config
amore doctor    # validates Qdrant + Ollama reachable
amore serve     # starts MCP server on stdio
```

See `docs/quickstart/<ide>.md` for IDE-specific adapter setup.

## Sources

- brew.sh formula docs
- learn.microsoft.com/en-us/windows/package-manager
- wiki.archlinux.org/title/AUR
- docs.docker.com
