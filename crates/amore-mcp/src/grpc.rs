// grpc.rs — gRPC transport surface for amore-mcp (Phase H.6, ADR 0009).
//
// AmoreServiceImpl wraps the same Arc<HybridRecall> used by the stdio MCP
// path, ensuring both transports share a single retrieval backend with no
// duplicated state.
//
// v0.4.x scope (ADR 0009):
//   • Recall    — wired end-to-end via HybridRecall::search
//   • Health    — wired end-to-end (liveness + uptime)
//   • CanonicalDocLookup — unimplemented (TODO v0.5.0 ticket #amore-grpc-canonical)
//   • ProvenanceVerify   — unimplemented (TODO v0.5.0 ticket #amore-grpc-provenance)
//
// Rate limiting: simple in-memory token bucket per connection (100 req/min
// default; AMORE_GRPC_RATE_LIMIT_RPM env overrides). Bucket exhaustion returns
// Status::resource_exhausted — no silent fail-open (CLAUDE.md hard gate).

use std::sync::Arc;
use std::time::Instant;

use amore_core::ollama::OllamaClient;
use amore_core::recall::{Embedder, HybridRecall};
use tonic::{Request, Response, Status};

// Include the tonic-generated code from OUT_DIR.
pub mod proto {
    tonic::include_proto!("amore");
}

pub use proto::amore_service_server::{AmoreService, AmoreServiceServer};
use proto::{
    CanonicalDocRequest, CanonicalDocResponse, HealthRequest, HealthResponse,
    ProvenanceVerifyRequest, ProvenanceVerifyResponse, RecallHit, RecallRequest, RecallResponse,
};

// ── Rate-limit token bucket ───────────────────────────────────────────────────
//
// One bucket per server instance (shared across all connections in this
// process). A per-connection bucket would require Tower middleware; the simpler
// per-process bucket is sufficient for Phase H.6's DoS-resistance goal while
// staying under the ~30 LOC budget.
//
// Algorithm: leaky-bucket refill. `tokens` replenishes at `refill_per_ms`
// per millisecond. A request consumes 1 token. If `tokens < 1.0` the
// bucket is exhausted and we return resource_exhausted.

struct RateBucket {
    tokens: f64,
    max_tokens: f64,
    refill_per_ms: f64,
    last_refill: Instant,
}

impl RateBucket {
    fn new(rpm: u32) -> Self {
        let max = rpm as f64;
        Self {
            tokens: max,
            max_tokens: max,
            // Refill the full bucket over 60 000 ms.
            refill_per_ms: max / 60_000.0,
            last_refill: Instant::now(),
        }
    }

    /// Returns Ok(()) if a token is available, Err with retry-in seconds
    /// when the bucket is exhausted.
    fn try_consume(&mut self) -> Result<(), u64> {
        let now = Instant::now();
        let elapsed_ms = now.duration_since(self.last_refill).as_millis() as f64;
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed_ms * self.refill_per_ms).min(self.max_tokens);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            // Seconds until one token refills.
            let wait_secs = ((1.0 - self.tokens) / self.refill_per_ms / 1000.0).ceil() as u64;
            Err(wait_secs.max(1))
        }
    }
}

// ── Server start time (monotonic) for Health.uptime_ms ───────────────────────

static SERVER_START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Call once when the gRPC server is spawned. Records the monotonic start time
/// used by the Health RPC's `uptime_ms` field.
pub fn record_start_time() {
    SERVER_START.get_or_init(Instant::now);
}

fn uptime_ms() -> u64 {
    SERVER_START
        .get()
        .map(|t| t.elapsed().as_millis() as u64)
        .unwrap_or(0)
}

// ── AmoreServiceImpl ─────────────────────────────────────────────────────────
//
// Generic over the embedder type E so tests can wire a no-op embedder while
// production wires `OllamaClient` (the default). All call sites that pass a
// concrete embedder rely on type inference; the default `= OllamaClient`
// preserves the historical `Arc<HybridRecall>` ergonomics in main.rs.

