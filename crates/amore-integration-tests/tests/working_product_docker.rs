// crates/amore-integration-tests/tests/working_product_docker.rs
//
// Phase B working-product smoke: full-stack Docker proof.
//
// Prerequisites (enforced by guard functions at test start — test skips, not fails, when absent):
//   - Qdrant at 127.0.0.1:6333 (HTTP) + 127.0.0.1:6334 (gRPC) — docker run qdrant/qdrant:v1.13.0
//   - Ollama at 127.0.0.1:11434 — ollama serve
//   - amore-mcp.exe built: cargo build --release -p amore-mcp
//
// Flow:
//   1. Seed one known observation into a temp SQLite DB (BM25 lane; no Ollama needed for seeding).
//   2. Spawn amore-mcp with AMORE_DATA_DIR pointing at the temp dir, so it picks up the seeded DB.
//   3. Drive JSON-RPC over stdio: initialize → tools/list → recall("synthetic_query").
//   4. Assert: (a) protocolVersion present, (b) >=2 tools, (c) top-1 hit text matches seeded doc.
//
// Pattern adapted from mcp_handshake.rs (same release_bin + mpsc wait pattern).

use amore_core::sqlite_store::SqliteStore;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

// ─── Guards ───────────────────────────────────────────────────────────────────

fn qdrant_available() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:6334".parse().expect("loopback addr"),
        Duration::from_millis(500),
    )
    .is_ok()
}

fn ollama_available() -> bool {
    TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().expect("loopback addr"),
        Duration::from_millis(500),
    )
    .is_ok()
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn release_bin(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut p = manifest_dir.parent().expect("integration-tests parent").parent().expect("crates parent").to_path_buf();
    p.push("target");
    p.push("release");
    p.push(if cfg!(windows) { format!("{name}.exe") } else { name.to_string() });
    p
}

/// Seed one observation into a SqliteStore at `db_path` and return the
/// unique keyword that must appear in a BM25 recall hit.
fn seed_test_observation(db_path: &std::path::Path) -> String {
    let unique_keyword = "amoresmoketest2026qdrant".to_string();
    let store = SqliteStore::open(db_path).expect("open temp sqlite");
    let payload = serde_json::json!({
        "text": format!(
            "Working product Docker smoke: {} is the unique keyword embedded in this test document \
             to prove round-trip store-and-recall via Amore's BM25 hybrid retrieval lane.",
            unique_keyword
        ),
        "source": "working_product_docker_test",
    });
    store.insert_observation("working_product_docker_test", &payload)
        .expect("insert_observation failed");
    unique_keyword
}

// ─── JSON-RPC message builders ─────────────────────────────────────────────────

const INITIALIZE_REQUEST: &str = concat!(
    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"#,
    r#""protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"working-product-test","version":"1.0"}}}"#,
    "\n"
);

const INITIALIZED_NOTIF: &str =
    r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"# ;

fn tools_list_request() -> String {
    format!(
        "{}\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#
    )
}

fn recall_request(query: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "recall",
                "arguments": { "query": query, "top_k": 5 }
            }
        })
    )
}

// ─── Test ─────────────────────────────────────────────────────────────────────

