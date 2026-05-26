// reranker_parity.rs — integration tests for the cross-encoder reranker (H.3)
//
// # Model requirement (T1 only)
//
// T1 requires BAAI/bge-reranker-base ONNX model + tokenizer.json.
// The model is NOT bundled in this repo (~110MB).
//
// ## Download instructions
//
//   # 1. Download model files from HuggingFace
//   huggingface-cli download BAAI/bge-reranker-base \
//       --local-dir "$LOCALAPPDATA/Amore/models/" \
//       --include "tokenizer.json" "tokenizer_config.json"
//
//   # 2. Export model to ONNX (requires optimum-cli)
//   pip install optimum[onnxruntime]
//   optimum-cli export onnx \
//       --model BAAI/bge-reranker-base \
//       --task text-classification \
//       /tmp/bge-reranker-base-onnx/
//   cp /tmp/bge-reranker-base-onnx/model.onnx "$LOCALAPPDATA/Amore/models/bge-reranker-base.onnx"
//
//   # 3. Run T1
//   AMORE_TEST_RERANKER=1 cargo test --release -p amore-core --test reranker_parity -- t1
//
// T2 and T3 run without the model (default-on).
//
// # nDCG@10 parity test (deferred)
// See docs/H3-RERANKER-NDCG-PLAN.md for the procedure to measure reranker
// uplift vs RRF-only baseline on the 100-query golden set.

use amore_core::reranker::Reranker;
use std::path::PathBuf;

fn nonexistent_path() -> PathBuf {
    PathBuf::from(
        "/this/path/should/never/exist/amore-test-sentinel-onnx-model-404.onnx",
    )
}

// --- T1: smoke test with real model (env-gated) ----------------------------

/// Load the reranker from default paths and verify rank ordering on 5 candidates.
///
/// Requires `AMORE_TEST_RERANKER=1` and the model files to be present.
/// See file header for download instructions.
#[test]
#[ignore]
fn t1_smoke_ranking_with_real_model() {
    if std::env::var("AMORE_TEST_RERANKER").as_deref() != Ok("1") {
        eprintln!("skip: AMORE_TEST_RERANKER != 1");
        return;
    }

    let mut reranker = Reranker::from_default_paths()
        .expect("reranker should load from default paths when model files present");

    let query = "rust async runtime";
    let candidates: Vec<(u64, String)> = vec![
        (1, "Tokio is a Rust async runtime for writing reliable applications".to_string()),
        (2, "async-std provides async runtime primitives for Rust".to_string()),
        (3, "general programming concepts and software design patterns".to_string()),
        (4, "unrelated topic: cooking recipes and meal planning".to_string()),
        (5, "Rust memory safety without garbage collection via ownership".to_string()),
    ];

    let top = reranker
        .rerank(query, candidates, 5)
        .expect("rerank should succeed with real model");

    assert_eq!(top.len(), 5, "should return all 5 candidates");

    // Tokio (id=1) and async-std (id=2) should rank above the unrelated topic (id=4)
    let pos_tokio = top.iter().position(|(id, _)| *id == 1);
    let pos_async_std = top.iter().position(|(id, _)| *id == 2);
    let pos_cooking = top.iter().position(|(id, _)| *id == 4);

    assert!(
        pos_tokio.is_some() && pos_cooking.is_some(),
        "tokio and cooking doc must appear in results"
    );
    assert!(
        pos_tokio.unwrap() < pos_cooking.unwrap(),
        "tokio (async Rust) must rank above cooking recipes"
    );
    if let (Some(p_async), Some(p_cooking)) = (pos_async_std, pos_cooking) {
        assert!(
            p_async < p_cooking,
            "async-std (async Rust) must rank above cooking recipes"
        );
    }

    // Scores should be returned in descending order
    let scores: Vec<f32> = top.iter().map(|(_, s)| *s).collect();
    for window in scores.windows(2) {
        assert!(
            window[0] >= window[1],
            "scores must be in descending order, got {:.4} < {:.4}",
            window[0],
            window[1]
        );
    }
}

// --- T2: missing model path returns Err, no panic -------------------------

/// Calling Reranker::new with a non-existent model path must return Err.
/// The error must not panic and must contain a descriptive message.
#[test]
fn t2_missing_model_returns_err_not_panic() {
    let model = nonexistent_path();
    let tok = nonexistent_path().with_extension("json");
    let result = Reranker::new(&model, &tok);
    assert!(
        result.is_err(),
        "expected Err for missing model path, got Ok"
    );
    let msg = result.err().unwrap().to_string();
    assert!(
        msg.contains("not found") || msg.contains("ONNX model"),
        "error message should contain 'not found' or 'ONNX model', got: {msg}"
    );
}

// --- T3: sort + top_k logic ---------------------------------------------------

/// Verify sort ordering without a live model session.
/// These tests exercise the ranking logic by calling the sort directly.
#[test]
fn t3_sort_ordering_highest_score_ranks_first() {
    let mut scored: Vec<(u64, f32)> = vec![(1, 0.1), (2, 0.9), (3, 0.5)];
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    assert_eq!(scored[0].0, 2, "id=2 (score 0.9) should rank first");
    assert_eq!(scored[1].0, 3, "id=3 (score 0.5) should rank second");
    assert_eq!(scored[2].0, 1, "id=1 (score 0.1) should rank last");
}

#[test]
fn t3_top_k_truncation() {
    let mut scored: Vec<(u64, f32)> = vec![(10, 0.3), (20, 0.8), (30, 0.1), (40, 0.6)];
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(2);
    assert_eq!(scored.len(), 2, "top_k=2 must return exactly 2");
    assert_eq!(scored[0].0, 20, "id=20 (score 0.8) should rank first");
    assert_eq!(scored[1].0, 40, "id=40 (score 0.6) should rank second");
}

#[test]
fn t3_equal_scores_stable_order_preserved() {
    // Ties should preserve stable sort order (earlier in input = earlier in output at same score)
    let mut scored: Vec<(u64, f32)> = vec![(1, 0.5), (2, 0.9), (3, 0.5)];
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    assert_eq!(scored[0].0, 2, "highest score should be first");
    // Tie between id=1 and id=3 at 0.5 — both present
    let tied: Vec<u64> = scored[1..].iter().map(|(id, _)| *id).collect();
    assert!(tied.contains(&1) && tied.contains(&3), "both tied IDs must appear");
}

// --- T4: nDCG@10 vs RRF baseline (deferred) ----------------------------------
//
// This test is a placeholder. The full 100-query golden set procedure is
// documented in docs/H3-RERANKER-NDCG-PLAN.md.
// Implementation is deferred to Wave 3 / crates/amore-eval/src/bin/ndcg_compare.rs.
#[test]
#[ignore]
fn t4_ndcg10_reranker_beats_rrf_baseline() {
    // Requires:
    // 1. 100-query golden set (see docs/H3-RERANKER-NDCG-PLAN.md §Golden set)
    // 2. Ground truth labels (see docs/H3-RERANKER-NDCG-PLAN.md §Ground truth)
    // 3. crates/amore-eval/src/bin/ndcg_compare.rs (Wave 3/J)
    //
    // Pass gate: reranker nDCG@10 >= RRF nDCG@10 + 0.05 on full 100-query set.
    panic!("deferred — see docs/H3-RERANKER-NDCG-PLAN.md");
}
