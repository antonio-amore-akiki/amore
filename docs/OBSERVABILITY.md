---
stable: true
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
---
# Amore Observability

## Prometheus Metrics (W2-2B)

Scrape endpoint: `AMORE_METRICS_BIND` (default `0.0.0.0:9090`), path `/metrics`.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `amore_db_operation_duration_seconds` | histogram | operation, namespace | DB operation latency |
| `amore_db_operation_total` | counter | operation, namespace, status | DB operation count |
| `amore_cache_hit_ratio` | gauge | level (l1_moka, l2_sled) | Cache hit ratio |
| `amore_wal_writes_total` | counter | — | WAL append + fsync calls |
| `amore_wal_fsync_duration_seconds` | histogram | — | WAL fsync latency |
| `amore_circuit_breaker_state` | gauge | dependency | 0=closed, 1=half-open, 2=open |
| `amore_qdrant_pool_checkout_duration_seconds` | histogram | — | bb8 pool checkout latency |
| `amore_rate_limit_rejected_total` | counter | session | Rate-limited MCP requests |
| `amore_shutdown_initiated_total` | counter | — | Graceful shutdown count |

## OTel Traces (W2-2C)

Set `OTEL_EXPORTER_OTLP_ENDPOINT` to activate OTLP export (no-op if unset).

Resource attributes: `service.name=amore`, `service.version`, `service.instance.id`, `service.namespace=ai-memory`.

Instrumented hot paths: `HybridRecall::search`, `Wal::append`, `CompactionWorker::compact_once`.

## Structured Logs (W2-2C)

Log format: `AMORE_LOG_FORMAT=json|text` (default `json` in release, `text` in debug).

JSON logs include `trace_id` + `span_id` when OTel is active.

## Health Endpoints (W2-2D)

Bind: `AMORE_HEALTH_BIND` (default `0.0.0.0:9091`).

- `GET /healthz` — 200 if process alive.
- `GET /readyz` — 200 if `wal_replayed` AND `warmed_up`; 503 otherwise.

## Resilience (W3-3B)

- **SIGTERM**: `tokio::select!` on Ctrl-C + SIGTERM (Unix) + Ctrl-Break (Windows). 30-second drain window.
- **Rate limit**: `AMORE_RATE_LIMIT_RPS` (default 50 RPS per session). Excess returns MCP error -32099.
- **Pool tuning**: `AMORE_POOL_MAX_SIZE` (default 16), `AMORE_POOL_MIN_IDLE` (default 4), `AMORE_POOL_IDLE_TIMEOUT_SEC` (default 600).
