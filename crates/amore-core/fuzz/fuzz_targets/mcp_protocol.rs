#![no_main]
// Fuzzes the MCP JSON config parser path: arbitrary bytes -> merge_mcp_servers.
// merge_mcp_servers is the function that parses untrusted IDE config files (Claude Code,
// Cursor, Codex, etc.) containing mcpServers entries. Any panic is a bug; Err from
// invalid JSON or wrong root type is expected and discarded.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Exercise merge_mcp_servers with arbitrary existing-config content.
        // This covers serde_json::from_str -> object-key traversal -> pretty-print path.
        let _ = amore_core::ide_adapter::merge_mcp_servers(
            s,
            "amore",
            serde_json::json!({"command": "amore-mcp", "args": []}),
        );
    }
});
