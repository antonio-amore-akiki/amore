<!-- stable: true -->
# ADR 0014 — Runtime feature flag resolver (Build, not Adopt)

## Status

Accepted 2026-05-26.

## Context

Per Meta Gatekeeper pattern (engineering.fb.com/2017/08/31/web/rapid-release-at-massive-scale/),
capability rollouts must be toggleable without code revert. Candidates surveyed:

- `flipt-client-rust` — requires Flipt server (SaaS-shaped, overkill)
- `unleash-client-rust` — same (Unleash SaaS)
- `featureflags-rs` v0.0.3 — pre-release; inventory macro dep; adds external dep churn

## Decision

Build a minimal in-tree resolver: compile-time Cargo features + runtime env/file.
Approximately 80 LoC in `crates/amore-core/src/flags.rs`.

## Rationale

- Minimum-change bias: no new external dep when 80 LoC suffices
- Single-author scope: no need for SaaS UI / audit trail (env + git history sufficient)
- Composable: env > file > compile-time precedence covers all rollout patterns
- Fail-closed: unknown flags default to `false`

## Consequences

- Future contributors learn 1 small in-tree module instead of vendor SDK
- No analytics on flag toggles (acceptable — local-first product)
- Migration to SaaS later possible without API break (`Flags::is_enabled()` is stable)

## Sources

- engineering.fb.com/2017/08/31/web/rapid-release-at-massive-scale/
- crates.io survey: flipt-client-rust, unleash-client-rust, featureflag v0.0.3 (2026-05-26)
