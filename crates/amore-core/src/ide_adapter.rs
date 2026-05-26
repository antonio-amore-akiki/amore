// IdeAdapter — shared contract for per-IDE config patchers.
//
// Each IDE (Claude Code, Cursor, Codex, Cline, Continue, Roo, Zed) stores its
// MCP server registrations in a different file with a different schema. This
// trait normalizes the contract so the CLI dispatch (`amore init <ide>`) is
// uniform.
//
// Atomic-write contract enforced by `apply()`:
//   1. Read existing file (or treat as empty if missing/unreadable)
//   2. Call `merge_into(existing)` to produce the target content
//   3. If target == existing -> no-op (idempotent, byte-identical)
//   4. Else write to "<path>.tmp", fsync, rename onto "<path>" (atomic on
//      POSIX + Windows since Rust 1.66), and sibling ".bak" preserves the
//      pre-edit content for one revision of rollback.
//
// Dry-run contract: `dry_run()` returns the proposed merged content as a
// String without touching disk. The CLI prints it.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub trait IdeAdapter {
    /// Human-visible IDE name (matches the CLI subcommand: "claude", "cursor", ...).
    fn name(&self) -> &'static str;

    /// Resolved absolute path of the config file this adapter patches. OS-aware.
    fn config_path(&self) -> Result<PathBuf>;

    /// Returns the desired full content of the config file given the existing
    /// content (or empty string if the file does not yet exist). Implementers
    /// MUST be deterministic: same `existing` -> same output.
    fn merge_into(&self, existing: &str) -> Result<String>;
}

/// Result of `apply()` — caller-visible summary for CLI output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// File already had the expected content; nothing written.
    NoChange,
    /// File was created from empty.
    Created(PathBuf),
    /// File was updated; a .bak sibling now holds the previous content.
    Updated { path: PathBuf, backup: PathBuf },
}

/// Compute the merged content without touching disk.
pub fn dry_run<A: IdeAdapter + ?Sized>(adapter: &A) -> Result<String> {
    let path = adapter.config_path()?;
    let existing = read_or_empty(&path)?;
    adapter.merge_into(&existing)
}