pub struct AmoreServiceImpl<E: Embedder = OllamaClient> {
    recall: Arc<HybridRecall<E>>,
    rate_bucket: std::sync::Mutex<RateBucket>,
}

impl<E: Embedder + Send + Sync + 'static> AmoreServiceImpl<E> {
    /// Construct from the same HybridRecall arc used by the stdio MCP server.
    /// `rpm` is the per-process request-per-minute cap (default 100; env
    /// `AMORE_GRPC_RATE_LIMIT_RPM` overrides in `main.rs`).
    pub fn new(recall: Arc<HybridRecall<E>>, rpm: u32) -> Self {
        Self {
            recall,
            rate_bucket: std::sync::Mutex::new(RateBucket::new(rpm)),
        }
    }

    /// Enforce rate limit. Returns Err(Status) on exhaustion.
    fn check_rate(&self) -> Result<(), Status> {
        let mut bucket = self
            .rate_bucket
            .lock()
            .expect("rate_bucket Mutex poisoned — process should exit");
        bucket.try_consume().map_err(|wait_secs| {
            Status::resource_exhausted(format!(
                "rate limit exceeded; try again in {wait_secs} second(s)"
            ))
        })
    }
}

#[tonic::async_trait]
impl<E: Embedder + Send + Sync + 'static> AmoreService for AmoreServiceImpl<E> {
    async fn recall(
        &self,
        request: Request<RecallRequest>,
    ) -> Result<Response<RecallResponse>, Status> {
        self.check_rate()?;

        let req = request.into_inner();

        // Mirror the MCP handler's 16 KiB + top_k clamping logic so the
        // gRPC surface has the same security envelope (Security fix 6a).
        const MAX_QUERY_BYTES: usize = 16 * 1024;
        const MAX_TOP_K: u32 = 100;

        if req.query.len() > MAX_QUERY_BYTES {
            return Err(Status::invalid_argument(format!(
                "query exceeds {MAX_QUERY_BYTES} bytes (got {})",
                req.query.len()
            )));
        }
        let top_k = req.top_k.clamp(1, MAX_TOP_K) as usize;

        let envelope = self
            .recall
            .search(&req.query, top_k)
            .await
            .map_err(|e| Status::internal(format!("recall failed: {e}")))?;

        let hits: Vec<RecallHit> = envelope
            .hits
            .into_iter()
            .map(|h| RecallHit {
                doc_id: h.id,
                score: h.score as f64,
                excerpt: h.text,
                source: h.source,
            })
            .collect();

        let degraded = envelope.degraded.ollama_unavailable
            || envelope.degraded.qdrant_unavailable
            || envelope.degraded.bm25_unavailable;

        Ok(Response::new(RecallResponse { hits, degraded }))
    }

    async fn canonical_doc_lookup(
        &self,
        _request: Request<CanonicalDocRequest>,
    ) -> Result<Response<CanonicalDocResponse>, Status> {
        // TODO(v0.5.0): wire to CanonicalDocsRouter — ticket #amore-grpc-canonical
        Err(Status::unimplemented(
            "CanonicalDocLookup is scheduled for v0.5.0 (#amore-grpc-canonical)",
        ))
    }

    async fn provenance_verify(
        &self,
        _request: Request<ProvenanceVerifyRequest>,
    ) -> Result<Response<ProvenanceVerifyResponse>, Status> {
        // TODO(v0.5.0): wire to sha2 provenance envelope — ticket #amore-grpc-provenance
        Err(Status::unimplemented(
            "ProvenanceVerify is scheduled for v0.5.0 (#amore-grpc-provenance)",
        ))
    }

    async fn health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        self.check_rate()?;
        Ok(Response::new(HealthResponse {
            status: "ok".to_string(),
            uptime_ms: uptime_ms(),
        }))
    }
}

