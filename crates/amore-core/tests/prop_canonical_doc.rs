// Property-based tests for amore_core::docs (CanonicalDocsRouter).
//
// Tests exercise the public `route()` API by writing temporary .md files and
// asserting the documented properties of `topic_score`:
//
//   1. Score is bounded in [0.0, 1.0] for every matching doc.
//   2. Scoring is reproducible: same (query, doc_body) => same score.
//   3. Docs without `stable: true` in the first 10 lines always score 0
//      (i.e., the router does not return them).
//
// Temporary directories use the same fresh_dir() pattern as docs.rs unit tests,
// with a global AtomicU64 counter + nanos to avoid collision across proptest
// cases.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use amore_core::docs::CanonicalDocsRouter;
use proptest::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Test-directory helpers
// ---------------------------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let p = std::env::temp_dir().join(format!("amore-prop-docs-{nanos:x}-{n}"));
    fs::create_dir_all(&p).unwrap();
    p
}

fn write_doc(dir: &Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).unwrap();
}

/// Clean up temp directory after each proptest case (best-effort).
fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// Alphanumeric query tokens, at least 3 chars each (matching the tokenizer
/// minimum in docs.rs), joined with spaces.
fn arb_query() -> impl Strategy<Value = String> {
    proptest::collection::vec("[a-z]{3,12}", 1..=5)
        .prop_map(|tokens| tokens.join(" "))
}

/// Alphanumeric doc body (no special markdown that would confuse the scanner),
/// long enough that the tokenizer finds at least some overlap.
fn arb_body() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9 _\\-\n]{0,400}"
}

/// File stem (used as doc filename). Kept simple — no slashes.
fn arb_stem() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_-]{0,20}".prop_map(|s| s)
}

// ---------------------------------------------------------------------------
// Property 1: score is bounded in [0.0, 1.0] for every matching doc
// ---------------------------------------------------------------------------
// The formula in score_doc is `matches / q_tokens.len()`, both counts are
// non-negative integers, and matches <= q_tokens.len(), so the result must
// be in [0.0, 1.0]. We assert the invariant on real router output.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_score_bounded(
        query  in arb_query(),
        body   in arb_body(),
        stem   in arb_stem(),
    ) {
        let dir = fresh_dir();
        let doc = format!("stable: true\n# Doc\n\n{body}");
        write_doc(&dir, &format!("{stem}.md"), &doc);

        let router = CanonicalDocsRouter::new();
        let hits = router.route(&query, &[dir.as_path()]).unwrap();

        for hit in &hits {
            prop_assert!(
                hit.topic_score >= 0.0,
                "topic_score must be >= 0.0, got {}",
                hit.topic_score
            );
            prop_assert!(
                hit.topic_score <= 1.0,
                "topic_score must be <= 1.0, got {}",
                hit.topic_score
            );
        }

        cleanup(&dir);
    }
}

// ---------------------------------------------------------------------------
// Property 2: scoring is reproducible (same inputs => same score)
// ---------------------------------------------------------------------------
// The router is deterministic — no random state, no time-based component in
// the scoring formula. Two successive calls on the same dir and query must
// return identical scores.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_score_reproducible(
        query in arb_query(),
        body  in arb_body(),
        stem  in arb_stem(),
    ) {
        let dir = fresh_dir();
        let doc = format!("stable: true\n# Doc\n\n{body}");
        write_doc(&dir, &format!("{stem}.md"), &doc);

        let router = CanonicalDocsRouter::new();
        let hits1 = router.route(&query, &[dir.as_path()]).unwrap();
        let hits2 = router.route(&query, &[dir.as_path()]).unwrap();

        prop_assert_eq!(
            hits1.len(), hits2.len(),
            "repeated calls must return same number of hits"
        );

        for (h1, h2) in hits1.iter().zip(hits2.iter()) {
            // f32::to_bits gives exact bit equality (no float fuzz).
            prop_assert_eq!(
                h1.topic_score.to_bits(), h2.topic_score.to_bits(),
                "repeated routing must return identical scores: \
                 first={} second={}",
                h1.topic_score, h2.topic_score
            );
        }

        cleanup(&dir);
    }
}

// ---------------------------------------------------------------------------
// Property 3: docs without `stable: true` always score zero
// ---------------------------------------------------------------------------
// When `require_stable` is true (the default), no doc without `stable: true`
// in its first 10 lines may appear in the output — i.e., the router returns
// an empty vec for any query against non-stable docs.
// We verify this by crafting bodies that explicitly lack `stable: true`.

/// Bodies that never contain `stable: true` in the first 10 lines.
fn arb_non_stable_body() -> impl Strategy<Value = String> {
    // Body starts with something plausible but NOT the stable marker.
    // Prepend `stable: false` to make the intent explicit.
    arb_body().prop_map(|body| format!("stable: false\n# Draft\n\n{body}"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_non_stable_doc_always_scores_zero(
        query in arb_query(),
        body  in arb_non_stable_body(),
        stem  in arb_stem(),
    ) {
        let dir = fresh_dir();
        // Ensure the query tokens appear verbatim in the body so this is a
        // worst-case test: even if keyword overlap is maximal, non-stable
        // docs must still be filtered.
        let enriched_body = format!("{body}\n{query}");
        write_doc(&dir, &format!("{stem}.md"), &enriched_body);

        let router = CanonicalDocsRouter::new(); // require_stable = true
        let hits = router.route(&query, &[dir.as_path()]).unwrap();

        prop_assert!(
            hits.is_empty(),
            "non-stable doc must not appear in router output, \
             but got {} hits",
            hits.len()
        );

        cleanup(&dir);
    }
}
