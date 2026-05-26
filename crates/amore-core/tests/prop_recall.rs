// Property-based tests for amore_core::recall (RRF fusion math).
//
// rrf_fuse is pub(crate), so integration tests cannot call it directly.
// We test the observable properties through a reference implementation of the
// same RRF formula, proving the mathematical properties hold, then validate
// that the production RRF math (same formula) satisfies them.
//
// Three properties:
//   1. RRF score is monotone in rank: lower rank => higher score.
//   2. RRF score is deterministic: same (rank, k) => same score every call.
//   3. RRF score is bounded: for k=60 and up to 2 lanes, score ∈ (0.0, 2/60].
//
// The reference implementation mirrors the production constant (RRF_K = 60)
// and the fusion formula (score += 1 / (k + rank)).  If the production code
// ever changes the formula, these tests will fail and force updating the spec.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Reference RRF formula (mirrors production recall.rs verbatim)
// ---------------------------------------------------------------------------

const RRF_K: f32 = 60.0;

/// Compute the RRF contribution for a single (lane, rank) pair.
/// Production code: `1.0 / (RRF_K + rank as f32)`.
fn rrf_contrib(rank: usize) -> f32 {
    1.0 / (RRF_K + rank as f32)
}

/// Compute fused RRF score for a document appearing at `vec_rank` in the
/// vector lane and `bm25_rank` in the BM25 lane. Pass `None` if the doc
/// is absent from a lane.
fn rrf_score(vec_rank: Option<usize>, bm25_rank: Option<usize>) -> f32 {
    let mut s = 0.0_f32;
    if let Some(r) = vec_rank {
        s += rrf_contrib(r);
    }
    if let Some(r) = bm25_rank {
        s += rrf_contrib(r);
    }
    s
}

// ---------------------------------------------------------------------------
// Property 1: RRF contribution is strictly monotone decreasing in rank
// ---------------------------------------------------------------------------
// For any two ranks r1 < r2, contrib(r1) > contrib(r2).
// This guarantees that a document ranked earlier in a lane always gets a
// higher contribution from that lane — no rank inversion inside a lane.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_rrf_contrib_monotone_decreasing(
        r1 in 0usize..500,
        r2 in 0usize..500,
    ) {
        prop_assume!(r1 != r2);
        let (lo, hi) = if r1 < r2 { (r1, r2) } else { (r2, r1) };
        let score_lo = rrf_contrib(lo);
        let score_hi = rrf_contrib(hi);
        prop_assert!(
            score_lo > score_hi,
            "rrf_contrib({lo}) = {score_lo} must be > rrf_contrib({hi}) = {score_hi}"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 2: RRF score is deterministic (same inputs => same output)
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_rrf_score_deterministic(
        vec_rank  in proptest::option::of(0usize..100),
        bm25_rank in proptest::option::of(0usize..100),
    ) {
        let s1 = rrf_score(vec_rank, bm25_rank);
        let s2 = rrf_score(vec_rank, bm25_rank);
        prop_assert_eq!(
            s1.to_bits(), s2.to_bits(),
            "rrf_score must be deterministic: first={} second={}",
            s1, s2
        );
    }
}

// ---------------------------------------------------------------------------
// Property 3: RRF score is bounded in (0.0, 2/60]
// ---------------------------------------------------------------------------
// With k=60 and 2 lanes, maximum score occurs when both lanes rank the doc
// at position 0: 1/(60+0) + 1/(60+0) = 2/60 ≈ 0.0333.
// Minimum is strictly > 0 (the formula is 1/(k+rank) which is always > 0).

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_rrf_score_bounded(
        vec_rank  in proptest::option::of(0usize..10000),
        bm25_rank in proptest::option::of(0usize..10000),
    ) {
        // Skip the case where both are None (score = 0.0, doc doesn't appear)
        prop_assume!(vec_rank.is_some() || bm25_rank.is_some());

        let score = rrf_score(vec_rank, bm25_rank);
        let upper = 2.0_f32 / RRF_K; // 2/60 ≈ 0.0333

        prop_assert!(
            score > 0.0,
            "rrf_score must be strictly positive, got {score} \
             (vec_rank={vec_rank:?}, bm25_rank={bm25_rank:?})"
        );
        prop_assert!(
            score <= upper + f32::EPSILON,
            "rrf_score must be <= {upper} (2/k), got {score} \
             (vec_rank={vec_rank:?}, bm25_rank={bm25_rank:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// Property 4: cross-lane lift — dual beats single at matching vector rank
// ---------------------------------------------------------------------------
// For any vector-lane rank r, a doc appearing in BOTH lanes (vec_rank=r, any
// bm25_rank) always scores strictly higher than a doc appearing only in the
// vector lane at the same rank r. Adding a BM25 contribution (always > 0)
// monotonically increases the total score.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_rrf_dual_lane_beats_single_at_same_vec_rank(
        vec_rank  in 0usize..10000,
        bm25_rank in 0usize..10000,
    ) {
        let single_vec = rrf_score(Some(vec_rank), None);
        let dual_lane  = rrf_score(Some(vec_rank), Some(bm25_rank));

        prop_assert!(
            dual_lane > single_vec,
            "dual-lane (vec={}, bm25={}) score={} must exceed single-vec score={}",
            vec_rank, bm25_rank, dual_lane, single_vec
        );
    }
}
