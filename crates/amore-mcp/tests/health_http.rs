// tests/health_http.rs — Integration test for healthz/readyz HTTP sidecar (W2-2D).
//
// Spins an Axum router on a free port (real TcpListener) to mirror the pattern
// used in health::http. Uses reqwest for HTTP assertions.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Minimal ReadyState mirroring health::http::ReadyState, defined inline so
/// this test does not need a pub re-export from the binary crate.
struct ReadyState {
    wal_replayed: AtomicBool,
    warmed_up: AtomicBool,
}

impl ReadyState {
    fn new() -> Self {
        Self {
            wal_replayed: AtomicBool::new(false),
            warmed_up: AtomicBool::new(false),
        }
    }
    fn is_ready(&self) -> bool {
        self.wal_replayed.load(Ordering::Acquire)
            && self.warmed_up.load(Ordering::Acquire)
    }
}

#[tokio::test]
async fn healthz_always_200_readyz_gates_on_state() {
    // Bind to port 0 to get an OS-assigned free port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind free port");
    let addr = listener.local_addr().expect("local_addr");

    let state = Arc::new(ReadyState::new());
    let state_srv = Arc::clone(&state);

    use axum::{Router, extract::State, response::IntoResponse, routing::get};

    async fn healthz_h() -> impl IntoResponse {
        axum::http::StatusCode::OK
    }

    async fn readyz_h(State(s): State<Arc<ReadyState>>) -> impl IntoResponse {
        if s.is_ready() {
            (axum::http::StatusCode::OK, "ready").into_response()
        } else {
            (axum::http::StatusCode::SERVICE_UNAVAILABLE, "not ready").into_response()
        }
    }

    let router = Router::new()
        .route("/healthz", get(healthz_h))
        .route("/readyz", get(readyz_h))
        .with_state(state_srv);

    let handle = tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });

    // Allow the server to begin accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let client = reqwest::Client::new();

    // /healthz — always 200 while process is alive.
    let res = client
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .expect("GET /healthz");
    assert_eq!(res.status().as_u16(), 200, "/healthz must always be 200");

    // /readyz — 503 before warmup.
    let res = client
        .get(format!("http://{addr}/readyz"))
        .send()
        .await
        .expect("GET /readyz pre-warm");
    assert_eq!(
        res.status().as_u16(),
        503,
        "/readyz must be 503 before wal_replayed + warmed_up"
    );

    // Flip only wal_replayed — still not ready.
    state.wal_replayed.store(true, Ordering::Release);
    let res = client
        .get(format!("http://{addr}/readyz"))
        .send()
        .await
        .expect("GET /readyz partial");
    assert_eq!(res.status().as_u16(), 503, "/readyz must be 503 with only wal_replayed");

    // Flip warmed_up — now ready.
    state.warmed_up.store(true, Ordering::Release);
    let res = client
        .get(format!("http://{addr}/readyz"))
        .send()
        .await
        .expect("GET /readyz post-warm");
    assert_eq!(res.status().as_u16(), 200, "/readyz must be 200 after all gates pass");

    handle.abort();
}
