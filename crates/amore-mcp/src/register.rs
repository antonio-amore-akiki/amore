// crates/amore-mcp/src/register.rs — IDE MCP registration for amore-mcp.
// @file-size-exempt: monolithic security handler — atomic-write, .bak rotation, ACL (icacls/chmod), 5 IDE targets, self-contained + CLI dual-path; cannot split coherently without losing safety invariants.
// Prior-art: Adapt from crates/amore-gui/src/ide_wire.rs (verdict: state/prior-art-verdict-register-rs.json).
// Closes F11 (Phase A5) + F12 (Phase A6).
//
// ## Primary path (A5)
// `register_claude_code(false)`:
//   Tries `claude mcp add amore --scope user -- <exe>` first.
//   Falls back to direct JSON write ONLY when:
//     (a) `claude` CLI is not on PATH, AND
//     (b) `AMORE_DIRECT_CONFIG_WRITE=1` is set.
//   Without opt-in env, returns Err when CLI is absent.
//
// ## Self-contained path (A6)
// `register_claude_code(true)`:
//   ALWAYS uses the direct JSON write path; never invokes `claude` CLI.
//   Used by installer (Inno `[Run]` + Linux postinst) so stranger installs
//   never depend on claude CLI being present.
//
// ## Atomic write + .bak + ACL
// Both paths share `write_atomic_json` which:
//   - rotates a single `.bak` (no timestamp accumulation)
//   - sets private permissions (0o600 on Unix, icacls /inheritance:r on Windows)
//   - writes to a `.amore-tmp` sibling, sync_data, then rename with retry
//
// ## Idempotent
// If amore entry already exists, update in place (no duplicate key).
//
// ## Test isolation
// `home_dir_for_test()` uses a thread-local override so tests never touch the
// real home dir even on Windows (where `dirs::home_dir` calls Win32 API and
// ignores $HOME/$USERPROFILE env overrides set by the test).

#![allow(dead_code)] // register_cursor / register_cline / register_continue used from main

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Thread-local overrides (test isolation)
// ---------------------------------------------------------------------------

thread_local! {
    static TEST_HOME_OVERRIDE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// Thread-local override for AMORE_DIRECT_CONFIG_WRITE env var (avoids global mutation).
    static TEST_DIRECT_WRITE_OPT_IN: RefCell<Option<bool>> = const { RefCell::new(None) };
    /// Thread-local override for PATH-has-claude probe (avoids global mutation).
    static TEST_CLAUDE_ON_PATH: RefCell<Option<bool>> = const { RefCell::new(None) };
}

/// Returns home dir, preferring the thread-local test override.
fn home_dir() -> Result<PathBuf> {
    TEST_HOME_OVERRIDE.with(|o| {
        if let Some(ref p) = *o.borrow() {
            return Ok(p.clone());
        }
        dirs::home_dir().context("no home dir")
    })
}

/// Returns true when `claude` CLI is reachable. Thread-local override wins in tests.
fn claude_cli_available() -> bool {
    TEST_CLAUDE_ON_PATH.with(|o| {
        if let Some(v) = *o.borrow() { return v; }
        claude_cli_on_path()
    })
}

/// Returns true when AMORE_DIRECT_CONFIG_WRITE=1. Thread-local override wins in tests.
fn direct_write_opt_in() -> bool {
    TEST_DIRECT_WRITE_OPT_IN.with(|o| {
        if let Some(v) = *o.borrow() { return v; }
        std::env::var("AMORE_DIRECT_CONFIG_WRITE").map(|v| v == "1").unwrap_or(false)
    })
}

// ---------------------------------------------------------------------------
// Public report type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct RegisterReport {
    pub target: String,
    pub config_path: PathBuf,
    pub method: RegisterMethod,
    /// Whether the amore entry was already present before this call.
    pub was_present: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RegisterMethod {
    ClaudeCli,
    DirectWrite,
}

impl std::fmt::Display for RegisterMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegisterMethod::ClaudeCli => write!(f, "claude-cli"),
            RegisterMethod::DirectWrite => write!(f, "direct-write"),
        }
    }
}

