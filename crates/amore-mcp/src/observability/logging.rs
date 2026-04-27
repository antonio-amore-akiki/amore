// observability/logging.rs — structured logging layer wiring (W2-2C).
//
// Builds a tracing-subscriber Registry with:
//   - JSON fmt layer (default in release, text in debug, overridden by AMORE_LOG_FORMAT)
//   - OTel layer (added by caller if `otel_provider` is Some)
//
// The caller MUST pass the OTel layer from tracing.rs so trace_id/span_id appear
// inside JSON log events when the OTel tracer is active.
//
// AMORE_LOG_FORMAT=json|text (default: json in release, text in debug)

use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

/// Log format selected at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Text,
}

/// Resolve log format from env or build profile.
pub fn resolve_log_format() -> LogFormat {
    match std::env::var("AMORE_LOG_FORMAT")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "json" => LogFormat::Json,
        "text" => LogFormat::Text,
        _ => {
            #[cfg(debug_assertions)]
            {
                LogFormat::Text
            }
            #[cfg(not(debug_assertions))]
            {
                LogFormat::Json
            }
        }
    }
}

/// Install the global tracing subscriber.
///
/// `otel_provider` — when Some, an OTel tracing layer is inserted so all
/// structured log events carry `trace_id` + `span_id` fields.
pub fn install_logging_subscriber(
    otel_provider: Option<&opentelemetry_sdk::trace::TracerProvider>,
) {
    use tracing_subscriber::util::SubscriberInitExt;

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let format = resolve_log_format();

    match (format, otel_provider) {
        (LogFormat::Json, Some(provider)) => {
            use opentelemetry::trace::TracerProvider as _;
            let tracer = provider.tracer("amore");
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_writer(std::io::stderr)
                .with_span_events(FmtSpan::NONE);
            Registry::default()
                .with(env_filter)
                .with(otel_layer)
                .with(json_layer)
                .init();
        }
        (LogFormat::Json, None) => {
            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true)
                .with_writer(std::io::stderr)
                .with_span_events(FmtSpan::NONE);
            Registry::default()
                .with(env_filter)
                .with(json_layer)
                .init();
        }
        (LogFormat::Text, Some(provider)) => {
            use opentelemetry::trace::TracerProvider as _;
            let tracer = provider.tracer("amore");
            let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            let text_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .with_span_events(FmtSpan::NONE);
            Registry::default()
                .with(env_filter)
                .with(otel_layer)
                .with(text_layer)
                .init();
        }
        (LogFormat::Text, None) => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .with_writer(std::io::stderr)
                .with_ansi(false)
                .init();
        }
    }
}