// ── Helper: resolve RPM cap from env ─────────────────────────────────────────

/// Read `AMORE_GRPC_RATE_LIMIT_RPM` env; fall back to 100 rpm.
/// Invalid values log a warning and use the default.
pub fn resolve_rpm() -> u32 {
    match std::env::var("AMORE_GRPC_RATE_LIMIT_RPM") {
        Ok(v) => v.parse::<u32>().unwrap_or_else(|_| {
            tracing::warn!(
                "AMORE_GRPC_RATE_LIMIT_RPM={v:?} is not a valid u32; using default 100 rpm"
            );
            100
        }),
        Err(_) => 100,
    }
}

// ── Helper: resolve listener address from CLI/env ────────────────────────────

/// Listener kinds supported by the gRPC transport.
#[derive(Debug, Clone)]
pub enum GrpcListener {
    /// TCP socket. Only loopback is allowed unless AMORE_GRPC_ALLOW_NETWORK=1.
    Tcp(std::net::SocketAddr),

    /// Windows named pipe path (e.g. `\\.\pipe\amore-mcp`).
    NamedPipe(String),
}

/// Parse `--grpc-listen` value (or the platform default) into a `GrpcListener`.
///
/// Accepted forms:
/// - `tcp://127.0.0.1:PORT`   — loopback TCP (always allowed)
/// - `tcp://HOST:PORT`        — non-loopback; requires `AMORE_GRPC_ALLOW_NETWORK=1`
/// - `\\.\pipe\NAME`          — Windows named pipe
/// - absent / empty           — platform default
///
/// Returns Err(Status) on a non-loopback bind without the env opt-in (ADR 0007).
pub fn parse_grpc_listen(raw: Option<&str>) -> Result<GrpcListener, String> {
    let effective = match raw {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => default_grpc_listen(),
    };

    if effective.starts_with("tcp://") {
        let addr_str = effective.trim_start_matches("tcp://");
        let addr: std::net::SocketAddr = addr_str
            .parse()
            .map_err(|e| format!("invalid tcp address {addr_str:?}: {e}"))?;

        if !addr.ip().is_loopback()
            && std::env::var("AMORE_GRPC_ALLOW_NETWORK").as_deref() != Ok("1")
        {
            return Err(format!(
                "non-loopback gRPC bind ({addr}) is blocked by default. \
                 Set AMORE_GRPC_ALLOW_NETWORK=1 to allow (ADR 0007 + 0009)."
            ));
        }
        return Ok(GrpcListener::Tcp(addr));
    }

    if effective.starts_with(r"\\.\pipe\") {
        return Ok(GrpcListener::NamedPipe(effective));
    }

    Err(format!(
        "unrecognised --grpc-listen format: {effective:?}. \
         Expected tcp://HOST:PORT or \\\\.\\\\.\\pipe\\NAME"
    ))
}

/// Platform-specific default listener (local-only per ADR 0009).
fn default_grpc_listen() -> String {
    // On Windows we default to a named pipe; on other platforms a Unix socket
    // path is used (not yet wired as a listener type here — v0.5.0 scope).
    // For the v0.4.x skeleton we use TCP loopback on a well-known port so
    // the smoke test works on all CI platforms (Linux, macOS, Windows).
    if cfg!(target_os = "windows") {
        r"\\.\pipe\amore-mcp".to_string()
    } else {
        // Unix socket path omitted for v0.4.x; fall through to TCP loopback.
        // TODO(v0.5.0): switch to unix socket via tokio::net::UnixListener
        "tcp://127.0.0.1:7878".to_string()
    }
}

/// Convenience: parse the default loopback TCP address for smoke tests.
pub fn loopback_tcp(port: u16) -> GrpcListener {
    GrpcListener::Tcp(std::net::SocketAddr::from(([127, 0, 0, 1], port)))
}
