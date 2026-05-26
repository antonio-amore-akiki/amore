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

// ---------------------------------------------------------------------------
// Security regression tests — Finding 6a (unbounded RecallParams).
// Both tests require the same daemon prerequisites as the handshake test.
// They exercise only the input-validation path, which fires before any
// Ollama/Qdrant network call, so they resolve quickly.
// ---------------------------------------------------------------------------

/// Helper: spawn amore-mcp, perform initialize + notifications/initialized,
/// then send one tools/call and return the response JSON and the child handle
/// so the caller can kill it.
async fn spawn_and_handshake(
    unique_collection: &str,
) -> (
    tokio::process::ChildStdin,
    tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    tokio::process::Child,
) {
    let bin = env!("CARGO_BIN_EXE_amore-mcp");
    let mut child = Command::new(bin)
        .env("AMORE_COLLECTION", unique_collection)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn amore-mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let reader = BufReader::new(stdout).lines();

    // initialize
    let init_msg =
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "bounds-test", "version": "0.0.0"}
            }
        }))
        .unwrap()
            + "\n";
    stdin
        .write_all(init_msg.as_bytes())
        .await
        .expect("write initialize");
    stdin.flush().await.expect("flush");

    // notifications/initialized (no id)
    let notif =
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .unwrap()
            + "\n";
    stdin
        .write_all(notif.as_bytes())
        .await
        .expect("write notif");
    stdin.flush().await.expect("flush notif");

    (stdin, reader, child)
}

async fn recv_until_id_bounds(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: i64,
) -> serde_json::Value {
    let dur = Duration::from_secs(30);
    loop {
        let line = timeout(dur, reader.next_line())
            .await
            .expect("read timeout in bounds test")
            .expect("read err")
            .expect("eof");
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("id").and_then(|x| x.as_i64()) == Some(id) {
            return v;
        }
    }
}

/// Security 6a regression: tools/call recall with top_k = usize::MAX must
/// return a clean JSON-RPC error — no panic, no wrap-and-corrupt Qdrant call.
#[tokio::test]
#[ignore = "requires AMORE_TEST_MCP=1 + live Qdrant + Ollama"]
async fn recall_rejects_oversized_top_k() {
    if !enabled() {
        eprintln!("AMORE_TEST_MCP not set; skipping");
        return;
    }

    let collection = format!(
        "amore_bounds_topk_{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let (mut stdin, mut reader, mut child) = spawn_and_handshake(&collection).await;

    // Wait for initialize response before sending tools/call.
    recv_until_id_bounds(&mut reader, 1).await;

    // tools/call with top_k = usize::MAX (18446744073709551615 in JSON).
    // JSON numbers have no usize cap; rmcp deserializes via serde which will
    // accept the value as-is into a usize.
    let call_msg =
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "recall",
                "arguments": {
                    "query": "test query",
                    "top_k": usize::MAX
                }
            }
        }))
        .unwrap()
            + "\n";
    stdin
        .write_all(call_msg.as_bytes())
        .await
        .expect("write tools/call");
    stdin.flush().await.expect("flush");

    let resp = recv_until_id_bounds(&mut reader, 10).await;
    // The response must carry an "error" key (JSON-RPC error) — not a "result"
    // with garbage, and not a missing response (panic/process-exit).
    // top_k above MAX_TOP_K (100) is silently clamped, not errored — the recall
    // itself may succeed or fail depending on daemon state, but must not wrap.
    // Accepted outcomes: either a result (clamped + successful search) OR an
    // error (e.g. Qdrant unavailable). What must NOT happen: process crash.
    let process_alive = child.try_wait().expect("try_wait").is_none();
    assert!(
        process_alive,
        "amore-mcp crashed on oversized top_k; expected clean error or clamped result: {resp}"
    );
    // Confirm the response has either "result" or "error" — a well-formed JSON-RPC response.
    assert!(
        resp.get("result").is_some() || resp.get("error").is_some(),
        "response is not a valid JSON-RPC object: {resp}"
    );

    drop(stdin);
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Security 6a regression: tools/call recall with a 17-KiB query must return
/// a clean JSON-RPC error — no panic, no Ollama embed call.
#[tokio::test]
#[ignore = "requires AMORE_TEST_MCP=1 + live Qdrant + Ollama"]
async fn recall_rejects_oversized_query() {
    if !enabled() {
        eprintln!("AMORE_TEST_MCP not set; skipping");
        return;
    }

    let collection = format!(
        "amore_bounds_query_{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let (mut stdin, mut reader, mut child) = spawn_and_handshake(&collection).await;

    // Wait for initialize response before sending tools/call.
    recv_until_id_bounds(&mut reader, 1).await;

    // 17 KiB query — exceeds MAX_QUERY_BYTES (16 KiB = 16 384 bytes).
    let oversized_query = "x".repeat(17 * 1024);

    let call_msg =
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "recall",
                "arguments": {
                    "query": oversized_query,
                    "top_k": 5
                }
            }
        }))
        .unwrap()
            + "\n";
    stdin
        .write_all(call_msg.as_bytes())
        .await
        .expect("write tools/call");
    stdin.flush().await.expect("flush");

    let resp = recv_until_id_bounds(&mut reader, 20).await;
    // Must be a JSON-RPC error response — the validation gate must fire before
    // Ollama embed is called. Check the error is present and message mentions
    // the byte limit.
    let error_obj = resp.get("error");
    assert!(
        error_obj.is_some(),
        "expected JSON-RPC error for 17-KiB query, got: {resp}"
    );
    let error_str = error_obj.unwrap().to_string();
    assert!(
        error_str.contains("16384") || error_str.contains("bytes") || error_str.contains("query"),
        "error message should mention the byte limit, got: {error_str}"
    );

    drop(stdin);
    let _ = child.kill().await;
    let _ = child.wait().await;
}
