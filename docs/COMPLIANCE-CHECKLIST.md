stable: true

# Amore Compliance Checklist

Maps each compliance control to the artifact path that satisfies it.

| Control | Artifact | Source |
|---|---|---|
| SLSA Build L3 (ephemeral CI + signed provenance) | docs/SLSA-L3-ATTESTATION.md + dist/*.sigstore | slsa.dev/spec/v1.0/levels |
| NIST SP 800-218 SSDF PS.3.2 (provenance per component) | sbom.cdx.json + cosign attest-blob bundles | csrc.nist.gov/Projects/ssdf |
| OSSF Scorecard ≥ 7.5 | docs/SCORECARD-baseline.md + .github/workflows/scorecard.yml | github.com/ossf/scorecard |
| Fuzzing | crates/amore-core/fuzz/ + docs/FUZZING.md | rust-fuzz.github.io/book |
| SAST (CodeQL) | .github/workflows/codeql.yml + docs/SAST.md | github.com/github/codeql-action |
| Secret scanning (gitleaks) | .gitleaks.toml + .githooks/pre-commit + docs/SAST.md | github.com/gitleaks/gitleaks |
| Mutation testing baseline | docs/MUTATION-BASELINE-v0.8.0.md | github.com/sourcefrog/cargo-mutants |
| Adversarial eval (ML) | crates/amore-eval/src/bin/adversarial_eval.rs + docs/ADVERSARIAL-EVAL-RESULTS-v0.8.0.md | owasp.org/www-project-top-10-for-large-language-model-applications |
| CycloneDX SBOM | sbom.cdx.json (composition.aggregate=complete) | cyclonedx.org/specification/overview |
| STRIDE/DREAD threat model | docs/THREAT-MODEL.md | (in-house) |
| Reproducible builds | docs/RELEASING.md reproducible section + SOURCE_DATE_EPOCH | reproducible-builds.org |
| Secret hygiene (keyring) | docs/SECRETS.md + crates/amore-cli/src/secrets.rs | docs.rs/keyring/3.x |
| SLO + Error Budget | docs/SLO.md + docs/ERROR-BUDGET-POLICY.md + docs/SLI-DEFINITIONS.md | sre.google/sre-book/service-level-objectives |
| PRR Gate | docs/PRR-CHECKLIST-v1.0.0.md | sre.google/workbook/engagement-model |
| Blameless postmortem | docs/POSTMORTEM-TEMPLATE.md | sre.google/sre-book/example-postmortem |
| Feature flags (Meta Gatekeeper pattern) | docs/FEATURE-FLAGS.md + crates/amore-core/src/flags.rs | engineering.fb.com/2017/08/31/web/rapid-release-at-massive-scale |
| Canary release (3-stage) | docs/CANARY-RUNBOOK-v0.5.1.md | sre.google/workbook/canarying-releases |
| OpenTelemetry 3-signal | crates/amore-mcp/src/observability/ + Cargo.toml OTel deps | opentelemetry.io/docs/specs/otel |
| GDPR Article 25 (scoping) | docs/GDPR-SCOPING.md | gdpr-info.eu/art-25-gdpr |
| WCAG 2.2 AA + MSAA accessibility | docs/ACCESSIBILITY-STATEMENT.md | w3.org/TR/WCAG22 |
| Anthropic System Card pattern | docs/SYSTEM-CARD-reranker-v0.5.0.md | anthropic.com/rsp |

## Status

All artifacts listed exist in repo as of v1.0.0 (W9). Pre-release artifacts in progress: see git log.
