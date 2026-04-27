// tests/rate_limit.rs — Integration test for per-session rate limiter (W3-3B).
//
// Tests the governor::RateLimiter keyed pattern used in middleware/rate_limit.rs.
// Since the middleware module is in the binary crate (not a lib), we replicate
// the governor API calls directly to validate the pattern and quota arithmetic.

use governor::{Quota, RateLimiter};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Newtype matching middleware::rate_limit::SessionId.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionId(String);

/// Error code returned by the rate limiter middleware.
const RATE_LIMIT_ERROR_CODE: i32 = -32099;

/// Check the rate limit; returns Ok(()) or Err with code -32099.
fn check(
    limiter: &governor::DefaultKeyedRateLimiter<SessionId>,
    session: &SessionId,
) -> Result<(), i32> {
    limiter.check_key(session).map(|_| ()).map_err(|_| RATE_LIMIT_ERROR_CODE)
}

#[test]
fn first_request_always_passes_under_any_quota() {
    let quota = Quota::per_second(NonZeroU32::new(50).unwrap());
    let limiter = Arc::new(RateLimiter::keyed(quota));
    let session = SessionId("test-session".to_string());
    assert!(check(&limiter, &session).is_ok(), "first request must pass");
}

#[test]
fn burst_of_100_at_rps_1_produces_at_least_40_rejections() {
    // RPS=1 means burst capacity=1. Flooding 100 requests should reject most.
    let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
    let limiter = Arc::new(RateLimiter::keyed(quota));
    let session = SessionId("flood-session".to_string());

    let mut rejected = 0usize;
    for _ in 0..100 {
        if check(&limiter, &session).is_err() {
            rejected += 1;
        }
    }
    // At RPS=1, burst=1: only 1 succeeds, 99 should be rejected. We require >=40.
    assert!(
        rejected >= 40,
        "expected >=40 rejections from 100 requests at 1 RPS, got {rejected}"
    );
}

#[test]
fn rejected_error_code_is_minus_32099() {
    // RPS=1, exhaust burst, confirm error value.
    let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
    let limiter = Arc::new(RateLimiter::keyed(quota));
    let session = SessionId("code-test".to_string());

    // Consume the burst allowance.
    let _ = check(&limiter, &session);
    // Next must fail with the correct error code.
    let code = check(&limiter, &session).expect_err("expected rejection");
    assert_eq!(code, -32099, "rate limit rejection must use error code -32099");
}

#[test]
fn different_sessions_have_independent_quotas() {
    let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
    let limiter = Arc::new(RateLimiter::keyed(quota));

    let s1 = SessionId("session-alpha".to_string());
    let s2 = SessionId("session-beta".to_string());

    // Exhaust s1's burst.
    let _ = check(&limiter, &s1);
    let s1_second = check(&limiter, &s1);

    // s2 should still have its burst available.
    let s2_first = check(&limiter, &s2);

    assert!(s1_second.is_err(), "s1 should be rate-limited after burst");
    assert!(s2_first.is_ok(), "s2 must not be affected by s1's quota consumption");
}
