// observability/metrics.rs — Prometheus metrics exporter (W2-2B).
//
// Installs a Prometheus HTTP listener on AMORE_METRICS_BIND (default 0.0.0.0:9090).
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
/// Default Prometheus scrape bind address.
const METRICS_BIND_DEFAULT: &str = "0.0.0.0:9090";

/// Install the Prometheus exporter. Must be called once before any `counter!` /
/// `histogram!` / `gauge!` macros fire. Panics on duplicate installation (Tokio
/// runtime already running; call from `main` before spawning workers).
pub fn install_prometheus_exporter() -> Result<()> {
    let bind_addr: std::net::SocketAddr = std::env::var(METRICS_BIND_ENV)
        .unwrap_or_else(|_| METRICS_BIND_DEFAULT.to_string())
        .parse()
        .with_context(|| {
            format!(
                "invalid {METRICS_BIND_ENV} — expected host:port (e.g. 0.0.0.0:9090)"
            )
        })?;

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
