// H.4 Qdrant pool smoke test.
//
// Skipped by default — requires a live Qdrant daemon.
// Run with:
//   OBELION_TEST_QDRANT=1 cargo test -p amore-core --test qdrant_pool_smoke -- --ignored
//
// Test: borrow 4 connections concurrently from a pool with max_size=8;
//       all 4 round-trip a health_check(); no errors.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use amore_core::qdrant_pool::{build_pool, default_pool_size};

// gRPC port for qdrant-client.
const TEST_QDRANT_URL: &str = "http://127.0.0.1:6334";

fn enabled() -> bool {
    std::env::var("OBELION_TEST_QDRANT").ok().as_deref() == Some("1")
}

#[tokio::test]
#[ignore = "requires OBELION_TEST_QDRANT=1 + live Qdrant daemon at 6334"]
async fn pool_four_concurrent_health_checks() {
    if !enabled() {
        eprintln!("OBELION_TEST_QDRANT not set — skipping pool smoke test");
        return;
    }

    let pool = build_pool(TEST_QDRANT_URL, 8)
        .await
        .expect("build_pool must succeed with live Qdrant");

    // Spawn 4 concurrent tasks each borrowing a connection + calling health_check.
    let mut handles = Vec::with_capacity(4);
    for i in 0..4u32 {
        let p = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut conn = p.get().await.unwrap_or_else(|e| panic!("task {i}: pool.get failed: {e:?}"));
            conn.health_check()
                .await
                .unwrap_or_else(|e| panic!("task {i}: health_check failed: {e}"));
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        h.await
            .unwrap_or_else(|e| panic!("task {i} panicked: {e}"));
    }
}

#[test]
fn default_pool_size_is_in_range() {
    let size = default_pool_size();
    assert!(size >= 2, "default_pool_size must be >= 2, got {size}");
    assert!(size <= 16, "default_pool_size must be <= 16, got {size}");
}
