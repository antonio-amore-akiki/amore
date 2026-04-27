// crates/amore-gui/tests/ide_wire_tests.rs — W8.5D IDE wire integration tests.
//
// Five tests: one per IDE config format. Each:
//   1. Writes a fake config file with pre-existing content.
//   2. Calls wire_one() against the fake config.
//   3. Asserts: backup file created, amore entry present, pre-existing entries preserved.

use amore_gui::ide_detect::{ConfigFormat, DetectedIde};
use amore_gui::ide_wire::{WireVerdict, wire_one};
use std::fs;

// ── Helper ────────────────────────────────────────────────────────────────────

fn make_ide(path: std::path::PathBuf, format: ConfigFormat) -> DetectedIde {
    DetectedIde {
        name: "TestIDE".to_string(),
        path,
        config_format: format,
    }
}

fn has_backup(original: &std::path::Path) -> bool {
    let parent = original.parent().expect("parent");
    let name = original.file_name().expect("name").to_string_lossy();
    for entry in fs::read_dir(parent).expect("read_dir").flatten() {
        let n = entry.file_name().to_string_lossy().to_string();
        if n.contains(&*name) && n.contains(".bak-") {
            return true;
        }
    }
    false
}

// ── Test 1: Claude Desktop (JSON, object mcpServers) ──────────────────────────

#[test]
fn wire_claude_desktop_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let cfg_path = tmp.path().join("claude_desktop_config.json");
    // Pre-existing entry under a different key.
    let initial = serde_json::json!({
        "mcpServers": {
            "other-tool": { "command": "other", "args": [] }
        }
    });
    fs::write(
        &cfg_path,
        serde_json::to_string_pretty(&initial).expect("serialize"),
    )
    .expect("write");

    let ide = make_ide(cfg_path.clone(), ConfigFormat::Json);
    let verdict = wire_one(&ide);
    assert!(
        matches!(verdict, WireVerdict::Ok),
        "wire failed: {verdict:?}"
    );

    // Backup present.
    assert!(has_backup(&cfg_path), "backup file missing");

    // Post-write: parse and assert.
    let raw = fs::read_to_string(&cfg_path).expect("read back");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse back");
    let servers = v["mcpServers"].as_object().expect("mcpServers object");
    assert!(servers.contains_key("amore"), "amore key missing");
    assert!(
        servers.contains_key("other-tool"),
        "pre-existing key clobbered"
    );
    assert_eq!(v["mcpServers"]["amore"]["command"], "amore-mcp");
}

// ── Test 2: Claude Code (JSON, object mcpServers) ─────────────────────────────

#[test]
fn wire_claude_code_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let cfg_path = tmp.path().join("settings.json");
    // Empty object — no mcpServers yet.
    fs::write(&cfg_path, b"{}").expect("write");

    let ide = make_ide(cfg_path.clone(), ConfigFormat::Json);
    let verdict = wire_one(&ide);
    assert!(
        matches!(verdict, WireVerdict::Ok),
        "wire failed: {verdict:?}"
    );

    assert!(has_backup(&cfg_path), "backup file missing");

    let raw = fs::read_to_string(&cfg_path).expect("read back");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse back");
    assert_eq!(v["mcpServers"]["amore"]["command"], "amore-mcp");
    assert_eq!(v["mcpServers"]["amore"]["args"][0], "--stdio");
}

// ── Test 3: Cursor (JSON, object mcpServers) ──────────────────────────────────

#[test]
fn wire_cursor_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let cfg_path = tmp.path().join("mcp.json");
    let initial = serde_json::json!({ "mcpServers": { "cursor-builtin": { "command": "cbi" } } });
    fs::write(
        &cfg_path,
        serde_json::to_string_pretty(&initial).expect("serialize"),
    )
    .expect("write");

    let ide = make_ide(cfg_path.clone(), ConfigFormat::Json);
    let verdict = wire_one(&ide);
    assert!(
        matches!(verdict, WireVerdict::Ok),
        "wire failed: {verdict:?}"
    );

    assert!(has_backup(&cfg_path), "backup file missing");

    let raw = fs::read_to_string(&cfg_path).expect("read back");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse back");
    let servers = v["mcpServers"].as_object().expect("mcpServers object");
    assert!(servers.contains_key("amore"), "amore key missing");
    assert!(
        servers.contains_key("cursor-builtin"),
        "pre-existing key clobbered"
    );
}

// ── Test 4: Cline (JSON, object mcpServers) ───────────────────────────────────

#[test]
fn wire_cline_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let cfg_path = tmp.path().join("cline_mcp_settings.json");
    let initial = serde_json::json!({
        "mcpServers": {
            "cline-native": { "command": "cn", "autoApprove": ["read_file"] }
        }
    });
    fs::write(
        &cfg_path,
        serde_json::to_string_pretty(&initial).expect("serialize"),
    )
    .expect("write");

    let ide = make_ide(cfg_path.clone(), ConfigFormat::Json);
    let verdict = wire_one(&ide);
    assert!(
        matches!(verdict, WireVerdict::Ok),
        "wire failed: {verdict:?}"
    );

    assert!(has_backup(&cfg_path), "backup file missing");

    let raw = fs::read_to_string(&cfg_path).expect("read back");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("parse back");
    let servers = v["mcpServers"].as_object().expect("mcpServers object");
    assert!(servers.contains_key("amore"), "amore key missing");
    assert!(
        servers.contains_key("cline-native"),
        "pre-existing Cline entry clobbered"
    );
}

// ── Test 5: Continue (YAML, ARRAY mcpServers) ─────────────────────────────────

#[test]
fn wire_continue_yaml() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let cfg_path = tmp.path().join("config.yaml");
    // Pre-existing array entry.
    let initial = "mcpServers:\n  - name: existing-server\n    command: existing\n    args: []\n";
    fs::write(&cfg_path, initial).expect("write");

    let ide = make_ide(cfg_path.clone(), ConfigFormat::Yaml);
    let verdict = wire_one(&ide);
    assert!(
        matches!(verdict, WireVerdict::Ok),
        "wire failed: {verdict:?}"
    );

    assert!(has_backup(&cfg_path), "backup file missing");

    let raw = fs::read_to_string(&cfg_path).expect("read back");
    let v: serde_yaml::Value = serde_yaml::from_str(&raw).expect("parse back");
    let servers = v["mcpServers"].as_sequence().expect("mcpServers sequence");

    let amore_entry = servers.iter().find(|e| {
        e.as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String("name".to_string())))
            .and_then(|n| n.as_str())
            .map(|s| s == "amore")
            .unwrap_or(false)
    });
    assert!(amore_entry.is_some(), "amore entry missing from YAML array");

    let existing = servers.iter().find(|e| {
        e.as_mapping()
            .and_then(|m| m.get(serde_yaml::Value::String("name".to_string())))
            .and_then(|n| n.as_str())
            .map(|s| s == "existing-server")
            .unwrap_or(false)
    });
    assert!(existing.is_some(), "pre-existing Continue entry clobbered");
}
