# Amore Quickstart — Claude Code
stable: true
topic: amore quickstart claude mcp setup
tier: 1 (non-technical user)
last_verified: 2026-05-26 (amore v0.3.1-live-fire)

Connect Amore's long-term memory to Claude Code in under five minutes. No coding required.

---

## 1. Install Amore

**Windows**
1. Download `Amore-Setup-v0.3.1.exe` from
   https://github.com/antonio-amore-akiki/amore/releases/latest
2. Double-click the installer and follow the first-run wizard.
   The wizard installs Ollama and Qdrant automatically if they are absent.

**macOS / Linux**
1. Download the matching `.tar.gz` from the same release page.
2. Extract it: `tar -xzf Amore-*.tar.gz`
3. Verify: `./amore --version` → should print `amore 0.3.1`

**Cross-OS alternative (Node.js required)**
```
npm install -g @anto/amore
amore --version
```

**Verify the download's integrity (SHA-256)**
The GitHub Release notes list the expected hash beside each file.

- Windows PowerShell:
  ```powershell
  (Get-FileHash .\Amore-Setup-v0.3.1.exe -Algorithm SHA256).Hash
  ```
- macOS / Linux:
  ```bash
  sha256sum Amore-Setup-v0.3.1.exe
  ```
Compare the output character-by-character with the value in the Release notes.
If they differ, delete the file and re-download.

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
To opt in (v0.5.0+): set `AMORE_TELEMETRY=on` in your shell environment.

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
