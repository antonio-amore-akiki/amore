//! opencode (sst.dev) adapter — patches `opencode.json` to register the
//! `amore` MCP server.
//!
//! Schema: top-level `mcp` field (NOT `mcpServers`); each entry carries a
//! `type` discriminator and a `command` ARRAY (binary + flattened args):
//!
//! ```json
//! {
//!   "$schema": "https://opencode.ai/config.json",
//!   "mcp": {
//!     "amore": { "type": "local", "command": ["amore-mcp"] }
//!   }
//! }
//! ```
//!
//! Path per OS (resolved via `dirs::config_dir()`):
//!   - Linux:   `~/.config/opencode/opencode.json`
//!   - macOS:   `~/Library/Application Support/opencode/opencode.json`
//!   - Windows: `%APPDATA%\opencode\opencode.json`
// ADR 0010: no-unwrap policy. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

use amore_core::ide_adapter::IdeAdapter;
use anyhow::{Context, Result};
use std::path::PathBuf;

pub const ADAPTER_NAME: &str = "opencode";

pub struct OpencodeAdapter {
    pub config_path_override: Option<PathBuf>,
    pub command: String,
}

impl Default for OpencodeAdapter {
    fn default() -> Self {
        Self {
            config_path_override: None,
            command: "amore-mcp".to_string(),
        }
    }
}

impl OpencodeAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

impl IdeAdapter for OpencodeAdapter {
    fn name(&self) -> &'static str {
        ADAPTER_NAME
    }

    fn config_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.config_path_override {
            return Ok(p.clone());
        }
        let cfg = dirs::config_dir().context("could not resolve config dir")?;
        Ok(cfg.join("opencode").join("opencode.json"))
    }

    fn merge_into(&self, existing: &str) -> Result<String> {
        merge_opencode_mcp(existing, "amore", &self.command)
    }
}

/// Merge a `mcp.<name>` entry into an opencode.json config.
///
/// Preserves the `$schema` field and any other unrelated top-level keys.
/// Removes legacy `obelion` entry when inserting `amore`.
/// Idempotent: identical entry => input returned verbatim.
fn merge_opencode_mcp(existing: &str, name: &str, command: &str) -> Result<String> {
    use serde_json::{Map, Value, json};
    let mut root: Value = if existing.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(existing).with_context(|| "parsing existing opencode.json")?
    };
    if !root.is_object() {
        anyhow::bail!("opencode config root is not a JSON object");
    }
    let obj = root.as_object_mut().expect("checked above");
    let mcp = obj.entry("mcp".to_string()).or_insert_with(|| json!({}));
    if !mcp.is_object() {
        anyhow::bail!("opencode mcp field is present but not a JSON object");
    }
    let mcp_obj = mcp.as_object_mut().expect("checked above");
    // Remove legacy obelion entry when inserting amore.
    if name == "amore" {
        mcp_obj.remove("obelion");
    }
    let target = json!({
        "type": "local",
        "command": [command],
    });
    if mcp_obj.get(name) == Some(&target) {
        return Ok(existing.to_string());
    }
    mcp_obj.insert(name.to_string(), target);
    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_creates_mcp_block() {
        let a = OpencodeAdapter::new();
        let merged = a.merge_into("").unwrap();
        assert!(merged.contains("\"mcp\""));
        assert!(merged.contains("\"amore\""));
        assert!(merged.contains("\"type\""));
        assert!(merged.contains("\"local\""));
        assert!(merged.contains("\"command\""));
        assert!(merged.contains("\"amore-mcp\""));
    }

    #[test]
    fn merge_into_preserves_schema_and_other_servers() {
        let existing = r#"{"$schema":"https://opencode.ai/config.json","mcp":{"foo":{"type":"local","command":["foo-mcp"]}}}"#;
        let a = OpencodeAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(merged.contains("\"$schema\""));
        assert!(merged.contains("\"foo\""));
        assert!(merged.contains("\"amore\""));
    }

    #[test]
    fn idempotent_on_match() {
        let a = OpencodeAdapter::new();
        let first = a.merge_into("").unwrap();
        let second = a.merge_into(&first).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn config_path_resolves_to_opencode_subdir() {
        let a = OpencodeAdapter::new();
        let p = a.config_path().unwrap();
        let s = p.to_string_lossy();
        assert!(s.contains("opencode"));
        assert!(s.ends_with("opencode.json"));
    }

    #[test]
    fn replaces_legacy_obelion_entry() {
        let existing = r#"{"mcp":{"obelion":{"type":"local","command":["obelion-mcp"]}}}"#;
        let a = OpencodeAdapter::new();
        let merged = a.merge_into(existing).unwrap();
        assert!(
            !merged.contains("\"obelion\""),
            "legacy entry must be replaced"
        );
        assert!(merged.contains("\"amore\""));
    }
}
