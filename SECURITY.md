# Security Policy

## Supported versions

| Version       | Supported          |
| ------------- | ------------------ |
| v0.3.x        | ✅ (current)       |
| v0.2.x        | ⚠️  bug-fix only   |
| < v0.2        | ❌                 |

We support the **two most recent minor versions** with security patches once
v1.0.0 ships. Until then, only the latest tagged release receives fixes.

## Reporting a vulnerability

Please **do not open a public GitHub issue** for security bugs.

Email: `security@amore.dev` (PGP key fingerprint will be published at v1.0.0).

Until that mailbox is live, please send disclosure reports to the maintainer:
`antonioakiki15@gmail.com` with subject prefix `[AMORE-SECURITY]`.

We aim to respond within **5 business days** and ship a fix within
**30 days** for High / Critical severity, **90 days** for Medium / Low.

A security advisory + CVE (if applicable) will be published on the GitHub
Security tab once a patched release is available.

## Threat model

Amore is a **single-user single-machine memory backbone** by default.
Its security posture is calibrated to that.

### In scope

- **Stolen-laptop** — the laptop is lost / stolen with the encrypted disk
  unlocked. The attacker has wall-time access to whatever Amore wrote to
  disk. Mitigations:
  - Amore data lives in the OS-native user-config dir (`%APPDATA%\Amore` on
    Windows; `~/.config/amore` on Linux; `~/Library/Application Support/Amore`
    on macOS) — covered by the user's full-disk-encryption.
  - No plaintext secrets are stored by Amore itself. The only secret-class
    data is the optional cloud-LLM API key the user types into the GUI,
    which is held in the OS keyring via the `keyring` crate (planned v0.5.0).
- **Compromised IDE plugin attempting to exfiltrate memory** — an attacker
  installs a malicious IDE extension that connects to the local MCP server.
  Mitigations:
  - MCP server binds to **stdio only by default** — no network listener.
  - Optional gRPC mode (v0.7.0) binds to a **Unix socket / Windows named
    pipe** by default; TCP+TLS is an explicit opt-in via env var.
  - Recall results are sourced from local SQLite + Qdrant only; the server
    never contacts third-party services without an explicit user
    configuration change.
- **Local privilege-escalation via installer** — a malicious actor swaps the
  installer .exe before the user runs it. Mitigations:
  - GitHub Release assets include SHA-256 checksums in the release notes.
  - Sigstore keyless signing is applied to the Linux artifact and planned
    for macOS + Windows (v0.5.0+).
  - The Windows installer writes only to per-user paths and does not
    request UAC elevation.

### Out of scope

- **Nation-state adversary with physical access** — beyond the design.
- **Compromised OS kernel / hypervisor escape** — beyond the design.
- **Side-channel attacks on the embedded Qdrant / Ollama processes** —
  beyond the design.
- **Multi-tenant deployment** — Amore is designed for **single-user**
  deployment per host. The cluster-mode docker-compose (v0.7.0) is intended
  for power users running multi-machine self-host, not for multi-tenant
  service-provider deployment.
- **Insider threat at any third-party AI provider** — if you opt into a
  cloud-LLM provider (OpenAI / Anthropic / etc.) the data you send leaves
  your machine. This is the user's explicit choice; the GUI labels the
  toggle clearly.

## Security posture (production)

The following security gates **MUST** be green before any release tag:

- [x] **No secrets in repo** — `gitleaks` scan clean. Pre-commit hook enforces.
- [ ] **Supply chain**: `cargo audit` advisory-clean; `cargo deny` license + ban + source clean. *(v0.4.0)*
- [ ] **Unsafe Rust**: every `unsafe {}` block has a documented safety
      justification; `clippy::undocumented_unsafe_blocks = "deny"`. *(v0.4.0)*
- [ ] **No-unwrap policy**: `clippy::unwrap_used = "deny"` +
      `clippy::expect_used = "deny"` in production paths. *(v0.4.0)*
- [ ] **Input validation**: every MCP tool input + CLI arg + file path +
      network URL bounded by an `InputBound` enum with proven coverage. *(v0.5.0)*
- [ ] **TLS enforcement**: all `reqwest` clients explicitly require
      `rustls-tls`; no `accept_invalid_certs` overrides. *(v0.4.0)*
- [ ] **Default-off telemetry**: zero outbound network traffic when
      `AMORE_TELEMETRY != "on"`. Asserted by `tcpdump` snapshot in CI. *(v0.5.0)*
- [ ] **Atomic-write semantics**: every config-write path uses `.tmp +
      rename` with a `.bak` sibling. Asserted by integration test. *(v0.4.0)*
- [ ] **Process spawning**: `windowsHide: true` set on every `Command`;
      no shell-interpolation paths. *(v0.4.0)*
- [ ] **Cryptographic provenance**: SHA-256 + canonical-JSON chain on
      every observation; `verify_chain` rejects tampered payloads. *(v0.3.0
      done; coverage tests v0.4.0)*
- [ ] **SLSA L3 provenance** on every release artifact. *(v1.0.0)*
- [ ] **OSSF Scorecard ≥ 8.0** weekly cron. *(v0.9.0)*
- [ ] **Security-reviewer subagent verdict = GO** on every release candidate. *(every tag)*

The current security posture against this checklist is tracked in
`docs/SECURITY-REVIEW-<version>.md`.

## Threat model file

The detailed STRIDE / DREAD threat model lives in `docs/THREAT-MODEL.md`
(landing in v0.4.0). The shorter summary above is the security-policy
view; the detailed model is the architecture view.

## Acknowledgements

We thank the security researchers who have responsibly disclosed
vulnerabilities. Acknowledgements list will be published at v1.0.0.
