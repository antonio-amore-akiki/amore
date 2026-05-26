# 10. Deny clippy::unwrap_used in production paths

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore is shipped to non-technical users via a one-click installer.
A panic in a production code path surfaces as a cryptic crash dialog
or silent process exit with no user-facing explanation. The user mandate
requires plain-English errors at every failure point.

`unwrap()` and `expect()` on `Option` or `Result` values in production
code are guaranteed panic paths. CLAUDE.md hard gate "no silent
fail-open" applies: every failure must be named, propagated, and
surfaced in human-readable form.

Should Rust's `unwrap`/`expect` calls be allowed in production paths?

## Decision Drivers

* Panic is the antithesis of the non-technical-user UX requirement
* CLAUDE.md hard gate: "no silent fail-open" applies to panics
* `anyhow::Result` + `?` propagation is the Rust-idiomatic alternative
* Clippy lint enforcement is machine-checkable (CI gate, not convention)
* Tests and benchmarks legitimately use `unwrap` (no production impact)
* `expect` in production is only marginally better: it panics with a
  message that is not surfaced to the end user

## Considered Options

* Keep `unwrap` allowed (no lint; convention only)
* Deny via clippy lint (`clippy::unwrap_used` = "deny")
* Deny via lint + explicit CI gate (`cargo clippy --deny warnings`)

## Decision Outcome

Chosen option: **`clippy::unwrap_used = "deny"` and
`clippy::expect_used = "deny"` in production paths; tests and benches
are explicitly exempted**.

Configuration in `Cargo.toml` workspace root:

```toml
[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
```

And in each crate's `lib.rs` / `main.rs` test module block:

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests { ... }
```

Bench files under `benches/` carry the same `#[allow(...)]` at file
level.

The lint is enforced in CI via `cargo clippy --all-targets
--all-features -- -D warnings` (already present in
`scripts/security-baseline.ps1`).

This decision is scheduled for enforcement at v0.4.0 (Phase G entry).
Existing violations are tracked in `docs/unwrap-audit.md` and will be
remediated in the v0.4.0 milestone.

### Consequences

* Good: every failure path in production code is named and propagated
* Good: CI gate is machine-checkable; no convention drift
* Good: anyhow + thiserror + `?` propagation flows naturally to the
  MCP tool layer where errors surface as plain-English MCP error
  responses
* Good: reduces the class of "Amore exited unexpectedly" user support
  tickets to near-zero for logic errors (panics become `Err` returns)
* Bad: remediation of existing unwrap calls requires audit pass in v0.4.0
* Bad: test code is slightly more verbose when `unwrap()` is the clearest
  assertion form (mitigated by the blanket `#[allow]` in test modules)

## Pros and Cons of the Options

### Keep unwrap allowed (convention only)

* Good: no migration cost; tests and production have same rules
* Bad: convention drift inevitable over time; contributors add `unwrap`
  under time pressure
* Bad: not machine-checkable; code review cannot catch every instance
* Bad: panics surface as cryptic crashes to non-technical users

### Deny via clippy lint (CHOSEN)

* Good: machine-enforced; CI catches every new violation
* Good: clear rule: unwrap is banned in production, allowed in tests
* Good: gradual remediation possible — fix existing violations before
  v0.4.0, then enforce
* Bad: requires audit pass of existing codebase before enforcement

### Deny via lint + CI gate (superset of CHOSEN)

This is the same as the chosen option. `cargo clippy -- -D warnings`
in CI is the gate. The "lint + CI gate" framing was a redundant option
collapsed into the chosen path.

## More Information

* Unwrap audit tracker: `docs/unwrap-audit.md` (Phase G)
* Recommended replacement pattern:

```rust
// Instead of:
let val = map.get("key").unwrap();

// Use:
let val = map.get("key")
    .ok_or_else(|| anyhow::anyhow!("key not found in map"))?;
```

* `thiserror` is used for library error types (`crates/amore-core/`);
  `anyhow` is used for application-layer error propagation
  (`crates/amore-daemon/`, `crates/amore-mcp/`)
* See CLAUDE.md: "no silent fail-open (log the path)" — panics violate
  this on both counts (silent from the user's perspective + no log)
* Scheduled enforcement: v0.4.0 (Phase G entry)