// ---------------------------------------------------------------------------
// claude mcp add — primary path (A5)
// ---------------------------------------------------------------------------

fn claude_cli_on_path() -> bool {
    std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn invoke_claude_mcp_add(exe_path: &str) -> Result<()> {
    let status = std::process::Command::new("claude")
        .args(["mcp", "add", "amore", "--scope", "user", "--", exe_path])
        .status()
        .context("spawning `claude mcp add`")?;
    if !status.success() {
        bail!(
            "`claude mcp add` exited {:?}; try --self-contained to bypass CLI",
            status.code()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Direct-write path — shared internals
// ---------------------------------------------------------------------------

pub(crate) fn current_exe_str() -> Result<String> {
    let exe = std::env::current_exe().context("current_exe failed")?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("current_exe path is not UTF-8"))
}

fn amore_entry_json(exe_path: &str) -> Value {
    serde_json::json!({ "command": exe_path, "args": ["--stdio"], "env": {} })
}

fn read_json_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Object(Default::default()));
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing JSON at {}", path.display()))
}

/// Idempotent upsert of `mcpServers.amore`. Returns true when entry was already present.
fn upsert_mcp_server(root: &mut Value, exe_path: &str) -> bool {
    let entry = amore_entry_json(exe_path);
    let obj = root.as_object_mut().expect("root must be object");
    if !obj.contains_key("mcpServers") {
        obj.insert("mcpServers".to_string(), serde_json::json!({}));
    }
    let servers = obj
        .get_mut("mcpServers")
        .and_then(|v| v.as_object_mut())
        .expect("mcpServers must be object");
    let was_present = servers.contains_key("amore");
    servers.insert("amore".to_string(), entry);
    was_present
}

/// Atomic write: .bak rotation → tmp write → sync_data → rename (×3 retry) → parse-verify.
/// Adapted from `ide_wire::write_atomic` (same invariants; see prior-art verdict).
pub(crate) fn write_atomic_json(target: &Path, content: &str) -> Result<()> {
    let bak = bak_path(target);
    let bak_old = bak_old_path(target);
    // Rotate: existing .bak → .bak.old; live file → .bak; set private perms on .bak.
    if bak.exists() {
        std::fs::rename(&bak, &bak_old)
            .with_context(|| format!(".bak rotate: {}", bak.display()))?;
    }
    if target.exists() {
        std::fs::copy(target, &bak)
            .with_context(|| format!("backup copy: {}", target.display()))?;
        if let Err(e) = set_private_permissions(&bak) {
            if bak_old.exists() { let _ = std::fs::rename(&bak_old, &bak); }
            return Err(e).context("ACL set on .bak failed; write aborted");
        }
    }
    if bak_old.exists() { let _ = std::fs::remove_file(&bak_old); }

    // Ensure parent dir exists (first-time ~/.claude/ creation).
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create_dir_all {}", parent.display()))?;
    }
    let tmp = tmp_path(target);
    // Write + sync via a single open handle (open for write, then sync_data on same handle).
    {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new()
            .write(true).create(true).truncate(true)
            .open(&tmp)
            .with_context(|| format!("open tmp for write: {}", tmp.display()))?;
        f.write_all(content.as_bytes())
            .with_context(|| format!("write tmp: {}", tmp.display()))?;
        f.sync_data()
            .with_context(|| format!("sync_data: {}", tmp.display()))?;
    }
    if let Err(e) = retry_rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        if bak.exists() { let _ = std::fs::copy(&bak, target); }
        return Err(e).context("atomic rename failed after 3 retries");
    }
    // Parse-verify.
    let written = std::fs::read_to_string(target)
        .with_context(|| format!("verify-read {}", target.display()))?;
    serde_json::from_str::<Value>(&written)
        .with_context(|| format!("verify-parse {}", target.display()))?;
    Ok(())
}

