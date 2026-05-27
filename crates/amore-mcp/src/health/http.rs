// health/http.rs — Axum HTTP healthz/readyz sidecar (W2-2D).
//
// Listens on AMORE_HEALTH_BIND (default 127.0.0.1:9091 — loopback only):
//   GET /healthz  -> 200 always (process liveness)
//   GET /readyz   -> 200 if all ReadyState flags true; 503 otherwise
//
// Non-loopback binds require AMORE_HEALTH_ALLOW_NETWORK=1 (mirrors
// AMORE_GRPC_ALLOW_NETWORK gate from grpc.rs — ADR 0007 + 0009).
//
// ReadyState flags (all AtomicBool, set by runtime after each phase completes):
//   wal_replayed  — WAL replay complete
//   warmed_up     — first warmup query complete
//
// The Axum server is spawned as a background task; its JoinHandle is returned
// so the caller can abort it during graceful shutdown.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Router, routing::get};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::task::JoinHandle;

/// Shared readiness state threaded through the axum router.
pub struct ReadyState {
    /// WAL replay has completed. Flipped by main after `Wal::iter_from` finishes.
    pub wal_replayed: AtomicBool,
    /// First warmup query succeeded. Flipped by main after initial recall probe.
    pub warmed_up: AtomicBool,
}

impl ReadyState {
    pub fn new() -> Self {
        Self {
            wal_replayed: AtomicBool::new(false),
            warmed_up: AtomicBool::new(false),
        }
    }

    /// Returns true when all readiness gates have passed.
    pub fn is_ready(&self) -> bool {
        self.wal_replayed.load(Ordering::Acquire)
            && self.warmed_up.load(Ordering::Acquire)
    }
}

impl Default for ReadyState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Axum handlers
// ---------------------------------------------------------------------------

/// /healthz — always 200 if the process is alive.
async fn healthz_handler() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// /readyz — 200 when all ReadyState gates are true; 503 with detail otherwise.
async fn readyz_handler(
    axum::extract::State(state): axum::extract::State<Arc<ReadyState>>,
) -> impl IntoResponse {
    if state.is_ready() {
        (StatusCode::OK, "ready").into_response()
    } else {
        let wal = state.wal_replayed.load(Ordering::Acquire);
        let warm = state.warmed_up.load(Ordering::Acquire);
        let detail = format!(
            "not ready: wal_replayed={wal} warmed_up={warm}"
        );
        (StatusCode::SERVICE_UNAVAILABLE, detail).into_response()
    }
}

/// Parse and validate the health server bind address.
///
/// Default: `127.0.0.1:9091` (loopback only).
/// Override via `AMORE_HEALTH_BIND`. Non-loopback binds require
/// `AMORE_HEALTH_ALLOW_NETWORK=1` (mirrors `parse_grpc_listen` — ADR 0007 + 0009).
pub fn parse_health_bind() -> anyhow::Result<std::net::SocketAddr> {
    let raw = std::env::var("AMORE_HEALTH_BIND")
        .unwrap_or_else(|_| "127.0.0.1:9091".to_string());

    let addr: std::net::SocketAddr = raw.parse().map_err(|e| {
        anyhow::anyhow!("invalid AMORE_HEALTH_BIND — expected host:port: {e}")
    })?;

    if !addr.ip().is_loopback()
        && std::env::var("AMORE_HEALTH_ALLOW_NETWORK").as_deref() != Ok("1")
    {
        return Err(anyhow::anyhow!(
            "non-loopback health bind ({addr}) is blocked by default. \
             Set AMORE_HEALTH_ALLOW_NETWORK=1 to allow (ADR 0007 + 0009)."
        ));
    }

    if !addr.ip().is_loopback() {
        tracing::warn!(
            bind = %addr,
            "health sidecar is bound to a non-loopback address — \
             AMORE_HEALTH_ALLOW_NETWORK=1 is set. Ensure this is intentional."
        );
    }

    Ok(addr)
}

