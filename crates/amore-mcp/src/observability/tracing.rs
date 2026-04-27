// observability/tracing.rs — OTel OTLP trace exporter (W2-2C).
//
// Initialises an OpenTelemetry tracer backed by OTLP exporter when
// OTEL_EXPORTER_OTLP_ENDPOINT is set; no-ops (no tracer installed) when unset.
//
// Resource attributes:
//   service.name          = "amore"
//   service.version       = CARGO_PKG_VERSION
//   service.instance.id   = hostname (best-effort)
//   service.namespace     = "ai-memory"
//   host.os.type          = "linux" | "windows" | "macos"
//
// Returns `Some(tracer_provider)` when OTel is active so the caller can call
// `provider.shutdown()` on graceful exit.  Returns `None` when OTLP endpoint
// is not configured.
//
// API uses opentelemetry-otlp 0.27 stable builder pattern:
//   SpanExporter::builder().with_tonic().build()
//   TracerProvider::builder().with_batch_exporter(...)

use anyhow::{Context, Result};
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{Resource, runtime, trace as sdktrace};

/// Build and return an OTel tracer provider if OTEL_EXPORTER_OTLP_ENDPOINT is set.
/// Returns None when the env var is absent (no-op path).
pub fn init_otel_tracer() -> Result<Option<opentelemetry_sdk::trace::TracerProvider>> {
    let endpoint = match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            tracing::debug!("OTEL_EXPORTER_OTLP_ENDPOINT not set — OTel traces disabled");
            return Ok(None);
        }
    };

    let hostname = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "unknown".to_string());

    let os_type = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unknown"
    };

    let resource = Resource::new(vec![
        KeyValue::new("service.name", "amore"),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        KeyValue::new("service.instance.id", hostname),
        KeyValue::new("service.namespace", "ai-memory"),
        KeyValue::new("host.os.type", os_type),
    ]);

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&endpoint)
        .build()
        .with_context(|| format!("failed to build OTel SpanExporter for {endpoint}"))?;

    let provider = sdktrace::TracerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter, runtime::Tokio)
        .build();

    tracing::info!(
        endpoint = %endpoint,
        "OTel OTLP trace exporter active"
    );
    Ok(Some(provider))
}
