// Claude Code adapter — plugin manifest generator.

pub fn manifest_json() -> &'static str {
    r#"{
  "name": "obelion",
  "version": "0.1.0",
  "description": "Universal agent memory backbone (MCP server)",
  "mcp_server": { "command": "obelion-mcp", "transport": "stdio" },
  "hooks": { "UserPromptSubmit": ["obelion preflight-inject"] }
}"#
}
