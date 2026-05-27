<!-- stable: true -->
# RUSTSEC Triage — Amore v1.0.2

**Dated**: 2026-05-27
**Primary source**: `cargo audit` run against post-`cargo update` Cargo.lock
(advisory-db last-updated 2026-05-23T18:31:49-04:00, 1098 advisories, 769 lockfile deps)
**Verdict**: **PASS** — 0 `error:`-class vulnerabilities; 14 `warning:`-class advisories
(unmaintained/unsound; all transitive; none patchable via semver-compatible update)

> Predecessor: [RUSTSEC-TRIAGE-v1.0.0.md](./RUSTSEC-TRIAGE-v1.0.0.md) (6 warnings; 0 vulns)
> Delta: +8 advisories — GTK3 bindings cluster (atk, atk-sys, gdk, gdk-sys, gtk, gtk-sys,
> gtk3-macros, proc-macro-error) added; glib unsound advisory added. All are transitive via
> tray-icon → muda/libappindicator → gtk 0.18.2.

---

## Summary

| Category        | Count | Gate result |
|---|---|---|
| Vulnerabilities  | 0     | PASS        |
| Unmaintained    | 13    | accepted (transitive, no semver-compatible fix) |
| Unsound         | 1     | accepted with sunset (glib VariantStrIter; GUI-only path) |
| **Total warned** | **14** | PASS       |

`cargo update` applied 2026-05-27 (8 non-breaking patch bumps: displaydoc, hyper, libredox,
memchr, redox_syscall, toml_edit, zerocopy, zerocopy-derive). None of the advisory-triggering
crates are patchable via compatible update; they require major-version bumps blocked on
eframe/tray-icon ecosystem alignment.

---

## Advisory detail

### GTK3 bindings cluster (8 advisories — single root cause)

All 8 advisories share the same root: `tray-icon 0.24.0` pulls in `gtk 0.18.x` via
`muda 0.19.2` and `libappindicator 0.9.0`; `gtk 0.18.x` is the GTK3 binding crate family
that was superseded by GTK4 bindings (`gtk4`). These are Linux-only system-tray advisories;
the Windows/macOS builds do not use the GTK3 code path.

| Advisory              | Crate           | Version | Type         |
|---|---|---|---|
| RUSTSEC-2024-0413     | atk             | 0.18.2  | unmaintained |
| RUSTSEC-2024-0416     | atk-sys         | 0.18.2  | unmaintained |
| RUSTSEC-2024-0412     | gdk             | 0.18.2  | unmaintained |
| RUSTSEC-2024-0418     | gdk-sys         | 0.18.2  | unmaintained |
| RUSTSEC-2024-0415     | gtk             | 0.18.2  | unmaintained |
| RUSTSEC-2024-0420     | gtk-sys         | 0.18.2  | unmaintained |
| RUSTSEC-2024-0419     | gtk3-macros     | 0.18.2  | unmaintained |
| RUSTSEC-2024-0370     | proc-macro-error| 1.0.4   | unmaintained |

- **Fix-status**: transitive-no-fix — `tray-icon` has not yet released a version that drops
  GTK3 in favor of GTK4; upstream issue tracked at tray-icon/issues. Blocking fix:
  `tray-icon` must upgrade to GTK4 bindings or a non-GTK Linux tray backend.
- **Security impact**: no active vulnerability; crates are unmaintained, not exploitable.
  GTK3 bindings have no network attack surface; they are GUI glue compiled for Linux only.
- **Justification**: All transitive. Direct dependency chain:
  `amore-gui → tray-icon → muda/libappindicator → gtk 0.18.x → atk/gdk/gtk-sys/...`.
  Cannot be severed without replacing tray-icon entirely (v1.1 scope).
- **Sunset**: address in v1.1 — migrate from `tray-icon 0.24.x` to a GTK4-based or
  non-GTK system-tray implementation when the ecosystem aligns.

### RUSTSEC-2024-0429 — glib 0.18.5 (unsound)

- **Title**: Unsoundness in `Iterator` and `DoubleEndedIterator` impls for `glib::VariantStrIter`
- **Severity**: memory-safety class; no CVE; GHSA not assigned
- **Fix-status**: transitive-no-fix via semver-compatible update; fixed in `glib ≥ 0.19.x`
  (breaking API change; requires GTK4 migration of the whole binding family)
- **Justification**: Transitive via `tray-icon → gtk 0.18.2 → glib 0.18.5`. `VariantStrIter`
  is not exercised by Amore code; the affected iterator impl is internal to GTK3 variant
  iteration. GUI-only code path, not reachable from CLI or MCP server paths.
- **Sunset**: resolved as a side-effect of GTK3→GTK4 migration in v1.1.

### RUSTSEC-2025-0057 — fxhash 0.2.1 (unmaintained)

- Carried forward from v1.0.0 triage. Transitive via `sled → amore-core`.
- No fix available; v1.1 dependency audit pass.

### RUSTSEC-2024-0384 — instant 0.1.13 (unmaintained)

- Carried forward from v1.0.0 triage. Transitive via `sled → parking_lot 0.11.x`.
- No fix available; v1.1 dependency audit pass.

### RUSTSEC-2025-0119 — number_prefix 0.4.0 (unmaintained)

- Carried forward from v1.0.0 triage. Transitive via `tokenizers → indicatif`.
- No fix available; v1.1 dependency audit pass.

### RUSTSEC-2024-0436 — paste 1.0.15 (unmaintained)

- Carried forward from v1.0.0 triage. Transitive via `tokenizers → macro_rules_attribute`.
- No fix available; v1.1 dependency audit pass.

### RUSTSEC-2025-0134 — rustls-pemfile 2.2.0 (unmaintained)

- Carried forward from v1.0.0 triage. Transitive via `tonic → qdrant-client`.
- No fix available via compatible update; v1.1 dependency audit pass.

---

## Conclusion

Zero exploitable vulnerabilities. All 14 warned advisories are accepted with documented
justification and sunset dates. The GTK3 cluster (+8 vs v1.0.0) is a single-root transitive
issue blocked on `tray-icon` ecosystem alignment. Triage PASS is valid for v1.0.2.

Cargo.lock updated 2026-05-27 via `cargo update`: 8 compatible patch bumps applied;
advisory-triggering crates unchanged (major-version bump required, out of scope for v1.0.2).
