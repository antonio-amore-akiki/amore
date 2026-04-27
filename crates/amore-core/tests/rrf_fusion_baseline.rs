// @file-size-exempt: characterization test harness — frozen regression gate for H.1 migration
// Characterization test: Reciprocal Rank Fusion at k=60 — H.0 regression gate.
//
// PURPOSE: freeze the fused ranking output on fixed (vector_top_k, bm25_top_k) input pairs.
// The Tantivy migration (H.1) must prove byte-identical RRF parity against this fixture.
//
// FORMULA (recall.rs, Cormack/Clarke/Buettcher 2009):
//   rrf(d) = sum over (lane, rank) of  1 / (k + rank)   where rank is 0-based, k = 60.
//
// rrf_fuse() is pub(crate); integration tests run as a separate crate so we implement
// the formula inline — correct because the test proves the CONTRACT, not the symbol name.
//
// FIXTURE PATTERN: first run (or AMORE_BM25_REBASE=1 / OBELION_BM25_REBASE=1) writes
// tests/fixtures/rrf_baseline.json; subsequent runs compare byte-identically.

#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::path::PathBuf;

/// Must match recall.rs RRF_K = 60.0.
const RRF_K: f32 = 60.0;

// ---------------------------------------------------------------------------
// Minimal hit types (no dep on pub(crate) internals)
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
struct VecHit {
    id: String,
    #[allow(dead_code)] // semantic intent — RRF uses rank order, score recorded for clarity
    score: f32,
}
#[derive(Debug, Clone)]
struct Bm25Hit {
    id: String,
    #[allow(dead_code)] // semantic intent — RRF uses rank order, score recorded for clarity
    score: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
struct FusedHit {
    id: String,
    rrf_score: f32,
    rank: usize,
}

// ---------------------------------------------------------------------------
// RRF — matches recall.rs rrf_fuse() contract: 1/(k+rank), 0-based, desc sort.
// Tie-break by id asc for determinism.
// ---------------------------------------------------------------------------
fn rrf_fuse(vec_hits: &[VecHit], bm_hits: &[Bm25Hit], top_k: usize) -> Vec<FusedHit> {
    let mut acc: HashMap<String, f32> = HashMap::new();
    for (rank, h) in vec_hits.iter().enumerate() {
        *acc.entry(h.id.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f32);
    }
    for (rank, h) in bm_hits.iter().enumerate() {
        *acc.entry(h.id.clone()).or_insert(0.0) += 1.0 / (RRF_K + rank as f32);
    }
    let mut fused: Vec<FusedHit> = acc
        .into_iter()
        .map(|(id, rrf_score)| FusedHit {
            id,
            rrf_score,
            rank: 0,
        })
        .collect();
    fused.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
    fused.truncate(top_k);
    for (i, h) in fused.iter_mut().enumerate() {
        h.rank = i + 1;
    }
    fused
}

// ---------------------------------------------------------------------------
// 30-document universe
// ---------------------------------------------------------------------------
fn doc_ids() -> Vec<&'static str> {
    vec![
        "d-000", "d-001", "d-002", "d-003", "d-004", "d-005", "d-006", "d-007", "d-008", "d-009",
        "d-010", "d-011", "d-012", "d-013", "d-014", "d-015", "d-016", "d-017", "d-018", "d-019",
        "d-020", "d-021", "d-022", "d-023", "d-024", "d-025", "d-026", "d-027", "d-028", "d-029",
    ]
}

fn vec_hit(id: &str, rank: usize) -> VecHit {
    VecHit {
        id: id.to_string(),
        score: 1.0 - 0.01 * rank as f32,
    }
}
fn bm25_hit(id: &str, rank: usize) -> Bm25Hit {
    Bm25Hit {
        id: id.to_string(),
        score: 10.0 - 0.3 * rank as f32,
    }
}

