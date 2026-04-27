---
stable: true
topic: memory-ecosystem-supercharge-candidates + windows-code-signing-free-paths
date: 2026-05-27
research_basis: deep-research subagent a68d94c (Graphiti/LanceDB/Letta) + live signpath.io confirmation
---

# Memory ecosystem supercharge research + Windows code-signing free paths

Two research deliverables synthesized this session, with cited primary sources for each claim.

## Part A — Memory-backbone supercharge candidates (2026 ecosystem)

### Disambiguation: Graphify vs Graphiti vs Graphiti-the-other-thing

| Tool | Stars | Purpose | Amore relevance |
|---|---|---|---|
| **safishamsi/graphify** | 54.7k | One-shot codebase → knowledge-graph for AI coding assistants | Orthogonal — not a memory backbone; can be invoked via `/graphify` in same IDEs Amore wires |
| **getzep/graphiti** | 26.6k | Temporal knowledge-graph engine for agent memory | **Direct fit** — adds 3rd recall lane atop Amore's vector+BM25 |

User's "graphify, it is a navigation graph" colloquial naming → safishamsi/graphify is the most-likely match (54.7k stars, AI-coding-assistant skill). The deep-research surfaced getzep/graphiti as the actual memory-backbone tool worth integrating.

### Top supercharge candidates ranked

| # | Tool | License | Native Rust | Embedded | Verdict | Source |
|---|---|---|---|---|---|---|
| 1 | **LanceDB** | Apache-2.0 | ✅ `lancedb` crate | ✅ in-process | **Adopt (top-1)** | [docs.rs/lancedb](https://docs.rs/lancedb/latest/lancedb/) |
| 2 | **Graphiti** | Apache-2.0 | ❌ Python | ❌ FastAPI service | Adapt (3rd recall lane via REST) | [github.com/getzep/graphiti](https://github.com/getzep/graphiti) |
| 3 | LightRAG (HKUDS) | MIT | ❌ Python | ❌ | Reject (overlaps Graphiti+LanceDB) | [arxiv.org/html/2410.05779v1](https://arxiv.org/html/2410.05779v1) |
| 4 | Letta sleep-time | Apache-2.0 | ❌ Python | ❌ REST | Adapt-pattern only (idle-time consolidation behavior) | [letta.com/blog/sleep-time-compute](https://www.letta.com/blog/sleep-time-compute) |
| 5 | Mem0 | Apache-2.0 | ❌ Python | ❌ | Adapt (extraction algorithm pattern) | [github.com/mem0ai/mem0](https://github.com/mem0ai/mem0) |

### Recommended sequence

1. **LanceDB swap** (Karpathy subtraction — removes Qdrant daemon dep). ADR-0016 landed; full impl scheduled v1.2.
2. **Graphiti 3rd recall lane** (after LanceDB stable). Bi-temporal model is the only feature Amore's flat recall structurally cannot replicate. Benchmark: Graphiti hits 94.8% DMR + 18.5% LongMemEval improvement vs RAG baselines per [arxiv.org/abs/2501.13956](https://arxiv.org/abs/2501.13956).
3. Letta sleep-time pattern + Mem0 extraction pattern = absorb behaviors, not full framework adoption.

## Part B — Windows code-signing free paths (SmartScreen warning kill)

User's complaint 2026-05-27: `"amore-windows-x64.msi isn't commonly downloaded. Make sure you trust amore-windows-x64.msi before you open it"` — SmartScreen reputation warning on unsigned downloads.

### Free options evaluated

| Path | Cost | Eliminates SmartScreen | Status |
|---|---|---|---|
| **SignPath Foundation OSS** | $0 (free for Apache-2.0 GitHub OSS) | ✅ Authenticode cert | **Recommended**; app pre-fill at `docs/SIGNPATH-APPLICATION.md` |
| Sigstore cosign signing | $0 (free OIDC) | ❌ Windows doesn't trust Sigstore by default | Already in use for sha256sums.txt.sig but no SmartScreen benefit |
| Authenticode EV cert | $300-500/yr | ✅ (immediate, no reputation accumulation) | Excluded by user's "free unlimited only" constraint |
| Self-signed cert | $0 | ❌ Untrusted root → SmartScreen still warns | Rejected (no value) |
| Inno Setup `.exe` wrapper (no signing) | $0 | ❌ Wrapper inherits unsigned status | Shipping anyway for `.msi`-vs-`.exe` consumer preference (see release.yml windows-build job) |

### SignPath Foundation OSS — confirmed eligibility

Primary source: https://about.signpath.io/solutions/open-source-community fetched 2026-05-27 returns title "SignPath DevSec360 - The free Code Signing & Software Integrity solution for Open Source Projects".

**Eligibility**: open-source project on GitHub with OSI-approved license (Apache-2.0 qualifies). Amore meets criteria.

**Process**: user-action (Antonio applies via GitHub OAuth at signpath.io/foundation/apply; 3-5 business day review). Pre-fill answers ready at `docs/SIGNPATH-APPLICATION.md`.

**Once issued**: SignPath provides a GitHub Action snippet that signs `.exe` + `.msi` artifacts during the existing `release.yml windows-build` job. Single workflow-step addition. README first-launch-help drawer can drop the "click 'More info → Run anyway'" instruction.

## Part C — macOS Gatekeeper warning (NOT addressable free)

Apple Developer ID = only path to eliminate "macOS cannot verify the developer" warning. $99/yr recurring; excluded by user's free-only constraint. Status quo: README first-launch-help drawer documents the right-click → Open flow (one-time per binary).

Ad-hoc `codesign --sign -` on the binaries inside `.pkg` is a marginal improvement (Gatekeeper recognizes "signed but unidentified" over "completely unsigned") but does NOT eliminate the install-time `.pkg`/`.dmg` warning. Decision: skip ad-hoc codesign for v1.0.x; revisit if user buys Apple Dev ID at any future point.

## Decisions cross-ref

- ADR-0016: LanceDB adoption (`docs/adr/0016-lancedb-vector-store-adoption.md`)
- SignPath application pre-fill: `docs/SIGNPATH-APPLICATION.md`
- Existing decisions log: `state/pending-decisions.jsonl` rows for installer-macos-signing / installer-windows-signing / bundled-runtime-deps
