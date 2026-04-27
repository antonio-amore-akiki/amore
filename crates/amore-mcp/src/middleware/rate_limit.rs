// middleware/rate_limit.rs — per-session governor rate-limiter (W3-3B).
//
// Keyed by SessionId (String). Default 50 RPS per session; env override
// AMORE_RATE_LIMIT_RPS (must be non-zero).
//
// `check_rate_limit` is called per MCP request. Returns Ok(()) when under limit,
// or Err(McpError) with code -32099 on excess. Increments
// `amore_rate_limit_rejected_total{session}` on each rejection.

use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};
use metrics::counter;
use rmcp::ErrorData as McpError;
use std::num::NonZeroU32;
use std::sync::Arc;

/// Env key for per-session RPS limit.
const RATE_LIMIT_RPS_ENV: &str = "AMORE_RATE_LIMIT_RPS";
/// Default RPS per session when env var is absent.
const RATE_LIMIT_RPS_DEFAULT: u32 = 50;

/// Newtype wrapper for session identifier used as the rate-limit key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Build a keyed rate-limiter from env config.
///
/// Reads `AMORE_RATE_LIMIT_RPS` (default 50). Fails if the value is 0.
pub fn build_rate_limiter() -> anyhow::Result<Arc<DefaultKeyedRateLimiter<SessionId>>> {
    let rps_raw: u32 = std::env::var(RATE_LIMIT_RPS_ENV)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(RATE_LIMIT_RPS_DEFAULT);

    let rps = NonZeroU32::new(rps_raw).ok_or_else(|| {
        anyhow::anyhow!(
            "{RATE_LIMIT_RPS_ENV}=0 is invalid — must be ≥1 RPS"
        )
    })?;

    let quota = Quota::per_second(rps);
    let limiter = RateLimiter::keyed(quota);
    tracing::info!(rps = rps_raw, "rate limiter configured");
    Ok(Arc::new(limiter))
}

/// Check the per-session rate limit.
///
/// Returns Ok(()) when the request is within quota.
/// Returns Err with MCP error code -32099 on excess + increments the rejected counter.
pub fn check_rate_limit(
    limiter: &DefaultKeyedRateLimiter<SessionId>,
    session: &SessionId,
) -> Result<(), McpError> {
    match limiter.check_key(session) {
        Ok(_) => Ok(()),
        Err(_not_until) => {
            counter!(
                "amore_rate_limit_rejected_total",
                "session" => session.0.clone(),
            )
            .increment(1);
            Err(McpError {
                code: rmcp::model::ErrorCode(-32099),
                message: "rate limit exceeded — retry after 1s".into(),
                data: None,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_rate_limiter_uses_default_when_env_absent() {
        // Env var not set in test environment — should not fail.
        let limiter = build_rate_limiter().expect("build");
        // A single check must succeed under a 50 RPS limit.
        let session = SessionId("test-session".to_string());
        assert!(check_rate_limit(&limiter, &session).is_ok());
    }

    #[test]
    fn rate_limiter_rejects_after_burst() {
        // Set a very low limit (1 RPS) to trigger rejection quickly.
        // We override via direct construction rather than env var to avoid
        // test isolation issues.
        let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
        let limiter = Arc::new(RateLimiter::keyed(quota));
        let session = SessionId("burst-test".to_string());

        // The first call should succeed (burst capacity = 1 for Quota::per_second(1)).
        let first = check_rate_limit(&limiter, &session);
        // Immediately fire 5 more — at least one must fail.
        let mut rejected = 0usize;
        for _ in 0..5 {
            if check_rate_limit(&limiter, &session).is_err() {
                rejected += 1;
            }
        }
        // First allowed; at least some of the burst rejected.
        assert!(first.is_ok(), "first call should succeed");
        assert!(rejected >= 1, "expected at least 1 rejection from burst, got {rejected}");
    }

    #[test]
    fn rejected_error_has_code_minus_32099() {
        let quota = Quota::per_second(NonZeroU32::new(1).unwrap());
        let limiter = Arc::new(RateLimiter::keyed(quota));
        let session = SessionId("error-code-test".to_string());
        // Exhaust burst
        let _ = check_rate_limit(&limiter, &session);
        // Next should fail
        let err = check_rate_limit(&limiter, &session)
            .expect_err("expected rejection");
        assert_eq!(err.code.0, -32099);
    }
}
