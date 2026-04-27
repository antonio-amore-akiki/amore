// observability/mod.rs — 3-signal observability: metrics, traces, structured logs.
//
// W2-2B: Prometheus exporter (metrics.rs)
// W2-2C: OTel OTLP traces + JSON structured logs (tracing.rs, logging.rs)

pub mod logging;
pub mod metrics;
pub mod tracing;
