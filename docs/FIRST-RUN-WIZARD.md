---
stable: true
topic: first-run-wizard
---
# First-Run Wizard

6-screen egui wizard shipped with `amore-gui`. Target: ≤2 min from launch to done on a fresh OS.

## Screen Flow

```
Welcome → DataDir → BundledDeps → IdeDetect → WireConfirm → Done
```

Back is available on every screen except Welcome. Next requires screen-specific validation.

---

## Screen 1: Welcome + License

- Amore logo and tagline: "Local-first persistent memory for every AI tool"
- Scrollable Apache-2.0 license text
- "I accept the Apache 2.0 license terms" checkbox — **gates Next**

---

## Screen 2: Data Directory

- Pre-filled with OS-conventional default:
  - Windows: `%LOCALAPPDATA%\Amore`
  - macOS: `~/Library/Application Support/Amore`
  - Linux: `~/.local/share/Amore`
- "Browse…" opens a native folder picker (rfd 0.15)
- Free disk space shown below the path field
- Warning displayed if free space < 500 MB (needed for Ollama model + Qdrant data)

---

## Screen 3: Bundled Components

| Component | User-facing label | Version |
|-----------|------------------|---------|
| Ollama | Local AI model | v0.3.x |
| Qdrant | Memory index | v1.15.x |

First-run disk usage: approximately 2–4 GB.

---

## Screen 4: IDE Auto-Detect

Scans the filesystem for each of the 5 supported tools:

| Tool | Windows | macOS | Linux |
|------|---------|-------|-------|
| Claude Desktop | `%APPDATA%\Claude\claude_desktop_config.json` | `~/Library/…/Claude/claude_desktop_config.json` | `~/.config/Claude/claude_desktop_config.json` |
| Claude Code | `~/.claude/settings.json` | same | same |
| Cursor | `~/.cursor/mcp.json` | same | same |
| Cline | `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` | `~/Library/…/Code/User/globalStorage/…` | `~/.config/Code/User/globalStorage/…` |
| Continue | `~/.continue/config.yaml` | same | same |

Each detected tool appears in a table with a "Wire?" checkbox (default: checked).

---

## Screen 5: Wire Confirmation

- Shows a JSON or YAML diff preview for each selected tool
- **Apply** button executes the wire-up (reads existing config, merges, writes atomically)
- After Apply succeeds, a **Next** button advances to Screen 6
- Errors shown in red with a **Retry** button

---

## Screen 6: Done

| Button | Action |
|--------|--------|
| Open dashboard | Opens `http://localhost:3111` in the default browser |
| Run in background (tray) | Minimizes wizard; spawns the system tray icon |

---

## Implementation

- Source: `crates/amore-gui/src/wizard/mod.rs` + `wizard/screens.rs`
- State machine: `Screen` enum with `next()` / `prev()` + `WizardState::can_advance()`
- Tests: `crates/amore-gui/tests/wizard_state_tests.rs` — 8 tests PASS
