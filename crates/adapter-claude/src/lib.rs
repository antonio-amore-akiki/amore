//! Claude Code adapter — patches `~/.claude.json` to register the `obelion`
//! MCP server. Implements [`obelion_core::ide_adapter::IdeAdapter`].
//!
//! Claude Code reads `~/.claude.json` (cross-OS — `dirs::home_dir()` + literal
//! filename). The MCP server registry lives under `mcpServers` at the top
//! level, mirroring Cursor's convention. Each entry: `command` + optional
//! `args` + optional `env`.
//!
//! Atomic-write + .bak backup + idempotency are inherited from the trait
//! `apply()` helper in obelion-core.

use anyhow::{Context, Result};
use obelion_core::ide_adapter::{IdeAdapter, merge_mcp_servers};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "claude";

pub struct ClaudeAdapter {
    /// Override of the default `~/.claude.json` path. Tests use this to point
    /// at a sandbox path; production callers leave it None.
    pub config_path_override: Option<PathBuf>,
    /// Override of the resolved `obelion-mcp` binary the entry will invoke.
    /// Production resolves at runtime; tests pin a stub path for determinism.
    pub command: String,
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "obelion-mcp".to_string(),
        }
    }
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for ClaudeAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        let home = dirs::home_dir().context("could not resolve user home directory")?;
        Ok(home.join(".claude.json"))
    }

    fn merge_into(&self, existing: &str) -> Result<String> {
        let entry = serde_json::json!({
            "command": self.command,
            "args": [],
        });
        merge_mcp_servers(existing, "obelion", entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use obelion_core::ide_adapter::dry_run;

    #[test]
    fn merge_into_empty_creates_block() {
        let a = ClaudeAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("\"mcpServers\""));
        assert!(merged.contains("\"obelion\""));
        assert!(merged.contains("\"obelion-mcp\""));
    }

    #[test]
    fn merge_into_existing_preserves_unrelated_keys() {
        let existing = r#"{"theme":"dark","mcpServers":{"foo":{"command":"foo-mcp"}}}"#;
        let a = ClaudeAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(merged.contains("\"theme\""));
        assert!(merged.contains("\"foo\""));
        assert!(merged.contains("\"obelion\""));
    }

    #[test]
    fn idempotent_after_first_apply() {
        let a = ClaudeAdapter::new();
        let first = a.merge_into("").unwrap();
        let second = a.merge_into(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn dry_run_smoke_with_override_path() {
        let mut a = ClaudeAdapter::new();
        let tmp = std::env::temp_dir().join("obelion-claude-test.json");
        let _ = std::fs::remove_file(&tmp);
        a.config_path_override = Some(tmp.clone());
        let s = dry_run(&a).unwrap();
        assert!(s.contains("obelion"));
    }
}
