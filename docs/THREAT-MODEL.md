# Amore Threat Model (STRIDE-class)

stable: true
topic: amore threat model security stride dread non-technical-user privacy local-first
tag_baseline: v0.3.0-live-fire (commit ebbb5f0)

## Purpose

Formal STRIDE-style threat enumeration for Amore — a local-first MCP agent
memory backbone for non-technical users on Windows / macOS / Linux. This
document maps each threat to a mitigation that is either already in code or
scheduled on the road to v1.0.0.

See `SECURITY.md` for the disclosure policy and the higher-level posture
checklist; this file is the detailed architecture-level view.

## System overview

```
+--------------------+        +-----------------+
|  IDE (Claude Code, |        |  amore-gui.exe  |
|  Cursor, Codex,    |--MCP-->|  (egui native)  |
|  Cline, opencode,  |  stdio |                 |
|  Windsurf, Hermes) |        +-----------------+
+--------------------+                |
        |                             | spawn
        | tools/call                  v
        v                       +-----------------+
+----------------------+        |  amore-mcp.exe  |
|  amore-mcp.exe       |<------>|  + amore.exe    |
|  (rmcp server stdio) |        +-----------------+
+----------------------+                |
        |                               | RPC / IPC
        v                               v
+--------------------------+   +----------------------+
|  amore-core (Rust lib)   |   |  Ollama (localhost)  |
|  recall + canonical-docs |   |  qdrant (localhost)  |
|  ensemble + mining       |   +----------------------+
|  provenance + world-mdl  |
+--------------------------+
        |
        v
+--------------------------+
|  SQLite + Tantivy +      |
|  Qdrant storage          |
|  %APPDATA%/Amore/        |
+--------------------------+
```

## Trust boundaries

| Boundary | From → To | Trust delta | Notes |
|---|---|---|---|
| IDE ↔ amore-mcp | IDE process → amore-mcp via stdio | none | IDE has full local privilege; amore-mcp inherits user-level access |
| amore-mcp ↔ Ollama | localhost:11434 HTTP | local-only | TLS not needed for loopback; firewall rule MUST block external |
| amore-mcp ↔ Qdrant | localhost:6334 gRPC | local-only | Same; loopback only |
| amore ↔ user data | %APPDATA%/Amore/ | user-level | DPAPI encryption not applied by default (single-machine threat model — "stolen-laptop only" per SECURITY.md) |
| installer ↔ network | Internet during install (model + Ollama download) | high | Linux artifact Sigstore-signed; macOS + Windows self-signed pending paid certs |
| auto-update ↔ network | Internet for GitHub Releases poll | medium | Code-signed downloads; user prompted before install |

## STRIDE per asset

### Asset 1: User observations (in SQLite + Tantivy + Qdrant)

| Threat | Vector | Mitigation | Status |
|---|---|---|---|
| **S**poofing — fake IDE writing observations | Malicious MCP client connects via stdio | All write tools require explicit IDE handshake + per-session token | v1.0 (planned) |
| **T**ampering — observation log modified post-write | Local adversary edits SQLite directly | Cryptographic provenance chain (sha256 + canonical JSON) — shipped F4; `amore doctor` flags chain breaks | ✅ shipped |
| **R**epudiation — agent denies writing observation | n/a (agent has no identity beyond session) | provenance chain identifies writer ts + prev_hash; sessions logged | ✅ shipped |
| **I**nfo disclosure — backup contains sensitive prompts | Stolen laptop or backup leak | Threat model is "stolen-laptop only"; user-machine disk encryption is the layer | inherited |
| **D**oS — fill the SQLite + Qdrant with garbage | Malicious IDE floods observations | Rate limit + backpressure planned in Phase H | planned (v0.7.0) |
| **E**oP — observation write triggers code execution | sqlite injection / qdrant injection | Parameterised queries (rusqlite + qdrant-client) — no string concat in storage layer | ✅ shipped |

### Asset 2: MCP tool surface (recall, canonical_doc_lookup, ensemble_decide, observe, etc.)

| Threat | Vector | Mitigation | Status |
|---|---|---|---|
| **S**poofing — attacker spoofs an MCP tool call | n/a — stdio is process-local | Process isolation via OS | inherited |
| **T**ampering — modified MCP request payload | Local MITM impossible on stdio | n/a | n/a |
| **R**epudiation — tool call denied later | n/a | every tool call logged via tracing; structured spans planned in Phase G (E1) | partial |
| **I**nfo disclosure — recall returns docs the IDE shouldn't see | IDE A queries; gets IDE B's observations | Per-session namespace + workspace-scoped recall | planned (v0.7.0) |
| **D**oS — long-running ensemble call hangs the IDE | Crafted slow LLM response | AMORE_TIMEOUT_MS cap (B3 shipped); asserts timeout flips lane_unavailable | ✅ shipped |
| **E**oP — MCP tool exposes shell exec | n/a — no exec tool | Tool schema disallows; rmcp typed JsonSchema enforced | ✅ shipped |

### Asset 3: User PII / prompts in transit

