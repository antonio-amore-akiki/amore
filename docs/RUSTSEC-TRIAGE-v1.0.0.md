<!-- stable: true -->
# RUSTSEC Triage — Amore v1.0.0

**Dated**: 2026-05-27  
**Primary source**: `state/w8-cargo-audit-final.json` (db last-updated 2026-05-23T18:31:49-04:00,
advisory-count 1098, lockfile dependency-count 722)  
**Verdict**: **PASS** — 0 vulnerabilities; 6 warned-only advisories (5 unmaintained, 1 unsound)

> No predecessor RUSTSEC triage doc existed for this project; this is the initial triage record.

---

## Summary

| Category       | Count | Gate result |
|---|---|---|
| Vulnerabilities | 0 | PASS |
| Unmaintained   | 5 | accepted (transitive, no-fix available) |
| Unsound        | 1 | accepted with sunset (fix available in upstream) |
| **Total warned** | **6** | PASS |

---

## Advisory detail

### RUSTSEC-2025-0057 — fxhash 0.2.1 (unmaintained)

- **Title**: fxhash — no longer maintained
- **Severity**: informational (no CVE, no CVSS)
- **Fix-status**: transitive-no-fix — no patched version listed; crate abandoned
- **Justification**: Transitive dependency pulled in by indirect dep chain; not a direct
  dependency. Repository stale; no security impact — fxhash is a non-cryptographic hash.
  Upstream recommends `rustc-hash`; migration is a v1.1 candidate for dependency cleanup.
- **Sunset**: address in v1.1 dependency audit pass

### RUSTSEC-2024-0384 — instant 0.1.13 (unmaintained)

- **Title**: `instant` is unmaintained
- **Severity**: informational (no CVE, no CVSS)
- **Fix-status**: transitive-no-fix — no patched version listed; author recommends `web-time`
- **Justification**: Transitive dependency; not a direct dependency. No security impact —
  `instant` provides a cross-platform `Instant` abstraction. `web-time` migration is a
  v1.1 candidate.
- **Sunset**: address in v1.1 dependency audit pass

### RUSTSEC-2025-0119 — number_prefix 0.4.0 (unmaintained)

- **Title**: number_prefix crate is unmaintained
- **Severity**: informational (no CVE, no CVSS)
- **Fix-status**: transitive-no-fix — no patched version listed; alternative is `unit-prefix`
- **Justification**: Transitive dependency used for human-readable size formatting.
  No security impact. Migration is a v1.1 candidate.
- **Sunset**: address in v1.1 dependency audit pass

### RUSTSEC-2024-0436 — paste 1.0.15 (unmaintained)

- **Title**: paste — no longer maintained (repository archived by dtolnay)
- **Severity**: informational (no CVE, no CVSS)
- **Fix-status**: transitive-no-fix — no patched version; alternatives: `pastey`, `with_builtin_macros`
- **Justification**: Proc-macro for token concatenation; compile-time only, no runtime attack
  surface. No security impact. Migration is a v1.1 candidate.
- **Sunset**: address in v1.1 dependency audit pass

### RUSTSEC-2025-0134 — rustls-pemfile 2.2.0 (unmaintained)

- **Title**: rustls-pemfile is unmaintained (archived August 2025)
- **Severity**: informational (no CVE, no CVSS)
- **Fix-status**: transitive-no-fix — migrate to `rustls-pki-types` PemObject API (thin wrapper
  parity); no security vulnerability in the library itself
- **Justification**: PEM parsing utility; no active vulnerability. Upstream recommends migrating
  to `rustls-pki-types ≥ 1.9.0`. Migration is a v1.1 candidate.
- **Sunset**: address in v1.1 dependency audit pass

### RUSTSEC-2026-0002 — lru 0.12.5 (unsound)

- **Title**: `IterMut` violates Stacked Borrows by invalidating internal pointer
- **Severity**: memory-corruption class; CVSS not assigned; alias GHSA-rhfx-m35p-ff5j
- **Fix-status**: fix-available — patched in `lru ≥ 0.16.3`
- **Justification**: Soundness issue in `IterMut::next`/`next_back`; affects safe Rust code
  that iterates mutably over an LRU cache. In Amore, LRU cache iteration is read-only in
  hot paths; mutable iteration is not exercised in production. Nonetheless, upgrade to
  `lru ≥ 0.16.3` is a v1.0.x priority (breaking API change; requires caller audit).
- **Sunset**: upgrade to `lru ≥ 0.16.3` in v1.0.3 patch cycle; track in issue backlog

---

## Conclusion

Zero exploitable vulnerabilities. All 6 warned advisories are accepted with documented
justification and sunset dates. Triage PASS is valid for v1.0.0 release.
