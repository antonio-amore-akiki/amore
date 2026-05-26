//! Hermes Agent (NousResearch) adapter — patches `~/.hermes/config.yaml` to
//! register the `amore` MCP server.
//!
//! Schema: YAML, top-level `mcp_servers` mapping. Each entry: `command` +
//! optional `args` + optional `env`:
//!
//! ```yaml
//! mcp_servers:
//!   amore:
//!     command: amore-mcp
//!     args: []
//! ```
//!
//! Path: `~/.hermes/config.yaml` (flat per-user dir, every OS — Hermes uses a
//! literal `~/.hermes/` convention regardless of XDG/AppData norms).

use amore_core::ide_adapter::IdeAdapter;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "hermes";

pub struct HermesAdapter {
    pub config_path_override: Option<PathBuf>,
    pub command: String,
}

impl Default for HermesAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "amore-mcp".to_string(),
        }
    }
}

impl HermesAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for HermesAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        let home = dirs::home_dir().context("could not resolve user home directory")?;
        Ok(home.join(".hermes").join("config.yaml"))
    }

    fn merge_into(&self, existing: &str) -> Result<String> {
        merge_hermes_yaml(existing, "amore", &self.command)
    }
}

/// Merge a `mcp_servers.<name>` entry into a Hermes `config.yaml`.
///
/// Preserves every other top-level key (model config, hooks, etc.).
/// Removes legacy `obelion` entry when inserting `amore`.
/// Idempotent: if `mcp_servers.<name>` already equals the target, returns the
/// input unchanged so atomic-write's NoChange path fires.
fn merge_hermes_yaml(existing: &str, name: &str, command: &str) -> Result<String> {
    use serde_yaml::{Mapping, Value};
    let mut root: Value = if existing.trim().is_empty() {
        Value::Mapping(Mapping::new())
    } else {
        serde_yaml::from_str(existing).with_context(|| "parsing existing config.yaml")?
    };
    let root_map = root
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("config root is not a YAML mapping"))?;
    let key = Value::String("mcp_servers".to_string());
    if !root_map.contains_key(&key) {
        root_map.insert(key.clone(), Value::Mapping(Mapping::new()));
    }
    let mcp_servers = root_map
        .get_mut(&key)
        .expect("inserted above")
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("mcp_servers is not a YAML mapping"))?;
    // Remove legacy obelion entry when inserting amore.
    if name == "amore" {
        mcp_servers.remove(Value::String("obelion".to_string()));
    }
    // Build target entry.
    let mut entry = Mapping::new();
    entry.insert(
        Value::String("command".to_string()),
        Value::String(command.to_string()),
    );
    entry.insert(
        Value::String("args".to_string()),
        Value::Sequence(Vec::new()),
    );
    let target = Value::Mapping(entry);
    let name_key = Value::String(name.to_string());
    if mcp_servers.get(&name_key) == Some(&target) {
        return Ok(existing.to_string());
    }
    mcp_servers.insert(name_key, target);
    let serialized = serde_yaml::to_string(&root).with_context(|| "serializing config.yaml")?;
    Ok(serialized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_creates_mcp_servers_block() {
        let a = HermesAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("mcp_servers"));
        assert!(merged.contains("amore"));
        assert!(merged.contains("amore-mcp"));
    }

    #[test]
    fn merge_into_preserves_other_top_level_keys() {
        let existing =
            "model: hermes-3\n\nmcp_servers:\n  foo:\n    command: foo-mcp\n    args: []\n";
        let a = HermesAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(merged.contains("model"));
        assert!(merged.contains("hermes-3"));
        assert!(merged.contains("foo"));
        assert!(merged.contains("amore"));
    }

    #[test]
    fn idempotent_on_match() {
        let a = HermesAdapter::new();
        let first = a.merge_into("").unwrap();
        let second = a.merge_into(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn config_path_resolves_to_hermes_subdir() {
        let a = HermesAdapter::new();
        let p = a.config_path().unwrap();
        let s = p.to_string_lossy();
        assert!(s.contains(".hermes"));
        assert!(s.ends_with("config.yaml"));
    }

    #[test]
    fn replaces_legacy_obelion_entry() {
        let existing = "mcp_servers:\n  obelion:\n    command: obelion-mcp\n    args: []\n";
        let a = HermesAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(
            !merged.contains("obelion"),
            "legacy obelion entry must be removed"
        );
        assert!(merged.contains("amore"));
    }
}
