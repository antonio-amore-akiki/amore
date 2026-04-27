// health/http.rs — Axum HTTP healthz/readyz sidecar (W2-2D).
//
// Listens on AMORE_HEALTH_BIND (default 0.0.0.0:9091):
//   GET /healthz  -> 200 always (process liveness)
//   GET /readyz   -> 200 if all ReadyState flags true; 503 otherwise
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

/// Spawn the healthz/readyz Axum sidecar.
///
/// Returns the `JoinHandle` so the caller can abort during graceful shutdown.
pub async fn spawn_health_server(state: Arc<ReadyState>) -> anyhow::Result<JoinHandle<()>> {
    let bind_addr: std::net::SocketAddr = std::env::var("AMORE_HEALTH_BIND")
        .unwrap_or_else(|_| "0.0.0.0:9091".to_string())
        .parse()
        .map_err(|e| {
            anyhow::anyhow!(
                "invalid AMORE_HEALTH_BIND — expected host:port: {e}"
            )
        })?;

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
    use tower::ServiceExt;

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
}
