//! Cline (VSCode extension by saoudrizwan) adapter — patches
//! `cline_mcp_settings.json` inside VSCode's globalStorage to register the
//! `amore` MCP server.
//!
//! Schema: shared `mcpServers` JSON (same as Claude Code + Cursor + Windsurf).
//!
//! Cline path per OS (canonical VSCode globalStorage layout):
//!   - Linux:   `~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`
//!   - macOS:   `~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`
//!   - Windows: `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json`
//!
//! `dirs::config_dir()` returns the right root on each OS:
//!   - Linux:   `~/.config`
//!   - macOS:   `~/Library/Application Support`
//!   - Windows: `%APPDATA%`

use amore_core::ide_adapter::{IdeAdapter, merge_mcp_servers};
use anyhow::{Context, Result};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "cline";

pub struct ClineAdapter {
    pub config_path_override: Option<PathBuf>,
    pub command: String,
}

impl Default for ClineAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "amore-mcp".to_string(),
        }
    }
}

impl ClineAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for ClineAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        let cfg = dirs::config_dir().context("could not resolve config dir")?;
        Ok(cfg
            .join("Code")
            .join("User")
            .join("globalStorage")
            .join("saoudrizwan.claude-dev")
            .join("settings")
            .join("cline_mcp_settings.json"))
    }

    fn merge_into(&self, existing: &str) -> Result<String> {
        let entry = serde_json::json!({
            "command": self.command,
            "args": [],
        });
        merge_mcp_servers(existing, "amore", entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_creates_block() {
        let a = ClineAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("\"mcpServers\""));
        assert!(merged.contains("\"amore\""));
    }

    #[test]
    fn idempotent_on_match() {
        let a = ClineAdapter::new();
        let first = a.merge_into("").unwrap();
        let second = a.merge_into(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn config_path_resolves_to_vscode_globalstorage() {
        let a = ClineAdapter::new();
        let p = a.config_path().unwrap();
        let s = p.to_string_lossy();
        assert!(s.contains("Code"));
        assert!(s.contains("globalStorage"));
        assert!(s.contains("saoudrizwan.claude-dev"));
        assert!(s.ends_with("cline_mcp_settings.json"));
    }

    #[test]
    fn replaces_legacy_obelion_entry() {
        let existing = r#"{"mcpServers":{"obelion":{"command":"obelion-mcp","args":[]}}}"#;
        let a = ClineAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(
            !merged.contains("\"obelion\""),
            "legacy entry must be replaced"
        );
        assert!(merged.contains("\"amore\""));
    }
}
