//! Cursor adapter — patches `~/.cursor/mcp.json` (global) or per-workspace
//! `<workspace>/.cursor/mcp.json` to register the `amore` MCP server.
//!
//! v0.1.0 default: global config at `~/.cursor/mcp.json`. Workspace-scoped
//! patching arrives with the `--workspace <path>` CLI flag in S9.
//!
//! Cursor's schema is identical to Claude Code's `mcpServers` map; we reuse
//! `amore_core::ide_adapter::merge_mcp_servers`.
// ADR 0010: no-unwrap policy. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

use amore_core::ide_adapter::{IdeAdapter, merge_mcp_servers};
use anyhow::{Context, Result};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "cursor";

pub struct CursorAdapter {
    pub config_path_override: Option<PathBuf>,
    pub command: String,
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "amore-mcp".to_string(),
        }
    }
}

impl CursorAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for CursorAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        let home = dirs::home_dir().context("could not resolve user home directory")?;
        Ok(home.join(".cursor").join("mcp.json"))
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
        let a = CursorAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("\"mcpServers\""));
        assert!(merged.contains("\"amore\""));
    }

    #[test]
    fn idempotent_after_first_apply() {
        let a = CursorAdapter::new();
        let first = a.merge_into("").unwrap();
        let second = a.merge_into(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn preserves_existing_servers() {
        let existing = r#"{"mcpServers":{"github":{"command":"gh-mcp","args":["serve"]}}}"#;
        let a = CursorAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(merged.contains("\"github\""));
        assert!(merged.contains("\"amore\""));
    }

    #[test]
    fn replaces_legacy_obelion_entry() {
        let existing = r#"{"mcpServers":{"obelion":{"command":"obelion-mcp","args":[]}}}"#;
        let a = CursorAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(
            !merged.contains("\"obelion\""),
            "legacy entry must be replaced"
        );
        assert!(merged.contains("\"amore\""));
    }
}
