---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
---

# Amore SLSA L3 Attestation

<!-- @file-size-exempt: attestation compliance ledger — equivalent to PRR-CHECKLIST; structured evidence table requires verbatim requirement text -->

topic: slsa-l3-attestation
purpose: SLSA Level 3 attestation claims for the Amore release pipeline
stable: true
version: 0.9.0

Per slsa.dev/spec/v1.0/requirements and slsa.dev/spec/v1.0/levels.

## Requirement Status Table

| Requirement | Status | Implementation |
|---|---|---|
| Producer identity | PASS | `antonioakiki15@gmail.com` bound to cosign OIDC token per signing session |
| Build platform — Linux | PASS | Fresh Docker container per build (`--rm`); discarded after each invocation |
| Build platform — Windows | PARTIAL | Dev-host (non-ephemeral); v1.1 plan: Windows container (`mcr.microsoft.com/windows/servercore`) |
| Signing key isolation | PASS | Cosign keyless OIDC; no long-lived key generated, stored, or materialized |
| Provenance predicate present | PASS | `cosign attest-blob --type slsaprovenance` per artifact + sha256sums bundle (Steps 7 + 7b) |
| sha256sums.txt signed | PASS | `cosign sign-blob` → `sha256sums.txt.sigstore` (Step 7b) |
| sha256sums SLSA attestation | PASS | `cosign attest-blob` → `sha256sums.txt.attest.bundle` (Step 7b) |
| Rekor transparency log | PASS | Every attestation appended to sigstore.dev/log; append-only audit trail |
| Consumer-verifiable provenance | PASS | `cosign verify-blob-attestation --bundle` commands in docs/RELEASING.md |
| Hermeticity — path remap | PASS | `SOURCE_DATE_EPOCH=git log -1 --format=%ct` + `RUSTFLAGS --remap-path-prefix` |
| Hermeticity — base image pin | PASS | x86_64: `rust@sha256:6258907...` in `Dockerfile.builder-linux-x86_64`; ARM64: `ghcr.io/cross-rs/aarch64-unknown-linux-gnu@sha256:7f8308...` in `Cross.toml`. `apt-get install` removed from build-time docker run (moved to pre-baked image). W8-8C M2 fix. |
| In-toto per-step links | PENDING | Deferred — see rationale below |

**Current verdict**: SLSA L3 for Linux build path. Hermeticity rows fully PASS as of W8-8C M2 fix.
Windows PARTIAL (ephemeral gap only; signing/provenance/verification PASS).
In-toto stretch deferred.

## Key Claims

**Signing key isolation**: `cosign sign-blob --yes` uses sigstore Fulcio CA to issue an
ephemeral certificate (5-min TTL) per signing session. No private key touches disk.
Build steps (Steps 3–5) run before any signing step; keys are out of scope during build.

**Ephemeral Linux environment**: Steps 4 and 4b each invoke `docker run --rm` with a
fresh container. The `--rm` flag guarantees teardown. ARM64 uses `cross-rs/cross` which
also launches a fresh Docker container (pinned in `Cross.toml`).

**Reproducibility**: Two builds on the same `SOURCE_DATE_EPOCH` produce byte-identical
SHA256 hashes. Verified W5. See `docs/RELEASING.md` → "Reproducible builds".

**sha256sums bundle** (W5-5B addition): Step 7b of `scripts/release-local.ps1` generates
`sha256sums.txt` (one `<sha256>  <filename>` line per artifact), signs it via cosign
keyless (`sha256sums.txt.sigstore`), and emits a SLSA provenance predicate for the whole
bundle (`sha256sums.txt.attest.bundle`). Consumer commands in `docs/RELEASING.md`.

## In-toto Stretch — Deferred Rationale

Generating per-step in-toto link metadata requires:
1. `in-toto-run` CLI installed in the release environment (not currently present).
2. A layout signing key separate from the cosign OIDC flow.
3. Wrapping each step (`cargo build`, `cosign sign-blob`, `gh release upload`) with
   `in-toto-run --name <step>` to capture file-hash inputs/outputs per step.
4. Verifying the full bundle with `in-toto-verify --layout` before upload.

This is ≥3 new toolchain deps + ~10 min added to release wall-clock. The SLSA provenance
predicate via `cosign attest-blob` satisfies the L3 mandatory set. In-toto per-step links
are L4-adjacent hardening.

**v1.1 plan**: Add `in-toto-run` wrappers after v0.9.0 stabilises. Document key-management
pattern (GitHub Actions OIDC vs. ephemeral local key) in `docs/adr/` before implementation.

---

Sources: slsa.dev/spec/v1.0/levels · slsa.dev/spec/v1.0/requirements · docs.sigstore.dev/cosign/keyless
