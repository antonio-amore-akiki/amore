# Amore Quickstart — Claude Code
stable: true
topic: amore quickstart claude mcp setup
tier: 1 (non-technical user)
last_verified: 2026-05-27 (amore v1.0.0)

Connect Amore's long-term memory to Claude Code in under five minutes. No coding required.

---

## 1. Install Amore

**Windows**
1. Download `amore-windows-x64.msi` from
   https://github.com/antonio-amore-akiki/amore/releases/latest
2. Double-click the installer and follow the first-run wizard (6 screens, ~2 min).
   The wizard auto-detects + auto-wires Claude Desktop / Claude Code / Cursor / Cline / Continue.

**Linux**
1. Download `amore-gui-x86_64.AppImage` (portable; no sudo) OR `amore_1.0.0-1_amd64.deb` (Ubuntu/Debian) OR `amore-1.0.0-1.x86_64.rpm` (Fedora/RHEL) from the release page.
2. AppImage: `chmod +x amore-gui-x86_64.AppImage && ./amore-gui-x86_64.AppImage`
3. .deb: `sudo apt install ./amore_1.0.0-1_amd64.deb`
4. .rpm: `sudo dnf install amore-1.0.0-1.x86_64.rpm`

**macOS** — Homebrew (formula SHA stamp pending v1.1 macOS binary upload):
```
brew install antonio-amore-akiki/tap/amore
```

**Verify the download's integrity (SHA-256)**
Each release ships `sha256sums.txt` + `sha256sums.txt.sig` (SSH-signed) + `allowed_signers`. Verify before installing:

- Windows PowerShell:
  ```powershell
  (Get-FileHash .\amore-windows-x64.msi -Algorithm SHA256).Hash
  # Compare against the line in sha256sums.txt
  Get-Content sha256sums.txt | ssh-keygen -Y verify -f allowed_signers -I "antonioakiki15@gmail.com" -n "file" -s sha256sums.txt.sig
  ```
- macOS / Linux:
  ```bash
  sha256sum -c sha256sums.txt
  ssh-keygen -Y verify -f allowed_signers -I "antonioakiki15@gmail.com" -n "file" -s sha256sums.txt.sig < sha256sums.txt
  ```
If the SSH-signature verify fails or the SHA mismatches, the artifact has been tampered with — delete and re-download.

---

## 2. Connect Claude Code to Amore

Run once in any terminal (no administrator rights needed):

```
amore init claude
```

**What it writes:** `~/.claude.json` — a cross-OS file that Claude Code reads on
every session start. Amore merges one entry into the top-level `mcpServers` map:

```json
{
  "mcpServers": {
    "amore": {
      "command": "amore-mcp",
      "args": []
    }
  }
}
```

A `.bak` backup of the previous `~/.claude.json` is created automatically.
Existing entries (other MCP servers, theme settings, etc.) are preserved.
Running the command a second time is safe — it is idempotent.

**Open Claude Code.** The `amore-mcp` server is loaded automatically at the
start of the next session. No manual config file editing is needed.

---

## 3. Verify the handshake

Paste this prompt in a new Claude Code conversation:

> "What did we discuss last week about authentication?"

Expected: Amore returns ≥ 1 recall hit (or a message that no matching memory
exists yet if this is a fresh install — that is still a successful connection).

**If you get 0 hits and suspect a connection problem:**

1. Run `amore doctor --json` — exit code 0 means all services are healthy.
2. Open the Claude Code output panel and search for `amore-mcp`. A line like
   `[mcp] amore-mcp started` confirms the server loaded.
3. If Ollama is missing: the first-run wizard should have installed it.
   Verify with `ollama --version`.
4. If Qdrant is missing: `qdrant --version` or restart Amore from the system
   tray icon (Windows) / menu bar (macOS).

---

## 4. Where Amore stores your memory

| OS      | Root path                  |
|---------|---------------------------|
| Windows | `%APPDATA%\Amore\`        |
| macOS   | `~/.config/amore/`        |
| Linux   | `~/.config/amore/`        |

Inside that directory:
- `amore.db` — SQLite database of structured memory
- `qdrant-storage/` — vector index
- `models/bge-small.onnx` — local embedding model

Total disk usage: ~600 MB after the first install (bundled `bge-small.onnx`
is the largest component; no cloud model downloads are required).

---

## 5. Privacy + telemetry

Amore does **not** phone home by default.
No telemetry, no analytics, and no crash reports leave your machine.
No telemetry surface exists; the env var is not read by any binary. Privacy-by-default per `docs/GDPR-SCOPING.md`.

---

## 6. Uninstall

- **Windows:** Settings → Apps → Amore → Uninstall.
  You will be asked: *"Keep your memory? [Keep / Delete]"*
- **macOS / Linux:** `amore uninstall` in a terminal. Same prompt.

---

## 7. Troubleshooting

**"SmartScreen blocked the installer"**
Click *More info* → *Run anyway*. Amore is self-signed; a paid EV certificate
is planned for v1.0.

**"Checksum doesn't match the Release notes"**
Re-download from the official releases page. If the mismatch persists, file a
security report following the instructions in `SECURITY.md`.

**"Claude Code doesn't see Amore after `amore init claude`"**
Restart Claude Code. It caches the MCP server list at startup; a full restart
is required to pick up changes to `~/.claude.json`.
