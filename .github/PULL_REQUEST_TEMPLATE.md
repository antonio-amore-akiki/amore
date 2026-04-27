## Summary
<1-2 sentences: what + why>

## Motivation
<Why is this change needed? Link to issue if applicable.>

## Proof of Behavior
- [ ] Tests added/updated (cite test file)
- [ ] `cargo test --workspace --release` green
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- [ ] `cargo fmt --check` clean
- [ ] `cargo audit` green or justified-ignored

## Regressions Verified
<How was behavior preservation verified for affected modules?>

## Checklist
- [ ] CHANGELOG.md updated
- [ ] Docs updated (README, ADRs, etc.)
- [ ] Backward-compatible OR migration path documented in UPGRADING.md
- [ ] No new direct deps (or justified in commit message)
- [ ] No `--no-verify` / `--no-gpg-sign` used
