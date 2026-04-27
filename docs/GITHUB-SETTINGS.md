<!-- stable: true -->
# GitHub Repository Settings — Amore

## Branch protection (main)

Enable post-public-flip (currently private, no enforcement):
- Require pull request reviews before merge: 1 reviewer
- Require status checks: `cargo test`, `cargo audit`, `gitleaks`, `scorecard`
- Require signed commits
- Require linear history
- Block force push
- Block branch deletion
- Restrict who can push to main: @antonio-amore-akiki only

## Secrets

None required for v1.0 (release pipeline is local-only per "never use git actions" constraint). Post-flip if CodeQL activates: no secrets needed (uses GITHUB_TOKEN).

## Source
- docs.github.com/about/branch-protection-rules
- docs.github.com/about/code-owners
