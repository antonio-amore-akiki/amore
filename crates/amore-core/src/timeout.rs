// Bounded-time wrapper for dep-call futures.
//
// Production rule: every cross-process call (Ollama embed, Qdrant search,
// rmcp request, etc.) is capped at AMORE_TIMEOUT_MS milliseconds (default
// 4000). Beyond the cap the call returns Err("timeout after Xs") which the
// caller's degraded-path logic surfaces as a structured WARN + the matching
// lane_unavailable flag, instead of hanging the caller's whole request.
// Legacy OBELION_TIMEOUT_MS is accepted with a deprecation warning.
//
// QA gate: B3 (network timeout / slow dep) — see addendum.

use anyhow::Result;
use std::time::Duration;

/// Default per-call timeout in milliseconds. Overridable via env.
pub const DEFAULT_TIMEOUT_MS: u64 = 4000;

/// Parse a timeout value (from env or any string source) into Duration.
/// Floors at 100 ms to prevent pathological 0-cap configs. Public so tests
/// can hit it directly without mutating shared env state.
pub fn parse_timeout(raw: Option<&str>) -> Duration {
    let ms = raw
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS);
    Duration::from_millis(ms.max(100))
}

/// Resolve the active timeout from `AMORE_TIMEOUT_MS` env.
/// Accepts legacy `OBELION_TIMEOUT_MS` with a deprecation warning (removed v0.4.0).
pub fn resolve_timeout() -> Duration {
    if let Ok(v) = std::env::var("AMORE_TIMEOUT_MS") {
        return parse_timeout(Some(v.as_str()));
    }
    if let Ok(v) = std::env::var("OBELION_TIMEOUT_MS") {
        tracing::warn!(
            "deprecated: OBELION_TIMEOUT_MS — use AMORE_TIMEOUT_MS instead (removed in v0.4.0)"
        );
        return parse_timeout(Some(v.as_str()));
    }
    parse_timeout(None)
}

/// Cap a Result-producing future at `d`. On elapsed, return Err with the
/// elapsed duration — caller's match handles it as any other Err.
pub async fn with_timeout<F, T>(d: Duration, fut: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    match tokio::time::timeout(d, fut).await {
        Ok(inner) => inner,
        Err(_) => anyhow::bail!("timeout after {:?}", d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn under_cap_returns_ok() {
        let r: Result<u32> = with_timeout(Duration::from_secs(1), async { Ok(42_u32) }).await;
        assert_eq!(r.unwrap(), 42);
    }

    #[tokio::test]
    async fn over_cap_returns_timeout_err() {
        let r: Result<u32> = with_timeout(Duration::from_millis(50), async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(0_u32)
        })
        .await;
        let err = r.expect_err("must time out");
        assert!(format!("{err}").contains("timeout"));
    }

    #[test]
    fn parse_timeout_clamps_to_floor() {
        // 0 ms is pathological — floor must clamp to 100 ms.
        assert_eq!(parse_timeout(Some("0")), Duration::from_millis(100));
        assert_eq!(parse_timeout(Some("50")), Duration::from_millis(100));
    }

    #[test]
    fn parse_timeout_respects_value() {
        assert_eq!(parse_timeout(Some("7777")), Duration::from_millis(7777));
    }

    #[test]
    fn parse_timeout_falls_back_to_default() {
        // None (env unset) or garbage value -> default 4000 ms.
        assert_eq!(
            parse_timeout(None),
            Duration::from_millis(DEFAULT_TIMEOUT_MS)
        );
        assert_eq!(
            parse_timeout(Some("not-a-number")),
            Duration::from_millis(DEFAULT_TIMEOUT_MS)
        );
    }
}
