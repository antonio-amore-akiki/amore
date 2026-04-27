// grpc_smoke.rs — Phase H.6 gRPC transport smoke test.
//
// Spins up AmoreServiceImpl on an ephemeral loopback TCP port; connects a
// tonic client; calls Health(); asserts `status == "ok"`.
//
// The test is marked #[ignore] for CI environments that have neither Qdrant
// nor Ollama running — it requires only that the gRPC server can bind and
// respond, not that the recall backend is reachable. Run explicitly with:
//
//   cargo test -p amore-mcp --test grpc_smoke -- --include-ignored
//
// Root-cause: Health() never calls HybridRecall::search(); the lazy Qdrant
// client + panicking embedder are safe for this test's scope.

// Allow unwrap in test helpers — project policy exempts test modules.
#![allow(clippy::unwrap_used)]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use amore_core::qdrant_store::QdrantStore;
use amore_core::recall::HybridRecall;

// Path-include the grpc module from src/ — amore-mcp is a bin-only crate,
// so we cannot do `use amore_mcp::grpc;` without adding a lib target. This
// integration-test pattern (per the Rust Reference) keeps the crate shape
// unchanged while letting the test exercise grpc internals.
//
// The allow attrs suppress lints triggered by path-include context:
//   - dead_code: gRPC server is not yet wired into main.rs (v-next scope);
//     items like resolve_rpm/GrpcListener/parse_grpc_listen are pub API for
//     a future bin entry-point that the test does not exercise.
//   - result_large_err: tonic::Status is ~176 bytes — Tonic ecosystem-wide.
#[allow(dead_code, clippy::result_large_err)]
#[path = "../src/grpc.rs"]
mod grpc;

use grpc::proto::HealthRequest;
use grpc::proto::amore_service_client::AmoreServiceClient;
use grpc::{AmoreServiceImpl, AmoreServiceServer};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

/// Dummy embedder that panics if called — Health never calls embed_query.
struct NoOpEmbedder;

impl amore_core::recall::Embedder for NoOpEmbedder {
    async fn embed_query(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        panic!("NoOpEmbedder::embed_query must not be called in grpc_smoke test")
    }
}

/// Bind an ephemeral loopback port, serve the gRPC server, return
/// the bound address and a one-shot shutdown handle.
async fn spawn_test_server(
    impl_: AmoreServiceImpl<NoOpEmbedder>,
) -> (SocketAddr, tokio::sync::oneshot::Sender<()>) {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .await
        .expect("bind ephemeral loopback TCP for grpc_smoke");
    let addr = listener.local_addr().expect("local_addr");
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let svc = AmoreServiceServer::new(impl_);
    tokio::spawn(async move {
        Server::builder()
            .add_service(svc)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                drop(rx.await)
            })
            .await
            .expect("gRPC smoke server error");
    });

    (addr, tx)
}

/// Build an AmoreServiceImpl backed by a lazy (no-RPC) QdrantStore and a
/// no-op embedder. Safe for Health smoke tests; will panic if recall is called.
fn build_test_impl() -> AmoreServiceImpl<NoOpEmbedder> {
    let qdrant = QdrantStore::open_lazy("http://127.0.0.1:6334", "smoke_test")
        .expect("open_lazy must not make any network call");
    let recall = HybridRecall::with_embedder(NoOpEmbedder, qdrant);
    AmoreServiceImpl::new(Arc::new(recall), 100)
}

#[tokio::test]
#[ignore = "requires tokio runtime; run with --include-ignored"]
async fn health_returns_ok() {
    let impl_ = build_test_impl();
    grpc::record_start_time();

    let (addr, _shutdown) = spawn_test_server(impl_).await;

    // Give the server a tick to start accepting.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let mut client = AmoreServiceClient::connect(format!("http://{addr}"))
        .await
        .expect("connect to smoke gRPC server");

    let response = client
        .health(HealthRequest {})
        .await
        .expect("Health RPC")
        .into_inner();

    assert_eq!(
        response.status, "ok",
        "Health.status must be 'ok'; got {:?}",
        response.status
    );
    // uptime_ms may be 0 or small; just assert it's a valid u64 (it always is).
    let _ = response.uptime_ms;
}

#[tokio::test]
#[ignore = "requires tokio runtime; run with --include-ignored"]
async fn health_rate_limit_exhaustion_returns_resource_exhausted() {
    // Construct with rpm=1 so the second request exhausts the bucket.
    let qdrant = QdrantStore::open_lazy("http://127.0.0.1:6334", "smoke_rl")
        .expect("open_lazy must not make any network call");
    let recall = HybridRecall::with_embedder(NoOpEmbedder, qdrant);
    let impl_ = AmoreServiceImpl::new(Arc::new(recall), 1);
    grpc::record_start_time();

    let (addr, _shutdown) = spawn_test_server(impl_).await;
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let mut client = AmoreServiceClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    // First request: should succeed.
    client
        .health(HealthRequest {})
        .await
        .expect("first Health must succeed with rpm=1");

    // Second request immediately: bucket exhausted → resource_exhausted.
    let err = client
        .health(HealthRequest {})
        .await
        .expect_err("second Health must fail with resource_exhausted");

    assert_eq!(
        err.code(),
        tonic::Code::ResourceExhausted,
        "expected ResourceExhausted, got: {err:?}"
    );
    assert!(
        err.message().contains("rate limit exceeded"),
        "error message must mention 'rate limit exceeded'; got: {:?}",
        err.message()
    );
}