/// Spawn the healthz/readyz Axum sidecar.
///
/// Returns the `JoinHandle` so the caller can abort during graceful shutdown.
pub async fn spawn_health_server(state: Arc<ReadyState>) -> anyhow::Result<JoinHandle<()>> {
    let bind_addr = parse_health_bind()?;

    let router = Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/readyz", get(readyz_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| anyhow::anyhow!("health server bind {bind_addr} failed: {e}"))?;

    tracing::info!(bind = %bind_addr, "health sidecar listening");

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!(error = %e, "health sidecar exited with error");
        }
    });
    Ok(handle)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::Mutex;
    use tower::ServiceExt;

    /// Global mutex to serialize tests that mutate process-level env vars.
    /// Rust runs tests in parallel threads; env mutations are process-wide.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_router(state: Arc<ReadyState>) -> Router {
        Router::new()
            .route("/healthz", get(healthz_handler))
            .route("/readyz", get(readyz_handler))
            .with_state(state)
    }

    #[tokio::test]
    async fn healthz_always_200() {
        let state = Arc::new(ReadyState::new());
        let app = test_router(state);
        let req = Request::builder().uri("/healthz").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_503_before_warmup() {
        let state = Arc::new(ReadyState::new());
        let app = test_router(state);
        let req = Request::builder().uri("/readyz").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn readyz_200_after_warmup() {
        let state = Arc::new(ReadyState::new());
        state.wal_replayed.store(true, Ordering::Release);
        state.warmed_up.store(true, Ordering::Release);
        let app = test_router(state);
        let req = Request::builder().uri("/readyz").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readyz_503_wal_not_replayed() {
        let state = Arc::new(ReadyState::new());
        // warmed_up = true but wal_replayed = false -> still not ready
        state.warmed_up.store(true, Ordering::Release);
        let app = test_router(state);
        let req = Request::builder().uri("/readyz").body(Body::empty()).unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // ── parse_health_bind tests ───────────────────────────────────────────────

    /// Default (no env vars set) must resolve to loopback 127.0.0.1:9091.
    #[test]
    fn default_bind_is_loopback() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized via ENV_LOCK; no concurrent env mutation in this scope.
        unsafe {
            std::env::remove_var("AMORE_HEALTH_BIND");
            std::env::remove_var("AMORE_HEALTH_ALLOW_NETWORK");
        }

        let addr = parse_health_bind().expect("default parse must succeed");
        assert!(addr.ip().is_loopback(), "default bind must be loopback");
        assert_eq!(addr.port(), 9091);
    }

    /// AMORE_HEALTH_ALLOW_NETWORK=1 must permit a non-loopback address.
    #[test]
    fn allow_network_unlocks_nonloopback() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized via ENV_LOCK; no concurrent env mutation in this scope.
        unsafe {
            std::env::set_var("AMORE_HEALTH_BIND", "0.0.0.0:9091");
            std::env::set_var("AMORE_HEALTH_ALLOW_NETWORK", "1");
        }

        let result = parse_health_bind();

        // Clean up before asserting so a failure doesn't poison later tests.
        // SAFETY: serialized via ENV_LOCK; no concurrent env mutation in this scope.
        unsafe {
            std::env::remove_var("AMORE_HEALTH_BIND");
            std::env::remove_var("AMORE_HEALTH_ALLOW_NETWORK");
        }

        let addr = result.expect("non-loopback with opt-in env must succeed");
        assert!(!addr.ip().is_loopback(), "opt-in must allow non-loopback");
        assert_eq!(addr.port(), 9091);
    }

    /// Without the opt-in, a non-loopback address must be rejected.
    #[test]
    fn nonloopback_without_optin_is_rejected() {
        let _guard = ENV_LOCK.lock().unwrap();
        // SAFETY: serialized via ENV_LOCK; no concurrent env mutation in this scope.
        unsafe {
            std::env::set_var("AMORE_HEALTH_BIND", "0.0.0.0:9091");
            std::env::remove_var("AMORE_HEALTH_ALLOW_NETWORK");
        }

        let result = parse_health_bind();

        // SAFETY: serialized via ENV_LOCK; no concurrent env mutation in this scope.
        unsafe {
            std::env::remove_var("AMORE_HEALTH_BIND");
        }

        assert!(
            result.is_err(),
            "non-loopback without opt-in must be rejected"
        );
    }
}