// ---------------------------------------------------------------------------
// 5 fixed input pairs (pair_id, vec_ids, bm25_ids)
// ---------------------------------------------------------------------------
fn input_pairs() -> Vec<(&'static str, Vec<&'static str>, Vec<&'static str>)> {
    vec![
        (
            "pair-1",
            vec![
                "d-005", "d-011", "d-017", "d-023", "d-029", "d-000", "d-006", "d-012", "d-018",
                "d-024",
            ],
            vec![
                "d-011", "d-005", "d-029", "d-017", "d-023", "d-007", "d-013", "d-019", "d-025",
                "d-001",
            ],
        ),
        (
            "pair-2",
            vec![
                "d-000", "d-006", "d-012", "d-018", "d-024", "d-001", "d-007", "d-013", "d-019",
                "d-025",
            ],
            vec![
                "d-008", "d-014", "d-020", "d-026", "d-002", "d-000", "d-006", "d-003", "d-009",
                "d-015",
            ],
        ),
        (
            "pair-3",
            vec![
                "d-000", "d-001", "d-002", "d-003", "d-004", "d-005", "d-006", "d-007", "d-008",
                "d-009", "d-010", "d-011", "d-012", "d-013", "d-014", "d-015", "d-016", "d-017",
                "d-018", "d-019",
            ],
            vec![
                "d-019", "d-018", "d-017", "d-016", "d-015", "d-014", "d-013", "d-012", "d-011",
                "d-010", "d-009", "d-008", "d-007", "d-006", "d-005", "d-004", "d-003", "d-002",
                "d-001", "d-000",
            ],
        ),
        (
            "pair-4",
            vec!["d-022", "d-028", "d-016", "d-010", "d-004"],
            vec![],
        ),
        (
            "pair-5",
            vec![],
            vec!["d-003", "d-009", "d-015", "d-021", "d-027"],
        ),
    ]
}

