# 12. Move audit/deny/geiger/fuzz/mutants to local Task Scheduler

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore's security baseline requires regular runs of:

* `cargo audit` — advisory database checks
* `cargo deny` — licence + duplicate + ban checks
* `cargo geiger` — unsafe code inventory
* `cargo fuzz` — coverage-guided fuzzing (hours-scale)
* `cargo mutants` — mutation testing (~30 min on the core crate)

The user mandate is "never use my git actions or credits". Free-tier
GitHub Actions provides 2000 minutes/month. The full security baseline
suite takes ~30-45 minutes per run. Running it on every push would burn
~30% of the monthly free-tier budget on security tooling alone, leaving
insufficient capacity for build + test CI.

How should the security baseline be scheduled without burning GHA credits?

## Decision Drivers

* User mandate: never use GHA minutes for audit/fuzz/mutants
* Free-tier GHA budget: ~2000 min/month; release-tag CI uses ~12 min/tag
* Cargo audit + deny + geiger: fast (~2-4 min); sensible to run nightly
* Cargo fuzz: hours-scale; never appropriate for per-push CI
* Cargo mutants: ~30 min on `amore-core`; too slow for per-push CI
* Windows-native scheduling: Task Scheduler is available on the dev
  machine (Windows 11 Pro); no Docker, no extra services required
* Results must be persisted to disk for review; not just stdout

## Considered Options

* GHA on every push (all security tools in CI matrix)
* GHA on release tag only (security tools gate the release)
* Local Windows Task Scheduler nightly cron
* No automated security audit (manual only)

## Decision Outcome

Chosen option: **local Windows Task Scheduler nightly cron
(`scripts/security-baseline.ps1`); release-tag GHA only (~12 min/tag)**.

`scripts/security-baseline.ps1` runs nightly at 02:00 local time via a
Task Scheduler entry created by `scripts/install-task.ps1`. It runs in
order:

1. `cargo audit` — fail if any RUSTSEC advisory matches
2. `cargo deny check` — fail on licence violations or banned crates
3. `cargo geiger --all-features` — emit unsafe counts; fail if new
   unsafe blocks appeared since last baseline snapshot
4. `cargo mutants -p amore-core --timeout 120` — mutation score;
   fail if score drops below 70%
5. `cargo fuzz run fuzz_recall -- -max_total_time=3600` — 1-hour fuzz;
   report new crashes to `fuzz/artifacts/`

Results are written to `state/security-baseline-{date}.json` (gitignored;
tier-2 operational state). A summary line is appended to
`test logs` (append-only; proof row per run).

Release-tag GHA (`.github/workflows/release.yml`) runs only:
`cargo audit` + `cargo deny check` + `cargo test --release` (~12 min).
Fuzz and mutants stay local-only.

This pattern landed in v0.3.1 (Phase G entry); it is the accepted
ongoing policy.

### Consequences

* Good: zero GHA minutes consumed by security baseline
* Good: release-tag GHA stays lean (~12 min); free-tier headroom
  preserved for build + test matrix
* Good: nightly fuzz has 1 hour of coverage per night; much deeper than
  any per-push run
* Good: results persist to disk; can be reviewed the next morning
* Bad: security baseline runs only when the dev machine is powered on
  and not in sleep; missed nights are possible
* Bad: results are not visible in the GitHub PR interface
* Bad: a new contributor cloning the repo will not have the Task
  Scheduler entry; they must run `scripts/install-task.ps1` manually

## Pros and Cons of the Options

### GHA on every push

* Good: visible in PR; blocks merge on failure
* Bad: ~30-45 min/run × N pushes/day burns free-tier budget in days
* Bad: violates user mandate "never use my git actions or credits"
* Bad: fuzz/mutants are incompatible with per-push timing

### GHA on release tag only (CHOSEN partial)

* Good: gates every release; high-confidence before ship
* Good: only ~12 min; fits free-tier budget per tag
* Bad: security regressions can accumulate between releases
* Note: this is kept as the release gate alongside the local schedule

### Local Windows Task Scheduler nightly (CHOSEN primary)

* Good: zero GHA credit consumption
* Good: fuzz gets 1 hour/night of coverage
* Good: runs on real dev-machine hardware (no runner spin-up latency)
* Bad: machine must be on; missed nights possible
* Bad: not visible in GitHub PR interface

### No automated security audit

* Good: zero setup cost
* Bad: RUSTSEC advisories accumulate undetected
* Bad: unsafe code surface grows silently
* Bad: completely fails the security baseline mandate

## More Information

* Task Scheduler installer: `scripts/install-task.ps1`
* Security baseline script: `scripts/security-baseline.ps1`
* Results sink: `test logs` (append-only; enforced by
  `results-tsv-append-only` rule in CLAUDE.md governance)
* State dir: `state/` (gitignored; tier-2 operational)
* GHA release workflow: `.github/workflows/release.yml`
* Landed in v0.3.1 (Phase G); ongoing accepted policy