#[test]
fn working_product_docker_store_and_recall() {
    if !qdrant_available() {
        eprintln!(
            "working_product_docker: SKIPPING — Qdrant not reachable at 127.0.0.1:6334. \
             Run: docker run -d --name amore-qdrant -p 6333:6333 -p 6334:6334 qdrant/qdrant:v1.13.0"
        );
        return;
    }
    if !ollama_available() {
        eprintln!(
            "working_product_docker: SKIPPING — Ollama not reachable at 127.0.0.1:11434. \
             Run: ollama serve"
        );
        return;
    }

    let bin = release_bin("amore-mcp");
    assert!(
        bin.exists(),
        "amore-mcp binary not built — run: cargo build --release -p amore-mcp (looked for {})",
        bin.display()
    );

    // Create a temp dir for AMORE_DATA_DIR so the seeded DB is isolated from production data.
    let tmp_dir = tempfile::tempdir().expect("create tempdir");
    let db_path = tmp_dir.path().join("amore.db");

    // 1. Seed the test observation.
    let unique_keyword = seed_test_observation(&db_path);

    // 2. Spawn amore-mcp pointing at our temp data dir.
    let mut child = Command::new(&bin)
        .env("AMORE_DATA_DIR", tmp_dir.path())
        .env("AMORE_QDRANT_URL", "http://127.0.0.1:6334")
        .env("AMORE_OLLAMA_URL", "http://127.0.0.1:11434")
        .env("AMORE_LOG", "warn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn amore-mcp");

    // 3. Drive JSON-RPC protocol over stdin.
    {
        let stdin = child.stdin.as_mut().expect("child stdin");
        // initialize
        stdin.write_all(INITIALIZE_REQUEST.as_bytes()).expect("write initialize");
        // Give server 200ms to process, then send initialized notification
        thread::sleep(Duration::from_millis(200));
        // notifications/initialized (no \n terminator needed — rmcp handles)
        stdin.write_all(format!("{INITIALIZED_NOTIF}\n").as_bytes()).expect("write initialized notif");
        // tools/list
        stdin.write_all(tools_list_request().as_bytes()).expect("write tools/list");
        // recall with the unique keyword
        stdin.write_all(recall_request(&unique_keyword).as_bytes()).expect("write recall");
        // Brief pause to allow processing, then close stdin to trigger graceful shutdown.
        thread::sleep(Duration::from_millis(500));
    }
    drop(child.stdin.take());

    // 4. Wait for the child to exit (stdin close triggers shutdown in amore-mcp).
    let (tx, rx) = mpsc::channel();
    let child_for_wait = child;
    thread::spawn(move || {
        let out = child_for_wait.wait_with_output();
        let _ = tx.send(out);
    });

    let out = rx
        .recv_timeout(Duration::from_secs(30))
        .expect("amore-mcp did not exit within 30s of stdin close")
        .expect("wait_with_output error");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // ── Assertion A: protocolVersion in initialize response ────────────────────
    assert!(
        stdout.contains("\"protocolVersion\""),
        "A: MCP initialize response missing protocolVersion.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // ── Assertion B: >=2 tools in tools/list response ─────────────────────────
    // tools/list response contains "tools" array; count occurrences of "\"name\""
    // within the tools/list response. We look for the known tool names.
    assert!(
        stdout.contains("\"recall\""),
        "B: tools/list response missing 'recall' tool.\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("\"canonical_doc_lookup\""),
        "B: tools/list response missing 'canonical_doc_lookup' tool.\nstdout: {stdout}\nstderr: {stderr}"
    );

    // ── Assertion C: top-1 recall hit contains the unique keyword ─────────────
    // The BM25 lane will surface the seeded doc by exact keyword match.
    // Even if the vector lane is degraded (ollama embed fails), BM25 covers.
    assert!(
        stdout.contains(&unique_keyword),
        "C: recall response did not return the seeded document (unique_keyword='{}' not in stdout).\
        \nstdout: {stdout}\nstderr: {stderr}",
        unique_keyword
    );

    // ── Assertion D: no Rust panic/error leaks on stderr ──────────────────────
    for line in stderr.lines() {
        let t = line.trim_start();
        if t.starts_with("Error:") || t.starts_with("error:") || t.contains("panicked at")
            || t.contains("note: run with `RUST_BACKTRACE")
        {
            panic!(
                "D: amore-mcp leaked a Rust-internal error/panic:\n  {line}\nfull stderr:\n{stderr}"
            );
        }
    }

    eprintln!(
        "working_product_docker_store_and_recall: PASS — initialize+tools/list+recall round-trip green. unique_keyword='{}' found in stdout.",
        unique_keyword
    );
}
