<!-- stable: true -->
# Mutation Baseline — Amore v1.0.2

**Date:** 2026-05-27
**Tool:** cargo-mutants 27.0.0
**Commit:** 5f6c336 (HEAD)
**Bigtech-grade threshold:** ≥ 60% caught

---

## Summary

| File | Mutants identified | Tested (excl. unviable) | Caught | Missed | Unviable | Score | Threshold |
|---|---|---|---|---|---|---|---|
| crates/amore-core/src/docs.rs | 36 | 34 | 21 | 13 | 2 | **61.8%** | ≥ 60% |
| crates/amore-core/src/wal.rs | 46 | 40 | 23 | 17 | 6 | **57.5%** | ≥ 60% |
| **Combined** | **82** | **74** | **44** | **30** | **8** | **59.5%** | ≥ 60% |

**Overall verdict: MARGINALLY BELOW THRESHOLD (59.5%).**
docs.rs clears the bar (61.8%); wal.rs is 2.5 points below (57.5%).

**Runtime:** 24 minutes. Tool: `cargo mutants --in-place --baseline skip`.
**Commit:** 5f6c336 (all 9 docs::tests green; `cargo check --workspace` exit 0).

---

## docs.rs — 61.8% PASS

**Mutants:** 36 identified | 34 tested | 21 caught | 13 missed | 2 unviable

### Missed Mutants (docs.rs)

| Location | Mutation | Root cause |
|---|---|---|
| `score_doc:140` | replace `/` with `*` in topic_score | Tests check ranking but not absolute score values |
| `tokenize:172` | replace `<` with `==` | No test exercises ≤2-char token boundary |
| `tokenize:172` | replace `<` with `<=` | No test exercises ≤3-char token boundary |
| `extract_excerpt:202` | delete `!` in after_header flag | Tests don't assert on excerpt format precisely |
| `extract_excerpt:208` | replace `>` with `==` / `<` / `>=` (3) | Excerpt length boundary not tested exactly |
| `extract_excerpt:208` | replace `+` with `-` / `*` (2) | Excerpt length calculation not tested exactly |
| `extract_excerpt:211` | delete `!` in buf.is_empty() | Fallback path not exercised |
| `default_search_paths:226` | replace return with `vec![]` / `vec![Default]` | No test calls `default_search_paths()` directly |

**Test gaps:** excerpt length boundary logic and `default_search_paths` function lack targeted tests.

---

## wal.rs — 57.5% (BELOW THRESHOLD)

**Mutants:** 46 identified | 40 tested | 23 caught | 17 missed | 6 unviable

### Missed Mutants (wal.rs)

| Location | Mutation | Root cause |
|---|---|---|
| `MAX_WAL_PAYLOAD_BYTES:46` | replace `*` with `+` | Constant value not tested (16+1024 ≠ 16*1024 but tests don't check boundary exactly) |
| `ack_key:107` | replace return with `[0;24]` / `[1;24]` | Key derivation tested via roundtrip but key value itself not pinned |
| `compute_tag:116` | replace return with `[1;32]` | Tag value verified by verification tests but not pinned against fixed key |
| `read_fingerprint:141` | match guard `== NotFound` → true/false | Error classification path: test fp4 covers missing entry but not NotFound path |
| `read_fingerprint:141` | replace `==` with `!=` | Same as above |
| `load_or_create_machine_key:232` | replace return with `vec![]`/`[0]`/`[1]` | Key generation tested indirectly; no direct unit test on key bytes |
| `load_or_create_machine_key:240` | guard `stored_fp.is_none()` → true/false | Fingerprint-absent branch tested but not guard flip |
| `Wal::open_with_key:315` | replace `+` with `*` | Seek offset arithmetic not pinned |
| `Wal::append:322` | replace `>` with `>=` | MAX_WAL_PAYLOAD_BYTES boundary: `> 16384` vs `>= 16384` (off-by-one missed) |
| `Wal::ack:398` | replace with `Ok(())` | `test_roundtrip_tag_verifies` doesn't verify ack() side effect |
| `Wal::unacked:409` | replace with `Ok(vec![])` | `test_roundtrip_tag_verifies` doesn't read back unacked list |
| `Wal::unacked:416` | delete `!` | Same — unacked returns wrong polarity but test doesn't exercise this |

**Test gaps:** `wal.rs` needs tests pinning `Wal::ack`/`Wal::unacked` round-trip, the `> vs >=` boundary for `MAX_WAL_PAYLOAD_BYTES`, and the `read_fingerprint` NotFound guard path.

---

## How to reach ≥ 60% on wal.rs

The 3 highest-value gaps to close (each covers multiple missed mutants):

1. **`Wal::ack`/`Wal::unacked` round-trip** — add a test that calls `append`, `ack`, then asserts `unacked()` returns empty; add another without ack to assert `unacked()` returns the record. Closes `wal.rs:398`, `wal.rs:409`, `wal.rs:416` (3 missed).

2. **`MAX_WAL_PAYLOAD_BYTES` boundary** — add a test with `payload.len() == MAX_WAL_PAYLOAD_BYTES` asserting success and `len() == MAX_WAL_PAYLOAD_BYTES + 1` asserting error. Closes `wal.rs:322` (1 missed; the `>` vs `>=` flip).

3. **`read_fingerprint` error path** — add a test injecting an IoError with `kind() != NotFound` and asserting it propagates instead of returning `Ok(None)`. Closes `wal.rs:141` (3 missed).

---

## Unviable Mutants (8 total)

Mutants where cargo-mutants could not compile a valid replacement (trait bounds, iterator signatures, etc.). Not counted in the score denominator.

- `docs.rs:59` — `route() → Ok(vec![Default::default()])` (DocHit not Default)
- `docs.rs:92` — `score_doc() → Ok(Some(Default::default()))` (DocHit not Default)
- `wal.rs:290` — `Wal::open() → Ok(Default::default())` (Wal not Default)
- `wal.rs:299` — `Wal::open_with_key() → Ok(Default::default())` (Wal not Default)
- `wal.rs:352` — `iter_from() → once(Ok((0/1, Default::default())))` (WalRecord not Default) ×2
- `wal.rs:409` — `unacked() → Ok(vec![(0/1, Default::default())])` (WalRecord not Default) ×2

---

## Prior Baseline

Previous file: partial measurement (1/2 mutants tested, 60s cap, docs.rs blocked).
This file supersedes it with a full 82-mutant run (24 min, no cap).

---

## Methodology

```sh
cargo mutants --no-shuffle --in-place --baseline skip --timeout 120 \
  --output state/mutants-v1.0.2-full \
  --package amore-core \
  --file crates/amore-core/src/wal.rs \
  --file crates/amore-core/src/docs.rs
```

- `--in-place`: mutate source tree directly (avoids workspace copy overhead)
- `--baseline skip`: tests confirmed green at HEAD before run; no re-run needed
- `--timeout 120`: per-mutant 120s ceiling (actual median ~15s)
- Results in `state/mutants-v1.0.2-full/mutants.out/`