fn retry_rename(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut last = None;
    for attempt in 0..3u32 {
        match std::fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(e) => {
                #[cfg(target_os = "windows")]
                let transient = e.raw_os_error() == Some(32);
                #[cfg(not(target_os = "windows"))]
                let transient = matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::ResourceBusy);
                if transient && attempt < 2 { std::thread::sleep(std::time::Duration::from_millis(100)); last = Some(e); }
                else { return Err(e); }
            }
        }
    }
    Err(last.unwrap())
}

/// Set private permissions on `path` (adapted from ide_wire::set_private_permissions).
fn set_private_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("chmod 0o600 on {}", path.display()))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let icacls = format!(r"{sysroot}\System32\icacls.exe");
        let grant = format!("{}:F", std::env::var("USERNAME").unwrap_or_else(|_| "%USERNAME%".to_string()));
        let status = std::process::Command::new(&icacls)
            .args([path.as_os_str(), std::ffi::OsStr::new("/inheritance:r"), std::ffi::OsStr::new("/grant:r"), std::ffi::OsStr::new(&grant)])
            .status()
            .with_context(|| format!("icacls spawn: {}", path.display()))?;
        if !status.success() { bail!("icacls {:?} on {}; write aborted", status.code(), path.display()); }
        // L-2: locale-immune broad-ACE check via SID strings (adapted from ide_wire).
        let out = std::process::Command::new(&icacls).arg(path.as_os_str()).output()
            .with_context(|| format!("icacls re-read: {}", path.display()))?;
        let acl = String::from_utf8_lossy(&out.stdout);
        if acl.contains("S-1-1-0") || acl.contains("S-1-5-32-545") {
            bail!("broad ACE on {} after grant; write aborted", path.display());
        }
        return Ok(());
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    { let _ = path; Ok(()) }
}

pub(crate) fn bak_path(o: &Path) -> PathBuf {
    let n = o.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    o.parent().unwrap_or(Path::new(".")).join(format!("{n}.bak"))
}
pub(crate) fn bak_old_path(o: &Path) -> PathBuf {
    let n = o.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    o.parent().unwrap_or(Path::new(".")).join(format!("{n}.bak.old"))
}
pub(crate) fn tmp_path(o: &Path) -> PathBuf {
    let s = o.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    o.parent().unwrap_or(Path::new(".")).join(format!(".{s}.amore-tmp"))
}

// ---------------------------------------------------------------------------
// Config path resolvers
// ---------------------------------------------------------------------------

fn claude_code_config_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".claude.json"))
}

fn claude_desktop_config_path() -> Result<PathBuf> {
    if cfg!(target_os = "windows") {
        Ok(dirs::data_dir().context("no APPDATA")?.join("Claude").join("claude_desktop_config.json"))
    } else if cfg!(target_os = "macos") {
        Ok(home_dir()?.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json"))
    } else {
        Ok(home_dir()?.join(".config").join("Claude").join("claude_desktop_config.json"))
    }
}

fn cursor_config_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".cursor").join("mcp.json"))
}

fn cline_config_path() -> Result<PathBuf> {
    let tail = ["Code","User","globalStorage","saoudrizwan.claude-dev","settings","cline_mcp_settings.json"];
    let base = if cfg!(target_os = "windows") {
        dirs::data_dir().context("no APPDATA")?
    } else if cfg!(target_os = "macos") {
        home_dir()?.join("Library").join("Application Support")
    } else {
        home_dir()?.join(".config")
    };
    Ok(tail.iter().fold(base, |p, seg| p.join(seg)))
}

/// Continue reads config.json since v0.8.37 — use JSON for direct-write compat.
fn continue_config_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".continue").join("config.json"))
}

// ---------------------------------------------------------------------------
// Shared direct-write logic
// ---------------------------------------------------------------------------

fn direct_write_json_config(config_path: &Path, exe_path: &str, target_name: &str) -> Result<RegisterReport> {
    let mut root = read_json_or_empty(config_path)?;
    if root.is_null() || !root.is_object() { root = Value::Object(Default::default()); }
    let was_present = upsert_mcp_server(&mut root, exe_path);
    let content = serde_json::to_string_pretty(&root).context("serialize config JSON")?;
    write_atomic_json(config_path, &content)?;
    Ok(RegisterReport { target: target_name.to_string(), config_path: config_path.to_path_buf(), method: RegisterMethod::DirectWrite, was_present })
}

