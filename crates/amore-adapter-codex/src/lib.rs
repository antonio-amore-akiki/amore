//! Codex CLI adapter — patches `~/.codex/config.toml` to register the
//! `amore` MCP server. Implements [`amore_core::ide_adapter::IdeAdapter`].
//!
//! Codex (OpenAI Codex CLI + VSCode extension share the same config layer)
//! uses TOML. Schema:
//!
//! ```toml
//! [mcp_servers.amore]
//! command = "amore-mcp"
//! args = []
//! ```
//!
//! CODEX_HOME env var overrides `~/.codex`. We honor it.

use amore_core::ide_adapter::IdeAdapter;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "codex";

pub struct CodexAdapter {
    pub config_path_override: Option<PathBuf>,
    pub command: String,
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "amore-mcp".to_string(),
        }
    }
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for CodexAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        if let Ok(codex_home) = std::env::var("CODEX_HOME") {
            return Ok(PathBuf::from(codex_home).join("config.toml"));
        }
        let home = dirs::home_dir().context("could not resolve user home directory")?;
        Ok(home.join(".codex").join("config.toml"))
    }

    fn merge_into(&self, existing: &str) -> Result<String> {
        merge_codex_toml(existing, "amore", &self.command)
    }
}

/// Merge the `[mcp_servers.<name>]` table into a Codex config.toml.
///
/// Preserves all other top-level tables and pre-existing mcp_servers entries.
/// Removes a legacy `obelion` entry if present (idempotent migration).
/// Idempotent: if the entry already matches command + empty args, returns the
/// input unchanged so atomic-write's NoChange path fires.
fn merge_codex_toml(existing: &str, name: &str, command: &str) -> Result<String> {
    use toml::Value;
    let mut root: Value = if existing.trim().is_empty() {
        Value::Table(toml::map::Map::new())
    } else {
        existing
            .parse()
            .with_context(|| "parsing existing config.toml")?
    };
    let root_tbl = root
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("config root is not a TOML table"))?;
    // Ensure mcp_servers table exists.
    let mcp_servers = root_tbl
        .entry("mcp_servers".to_string())
        .or_insert_with(|| Value::Table(toml::map::Map::new()));
    let mcp_tbl = mcp_servers
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("mcp_servers is not a TOML table"))?;
    // Remove legacy obelion entry when inserting amore.
    if name == "amore" {
        mcp_tbl.remove("obelion");
    }
    // Build target entry.
    let mut entry = toml::map::Map::new();
    entry.insert("command".to_string(), Value::String(command.to_string()));
    entry.insert("args".to_string(), Value::Array(vec![]));
    let target = Value::Table(entry);
    if mcp_tbl.get(name) == Some(&target) {
        return Ok(existing.to_string());
    }
    mcp_tbl.insert(name.to_string(), target);
    let serialized =
        toml::to_string_pretty(&root).with_context(|| "serializing Codex config.toml")?;
    Ok(serialized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_creates_table() {
        let a = CodexAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("[mcp_servers.amore]"));
        assert!(merged.contains("command = \"amore-mcp\""));
        assert!(merged.contains("args = []"));
    }

    #[test]
    fn merge_into_preserves_other_tables() {
        let existing =
            "[model]\nname = \"o3\"\n\n[mcp_servers.foo]\ncommand = \"foo-mcp\"\nargs = []\n";
        let a = CodexAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(merged.contains("[model]"));
        assert!(merged.contains("name = \"o3\""));
        assert!(merged.contains("[mcp_servers.foo]"));
        assert!(merged.contains("[mcp_servers.amore]"));
    }

    #[test]
    fn idempotent_on_matching_entry() {
        let existing = "[mcp_servers.amore]\ncommand = \"amore-mcp\"\nargs = []\n";
        let a = CodexAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert_eq!(merged, existing, "matching entry must produce no diff");
    }

    #[test]
    fn config_path_honors_codex_home_env() {
        // SAFETY: tests are not parallel-safe across env mutation, but std::env::set_var is
        // safe per the rust contract when no other thread is reading; #[test] modules
        // serialize within a single test binary by default in nextest, fine for cargo test.
        unsafe {
            std::env::set_var("CODEX_HOME", "/custom/codex");
        }
        let a = CodexAdapter::new();
        let p = a.config_path().unwrap();
        assert!(
            p.to_string_lossy().contains("custom/codex"),
            "CODEX_HOME must drive the path: {}",
            p.display()
        );
        unsafe {
            std::env::remove_var("CODEX_HOME");
        }
    }

    #[test]
    fn replaces_legacy_obelion_entry() {
        let existing = "[mcp_servers.obelion]\ncommand = \"obelion-mcp\"\nargs = []\n";
        let a = CodexAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(
            !merged.contains("obelion]"),
            "legacy obelion entry must be removed"
        );
        assert!(merged.contains("[mcp_servers.amore]"));
    }
}
