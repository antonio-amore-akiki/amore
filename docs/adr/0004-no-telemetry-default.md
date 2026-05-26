# 4. Telemetry is opt-in, never default-on

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore handles user memory: prompts, document excerpts, agent
observations. The user mandate is "privacy by default — no telemetry,
no phone-home, no analytics". The threat model is stolen-laptop only —
any outbound network call by default would create surprise data egress
that violates the trust contract with non-technical users.

## Decision Drivers

* User mandate "no telemetry by default"
* Threat model: stolen-laptop only
* Non-technical user trust: zero surprise outbound traffic
* CLAUDE.md hard gate "never use silent fail-open paths"
* Auditability: every network call must be traceable to user-explicit
  configuration

## Considered Options

* Telemetry on by default with opt-out
* Telemetry off by default with opt-in
* No telemetry at any time

## Decision Outcome

Chosen option: **telemetry OFF by default; opt-in via
`AMORE_TELEMETRY=on`**.

When opt-in is true, the payload is bounded:
- Amore version
- Anonymous run-id (generated per install, persisted in user config)
- Degraded-lane flags (which dep was unreachable, no content)
- No prompt content, no document content, no user paths

A tcpdump test in CI asserts ZERO outbound network traffic when
telemetry is off and only the user-invoked Ollama / Qdrant calls fire.

### Consequences

* Good: default install behaves exactly as the user expects — local-only
* Good: power users can opt in to support diagnostic work
* Good: clear audit trail — single env var controls the entire
  outbound-network gate
* Good: cumulative trust accrues over time as users discover that
  Amore really does what it says
* Bad: maintainers receive less crash data than they would with
  opt-out telemetry
* Bad: power-user adoption of opt-in telemetry historically <5%; we
  largely fly blind on production crashes

## Pros and Cons of the Options

### Telemetry on by default with opt-out

* Good: maintainer gets the data needed to improve product
* Bad: violates user mandate explicitly
* Bad: non-technical users do not know to opt out
* Bad: trains the user community to treat "off by default" claims as
  marketing
* Bad: legal risk under GDPR Article 6 (no lawful basis without consent)

### Telemetry off by default with opt-in (CHOSEN)

* Good: respects user mandate
* Good: GDPR Article 6 consent is explicit
* Good: tcpdump CI test makes the claim falsifiable
* Bad: maintainer crash visibility is limited

### No telemetry at any time

* Good: simplest privacy posture
* Bad: leaves zero path for power users to share diagnostic data
* Bad: makes performance regression hunts harder
* Bad: forecloses on the option later when opt-in is the natural
  middle ground

## Implementation

The opt-in path lands in v0.5.0 per the Phase G roadmap:

```rust
// crates/amore-core/src/telemetry.rs (NEW in v0.5.0)
pub fn is_enabled() -> bool {
    std::env::var("AMORE_TELEMETRY")
        .map(|v| v.eq_ignore_ascii_case("on"))
        .unwrap_or(false)
}
```

A CI test asserts the tcpdump capture for the default-off path is
empty across 30 seconds of `amore recall` plus `amore status` plus
`amore-gui --no-gui`.

## More Information

* See `SECURITY.md` for the privacy posture summary
* See `docs/THREAT-MODEL.md` for the stolen-laptop threat model
* The opt-in path matches Mozilla's pre-Firefox-Quantum opt-in
  telemetry model; we follow that precedent
* Crash reports (`AMORE_CRASH_REPORTS_DIR`) are likewise opt-in and
  write to local XDG cache only; no network