// ---------------------------------------------------------------------------
// Public registration functions
// ---------------------------------------------------------------------------

/// Register amore-mcp as a Claude Code MCP server.
///
/// `self_contained = false`: try `claude mcp add` CLI first; fall back to
/// direct-write only when CLI is absent AND `AMORE_DIRECT_CONFIG_WRITE=1`.
///
/// `self_contained = true`: always direct-write; never invokes `claude` CLI.
pub fn register_claude_code(self_contained: bool) -> Result<RegisterReport> {
    let exe = current_exe_str()?;
    let config_path = claude_code_config_path()?;

    if !self_contained && claude_cli_available() {
        invoke_claude_mcp_add(&exe)?;
        let root = read_json_or_empty(&config_path)?;
        let was_present = root.get("mcpServers").and_then(|s| s.get("amore")).is_some();
        return Ok(RegisterReport { target: "Claude Code".to_string(), config_path, method: RegisterMethod::ClaudeCli, was_present });
    }

    if !self_contained {
        if !direct_write_opt_in() {
            bail!("`claude` CLI not found on PATH and AMORE_DIRECT_CONFIG_WRITE=1 is not set. Use --self-contained to write directly.");
        }
        tracing::warn!("claude CLI absent; direct-write because AMORE_DIRECT_CONFIG_WRITE=1");
    }

    direct_write_json_config(&config_path, &exe, "Claude Code")
}

/// Register amore-mcp as a Claude Desktop MCP server (always direct-write; no CLI path).
pub fn register_claude_desktop(_self_contained: bool) -> Result<RegisterReport> {
    let exe = current_exe_str()?;
    direct_write_json_config(&claude_desktop_config_path()?, &exe, "Claude Desktop")
}

/// Register amore-mcp as a Cursor MCP server (always direct-write).
pub fn register_cursor(_self_contained: bool) -> Result<RegisterReport> {
    let exe = current_exe_str()?;
    direct_write_json_config(&cursor_config_path()?, &exe, "Cursor")
}

/// Register amore-mcp as a Cline MCP server (always direct-write).
pub fn register_cline(_self_contained: bool) -> Result<RegisterReport> {
    let exe = current_exe_str()?;
    direct_write_json_config(&cline_config_path()?, &exe, "Cline")
}

