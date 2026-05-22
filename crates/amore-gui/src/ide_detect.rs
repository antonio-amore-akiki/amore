// crates/amore-gui/src/ide_detect.rs IDE auto-detect.
//
// Scans well-known config file locations for each of the 5 supported IDEs.
// Sources for config paths are cited per-IDE and in docs/IDE-AUTO-WIRE.md.
//
// Prior-art: Adopt — docs/prior-art-w8.5.md §7, state/prior-art-verdict.json.

use std::path::PathBuf;

/// Wire format for an IDE config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigFormat {
    /// JSON — Claude Desktop, Claude Code, Cursor, Cline.
    Json,
    /// YAML — Continue (config.yaml). mcpServers is an ARRAY in this format.
    Yaml,
}

/// A detected IDE install with the config file that will be wired.
#[derive(Debug, Clone)]
pub struct DetectedIde {
    pub name: String,
    pub path: PathBuf,
    pub config_format: ConfigFormat,
}

/// Detect all 5 IDEs. Returns only those whose config file exists on disk.
pub fn detect_all() -> Vec<DetectedIde> {
    detect_with_roots(dirs::home_dir(), dirs::data_dir())
}

/// Internal variant accepting injected root dirs for test isolation.
pub fn detect_with_roots(
    home: Option<PathBuf>,
    appdata_roaming: Option<PathBuf>,
) -> Vec<DetectedIde> {
    [
        detect_claude_desktop(&home, &appdata_roaming),
        detect_claude_code(&home),
        detect_cursor(&home),
        detect_cline(&home, &appdata_roaming),
        detect_continue_ide(&home),
    ]
    .into_iter()
    .flatten()
    .collect()
}

// ── Per-IDE detectors ─────────────────────────────────────────────────────────

/// Claude Desktop — source: https://modelcontextprotocol.io/quickstart/user
///   Windows:  %APPDATA%\Claude\claude_desktop_config.json
///   macOS:    ~/Library/Application Support/Claude/claude_desktop_config.json
///   Linux:    ~/.config/Claude/claude_desktop_config.json
fn detect_claude_desktop(
    home: &Option<PathBuf>,
    appdata_roaming: &Option<PathBuf>,
) -> Option<DetectedIde> {
    let path = if cfg!(target_os = "windows") {
        appdata_roaming.as_ref()?.join("Claude").join("claude_desktop_config.json")
    } else if cfg!(target_os = "macos") {
        home.as_ref()?
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude_desktop_config.json")
    } else {
        home.as_ref()?.join(".config").join("Claude").join("claude_desktop_config.json")
    };
    path.exists().then(|| DetectedIde {
        name: "Claude Desktop".to_string(),
        path,
        config_format: ConfigFormat::Json,
    })
}

/// Claude Code — source: https://code.claude.com/docs/en/mcp
///   All OS: ~/.claude/settings.json (global scope)
pub fn detect_claude_code(home: &Option<PathBuf>) -> Option<DetectedIde> {
    let path = home.as_ref()?.join(".claude").join("settings.json");
    path.exists().then(|| DetectedIde {
        name: "Claude Code".to_string(),
        path,
        config_format: ConfigFormat::Json,
    })
}

/// Cursor — source: https://forum.cursor.com/t/what-are-the-capabilities-of-mcp-json/63130
///   All OS: ~/.cursor/mcp.json
fn detect_cursor(home: &Option<PathBuf>) -> Option<DetectedIde> {
    let path = home.as_ref()?.join(".cursor").join("mcp.json");
    path.exists().then(|| DetectedIde {
        name: "Cursor".to_string(),
        path,
        config_format: ConfigFormat::Json,
    })
}

/// Cline (VS Code extension) — source: https://docs.cline.bot/mcp/configuring-mcp-servers
///   Windows: %APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json
///   macOS/Linux: equivalent under Application Support / .config
fn detect_cline(
    home: &Option<PathBuf>,
    appdata_roaming: &Option<PathBuf>,
) -> Option<DetectedIde> {
    let tail = [
        "Code",
        "User",
        "globalStorage",
        "saoudrizwan.claude-dev",
        "settings",
        "cline_mcp_settings.json",
    ];
    let base = if cfg!(target_os = "windows") {
        appdata_roaming.as_ref()?.clone()
    } else if cfg!(target_os = "macos") {
        home.as_ref()?.join("Library").join("Application Support")
    } else {
        home.as_ref()?.join(".config")
    };
    let path = tail.iter().fold(base, |p, seg| p.join(seg));
    path.exists().then(|| DetectedIde {
        name: "Cline".to_string(),
        path,
        config_format: ConfigFormat::Json,
    })
}

/// Continue — source: https://docs.continue.dev/customize/deep-dives/mcp
///   All OS: ~/.continue/config.yaml
///   NOTE: Continue uses an ARRAY for mcpServers (critical divergence from all others).
fn detect_continue_ide(home: &Option<PathBuf>) -> Option<DetectedIde> {
    let path = home.as_ref()?.join(".continue").join("config.yaml");
    path.exists().then(|| DetectedIde {
        name: "Continue".to_string(),
        path,
        config_format: ConfigFormat::Yaml,
    })
}
