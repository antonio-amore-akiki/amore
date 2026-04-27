<!-- stable: true -->
# GDPR Article 25 Scoping Memo

## Material scope analysis (Articles 2–3)

Amore is a local-first, single-user binary that processes ONLY the operator's own data on the operator's own machine. The operator is simultaneously:
- The data subject (their own data)
- The "controller" (decides processing purposes)
- The "processor" (executes processing)

There is no offering of services to EU data subjects beyond the operator; no cross-border data transfer; no third-party data sharing. Therefore Amore as a software artifact is outside GDPR Article 2–3 material scope.

The operator is responsible for their own compliance if THEY process EU subjects' data using Amore — Amore as a tool does not create controller/processor relationships with downstream users.

## Documentation per Article 25 spirit (privacy-by-design)

Even outside material scope, Amore follows Article 25 design principles:

- **Data minimization**: stores only what the operator explicitly feeds in (no implicit collection)
- **Storage limitation**: operator controls retention via compaction policy (configurable TTL per namespace)
- **Restricted accessibility**: local-only by default; no network egress except (a) optional Ollama remote (operator-configured) and (b) optional cloud LLM API (operator-configured, API key in keyring)
- **Pseudonymization**: namespace IDs allow operator to compartmentalize data per project / per persona

## What Amore explicitly DOES NOT do

- No telemetry / analytics / crash reporting
- No model fine-tuning on user data
- No third-party data sharing
- No automatic cloud sync
- No mandatory authentication / no user account
- No update-server pings

## Operator responsibilities

If the operator processes EU subjects' personal data using Amore:
- Operator is the controller per GDPR Article 4(7)
- Operator must establish their own Records of Processing per Article 30
- Operator must implement appropriate technical + organizational measures per Article 32
- Amore provides: disk encryption-compatible storage paths, keyring-backed API keys, OS-level ACL on $AMORE_DATA_DIR

## Threat model link

See docs/THREAT-MODEL.md (Amore stolen-laptop threat model) — recommends OS-level disk encryption (BitLocker / LUKS / FileVault).

## Source
- gdpr-info.eu/art-25-gdpr/
- gdpr-info.eu/art-4-gdpr/ (definitions)
- gdpr-info.eu/art-30-gdpr/ (Records of Processing — operator responsibility)
- gdpr-info.eu/art-32-gdpr/ (Security of processing — operator responsibility)
