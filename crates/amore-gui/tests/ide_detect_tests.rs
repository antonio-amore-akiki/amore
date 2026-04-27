// crates/amore-gui/tests/ide_detect_tests.rs — W8.5D IDE detect integration tests.
//
// Five tests, one per IDE. Each writes a fake config file under a tempfile::TempDir
// and calls detect_with_roots() with the temp dir as the injected home/appdata root.
// Tests are hermetic — they never read or write the real user dirs.

use amore_gui::ide_detect::{detect_with_roots, ConfigFormat};
use std::fs;

// ── Helper: create a file and all parent dirs ─────────────────────────────────

fn touch(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("path has parent")).expect("create_dir_all");
    fs::write(path, b"{}").expect("write fixture");
}

// ── Test 1: Claude Desktop ────────────────────────────────────────────────────

#[test]
fn detect_claude_desktop_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let home = tmp.path().to_path_buf();

    // Windows path: appdata_roaming\Claude\claude_desktop_config.json
    // We pass the same temp dir for both home and appdata_roaming.
    let cfg = if cfg!(target_os = "windows") {
        home.join("Claude").join("claude_desktop_config.json")
    } else if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json")
    } else {
        home.join(".config").join("Claude").join("claude_desktop_config.json")
    };
    touch(&cfg);

    // On Windows pass home as appdata_roaming; on other OS pass None for appdata_roaming.
    #[cfg(target_os = "windows")]
    let ides = detect_with_roots(Some(home.clone()), Some(home.clone()));
    #[cfg(not(target_os = "windows"))]
    let ides = detect_with_roots(Some(home.clone()), None);

    let found = ides.iter().find(|i| i.name == "Claude Desktop");
    assert!(found.is_some(), "Claude Desktop not detected");
    let ide = found.expect("just checked");
    assert_eq!(ide.config_format, ConfigFormat::Json);
    assert_eq!(ide.path, cfg);
}

// ── Test 2: Claude Code ───────────────────────────────────────────────────────

#[test]
fn detect_claude_code_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let home = tmp.path().to_path_buf();
    let cfg = home.join(".claude").join("settings.json");
    touch(&cfg);

    let ides = detect_with_roots(Some(home), None);
    let found = ides.iter().find(|i| i.name == "Claude Code");
    assert!(found.is_some(), "Claude Code not detected");
    let ide = found.expect("just checked");
    assert_eq!(ide.config_format, ConfigFormat::Json);
    assert_eq!(ide.path, cfg);
}

// ── Test 3: Cursor ────────────────────────────────────────────────────────────

#[test]
fn detect_cursor_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let home = tmp.path().to_path_buf();
    let cfg = home.join(".cursor").join("mcp.json");
    touch(&cfg);

    let ides = detect_with_roots(Some(home), None);
    let found = ides.iter().find(|i| i.name == "Cursor");
    assert!(found.is_some(), "Cursor not detected");
    let ide = found.expect("just checked");
    assert_eq!(ide.config_format, ConfigFormat::Json);
    assert_eq!(ide.path, cfg);
}

// ── Test 4: Cline ─────────────────────────────────────────────────────────────

#[test]
fn detect_cline_json() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let home = tmp.path().to_path_buf();

    #[cfg(target_os = "windows")]
    let cfg = home.join("Code").join("User").join("globalStorage")
        .join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json");
    #[cfg(target_os = "macos")]
    let cfg = home.join("Library").join("Application Support").join("Code").join("User")
        .join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let cfg = home.join(".config").join("Code").join("User").join("globalStorage")
        .join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json");
    touch(&cfg);

    #[cfg(target_os = "windows")]
    let ides = detect_with_roots(Some(home.clone()), Some(home.clone()));
    #[cfg(not(target_os = "windows"))]
    let ides = detect_with_roots(Some(home.clone()), None);

    let found = ides.iter().find(|i| i.name == "Cline");
    assert!(found.is_some(), "Cline not detected");
    let ide = found.expect("just checked");
    assert_eq!(ide.config_format, ConfigFormat::Json);
    assert_eq!(ide.path, cfg);
}

// ── Test 5: Continue ──────────────────────────────────────────────────────────

#[test]
fn detect_continue_yaml() {
    let tmp = tempfile::TempDir::new().expect("TempDir");
    let home = tmp.path().to_path_buf();
    let cfg = home.join(".continue").join("config.yaml");
    fs::create_dir_all(cfg.parent().expect("parent")).expect("create_dir_all");
    fs::write(&cfg, b"mcpServers: []\n").expect("write fixture");

    let ides = detect_with_roots(Some(home), None);
    let found = ides.iter().find(|i| i.name == "Continue");
    assert!(found.is_some(), "Continue not detected");
    let ide = found.expect("just checked");
    assert_eq!(ide.config_format, ConfigFormat::Yaml);
    assert_eq!(ide.path, cfg);
}
