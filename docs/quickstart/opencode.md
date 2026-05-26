# Amore Quickstart — opencode
stable: true
topic: amore quickstart opencode mcp setup
tier: 1 (non-technical user)
last_verified: 2026-05-26 (amore v0.3.1-live-fire)

Connect Amore's long-term memory to opencode (sst.dev) in under five minutes.

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
Compare the output with the value in the Release notes. If they differ,
delete the file and re-download.

---

## 2. Connect opencode to Amore

Run once in any terminal:

```
amore init opencode
```

**What it writes:** `opencode.json` in the OS config directory.

| OS      | Path |
|---------|------|
| Windows | `%APPDATA%\opencode\opencode.json` |
| macOS   | `~/Library/Application Support/opencode/opencode.json` |
| Linux   | `~/.config/opencode/opencode.json` |

Amore merges one entry into the top-level `mcp` map using opencode's
`"type": "local"` discriminator:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "amore": {
      "type": "local",
      "command": ["amore-mcp"]
    }
  }
}
```

Note: opencode's `mcp` field uses a `command` **array** (binary + flattened
args), unlike other IDEs that use a separate `args` field.

The `$schema` field and existing `mcp` entries are preserved.
Running the command a second time is safe — it is idempotent.

**Restart opencode** after running `amore init opencode`.

---

## 3. Verify the handshake

Start opencode and submit this prompt:

> "What did we discuss last week about authentication?"

Expected: Amore returns ≥ 1 recall hit (or a message that no matching memory
exists yet if this is a fresh install — that is still a successful connection).

**If you suspect a connection problem:**

1. Run `amore doctor --json` — exit code 0 means all services are healthy.
2. Check opencode's log output for an `amore-mcp` line showing it started.
3. If Ollama is missing: the first-run wizard should have installed it.
   Verify with `ollama --version`.
4. If Qdrant is missing: `qdrant --version` or restart Amore from the system
   tray (Windows) / menu bar (macOS).

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

Total disk usage: ~600 MB after the first install.

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

**"opencode doesn't see Amore after `amore init opencode`"**
Restart opencode. It caches the MCP server list at startup; a full restart is
required to pick up changes to `opencode.json`.
