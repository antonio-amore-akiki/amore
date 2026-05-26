# Contributing to Amore

Brief and action-first. Read this before opening a PR.

## Prerequisites

- Rust 1.95 or newer (`rustup install stable`)
- Node 20 or newer (only for `npm/` package work)
- `cargo` + `git`
- Optional: PowerShell 7+ for `scripts/*.ps1`
- Optional: Inno Setup 6.7+ for `installer/windows/` work
- Optional: `oha` (Rust HTTP load tester) for `tests/qa/h10_*` — install via `cargo install oha`

## First-time setup

```bash
git clone https://github.com/antonio-amore-akiki/amore.git
cd amore
cargo build --release --workspace
cargo test --release --workspace
```

## How to propose a change

1. Open an issue first with one of these subject prefixes:
   - `[bug]` broken behaviour
   - `[feat]` new functionality
   - `[security]` DO NOT use; see `SECURITY.md` for private disclosure
   - `[docs]` documentation only
   - `[refactor]` internal cleanup
2. Non-trivial changes (>50 LOC or cross-crate): wait for maintainer
   feedback before coding.
3. Open a PR targeting `main`. Rebase, do not merge.

## Commit message format

- Imperative subject ≤ 72 chars.
- Body wrapped at 72 cols.
- One commit per logical change. Squash before review.
- Security-relevant commits start with `security:` prefix.
- Co-authored commits include the `Co-Authored-By:` trailer.
- DCO sign-off encouraged today; required at v1.0.0:
  `git commit --signoff`.

Example:

```
security: pin Ollama installer SHA-256 + fail-closed on mismatch

Closes Critical 10a from docs/SECURITY-REVIEW-v0.3.0-live-fire.md.
Hasher is incremental over the download stream so we do not read the
857 MB file twice.
```

## Branch policy

- Trunk-based: every PR merges to `main` via rebase.
- Never force-push to `main`.
- Never use `--no-verify` or `--no-gpg-sign` without explicit maintainer
  approval in the PR body.
- Pre-commit hooks (`cargo fmt --check`, `cargo clippy`, `cargo test`)
  must pass locally before pushing.

## DCO and signed commits

Today: `git commit --signoff` (Developer Certificate of Origin) is
encouraged for every commit.

v1.0.0 onwards: signed commits become a hard gate via branch
protection. Configure either:

- `git config --global commit.gpgsign true` + GPG key on GitHub, or
- `git config --global gpg.format ssh` + SSH key registered as a
  signing key on GitHub.

## Test policy

Every code change ships with a test. Zero-regression bar.

- Unit: `#[cfg(test)] mod tests { ... }` inside the source file.
- Integration: `crates/<crate>/tests/<name>.rs`.
- Workspace binary-spawn: `crates/amore-integration-tests/tests/`.
- Property tests (`proptest`) and fuzz harnesses (`cargo-fuzz`) land
  for every parser in v0.4.0+.

Run before pushing:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --release --workspace
```

At v0.4.0+ clippy also enforces `-D clippy::unwrap_used` and
`-D clippy::expect_used` in production paths.

## Security

Do NOT open a public issue for security bugs — see `SECURITY.md`.
Maintainer responds within 5 business days for High / Critical
severity; fix lands within 30 days.

## Style

- `rustfmt` defaults — `cargo fmt` formats everything.
- Prefer `?` propagation over `unwrap()` / `expect()` in prod paths.
- Document every `unsafe { }` with a `// SAFETY:` comment.
- Public APIs get rustdoc; private fns get a one-liner if non-obvious.

## Documentation

Any product or behaviour change updates `CHANGELOG.md` in the
`[Unreleased]` section using keep-a-changelog format.

Architecture decisions taking more than a day of thinking get an ADR
in `docs/adr/` using the MADR 3.0 template (see existing ADRs).

## Maintainer + bus factor

Amore is single-maintainer (Antonio Amore Akiki). Reviews come from
Antonio. The project aims for at least one co-maintainer by v0.5.0.
Co-maintainership comes from consistently shipping high-quality PRs
across multiple releases and is offered by invitation.

## License

Contributions are licensed Apache-2.0 (same as the project). By
submitting a PR you assert you authored the code, or have explicit
permission to relicense it under Apache-2.0.

## Code of Conduct

The Contributor Covenant 2.1 applies. See `CODE_OF_CONDUCT.md`.

## Reproducible builds

`cargo build --release --locked` produces byte-identical output across
two runs on the same git sha. PRs that break that fail CI at v0.4.0+.

## Questions

Open an issue with the `[question]` prefix.