| Threat | Vector | Mitigation | Status |
|---|---|---|---|
| **I**nfo disclosure — telemetry leaks prompts | Default-on telemetry | Telemetry DEFAULT OFF — AMORE_TELEMETRY=on explicit opt-in only; tcpdump test asserts zero outbound when off | planned (v0.5.0) |
| **I**nfo disclosure — auto-update polls GitHub with identifying headers | User-Agent leak | self-update crate sets generic UA; no anonymous run-id sent without opt-in | planned (v0.5.0) |
| **I**nfo disclosure — crash report sends stack to remote | Default-on crash report | Crash reports DEFAULT OFF; opt-in writes to local XDG cache only | planned (v0.5.0) |

### Asset 4: Installer + auto-update path

| Threat | Vector | Mitigation | Status |
|---|---|---|---|
| **T**ampering — installer .exe replaced with malware | MITM during download | Linux artifact Sigstore-signed (cosign verify-blob proven on clean Debian; A5 PASS); macOS + Windows self-signed pending Apple Dev ID + EV cert | partial |
| **T**ampering — auto-updater accepts unsigned newer version | Compromised GitHub Release | Signature verification before swap; auto-update prompts user; SLSA L3 provenance verified | planned (v1.0.0) |
| **I**nfo disclosure — installer logs sensitive paths | Install log shipped to user | %APPDATA%/Amore/install.log is user-scoped; never auto-uploaded | ✅ shipped |
| **E**oP — installer needs admin → privilege escalation | UAC prompt | PrivilegesRequired=lowest in Inno Setup script; installs to %LOCALAPPDATA% — no admin needed | ✅ shipped |

### Asset 5: IDE handshake config files

| Threat | Vector | Mitigation | Status |
|---|---|---|---|
| **T**ampering — `amore init <ide>` corrupts user's existing IDE config | Bug in adapter merge logic | Atomic write (.tmp + rename) + .bak sibling preserves prior content; idempotency tested | ✅ shipped |
| **I**nfo disclosure — IDE config exposes API keys | User has cloud-API key configured in IDE | Amore reads IDE config only on init; never persists IDE-side secrets | ✅ shipped |

### Asset 6: Bundled embedding model + Ollama silent install

| Threat | Vector | Mitigation | Status |
|---|---|---|---|
| **T**ampering — bge-small.onnx replaced with malicious model | MITM during model download or supply chain | Model SHA-256 pinned in installer manifest; verified before load | planned (v0.5.0) |
| **T**ampering — Ollama installer replaced | Network MITM during silent install | HTTPS-only download URL + SHA-256 verified before execution | planned (v0.5.0) |
| **E**oP — model file execution treated as code | Loading malformed ONNX triggers code path | `ort` crate validates input; no `eval()` on model bytes | ✅ shipped |

## DREAD scoring on top-3 highest-risk threats

| # | Threat | Damage | Reproducibility | Exploitability | Affected users | Discoverability | Score |
|---|---|---|---|---|---|---|---|
| 1 | Telemetry leak of prompts (default-on bug) | 8 | 10 | 4 | 10 | 6 | 7.6 |
| 2 | Auto-updater accepts compromised release | 9 | 5 | 6 | 9 | 4 | 6.6 |
| 3 | Unsigned Win/macOS installer triggers user to bypass | 6 | 9 | 7 | 8 | 8 | 7.6 |

### Mitigation priority

- **Threat 1** (telemetry leak): DEFAULT OFF + tcpdump zero-outbound test
  in local CI (v0.5.0). HARD NO-GO if any default-on telemetry path leaks.
- **Threat 3** (unsigned installer): SmartScreen "More info → Run anyway"
  path documented; v1.0 ships with self-signed + transparent disclosure
  in the README. EV cert is a v1.0 polish item (~$300-500/yr — user-paid
  blocker); npm postinstall path also bypasses SmartScreen MOTW since
  the binary is fetched programmatically (A4 PASS on Linux + Windows).
- **Threat 2** (compromised release): Sigstore + SLSA L3 by v1.0.0.

## Out of scope (declared)

- Multi-user concurrent access on a shared machine (single-user model)
- Remote attacker beyond loopback (no external network exposure by default)
- Compromised host OS (entire trust chain rests on the OS; user-machine
  disk encryption is the layer beneath)
- Multi-machine sync (post-v1.0; requires its own threat model addendum)
- Quantum-resistant signatures (Sigstore + SHA-256 are baseline; post-
  quantum migration is post-v1.0)
- Nation-state adversary with physical access
- Hypervisor escape / sidechannel attack on Qdrant or Ollama process

## References

- `SECURITY.md` — disclosure policy + supported versions
- `docs/ACCEPTANCE-TESTS.md` — release-gate spec
- `docs/results.tsv` — provenance + binary-contract proof rows
- A5 cosign verify-blob proof — Sigstore Linux artifact chain proven on
  clean Debian (commit history `A5`)
- F4 cryptographic provenance — commit `6a8d7f1`

## Review cadence

- Every minor version bump: full re-review with dedicated security review.
- Pre v1.0 tag: full re-review with `security-reviewer` + `cargo audit` +
  `cargo deny` + Scorecard + `cargo-geiger` output attached as release
  asset.
- Post v1.0: 90-day review cycle; CVE disclosure email + 90-day response
  policy per SECURITY.md.
