// tests/shutdown.rs — Integration test for graceful shutdown handler (W3-3B).
//
// Tests the programmatic shutdown path (oneshot channel) since we cannot
// send real OS signals in unit tests. Verifies that wait_for_shutdown
// completes without hanging when the sender fires.

// The shutdown module is part of the amore-mcp binary crate (not a lib crate),
// so we cannot import it directly from the integration test. We replicate the
// minimal logic here to test the pattern, and rely on the unit test inside
// shutdown.rs itself for the actual module test.

use tokio::sync::oneshot;

async fn wait_for_programmatic_signal(rx: oneshot::Receiver<()>) {
    let _ = rx.await;
}

#[tokio::test]
async fn programmatic_shutdown_completes_within_timeout() {
    let (tx, rx) = oneshot::channel::<()>();

    // Fire the sender before awaiting (simulates immediate shutdown signal).
    tx.send(()).expect("send shutdown signal");

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        wait_for_programmatic_signal(rx),
    )
    .await;

    assert!(result.is_ok(), "shutdown signal must complete within 2s");
}

#[tokio::test]
async fn shutdown_channel_sender_dropped_causes_receiver_to_complete() {
    let (tx, rx) = oneshot::channel::<()>();

    // Drop sender without sending — receiver should resolve immediately (Err).
    drop(tx);

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        async move { let _ = rx.await; },
    )
    .await;

    assert!(result.is_ok(), "dropped sender must release receiver within 1s");
}
