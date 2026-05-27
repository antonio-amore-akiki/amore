// observability/metrics.rs — Prometheus metrics exporter (W2-2B).
//
// Installs a Prometheus HTTP listener on AMORE_METRICS_BIND (default 127.0.0.1:9090).
// Non-loopback binds require AMORE_METRICS_ALLOW_NETWORK=1 (mirrors C1 health gate).
// All metric names follow OTel db.* semantic-convention naming.
//
// Metric registry:
//   amore_db_operation_duration_seconds{operation,namespace}  histogram
//   amore_db_operation_total{operation,namespace,status}      counter
//   amore_cache_hit_ratio{level}                              gauge
//   amore_wal_writes_total                                    counter
//   amore_wal_fsync_duration_seconds                          histogram
//   amore_circuit_breaker_state{dependency}                   gauge  (0=closed,1=half-open,2=open)
//   amore_qdrant_pool_checkout_duration_seconds               histogram
//   amore_rate_limit_rejected_total{session}                  counter  (wired by rate-limit middleware)
//   amore_shutdown_initiated_total                            counter  (wired by shutdown handler)

use anyhow::{Context, Result};
use metrics_exporter_prometheus::PrometheusBuilder;

/// Bind address env key for the Prometheus scrape endpoint.
const METRICS_BIND_ENV: &str = "AMORE_METRICS_BIND";
/// Default Prometheus scrape bind address — loopback only.
const METRICS_BIND_DEFAULT: &str = "127.0.0.1:9090";
/// Env opt-in to allow non-loopback metrics binds (mirrors AMORE_HEALTH_ALLOW_NETWORK).
const METRICS_ALLOW_NETWORK_ENV: &str = "AMORE_METRICS_ALLOW_NETWORK";

/// Parse and validate the metrics bind address.
///
/// Default: `127.0.0.1:9090` (loopback only).
/// Override via `AMORE_METRICS_BIND`. Non-loopback binds require
/// `AMORE_METRICS_ALLOW_NETWORK=1` (mirrors `parse_health_bind` — ADR 0007 + 0009).
pub fn parse_metrics_bind() -> Result<std::net::SocketAddr> {
    let raw = std::env::var(METRICS_BIND_ENV)
        .unwrap_or_else(|_| METRICS_BIND_DEFAULT.to_string());

    let addr: std::net::SocketAddr = raw.parse().with_context(|| {
        format!("invalid {METRICS_BIND_ENV} — expected host:port (e.g. 127.0.0.1:9090)")
    })?;

    if !addr.ip().is_loopback()
        && std::env::var(METRICS_ALLOW_NETWORK_ENV).as_deref() != Ok("1")
    {
        return Err(anyhow::anyhow!(
            "non-loopback metrics bind ({addr}) is blocked by default. \
             Set AMORE_METRICS_ALLOW_NETWORK=1 to allow (ADR 0007 + 0009)."
        ));
    }

    if !addr.ip().is_loopback() {
        tracing::warn!(
            bind = %addr,
            "Prometheus metrics endpoint is bound to a non-loopback address — \
             AMORE_METRICS_ALLOW_NETWORK=1 is set. Cache stats, WAL counters, and \
             breaker state are now reachable from the LAN. Ensure this is intentional."
        );
    }

    Ok(addr)
}

/// Install the Prometheus exporter. Must be called once before any `counter!` /
/// `histogram!` / `gauge!` macros fire. Panics on duplicate installation (Tokio
/// runtime already running; call from `main` before spawning workers).
pub fn install_prometheus_exporter() -> Result<()> {
    let bind_addr = parse_metrics_bind()?;

    PrometheusBuilder::new()
        .with_http_listener(bind_addr)
        .install()
        .with_context(|| format!("failed to install Prometheus exporter on {bind_addr}"))?;

    tracing::info!(
        bind = %bind_addr,
        "prometheus exporter installed — scrape at http://{}/metrics", bind_addr
    );
    Ok(())
}

/// Describe all amore metrics so Prometheus scrapes include TYPE/HELP headers
/// even before the first observation. Call once after `install_prometheus_exporter`.
pub fn describe_metrics() {
    use metrics::{describe_counter, describe_gauge, describe_histogram, Unit};

    describe_histogram!(
        "amore_db_operation_duration_seconds",
        Unit::Seconds,
        "Duration of amore database operations (vector search, BM25, upsert, delete)"
    );
    describe_counter!(
        "amore_db_operation_total",
        "Total number of amore database operations by operation, namespace, and status"
    );
    describe_gauge!(
        "amore_cache_hit_ratio",
        "Cache hit ratio per level (l1_moka, l2_sled)"
    );
    describe_counter!(
        "amore_wal_writes_total",
        "Total WAL append calls (durable fsync writes)"
    );
    describe_histogram!(
        "amore_wal_fsync_duration_seconds",
        Unit::Seconds,
        "Duration of WAL fsync calls"
    );
    describe_gauge!(
        "amore_circuit_breaker_state",
        "Circuit breaker state per dependency: 0=closed, 1=half-open, 2=open"
    );
    describe_histogram!(
        "amore_qdrant_pool_checkout_duration_seconds",
        Unit::Seconds,
        "Duration of Qdrant bb8 pool checkout calls"
    );
    describe_counter!(
        "amore_rate_limit_rejected_total",
        "Total MCP requests rejected by the per-session rate limiter"
    );
    describe_counter!(
        "amore_shutdown_initiated_total",
        "Total graceful shutdown sequences initiated"
    );
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Global mutex to serialize tests that mutate process-level env vars.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Default (no env vars set) must resolve to loopback 127.0.0.1:9090.
    #[test]
    fn default_bind_is_loopback() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized via ENV_LOCK; no concurrent env mutation in this scope.
        unsafe {
            std::env::remove_var(METRICS_BIND_ENV);
            std::env::remove_var(METRICS_ALLOW_NETWORK_ENV);
        }
        let addr = parse_metrics_bind().expect("default parse must succeed");
        assert!(addr.ip().is_loopback(), "default bind must be loopback, got {addr}");
        assert_eq!(addr.port(), 9090);
        // SAFETY: same scope.
        unsafe { std::env::remove_var(METRICS_BIND_ENV); }
    }

    /// AMORE_METRICS_ALLOW_NETWORK=1 must permit a non-loopback address.
    #[test]
    fn allow_network_unlocks_nonloopback() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized via ENV_LOCK.
        unsafe {
            std::env::set_var(METRICS_BIND_ENV, "0.0.0.0:9090");
            std::env::set_var(METRICS_ALLOW_NETWORK_ENV, "1");
        }
        let result = parse_metrics_bind();
        // SAFETY: clean up before asserting.
        unsafe {
            std::env::remove_var(METRICS_BIND_ENV);
            std::env::remove_var(METRICS_ALLOW_NETWORK_ENV);
        }
        let addr = result.expect("non-loopback with opt-in env must succeed");
        assert!(!addr.ip().is_loopback(), "opt-in must allow non-loopback");
        assert_eq!(addr.port(), 9090);
    }

    /// Without the opt-in, a non-loopback address must be rejected.
    #[test]
    fn nonloopback_without_optin_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized via ENV_LOCK.
        unsafe {
            std::env::set_var(METRICS_BIND_ENV, "0.0.0.0:9090");
            std::env::remove_var(METRICS_ALLOW_NETWORK_ENV);
        }
        let result = parse_metrics_bind();
        // SAFETY: clean up.
        unsafe { std::env::remove_var(METRICS_BIND_ENV); }
        assert!(result.is_err(), "non-loopback without opt-in must be rejected");
    }
}
