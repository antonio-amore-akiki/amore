//! Windsurf (Codeium) adapter — patches `~/.codeium/windsurf/mcp_config.json`
//! to register the `amore` MCP server.
//!
//! Schema: shared `mcpServers` JSON (same shape as Claude Code + Cursor +
//! Cline). Each entry: `command` + optional `args` + optional `env`.
//!
//! Codeium uses a flat per-user `~/.codeium/` directory on every OS (no XDG
//! variation): `dirs::home_dir()` + `.codeium/windsurf/mcp_config.json` is the
//! canonical location.

use amore_core::ide_adapter::{IdeAdapter, merge_mcp_servers};
use anyhow::{Context, Result};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "windsurf";

pub struct WindsurfAdapter {
    pub config_path_override: Option<PathBuf>,
    pub command: String,
}

impl Default for WindsurfAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "amore-mcp".to_string(),
        }
    }
}

impl WindsurfAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for WindsurfAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        let home = dirs::home_dir().context("could not resolve user home directory")?;
        Ok(home
            .join(".codeium")
            .join("windsurf")
            .join("mcp_config.json"))
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
        let a = WindsurfAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("\"mcpServers\""));
        assert!(merged.contains("\"amore\""));
        assert!(merged.contains("\"amore-mcp\""));
    }

    #[test]
    fn merge_into_preserves_existing_servers() {
        let existing = r#"{"mcpServers":{"foo":{"command":"foo-mcp"}}}"#;
        let a = WindsurfAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(merged.contains("\"foo\""));
        assert!(merged.contains("\"amore\""));
    }

    #[test]
    fn idempotent_on_match() {
        let a = WindsurfAdapter::new();
        let first = a.merge_into("").unwrap();
        let second = a.merge_into(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn config_path_resolves_to_codeium_subdir() {
        let a = WindsurfAdapter::new();
        let p = a.config_path().unwrap();
        let s = p.to_string_lossy();
        assert!(s.contains(".codeium"));
        assert!(s.contains("windsurf"));
        assert!(s.ends_with("mcp_config.json"));
    }

    #[test]
    fn replaces_legacy_obelion_entry() {
        let existing = r#"{"mcpServers":{"obelion":{"command":"obelion-mcp","args":[]}}}"#;
        let a = WindsurfAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(
            !merged.contains("\"obelion\""),
            "legacy entry must be replaced"
        );
        assert!(merged.contains("\"amore\""));
    }
}
