// H.5 circuit breaker state-machine tests.
//
// All tests run without network access — pure time + counter mechanics.
//
// Test contract (5 cases per spec):
//   1. 5 consecutive failures trip breaker to Open.
//   2. Calls while Open return BreakerError::Open immediately (inner future not polled).
//   3. After reset_after elapses, next call enters HalfOpen.
//   4. 3 successes in HalfOpen close the breaker back to Closed.
//   5. 1 failure in HalfOpen re-opens immediately.

#![cfg_attr(test, allow(clippy::unwrap_used))]

use amore_core::circuit_breaker::{BreakerError, BreakerState, CircuitBreaker};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// ── helpers ────────────────────────────────────────────────────────────────

/// Build a breaker with tight timings for tests.
fn test_breaker() -> CircuitBreaker {
    CircuitBreaker::with_config(
        "test_dep",
        5,                        // threshold_failures
        Duration::from_millis(50), // reset_after — short for tests
        3,                        // close_after_successes
    )
}

async fn fail_once(breaker: &CircuitBreaker) -> Result<(), BreakerError<anyhow::Error>> {
    breaker
        .call(|| async { anyhow::bail!("simulated failure") })
        .await
}

async fn succeed_once(breaker: &CircuitBreaker) -> Result<u32, BreakerError<anyhow::Error>> {
    breaker.call(|| async { Ok::<u32, anyhow::Error>(1) }).await
}

// ── test 1 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test1_five_failures_open_breaker() {
    let breaker = test_breaker();

    // 4 failures must NOT open (threshold = 5)
    for _ in 0..4 {
        fail_once(&breaker).await.unwrap_err();
    }
    // Should still be Closed — succeed must work
    succeed_once(&breaker).await.unwrap();

    // Now the breaker reset its counter on success. Apply 5 consecutive failures.
    for _ in 0..5 {
        fail_once(&breaker).await.unwrap_err();
    }

    // 6th call must observe Open and return BreakerError::Open immediately.
    let err = succeed_once(&breaker).await.unwrap_err();
    assert!(
        matches!(err, BreakerError::Open { .. }),
        "expected BreakerError::Open after 5 failures, got: {err:?}"
    );
}

// ── test 2 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test2_open_breaker_does_not_poll_inner_future() {
    let breaker = test_breaker();

    // Trip to Open.
    for _ in 0..5 {
        fail_once(&breaker).await.unwrap_err();
    }

    // Track whether the inner future body executes at all.
    let polled = Arc::new(AtomicUsize::new(0));
    let polled2 = Arc::clone(&polled);

    let result: Result<u32, BreakerError<anyhow::Error>> = breaker
        .call(|| {
            let p = Arc::clone(&polled2);
            async move {
                p.fetch_add(1, Ordering::SeqCst);
                Ok::<u32, anyhow::Error>(42)
            }
        })
        .await;

    assert!(
        matches!(result, Err(BreakerError::Open { .. })),
        "breaker must return Open, not poll the inner future"
    );
    assert_eq!(
        polled.load(Ordering::SeqCst),
        0,
        "inner future body must NOT execute when breaker is Open"
    );
}

// ── test 3 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test3_after_reset_after_next_call_enters_half_open() {
    let breaker = test_breaker(); // reset_after = 50ms

    // Trip to Open.
    for _ in 0..5 {
        fail_once(&breaker).await.unwrap_err();
    }

    // Immediately — still Open.
    let err = succeed_once(&breaker).await.unwrap_err();
    assert!(
        matches!(err, BreakerError::Open { .. }),
        "immediately after tripping, must still be Open"
    );

    // Wait past reset_after.
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Next call should enter HalfOpen and run the inner future.
    let result = succeed_once(&breaker).await;
    assert!(
        result.is_ok(),
        "after reset_after elapsed, first probe call must succeed (enters HalfOpen)"
    );
}

// ── test 4 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test4_three_successes_in_halfopen_close_breaker() {
    let breaker = test_breaker();

    // Trip to Open, wait for HalfOpen.
    for _ in 0..5 {
        fail_once(&breaker).await.unwrap_err();
    }
    tokio::time::sleep(Duration::from_millis(80)).await;

    // 3 successes in HalfOpen must close the breaker.
    // (the first probe call transitions to HalfOpen{successes:0}, so we
    //  need 3 successes counted from there — call 3 times total)
    for i in 0..3 {
        let r = succeed_once(&breaker).await;
        assert!(r.is_ok(), "HalfOpen success {i} must not error");
    }

    // Breaker must now be Closed — failures should NOT trip it in 4 tries.
    for _ in 0..4 {
        fail_once(&breaker).await.unwrap_err();
    }
    // 5th failure trips it again — but we prove Closed by showing 4 failures
    // are fine and a success still works on the 5th slot.
    let r = succeed_once(&breaker).await;
    assert!(
        r.is_ok(),
        "after HalfOpen -> Closed, 4 failures must not re-open (threshold=5); \
         the 5th slot is a success and must return Ok"
    );
}

// ── test 5 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test5_failure_in_halfopen_reopens_breaker() {
    let breaker = test_breaker();

    // Trip to Open, wait for HalfOpen.
    for _ in 0..5 {
        fail_once(&breaker).await.unwrap_err();
    }
    tokio::time::sleep(Duration::from_millis(80)).await;

    // First probe enters HalfOpen and fails — must re-open.
    fail_once(&breaker).await.unwrap_err();

    // Immediately, breaker must be Open again.
    let err = succeed_once(&breaker).await.unwrap_err();
    assert!(
        matches!(err, BreakerError::Open { .. }),
        "failure in HalfOpen must re-open the breaker; got: {err:?}"
    );
}

// ── extra: dep name propagated in error ─────────────────────────────────────

#[tokio::test]
async fn open_error_carries_dep_name() {
    let breaker = CircuitBreaker::with_config(
        "ollama_embed",
        1,                         // threshold = 1 for fast trip
        Duration::from_secs(3600), // don't auto-reset during this test
        3,
    );

    // One failure trips it.
    fail_once(&breaker).await.unwrap_err();

    // Second call — should return Open with correct dep name.
    let err = succeed_once(&breaker).await.unwrap_err();
    match err {
        BreakerError::Open { dep, .. } => {
            assert_eq!(dep, "ollama_embed", "dep name must be propagated");
        }
        other => panic!("expected BreakerError::Open, got: {other:?}"),
    }
}

// ── extra: BreakerState is Send+Sync (compile-time check) ──────────────────

fn _assert_send_sync()
where
    BreakerState: Send,
    CircuitBreaker: Send + Sync,
{
}
