<!-- stable: true -->
# Mutation Baseline — Amore v1.0.2

**Date:** 2026-05-27
**Tool:** cargo-mutants 27.0.0
**Bigtech-grade threshold:** ≥ 60% caught

---

## Summary (updated after gap-closure patch)

| File | Mutants identified | Tested (excl. unviable) | Caught | Missed | Unviable | Score | Threshold |
|---|---|---|---|---|---|---|---|
| crates/amore-core/src/docs.rs | 36 | 34 | 21 | 13 | 2 | **61.8%** | ≥ 60% PASS |
| crates/amore-core/src/wal.rs | 46 | 40 | 31 | 9 | 6 | **77.5%** | ≥ 60% PASS |
| **Combined** | **82** | **74** | **52** | **22** | **8** | **70.3%** | ≥ 60% PASS |

**Overall verdict: PASS (70.3%).**
docs.rs: 61.8% PASS; wal.rs: 77.5% PASS (was 57.5% before gap-closure).

**Gap-closure commit:** `test(wal): close 3 mutation gaps — ack/unacked, payload boundary, fingerprint NotFound`
Tests added: `gap1_ack_unacked_round_trip`, `gap2_payload_boundary_exact_succeeds_plus_one_fails`,
`gap3_read_fingerprint_not_found_returns_ok_none_then_ok_some_after_write`.
Mutants newly caught vs prior baseline: lines 107 (2×), 322 (5×), 352 (1×), 398 (1×), 409 (1×), 416 (1×) = +8 caught.

**Original baseline runtime:** 24 minutes (commit 5f6c336, 9 tests green).
**Gap-closure rerun:** ~18 minutes (wal.rs only, `--in-place --baseline skip`).

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

## wal.rs — 77.5% PASS (updated after gap-closure)

**Mutants:** 46 identified | 40 tested | 31 caught | 9 missed | 6 unviable

### Missed Mutants (wal.rs) — remaining after gap-closure

| Location | Mutation | Root cause |
|---|---|---|
| `MAX_WAL_PAYLOAD_BYTES:46` | replace `*` with `+` | Constant used in both repeat() and assertion — test scale-invariant; would need hard-coded 16384 |
| `compute_tag:116` | replace return with `[1;32]` | Tag not pinned against a known-key fixed vector |
| `read_fingerprint:141` | match guard `== NotFound` → true | `guard → true` silences non-NotFound errors; no synthetic IO error injected |
| `load_or_create_machine_key:232` | replace return with `vec![]`/`[0]`/`[1]` (3×) | Key generation tested indirectly; no unit test pinning key bytes |
| `load_or_create_machine_key:240` | guard `stored_fp.is_none()` → true/false | Guard flip not exercised |
| `Wal::open_with_key:315` | replace `+` with `*` | Seek offset arithmetic not pinned |

### Mutants newly caught by gap-closure tests

| Location | Mutation | Caught by |
|---|---|---|
| `ack_key:107` | replace return with `[0;24]` / `[1;24]` (2×) | `gap1_ack_unacked_round_trip` |
| `Wal::append:322` | replace `>` with `==`/`<`/`>=` and full-fn noop (5×) | `gap2_payload_boundary_exact_succeeds_plus_one_fails` |
| `Wal::iter_from:352` | replace with `::std::iter::empty()` | `gap1_ack_unacked_round_trip` (reads back seqs) |
| `Wal::ack:398` | replace with `Ok(())` | `gap1_ack_unacked_round_trip` |
| `Wal::unacked:409` | replace with `Ok(vec![])` | `gap1_ack_unacked_round_trip` |
| `Wal::unacked:416` | delete `!` in acked predicate | `gap1_ack_unacked_round_trip` |

---

## Unviable Mutants (8 total)

Mutants where cargo-mutants could not compile a valid replacement (trait bounds, iterator signatures, etc.). Not counted in the score denominator.

- `docs.rs:59` — `route() → Ok(vec![Default::default()])` (DocHit not Default)
- `docs.rs:92` — `score_doc() → Ok(Some(Default::default()))` (DocHit not Default)
- `wal.rs:290` — `Wal::open() → Ok(Default::default())` (Wal not Default)
- `wal.rs:299` — `Wal::open_with_key() → Ok(Default::default())` (Wal not Default)
- `wal.rs:352` — `iter_from() → once(Ok((0/1, Default::default())))` (WalRecord not Default) ×2
- `wal.rs:409` — `unacked() → Ok(vec![(0/1, Default::default())])` (WalRecord not Default) ×2

(Gap-closure rerun confirmed the same 6 wal.rs unviable; 2 docs.rs unviable unchanged.)

---

## Prior Baseline

Previous file: partial measurement (1/2 mutants tested, 60s cap, docs.rs blocked).
This file supersedes it with a full 82-mutant run (24 min, no cap).

---

## Methodology

### Original full baseline run (commit 5f6c336)

```sh
cargo mutants --no-shuffle --in-place --baseline skip --timeout 120 \
  --output state/mutants-v1.0.2-full \
  --package amore-core \
  --file crates/amore-core/src/wal.rs \
  --file crates/amore-core/src/docs.rs
```

Results in `state/mutants-v1.0.2-full/mutants.out/`

### Gap-closure rerun (wal.rs only, after adding gap1/gap2/gap3 tests)

```sh
cargo mutants --in-place --baseline skip --timeout 120 \
  --output state/mutants-v1.0.2-wal-rerun \
  --package amore-core \
  --file crates/amore-core/src/wal.rs
```

Results in `state/mutants-v1.0.2-wal-rerun/mutants.out/`

- `--in-place`: mutate source tree directly (avoids workspace copy overhead)
- `--baseline skip`: 11 wal::tests green at HEAD before run; no re-run needed
- `--timeout 120`: per-mutant 120s ceiling (actual median ~20s with release build)
