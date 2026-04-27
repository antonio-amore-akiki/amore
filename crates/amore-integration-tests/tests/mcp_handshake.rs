// Integration test — amore-mcp must respond to a JSON-RPC `initialize` with
// a payload containing "protocolVersion" and no top-level "error".
//
// This test exposes DG-D/DG-E (Rust error leak + empty-stdin race) when they
// regress. If you change error wrapping in crates/amore-mcp/src/main.rs and
// this fails, restore plain-English error display.

use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Probe Qdrant gRPC port at 127.0.0.1:6334 with a 250 ms TCP connect.
/// amore-mcp's startup path requires a live Qdrant collection (ensure_collection
/// in QdrantStore::open issues a real RPC). When Qdrant is absent the binary
/// exits before responding to MCP initialize, so this integration test must
/// politely skip — matching the `skipped_when_no_daemon` pattern in
/// longmemeval_runner.rs and the `#[ignore]` guard on grpc_smoke.rs.
fn qdrant_available() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:6334".parse().expect("loopback addr"),
        Duration::from_millis(250),
    )
    .is_ok()
}

fn release_bin(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut p = manifest_dir.parent().unwrap().parent().unwrap().to_path_buf();
    p.push("target");
    p.push("release");
    p.push(if cfg!(windows) { format!("{name}.exe") } else { name.to_string() });
    p
}

const INITIALIZE_REQUEST: &str = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"integration-test\",\"version\":\"1.0\"}}}\n";

#[test]
fn mcp_initialize_returns_protocol_version() {
    if !qdrant_available() {
        eprintln!(
            "mcp_initialize_returns_protocol_version: skipping — Qdrant not reachable at 127.0.0.1:6334. \
             amore-mcp's startup path requires a live Qdrant daemon (ensure_collection RPC). \
             Start Qdrant locally to enable: docker run -p 6334:6334 qdrant/qdrant"
        );
        return;
    }
    let bin = release_bin("amore-mcp");
    assert!(bin.exists(), "amore-mcp binary not built — run cargo build --release --workspace first (looked for {})", bin.display());

    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn amore-mcp");

    child
        .stdin
        .as_mut()
        .expect("child stdin")
        .write_all(INITIALIZE_REQUEST.as_bytes())
        .expect("write initialize request");
    drop(child.stdin.take());

    // Wait up to 8 s for the child to exit (closing stdin signals graceful shutdown).
    let (tx, rx) = mpsc::channel();
    let child_for_wait = child;
    thread::spawn(move || {
        let out = child_for_wait.wait_with_output();
        let _ = tx.send(out);
    });

    let out = rx
        .recv_timeout(Duration::from_secs(8))
        .expect("amore-mcp did not exit within 8 s of stdin close")
        .expect("wait_with_output error");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // (1) JSON-RPC response on stdout MUST contain protocolVersion.
    assert!(
        stdout.contains("\"protocolVersion\""),
        "MCP initialize response missing protocolVersion on stdout.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // (2) JSON-RPC response on stdout MUST NOT contain a top-level "error":
    // (initialize must succeed cleanly; rmcp returning an Error result violates the contract).
    assert!(
        !stdout.contains("\"error\":"),
        "MCP initialize response carried a JSON-RPC error field.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // (3) stderr can contain INFO/DEBUG tracing log lines including rmcp:: module paths —
    // those are informational. ONLY flag genuine Rust error/panic leaks: lines that LOOK
    // like an `anyhow::Error` Display unwind (`^Error:` at line start), a panic abort
    // (`thread '...' panicked`), or a backtrace marker (`note: run with RUST_BACKTRACE`).
    for line in stderr.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Error:")
            || trimmed.starts_with("error:")
            || trimmed.contains("panicked at")
            || trimmed.contains("note: run with `RUST_BACKTRACE")
        {
            panic!(
                "amore-mcp leaked a Rust-internal error/panic line:\n  {line}\nfull stderr:\n{stderr}"
            );
        }
    }
}