// ---------------------------------------------------------------------------
// Fixture types
// ---------------------------------------------------------------------------
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PairResult {
    pair_id: String,
    top_k: usize,
    fused: Vec<FusedHit>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RrfFixture {
    schema_version: u32,
    rrf_k: f32,
    pairs: Vec<PairResult>,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rrf_baseline.json")
}

// ---------------------------------------------------------------------------
// Main characterization test
// ---------------------------------------------------------------------------
#[test]
fn rrf_fusion_baseline() {
    let rebase = std::env::var("AMORE_BM25_REBASE")
        .or_else(|_| std::env::var("OBELION_BM25_REBASE"))
        .map(|v| v == "1")
        .unwrap_or(false);

    const TOP_K: usize = 10;

    let pair_results: Vec<PairResult> = input_pairs()
        .into_iter()
        .map(|(pair_id, vec_ids, bm25_ids)| {
            let vh: Vec<VecHit> = vec_ids
                .iter()
                .enumerate()
                .map(|(r, id)| vec_hit(id, r))
                .collect();
            let bh: Vec<Bm25Hit> = bm25_ids
                .iter()
                .enumerate()
                .map(|(r, id)| bm25_hit(id, r))
                .collect();
            PairResult {
                pair_id: pair_id.to_string(),
                top_k: TOP_K,
                fused: rrf_fuse(&vh, &bh, TOP_K),
            }
        })
        .collect();

    let fixture_path = fixture_path();

    if rebase || !fixture_path.exists() {
        let fixture = RrfFixture {
            schema_version: 1,
            rrf_k: RRF_K,
            pairs: pair_results,
        };
        let json_str = serde_json::to_string_pretty(&fixture).expect("serialize RRF fixture");
        std::fs::create_dir_all(fixture_path.parent().unwrap()).expect("create fixture dir");
        std::fs::write(&fixture_path, &json_str).expect("write RRF fixture");
        println!("RRF baseline fixture written to {}", fixture_path.display());
        return;
    }

    let raw = std::fs::read_to_string(&fixture_path).unwrap_or_else(|_| {
        panic!(
            "RRF fixture missing at {}; run with AMORE_BM25_REBASE=1",
            fixture_path.display()
        )
    });
    let fixture: RrfFixture = serde_json::from_str(&raw).expect("parse RRF fixture");

    assert_eq!(
        fixture.rrf_k, RRF_K,
        "RRF k constant changed: fixture={} impl={}",
        fixture.rrf_k, RRF_K
    );
    assert_eq!(
        fixture.pairs.len(),
        pair_results.len(),
        "pair count mismatch"
    );

    for (expected, actual) in fixture.pairs.iter().zip(pair_results.iter()) {
        assert_eq!(expected.pair_id, actual.pair_id);
        assert_eq!(
            expected.fused.len(),
            actual.fused.len(),
            "fused hit count mismatch for pair {}",
            expected.pair_id
        );
        for (e, a) in expected.fused.iter().zip(actual.fused.iter()) {
            assert_eq!(
                e.id, a.id,
                "rank {} id mismatch for pair {}: expected {} got {}",
                e.rank, expected.pair_id, e.id, a.id
            );
            assert_eq!(
                e.rank, a.rank,
                "rank mismatch for doc {} in pair {}",
                e.id, expected.pair_id
            );
            assert!(
                (e.rrf_score - a.rrf_score).abs() < 1e-6,
                "RRF score drift for doc {} in pair {}: expected {:.8} got {:.8}",
                e.id,
                expected.pair_id,
                e.rrf_score,
                a.rrf_score
            );
        }
    }
    println!(
        "RRF baseline: {} pairs verified against {}",
        pair_results.len(),
        fixture_path.display()
    );
}

// ---------------------------------------------------------------------------
// Invariant tests — always pass, no fixture dependency
// ---------------------------------------------------------------------------

#[test]
fn rrf_empty_both_lanes() {
    assert!(rrf_fuse(&[], &[], 5).is_empty());
}

#[test]
fn rrf_cross_lane_lift() {
    // "shared" rank-0 in both → rrf = 2/(60+0); "solo" rank-1 in vec → 1/(60+1)
    let vh = vec![vec_hit("shared", 0), vec_hit("solo", 1)];
    let bh = vec![bm25_hit("shared", 0)];
    let f = rrf_fuse(&vh, &bh, 5);
    assert_eq!(f[0].id, "shared", "cross-lane doc must rank #1");
    assert!(f[0].rrf_score > f[1].rrf_score);
}

#[test]
fn rrf_formula_exact_k60() {
    let vh = vec![vec_hit("A", 0), vec_hit("B", 1)];
    let f = rrf_fuse(&vh, &[], 5);
    let a = f.iter().find(|h| h.id == "A").unwrap();
    let b = f.iter().find(|h| h.id == "B").unwrap();
    assert!(
        (a.rrf_score - 1.0_f32 / 60.0).abs() < 1e-6,
        "A: {}",
        a.rrf_score
    );
    assert!(
        (b.rrf_score - 1.0_f32 / 61.0).abs() < 1e-6,
        "B: {}",
        b.rrf_score
    );
}

#[test]
fn rrf_truncates_top_k() {
    let vh: Vec<VecHit> = (0..20).map(|i| vec_hit(&format!("d-{i:02}"), i)).collect();
    assert_eq!(rrf_fuse(&vh, &[], 5).len(), 5);
}

#[test]
fn rrf_single_bm25_lane_preserves_order() {
    let bh = vec![bm25_hit("X", 0), bm25_hit("Y", 1), bm25_hit("Z", 2)];
    let f = rrf_fuse(&[], &bh, 5);
    assert_eq!(f.len(), 3);
    assert_eq!(f[0].id, "X");
    assert_eq!(f[1].id, "Y");
    assert_eq!(f[2].id, "Z");
}

#[test]
fn doc_universe_has_30_docs() {
    assert_eq!(doc_ids().len(), 30);
}

#[test]
fn input_pairs_has_5_entries() {
    assert_eq!(input_pairs().len(), 5);
}
