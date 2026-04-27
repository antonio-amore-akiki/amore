# Amore SAST + Secret Scanning
<!-- stable: true -->

## Overview
- **Secret scanning**: gitleaks via `.gitleaks.toml` + local pre-commit hook (`.githooks/pre-commit`) + workflow_dispatch CI (`.github/workflows/gitleaks.yml`)
- **SAST**: semgrep via `.semgrep.yml` (rust + custom unwrap-blocker) + workflow_dispatch CI (`.github/workflows/semgrep.yml`) + CodeQL workflow_dispatch (`.github/workflows/codeql.yml`)

## Local invocation

```bash
# Secret scan staged diff (auto via pre-commit hook if `git config core.hooksPath .githooks`)
gitleaks protect --staged --config .gitleaks.toml

# Full repo scan
gitleaks detect --source . --config .gitleaks.toml

# Semgrep local scan
semgrep --config p/rust --config .semgrep.yml --error
```

## Activation
Workflows are `workflow_dispatch:` only to honor "never use GHA credits" constraint.
Post-public-flip: change to `push: { branches: [main] }` triggers.

## Source
- github.com/gitleaks/gitleaks
- semgrep.dev/r/rust.lang.security
- github.com/github/codeql-action
- OSSF Scorecard SAST check: github.com/ossf/scorecard/blob/main/docs/checks.md#sast
