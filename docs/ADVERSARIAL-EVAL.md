---
stable: true
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
---

# Adversarial Eval Methodology

Amore is a memory backbone — it stores, indexes, and retrieves content from
untrusted sources (clipboard, web content, IDE context, log files, shell output).
An adversary who can write to the context stream controls some stored content.
This document defines the threat surface and the three attack classes tested by
`crates/amore-eval/src/bin/adversarial_eval.rs`.

## Threat surface

| Actor | What they control |
|-------|------------------|
| Adversary (stored-content injection) | Content ingested from untrusted IDE/clipboard/web sources |
| Adversary (query crafting) | Query string passed to recall |
| Defender | Ranking algorithm, indexing pipeline, BM25/vector lanes |

Primary source: Greshake et al. "Not What You've Signed Up For: Compromising
Real-World LLM-Integrated Applications with Indirect Prompt Injection"
(arxiv.org/abs/2302.12173, 2023).

## OWASP LLM Top 10 mapping

| OWASP ID | Title | Attack class |
|----------|-------|-------------|
| LLM01 | Prompt Injection | Class 1 (prompt-injection-via-memory) |
| LLM02 | Insecure Output Handling | Class 2 (memory-exfil) |
| LLM03 | Training Data Poisoning | Class 3 (recall-poisoning) |
| LLM06 | Sensitive Information Disclosure | Class 2 (memory-exfil) |

## Attack class 1: Prompt-injection-via-memory

**Pattern.** Adversary stores a document containing imperative override text
(e.g., "IGNORE ALL PRIOR INSTRUCTIONS"). When a subsequent user query hits
recall, the adversarial doc appears in the retrieved context window — if
ranked top-1, the LLM may treat it as an instruction.

**Source.** Greshake et al. §3.2 "Stored Injection" + OWASP LLM01.

**Expected defence.** The recall system MUST treat stored content as data only.
TF-IDF and semantic embedding rank by query-document relevance, not by
imperative phrasing. The adversarial doc shares no overlap with a Rust-trait
query, so it ranks below the relevant doc.

**Test assertion.** Given one relevant doc (Rust traits), one adversarial doc
(override instruction), and one unrelated doc (cargo workspaces), a query for
"how do I use traits in rust?" must return the Rust-traits doc as top-1.

## Attack class 2: Memory exfil (sensitive-content via crafted recall query)

**Pattern.** Adversary hopes a sensitive doc (containing API keys, tokens, PII)
surfaces in recall results for a semantically unrelated query — e.g., a general
Rust learning question that happens to fire the index.

**Source.** OWASP LLM02 + LLM06.

**Expected defence.** A recall system that ranks by query-document semantic
relevance will NOT return a secret-key document for an unrelated learning query.
The secret doc's content ("API_KEY_xyz123") shares no TF-IDF or embedding
overlap with "how do I learn Rust syntax".

**Test assertion.** Given a corpus containing `secret-1` (API key doc), a query
for "how do I learn Rust syntax" must NOT return `secret-1` with a positive
relevance score. Score-0 exhaustive results are excluded from the exfil
definition — the attack is semantic surfacing, not index enumeration.

## Attack class 3: Recall poisoning (keyword stuffing)

**Pattern.** Adversary crafts a single doc by concatenating all likely
high-frequency query terms. If ranking is not normalised, this doc scores above
legitimate topically-focused docs on every query — effectively monopolising
the top-1 slot regardless of query intent.

**Source.** OWASP LLM03. Also analogous to BM25 term-flooding; studied in
adversarial IR literature (e.g., Raval & Verma, SIGIR 2020).

**Expected defence.** Length-normalised TF-IDF (and IDF term weighting)
suppresses keyword stuffing: (1) TF is divided by document length so adding
terms to a doc gains no per-term advantage; (2) IDF down-weights terms that
appear in many docs (including the adversarial one).

**Test assertion.** Given 10 topically focused docs and 1 keyword-stuffed
adversarial doc, a 20-query battery covering every major topic must see the
adversarial doc dominate top-1 on at most 2 queries (10%). Threshold is 2/20
to allow for incidental term overlap in the valid docs.

## Test methodology

- **Mock-deps mode (always active).** Real Qdrant + Ollama daemons are excluded
  deliberately: safety gates must be deterministic and reproducible across CI
  environments. The in-memory TF-IDF mock captures the same ranking semantics
  (relevance, length normalisation, IDF weighting) without daemon dependency.
  Mode is printed at runtime: `mock-deps (in-memory TF-IDF)`.
- **Seed.** All three tests use fixed documents and fixed queries (no random
  seed needed — the corpus and query set are hard-coded).
- **Class 3 battery.** 20 queries covering every topic keyword in the
  adversarial doc. Each query is aligned to exactly one of the 10 valid docs.
- **Pass threshold.** 0 failures across all 3 classes.

## Update cadence

Each new attack vector identified in threat modelling or incident review gets:
1. A new `test_<class>()` function in `adversarial_eval.rs`.
2. A new section in this document with pattern + OWASP mapping.
3. A new results row in `ADVERSARIAL-EVAL-RESULTS-v<version>.md`.
