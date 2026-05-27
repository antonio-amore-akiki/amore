<!-- stable: true -->
# Mutation Baseline — Amore v1.0.2

**Date:** 2026-05-27
**Tool:** cargo-mutants 27.0.0
**Commit:** 61b992bfd9aaaf47fff977c933e2fef7c051390a
**Bigtech-grade threshold:** ≥ 60% mutants caught

---

## Summary

| File | Mutants identified | Mutants tested | Caught | Missed | Score | Threshold |
|---|---|---|---|---|---|---|
| crates/amore-core/src/wal.rs | 46 | 2 (60s cap) | 1 | 1 | **50% (partial)** | ≥ 60% |
| crates/amore-core/src/docs.rs | N/A | 0 | 0 | 0 | **BLOCKED** | ≥ 60% |

**Overall verdict: BELOW THRESHOLD (partial measurement, 60s cap).**

---

## wal.rs — Partial Measurement

**Mutant genres identified (46 total):**

| Genre | Count |
|---|---|
| FnValue | 30 |
| BinaryOperator | 10 |
| MatchArmGuard | 4 |
| UnaryOperator | 2 |
| **Total** | **46** |

**Tested in 60s window (2 mutants):**

| Mutant | Location | Replacement | Result |
|---|---|---|---|
| replace `*` with `/` | wal.rs:46:45 | `/` | **CAUGHT** |
| replace `*` with `+` | wal.rs:46:45 | `+` | **MISSED** |

**Partial score: 1/2 = 50%.** The 60s window allowed only 2 of 46 mutants to be tested.
Each mutant requires ~17s (compile + test run). Full run requires ~46 × 17s ≈ 13 minutes.

**Test suite coverage for wal.rs:** 8/8 tests pass (`wal::tests::*`), covering:
- `fp1_first_open_creates_fingerprint_file`
- `fp2_subsequent_open_matching_key_succeeds`
- `fp3_mismatched_key_returns_fingerprint_mismatch_error`
- `fp4_missing_keyring_entry_when_fingerprint_exists_returns_error`
- `test_roundtrip_tag_verifies`
- `test_payload_too_large`
- `test_corrupted_tag_skipped`
- `test_legacy_record_without_tag_accepted`

---

## docs.rs — Blocked

**Reason:** 5 of 9 tests in `docs::tests` fail in the unmutated tree at HEAD 61b992b.
cargo-mutants requires a clean baseline before running mutations.

**Failing tests (pre-existing regression, not introduced by this session):**
- `docs::tests::router_can_relax_stable_requirement_for_debug`
- `docs::tests::router_finds_stable_doc_by_keyword`
- `docs::tests::router_matches_body_keywords_when_title_filename_topic_miss`
- `docs::tests::router_ranks_better_matches_higher`
- `docs::tests::router_uses_topic_header_line_for_matching`

**Action required:** Fix the 5 failing docs::tests before re-running mutation baseline on docs.rs.
This is a separate issue from this measurement run.

---

## Prior Baseline

No prior `MUTATION-BASELINE-v*.md` exists. This is the first measurement.

---

## How to Complete the Full Measurement

```sh
# 1. Fix 5 failing docs::tests (separate issue)
# 2. Run full mutation baseline (no 60s cap needed)
cargo mutants --no-shuffle --timeout 300 \
  --output state/mutants-v1.0.2 \
  --package amore-core \
  --file crates/amore-core/src/wal.rs \
  --file crates/amore-core/src/docs.rs \
  -- wal::tests docs::tests

# Expected duration: ~15–30 minutes for both files
# Expected total mutants: ~120 (46 wal + ~74 docs estimated)
```

---

## Methodology

```sh
cargo mutants --no-shuffle --timeout 60 \
  --output state/mutants-v1.0.2 \
  --package amore-core \
  --file crates/amore-core/src/wal.rs \
  -- wal::tests
```

Run was capped at 60s per task specification. Partial output in `state/mutants-v1.0.2/mutants.out/`.
cargo-mutants version: 27.0.0. Copy mode (not --in-place) to avoid leaving mutations in source.