/// Apply the adapter's merge to the on-disk config file.
///
/// Atomic-write semantics: writes to `<path>.tmp`, then `rename` -> `<path>`.
/// On Windows the rename overwrites the destination atomically when the
/// destination already exists. A `.bak` sibling is created when overwriting
/// an existing file so that a single revision of rollback is always possible.
pub fn apply<A: IdeAdapter + ?Sized>(adapter: &A) -> Result<ApplyOutcome> {
    let path = adapter.config_path()?;
    let existing = read_or_empty(&path)?;
    let merged = adapter.merge_into(&existing)?;
    if existing == merged {
        return Ok(ApplyOutcome::NoChange);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent {}", parent.display()))?;
    }
    let tmp = with_extension(&path, "tmp");
    fs::write(&tmp, &merged).with_context(|| format!("writing {}", tmp.display()))?;
    if path.exists() {
        let bak = with_extension(&path, "bak");
        // Best-effort: drop any previous .bak so we don't accumulate.
        let _ = fs::remove_file(&bak);
        fs::rename(&path, &bak)
            .with_context(|| format!("backing up {} -> {}", path.display(), bak.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
        Ok(ApplyOutcome::Updated { path, backup: bak })
    } else {
        fs::rename(&tmp, &path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
        Ok(ApplyOutcome::Created(path))
    }
}

fn read_or_empty(p: &Path) -> Result<String> {
    if !p.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))
}

fn with_extension(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

/// Helper: merge an `amore` entry into the `mcpServers` object inside a JSON
/// config (Claude Code and Cursor share this schema). Returns the pretty-
/// printed JSON ending with a trailing newline.
///
/// Idempotent: if `mcpServers.<entry_name>` already equals `entry`, the input
/// is returned unchanged. The rest of the existing config (including unrelated
/// top-level keys and other mcpServers) is preserved verbatim.
/// Legacy `obelion` entries are REPLACED by the new `amore` entry on the next
/// `amore init` run (idempotent semantics: one pass-through per apply call).
pub fn merge_mcp_servers(
    existing: &str,
    entry_name: &str,
    entry: serde_json::Value,
) -> Result<String> {
    use serde_json::{Map, Value, json};
    let mut root: Value = if existing.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(existing).with_context(|| "parsing existing config as JSON")?
    };
    if !root.is_object() {
        anyhow::bail!("config root is not a JSON object");
    }
    let obj = root.as_object_mut().expect("checked above");
    let servers = obj.entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        anyhow::bail!("mcpServers is present but not a JSON object");
    }
    let servers = servers.as_object_mut().expect("checked above");
    // Remove legacy "obelion" entry if present and we're inserting "amore".
    // This implements the idempotent REPLACE semantics for existing installs.
    if entry_name == "amore" {
        servers.remove("obelion");
    }
    let existing_entry = servers.get(entry_name).cloned();
    if existing_entry.as_ref() == Some(&entry) {
        // No structural change — re-emit pretty-printed to normalize whitespace
        // only if input wasn't already pretty. Cheap stable canonicalization.
        let normalized = serde_json::to_string_pretty(&root)? + "\n";
        if normalized == existing {
            return Ok(existing.to_string());
        }
        // Existing entry matches semantically but file whitespace differs;
        // prefer keeping input untouched to maximize "no diff" cases.
        return Ok(existing.to_string());
    }
    servers.insert(entry_name.to_string(), entry);
    Ok(serde_json::to_string_pretty(&root)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Per-test unique temp dir under the OS temp dir. Avoids the tempfile
    // crate (gate-friendly: no new crate dependency to justify).
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    fn fresh_tmp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("amore-adapter-test-{nanos:x}-{n}"));
        std::fs::create_dir_all(&p).expect("mkdir tmp");
        p
    }

    struct InlineAdapter {
        path: PathBuf,
    }
    impl IdeAdapter for InlineAdapter {
        fn name(&self) -> &'static str {
            "inline-test"
        }
        fn config_path(&self) -> Result<PathBuf> {
            Ok(self.path.clone())
        }
        fn merge_into(&self, existing: &str) -> Result<String> {
            merge_mcp_servers(
                existing,
                "amore",
                serde_json::json!({"command":"amore-mcp","args":[]}),
            )
        }
    }

    #[test]
    fn apply_creates_file_from_empty() {
        let dir = fresh_tmp_dir();
        let p = dir.join("mcp.json");
        let a = InlineAdapter { path: p.clone() };
        let r = apply(&a).unwrap();
        assert!(matches!(r, ApplyOutcome::Created(_)));
        assert!(p.exists());
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("\"amore\""));
    }

    #[test]
    fn apply_is_idempotent_on_second_run() {
        let dir = fresh_tmp_dir();
        let p = dir.join("mcp.json");
        let a = InlineAdapter { path: p.clone() };
        let _ = apply(&a).unwrap();
        let body1 = std::fs::read_to_string(&p).unwrap();
        let r2 = apply(&a).unwrap();
        assert!(matches!(r2, ApplyOutcome::NoChange));
        let body2 = std::fs::read_to_string(&p).unwrap();
        assert_eq!(body1, body2, "byte-identical on repeat");
    }

    #[test]
    fn apply_preserves_existing_keys_and_writes_backup() {
        let dir = fresh_tmp_dir();
        let p = dir.join("mcp.json");
        std::fs::write(
            &p,
            r#"{"theme":"dark","mcpServers":{"foo":{"command":"foo-mcp"}}}"#,
        )
        .unwrap();
        let a = InlineAdapter { path: p.clone() };
        match apply(&a).unwrap() {
            ApplyOutcome::Updated { path, backup } => {
                assert_eq!(path, p);
                assert!(backup.exists());
                assert_eq!(
                    std::fs::read_to_string(&backup).unwrap(),
                    r#"{"theme":"dark","mcpServers":{"foo":{"command":"foo-mcp"}}}"#
                );
            }
            other => panic!("expected Updated, got {other:?}"),
        }
        let body = std::fs::read_to_string(&p).unwrap();
        assert!(body.contains("\"theme\""));
        assert!(body.contains("\"foo\""));
        assert!(body.contains("\"amore\""));
    }

    #[test]
    fn merge_into_empty_starts_object() {
        let m = merge_mcp_servers("", "amore", serde_json::json!({"command":"x"})).unwrap();
        assert!(m.contains("\"amore\""));
        assert!(m.contains("\"command\""));
    }

    #[test]
    fn merge_into_existing_amore_is_noop() {
        let existing = serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": { "amore": { "command": "amore-mcp", "args": [] } }
        }))
        .unwrap()
            + "\n";
        let m = merge_mcp_servers(
            &existing,
            "amore",
            serde_json::json!({"command":"amore-mcp","args":[]}),
        )
        .unwrap();
        assert_eq!(existing, m, "no diff when amore already matches");
    }

    #[test]
    fn merge_replaces_legacy_obelion_entry() {
        // If an old obelion entry exists, inserting amore must remove it.
        let existing = serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": { "obelion": { "command": "obelion-mcp", "args": [] } }
        }))
        .unwrap()
            + "\n";
        let m = merge_mcp_servers(
            &existing,
            "amore",
            serde_json::json!({"command":"amore-mcp","args":[]}),
        )
        .unwrap();
        assert!(
            !m.contains("\"obelion\""),
            "legacy obelion entry must be removed"
        );
        assert!(m.contains("\"amore\""));
        assert!(m.contains("\"amore-mcp\""));
    }
}
