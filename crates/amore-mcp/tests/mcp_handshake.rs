//! Integration test: spawn `amore-mcp` binary, perform JSON-RPC handshake
//! over stdio, assert tools/list returns the `recall` tool.
//!
//! Skipped by default. Run with:
//!     AMORE_TEST_MCP=1 cargo test -p amore-mcp --test mcp_handshake -- --ignored
//!
//! Prerequisites:
//!   - Qdrant daemon at AMORE_QDRANT_URL (default http://127.0.0.1:6334 gRPC)
//!   - Ollama daemon at AMORE_OLLAMA_URL (default http://127.0.0.1:11434)
//!
//! Why a spawned-binary test (not in-process): the v0.1.0 proof contract per
//! roadmap S6 is that an MCP client can connect to obelion-mcp over stdio and
//! see `recall` in tools/list. The truest end-to-end check spawns the actual
//! binary the way Claude Code / Cursor / Codex will spawn it. In-process
//! handler-only tests would miss transport bugs.

use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

fn enabled() -> bool {
    std::env::var("AMORE_TEST_MCP").ok().as_deref() == Some("1")
}

#[tokio::test]
#[ignore = "requires AMORE_TEST_MCP=1 + live Qdrant + Ollama"]
async fn handshake_lists_recall_tool() {
    if !enabled() {
        eprintln!("AMORE_TEST_MCP not set; skipping");
        return;
    }

    let unique_collection = format!(
        "amore_mcp_test_{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    // Spawn the binary via the cargo-published path. CARGO_BIN_EXE_<bin> is
    // injected by Cargo for integration tests in the same package.
    let bin = env!("CARGO_BIN_EXE_amore-mcp");
    let mut child = Command::new(bin)
        .env("AMORE_COLLECTION", &unique_collection)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn obelion-mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout).lines();

    async fn send(stdin: &mut tokio::process::ChildStdin, msg: serde_json::Value) {
        let line = serde_json::to_string(&msg).unwrap() + "\n";
        stdin.write_all(line.as_bytes()).await.expect("write");
        stdin.flush().await.expect("flush");
    }

    async fn recv_until_id(
        reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        id: i64,
    ) -> serde_json::Value {
        let dur = Duration::from_secs(15);
        loop {
            let line = timeout(dur, reader.next_line())
                .await
                .expect("read timeout")
                .expect("read err")
                .expect("eof");
            let v: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue, // non-JSON log noise
            };
            if v.get("id").and_then(|x| x.as_i64()) == Some(id) {
                return v;
            }
        }
    }

    // 1) initialize
    send(
        &mut stdin,
        serde_json::json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize",
            "params":{
                "protocolVersion":"2024-11-05",
                "capabilities":{},
                "clientInfo":{"name":"obelion-mcp-test","version":"0.1.0"}
            }
        }),
    )
    .await;
    let init = recv_until_id(&mut reader, 1).await;
    assert!(
        init.get("result").is_some(),
        "initialize response missing result: {init}"
    );

    // 2) notifications/initialized — fire-and-forget, no id
    send(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await;

    // 3) tools/list
    send(
        &mut stdin,
        serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    let list = recv_until_id(&mut reader, 2).await;
    let tools = list
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    assert!(!tools.is_empty(), "tools/list returned no tools: {list}");
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(
        names.contains(&"recall"),
        "expected 'recall' in tools, got {names:?}"
    );
    assert!(
        names.contains(&"canonical_doc_lookup"),
        "expected 'canonical_doc_lookup' in tools (S8), got {names:?}"
    );

    // Cleanup: kill child + drop Qdrant collection so repeated runs don't leak.
    drop(stdin);
    let _ = child.kill().await;
    let _ = child.wait().await;

    // Best-effort drop of the test collection on Qdrant.
    let qdrant_url =
        std::env::var("AMORE_QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());
    if let Ok(qs) =
        amore_core::qdrant_store::QdrantStore::open(&qdrant_url, &unique_collection).await
    {
        let _ = qs.drop_collection().await;
    }
}
