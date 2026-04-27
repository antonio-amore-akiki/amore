// shutdown.rs — SIGTERM/Ctrl-C graceful shutdown handler (W3-3B).
//
// Waits for any of:
//   - Ctrl-C (cross-platform)
//   - SIGTERM (Unix: tokio::signal::unix)
//   - Ctrl-Break (Windows: tokio::signal::windows)
//   - An explicit shutdown channel (for tests / programmatic shutdown)
//
// On signal, emits `amore_shutdown_initiated_total` counter increment.
// The caller is responsible for draining in-flight work, fsyncing WAL,
// closing sled, and dropping the Qdrant pool within the 30-second window.

use metrics::counter;
use tokio::sync::oneshot;

/// Wait for a shutdown signal (Ctrl-C, SIGTERM, or programmatic trigger).
///
/// Returns when any signal fires. The caller must then perform the drain
/// sequence (stop accepting → drain ≤30s → fsync WAL → close sled → drop pool).
pub async fn wait_for_shutdown(rx: Option<oneshot::Receiver<()>>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler")
    };

    #[cfg(unix)]
    let sigterm = async {
        let mut sig = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )
        .expect("failed to install SIGTERM handler");
        sig.recv().await;
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    #[cfg(windows)]
    let ctrl_break = async {
        let mut sig = tokio::signal::windows::ctrl_break()
            .expect("failed to install Ctrl-Break handler");
        sig.recv().await;
    };

    #[cfg(not(windows))]
    let ctrl_break = std::future::pending::<()>();

    match rx {
        Some(receiver) => {
            tokio::select! {
                _ = ctrl_c => {
                    tracing::info!("shutdown: Ctrl-C received");
                }
                _ = sigterm => {
                    tracing::info!("shutdown: SIGTERM received");
                }
                _ = ctrl_break => {
                    tracing::info!("shutdown: Ctrl-Break received");
                }
                _ = async { let _ = receiver.await; } => {
                    tracing::info!("shutdown: programmatic signal received");
                }
            }
        }
        None => {
            tokio::select! {
                _ = ctrl_c => {
                    tracing::info!("shutdown: Ctrl-C received");
                }
                _ = sigterm => {
                    tracing::info!("shutdown: SIGTERM received");
                }
                _ = ctrl_break => {
                    tracing::info!("shutdown: Ctrl-Break received");
                }
            }
        }
    }

    counter!("amore_shutdown_initiated_total").increment(1);
    tracing::info!("shutdown: initiated — draining in-flight work (≤30s)");
}

/// Create a (sender, receiver) pair for programmatic shutdown in tests.
pub fn shutdown_channel() -> (oneshot::Sender<()>, oneshot::Receiver<()>) {
    oneshot::channel()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn programmatic_shutdown_triggers_without_blocking() {
        let (tx, rx) = shutdown_channel();
        // Fire the sender before awaiting — the select must resolve immediately.
        tx.send(()).expect("send");
        // Should complete without hanging.
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            wait_for_shutdown(Some(rx)),
        )
        .await
        .expect("shutdown did not complete within 2s");
    }
}