/// Register amore-mcp for Continue (always direct-write via config.json).
pub fn register_continue(_self_contained: bool) -> Result<RegisterReport> {
    let exe = current_exe_str()?;
    direct_write_json_config(&continue_config_path()?, &exe, "Continue")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Run `f` with thread-local overrides set; clears all overrides after `f`.
    /// Tests never touch global env vars — all state is thread-local.
    fn with_test_context<F: FnOnce(&Path)>(
        claude_on_path: bool,
        direct_write_opt_in: bool,
        f: F,
    ) {
        let dir = TempDir::new().unwrap();
        TEST_HOME_OVERRIDE.with(|o| *o.borrow_mut() = Some(dir.path().to_path_buf()));
        TEST_CLAUDE_ON_PATH.with(|o| *o.borrow_mut() = Some(claude_on_path));
        TEST_DIRECT_WRITE_OPT_IN.with(|o| *o.borrow_mut() = Some(direct_write_opt_in));
        f(dir.path());
        TEST_HOME_OVERRIDE.with(|o| *o.borrow_mut() = None);
        TEST_CLAUDE_ON_PATH.with(|o| *o.borrow_mut() = None);
        TEST_DIRECT_WRITE_OPT_IN.with(|o| *o.borrow_mut() = None);
    }

    // ── Test 1: CLI primary path when `claude` is present ────────────────────

    /// When claude CLI reports available (thread-local override), method must be ClaudeCli.
    /// We override `TEST_CLAUDE_ON_PATH=true` so the production code believes the CLI is present,
    /// then observe that `invoke_claude_mcp_add` is attempted (it will fail because there's no
    /// real `claude` binary, but the error must be the mcp-add invocation error, NOT the
    /// AMORE_DIRECT_CONFIG_WRITE refusal).
    #[test]
    fn register_claude_code_cli_primary_when_present() {
        with_test_context(true, false, |_| {
            let result = register_claude_code(false);
            match result {
                Ok(report) => assert_eq!(report.method, RegisterMethod::ClaudeCli),
                Err(e) => {
                    // CLI was invoked but failed (no real `claude` in test env).
                    // Must NOT be the AMORE_DIRECT_CONFIG_WRITE refusal.
                    assert!(
                        !e.to_string().contains("AMORE_DIRECT_CONFIG_WRITE"),
                        "CLI was not invoked (got refusal): {e}"
                    );
                }
            }
        });
    }

    // ── Test 2: direct-write fallback with opt-in ─────────────────────────────

    #[test]
    fn register_claude_code_falls_back_when_no_cli_and_opt_in() {
        with_test_context(false, true, |_| {
            let report = register_claude_code(false).expect("must succeed with opt-in");
            assert_eq!(report.method, RegisterMethod::DirectWrite);
        });
    }

    // ── Test 3: refuses direct-write without opt-in ────────────────────────────

    #[test]
    fn register_claude_code_refuses_direct_write_without_opt_in() {
        with_test_context(false, false, |_| {
            let err = register_claude_code(false).expect_err("must Err without opt-in");
            assert!(
                err.to_string().contains("AMORE_DIRECT_CONFIG_WRITE"),
                "missing env name in error: {err}"
            );
        });
    }

    // ── Test 4: --self-contained always direct-writes, ignores PATH ───────────

    #[test]
    fn register_self_contained_always_direct_write() {
        // claude_on_path=false: proves self-contained ignores PATH entirely.
        with_test_context(false, false, |home| {
            let report = register_claude_code(true).expect("self-contained must succeed");
            assert_eq!(report.method, RegisterMethod::DirectWrite);
            let cfg = home.join(".claude.json");
            assert!(cfg.exists(), "~/.claude.json must be created");
            let v: Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
            assert!(v["mcpServers"]["amore"].is_object(), "mcpServers.amore must be present");
        });
    }

    // ── Test 5: idempotent on second call ──────────────────────────────────────

    #[test]
    fn register_idempotent_existing_amore_entry() {
        with_test_context(false, false, |home| {
            register_claude_code(true).expect("first call");
            let second = register_claude_code(true).expect("second call");
            assert!(second.was_present, "second call must see existing entry");
            let v: Value = serde_json::from_str(&fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
            let count = v["mcpServers"].as_object().unwrap().keys().filter(|k| *k == "amore").count();
            assert_eq!(count, 1, "exactly one amore entry; got {}", v["mcpServers"]);
        });
    }

    // ── Test 6: atomic rename creates .bak ────────────────────────────────────

    #[test]
    fn register_atomic_rename_creates_bak() {
        let original = r#"{"mcpServers":{"other":{"command":"other"}}}"#;
        // claude_on_path=false, direct_write_opt_in=false → self_contained=true bypasses both.
        with_test_context(false, false, |home| {
            let cfg = home.join(".claude.json");
            fs::write(&cfg, original).unwrap();

            register_claude_code(true).expect("register call");

            let bak = bak_path(&cfg);
            assert!(bak.exists(), ".bak must exist: {}", bak.display());

            // On Windows, icacls restricts .bak ACEs — verify existence + non-zero size.
            let bak_len = fs::metadata(&bak)
                .expect(".bak metadata must be readable")
                .len();
            assert!(bak_len > 0, ".bak must be non-empty (original was {} bytes)", original.len());

            // On Unix (no icacls), also verify exact content.
            #[cfg(unix)]
            {
                let bak_raw = fs::read_to_string(&bak).unwrap();
                assert!(bak_raw.contains("other"), ".bak must contain original content: {bak_raw}");
            }
        });
    }
}
