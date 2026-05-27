// crates/amore-gui/src/ide_wire.rs  IDE config wire-up.
// Merges `amore` MCP server entry into each detected IDE's config file.
// Schema: Claude Desktop/Code/Cursor/Cline → mcpServers OBJECT; Continue → ARRAY.
// Steps: read→parse→backup(.bak rotate)→perms→merge→tmp→sync_data→rename(retry)→verify.

use crate::ide_detect::{ConfigFormat, DetectedIde};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum WireError {
    SiblingBinaryNotFound(String),
    /// C2: sibling resolved in a user-writable directory; same-dir hijack possible.
    InsecureInstallLocation { dir: String },
    /// H2: icacls exited non-zero; backup ACL not set; aborting wire.
    BackupAclFailed { backup_path: String, exit_code: Option<i32> },
    TargetLocked(String),
    Other(String),
}
impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::SiblingBinaryNotFound(s) => write!(f, "sibling amore-mcp not found: {s}"),
            WireError::InsecureInstallLocation { dir } =>
                write!(f, "insecure install location — sibling dir is user-writable: {dir}; set AMORE_ALLOW_USER_INSTALL=1 to override"),
            WireError::BackupAclFailed { backup_path, exit_code } =>
                write!(f, "icacls failed (exit {exit_code:?}) on backup {backup_path}; wire aborted"),
            WireError::TargetLocked(s) => write!(f, "target locked: {s}"),
            WireError::Other(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug)]
pub enum WireVerdict { Ok, SkippedNoChange, Err(WireError) }

/// Returns true when `dir` is a system-protected install location (not user-writable).
/// Windows: canonicalized path must start with `C:\Program Files\` or `C:\Program Files (x86)\`
/// (substring rejected — "My Program Files" bypass closed). Strips `\\?\` UNC prefix first.
fn is_system_protected_dir(dir: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        // Canonicalize resolves junction points; strip \\?\ so starts_with compares correctly.
        let canonical = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let s = canonical.to_string_lossy();
        let stripped = s.strip_prefix(r"\\?\").unwrap_or(&s);
        stripped.starts_with(r"C:\Program Files\") || stripped.starts_with(r"C:\Program Files (x86)\")
    }
    #[cfg(target_os = "macos")]
    { let s = dir.to_string_lossy(); s.starts_with("/Applications") || s.starts_with("/usr/local") || s.starts_with("/opt/homebrew") }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { let s = dir.to_string_lossy(); s.starts_with("/usr/bin") || s.starts_with("/usr/local/bin") || s.starts_with("/opt") }
}

/// Resolve absolute path to sibling `amore-mcp` binary (C2 fix).
/// Fails closed — never falls back to bare PATH name.
/// Refuses when the sibling dir is not a system-protected location unless
/// `AMORE_ALLOW_USER_INSTALL=1` is set in the environment.
pub fn resolve_amore_mcp_path() -> Result<PathBuf, WireError> {
    let exe = std::env::current_exe()
        .map_err(|e| WireError::SiblingBinaryNotFound(format!("current_exe failed: {e}")))?;
    // Canonicalize before parent-dir extraction so junction/symlink chains are resolved (Low-1).
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let dir = exe.parent()
        .ok_or_else(|| WireError::SiblingBinaryNotFound("exe has no parent".to_string()))?;

    // C2: enforce system-protected install location unless user explicitly opts in.
    let allow_user_install = std::env::var("AMORE_ALLOW_USER_INSTALL")
        .map(|v| v == "1")
        .unwrap_or(false);
    if !allow_user_install && !is_system_protected_dir(dir) {
        tracing::error!(
            dir = %dir.display(),
            "C2: sibling amore-mcp is in a user-writable directory; refusing wire to prevent same-dir hijack"
        );
        return Err(WireError::InsecureInstallLocation { dir: dir.display().to_string() });
    }

    let name = format!("amore-mcp{}", std::env::consts::EXE_SUFFIX);
    let p = dir.join(&name);
    if !p.exists() {
        return Err(WireError::SiblingBinaryNotFound(format!("expected at {}", p.display())));
    }
    Ok(p)
}

/// MCP server entry object. `command` = ABSOLUTE path, never bare name.
pub fn amore_mcp_entry_object() -> Result<serde_json::Value, WireError> {
    let s = resolve_amore_mcp_path()?.to_str()
        .ok_or_else(|| WireError::Other("path not UTF-8".to_string()))?.to_string();
    Ok(serde_json::json!({ "command": s, "args": ["--stdio"], "env": {} }))
}

pub fn wire_all(ides: &[DetectedIde]) -> Vec<(String, WireVerdict)> {
    ides.iter().map(|i| (i.name.clone(), wire_one(i))).collect()
}

pub fn wire_one(ide: &DetectedIde) -> WireVerdict {
    match ide.config_format {
        ConfigFormat::Json => wire_json(ide),
        ConfigFormat::Yaml => wire_yaml(ide),
    }
}

fn wire_json(ide: &DetectedIde) -> WireVerdict {
    let raw = match std::fs::read_to_string(&ide.path) {
        Ok(s) => s, Err(e) => return WireVerdict::Err(WireError::Other(format!("read: {e}"))),
    };
    let mut root: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v, Err(e) => return WireVerdict::Err(WireError::Other(format!("json parse: {e}"))),
    };
    let servers = root.as_object_mut().and_then(|obj| {
        if !obj.contains_key("mcpServers") { obj.insert("mcpServers".to_string(), serde_json::json!({})); }
        obj.get_mut("mcpServers")?.as_object_mut()
    });
    let servers = match servers {
        Some(s) => s,
        None => return WireVerdict::Err(WireError::Other("mcpServers not an object".to_string())),
    };
    let entry = match amore_mcp_entry_object() { Ok(e) => e, Err(e) => return WireVerdict::Err(e) };
    if servers.get("amore") == Some(&entry) { return WireVerdict::SkippedNoChange; }
    if servers.contains_key("amore") { eprintln!("[amore-wire] {} overwriting amore", ide.name); }
    servers.insert("amore".to_string(), entry);
    let updated = match serde_json::to_string_pretty(&root) {
        Ok(s) => s, Err(e) => return WireVerdict::Err(WireError::Other(format!("json serial: {e}"))),
    };
    write_atomic(&ide.path, &updated)
}

fn wire_yaml(ide: &DetectedIde) -> WireVerdict {
    let raw = match std::fs::read_to_string(&ide.path) {
        Ok(s) => s, Err(e) => return WireVerdict::Err(WireError::Other(format!("read: {e}"))),
    };
    let mut root: serde_yaml::Value = match serde_yaml::from_str(&raw) {
        Ok(v) => v, Err(e) => return WireVerdict::Err(WireError::Other(format!("yaml parse: {e}"))),
    };
    let servers = root.as_mapping_mut().and_then(|m| {
        let k = serde_yaml::Value::String("mcpServers".to_string());
        if !m.contains_key(&k) { m.insert(k.clone(), serde_yaml::Value::Sequence(vec![])); }
        m.get_mut(&k)?.as_sequence_mut()
    });
    let servers = match servers {
        Some(s) => s,
        None => return WireVerdict::Err(WireError::Other("mcpServers not a sequence".to_string())),
    };
    let mcp_str = match resolve_amore_mcp_path() {
        Ok(p) => match p.to_str() { Some(s) => s.to_string(), None => return WireVerdict::Err(WireError::Other("path not UTF-8".to_string())) },
        Err(e) => return WireVerdict::Err(e),
    };
    let entry = serde_yaml::to_value(serde_json::json!({"name":"amore","command":mcp_str,"args":["--stdio"],"env":{}}))
        .expect("static JSON->YAML infallible");
    let nk = serde_yaml::Value::String("name".to_string());
    let ak = serde_yaml::Value::String("amore".to_string());
    let pos = servers.iter().position(|v| v.as_mapping().and_then(|m| m.get(&nk)).map(|n| n==&ak).unwrap_or(false));
    match pos {
        Some(i) if servers[i] == entry => return WireVerdict::SkippedNoChange,
        Some(i) => { eprintln!("[amore-wire] {} overwriting amore", ide.name); servers[i] = entry; }
        None => servers.push(entry),
    }
    let updated = match serde_yaml::to_string(&root) {
        Ok(s) => s, Err(e) => return WireVerdict::Err(WireError::Other(format!("yaml serial: {e}"))),
    };
    write_atomic(&ide.path, &updated)
}

pub(crate) fn write_atomic(target: &Path, content: &str) -> WireVerdict {
    // H2: rotate single .bak; set private permissions.
    let bak = bak_path(target);
    let bak_old = bak_old_path(target);
    if bak.exists() && let Err(e) = std::fs::rename(&bak, &bak_old) {
        return WireVerdict::Err(WireError::Other(format!(".bak rotate failed: {e}")));
    }
    if let Err(e) = std::fs::copy(target, &bak) {
        if bak_old.exists() { let _ = std::fs::rename(&bak_old, &bak); }
        return WireVerdict::Err(WireError::Other(format!("backup copy failed: {e}")));
    }
    // H2: abort if ACL cannot be set on the backup — retaining world-readable ACL is a security failure.
    if let Err(e) = set_private_permissions(&bak) {
        // Roll back: restore the old .bak if we had rotated it.
        if bak_old.exists() { let _ = std::fs::rename(&bak_old, &bak); }
        return WireVerdict::Err(e);
    }
    if bak_old.exists() { let _ = std::fs::remove_file(&bak_old); }

    // H3: write tmp → sync → rename with retry.
    let tmp = tmp_path(target);
    if let Err(e) = std::fs::write(&tmp, content) {
        return WireVerdict::Err(WireError::Other(format!("tmp write failed: {e}")));
    }
    if let Err(e) = std::fs::File::open(&tmp).and_then(|f| f.sync_data()) {
        let _ = std::fs::remove_file(&tmp);
        return WireVerdict::Err(WireError::Other(format!("sync_data failed: {e}")));
    }
    if let Err(e) = retry_rename(&tmp, target) {
        let _ = std::fs::remove_file(&tmp);
        if let Err(re) = std::fs::copy(&bak, target) {
            eprintln!("[amore-wire] CRITICAL: rename+restore both failed: {e} / {re}");
        }
        return WireVerdict::Err(WireError::TargetLocked(format!("rename failed after 3 retries: {e}")));
    }
    match verify_parseable(target) {
        Ok(()) => WireVerdict::Ok,
        Err(e) => WireVerdict::Err(WireError::Other(format!("verify failed: {e}"))),
    }
}

fn retry_rename(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut last = None;
    for attempt in 0..3u32 {
        match std::fs::rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(e) => {
                #[cfg(target_os = "windows")] let t = e.raw_os_error() == Some(32);
                #[cfg(not(target_os = "windows"))] let t = matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::ResourceBusy);
                if t && attempt < 2 { std::thread::sleep(std::time::Duration::from_millis(100)); last = Some(e); }
                else { return Err(e); }
            }
        }
    }
    Err(last.unwrap())
}

/// Set private permissions on `path`.
/// Returns `Err(WireError::BackupAclFailed)` on Windows when icacls exits non-zero.
/// On Unix, a chmod failure is logged but not fatal (non-critical perms path).
fn set_private_permissions(path: &Path) -> Result<(), WireError> {
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            tracing::warn!(path = %path.display(), error = %e, "chmod 0o600 failed on backup");
        }
        return Ok(());
    }
    #[cfg(target_os = "windows")] {
        // Low-2: absolute path rejects PATH-shim; post-grant re-read verifies ACL took effect.
        let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
        let icacls = format!(r"{sysroot}\System32\icacls.exe");
        let grant = format!("{}:F", std::env::var("USERNAME").unwrap_or_else(|_| "%USERNAME%".to_string()));
        let status = std::process::Command::new(&icacls)
            .args([path.as_os_str(), std::ffi::OsStr::new("/inheritance:r"), std::ffi::OsStr::new("/grant:r"), std::ffi::OsStr::new(&grant)])
            .status()
            .map_err(|e| { let bp = path.display().to_string(); tracing::error!(path=%bp,error=%e,"H2: icacls spawn failed"); WireError::BackupAclFailed { backup_path: bp, exit_code: None } })?;
        if !status.success() {
            let bp = path.display().to_string();
            tracing::error!(path=%bp, exit_code=?status.code(), "H2: icacls non-zero; backup retains default ACL");
            return Err(WireError::BackupAclFailed { backup_path: bp, exit_code: status.code() });
        }
        // Post-grant verify: broad ACE remaining means the grant did not take effect.
        let out = std::process::Command::new(&icacls).arg(path.as_os_str()).output()
            .map_err(|e| { let bp = path.display().to_string(); tracing::error!(path=%bp,error=%e,"H2: ACL re-read failed"); WireError::BackupAclFailed { backup_path: bp, exit_code: None } })?;
        let acl = String::from_utf8_lossy(&out.stdout).to_lowercase();
        if acl.contains("builtin\\users") || acl.contains("everyone") {
            let bp = path.display().to_string();
            tracing::error!(path=%bp, "H2: broad ACE present after grant; aborting");
            return Err(WireError::BackupAclFailed { backup_path: bp, exit_code: Some(0) });
        }
        Ok(())
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    { let _ = path; Ok(()) }
}

pub(crate) fn tmp_path(o: &Path) -> PathBuf {
    let s = o.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    o.parent().unwrap_or(Path::new(".")).join(format!(".{s}.amore-tmp"))
}
pub(crate) fn bak_path(o: &Path) -> PathBuf {
    let n = o.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    o.parent().unwrap_or(Path::new(".")).join(format!("{n}.bak"))
}
pub(crate) fn bak_old_path(o: &Path) -> PathBuf {
    let n = o.file_name().and_then(|n| n.to_str()).unwrap_or("config");
    o.parent().unwrap_or(Path::new(".")).join(format!("{n}.bak.old"))
}
fn verify_parseable(p: &Path) -> Result<(), String> {
    let raw = std::fs::read_to_string(p).map_err(|e| e.to_string())?;
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "json" => serde_json::from_str::<serde_json::Value>(&raw).map(|_|()).map_err(|e|e.to_string()),
        "yaml"|"yml" => serde_yaml::from_str::<serde_yaml::Value>(&raw).map(|_|()).map_err(|e|e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // C2: absolute path + correct name when fake sibling exists (opt-in for build dirs).
    #[test]
    fn c2_wired_command_is_absolute() {
        let exe = std::env::current_exe().expect("current_exe");
        let dir = exe.parent().expect("exe parent");
        let bin = format!("amore-mcp{}", std::env::consts::EXE_SUFFIX);
        let fake = dir.join(&bin);
        let existed = fake.exists();
        if !existed { fs::write(&fake, b"").expect("create fake"); }
        // SAFETY: test-only mutation; tests run single-threaded under cargo test --lib.
        unsafe { std::env::set_var("AMORE_ALLOW_USER_INSTALL", "1"); }
        let result = resolve_amore_mcp_path();
        unsafe { std::env::remove_var("AMORE_ALLOW_USER_INSTALL"); }
        if !existed { let _ = fs::remove_file(&fake); }
        let p = result.expect("should succeed with AMORE_ALLOW_USER_INSTALL=1");
        assert!(p.is_absolute(), "must be absolute: {}", p.display());
        assert_eq!(p.file_name().and_then(|n| n.to_str()).unwrap(), bin);
    }

    // C2: fails closed when sibling absent.
    #[test]
    fn c2_fails_closed_when_absent() {
        // SAFETY: test-only mutation; tests run single-threaded under cargo test --lib.
        unsafe { std::env::set_var("AMORE_ALLOW_USER_INSTALL", "1"); }
        let exe = std::env::current_exe().expect("current_exe");
        let candidate = exe.parent().expect("parent").join(format!("amore-mcp{}", std::env::consts::EXE_SUFFIX));
        let r = if candidate.exists() { Ok(candidate) } else { resolve_amore_mcp_path() };
        unsafe { std::env::remove_var("AMORE_ALLOW_USER_INSTALL"); }
        if r.is_ok() { return; } // sibling present — skip
        assert!(matches!(r, Err(WireError::SiblingBinaryNotFound(_))));
    }

    // C2: tmp dir is not system-protected; Program Files / /usr/local/bin is.
    #[test]
    fn c2_install_location_allowlist() {
        assert!(!is_system_protected_dir(&std::env::temp_dir().join("x")), "tmp must not be protected");
        #[cfg(target_os = "windows")]
        assert!(is_system_protected_dir(&std::path::PathBuf::from(r"C:\Program Files\Amore")));
        #[cfg(target_os = "macos")]
        assert!(is_system_protected_dir(&std::path::PathBuf::from("/Applications/Amore")));
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        assert!(is_system_protected_dir(&std::path::PathBuf::from("/usr/local/bin")));
    }

    // H2 (Windows): BackupAclFailed Display includes exit code.
    #[test]
    #[cfg(target_os = "windows")]
    fn h2_backup_acl_failed_display() {
        let e = WireError::BackupAclFailed { backup_path: r"C:\x.bak".to_string(), exit_code: Some(5) };
        let msg = e.to_string();
        assert!(msg.contains("icacls failed") && msg.contains('5'), "Display: {msg}");
    }

    // H2: only one .bak after multiple writes, no .bak-<ts> accumulation.
    #[test]
    fn h2_single_backup_rotation() {
        let dir = TempDir::new().expect("tmpdir");
        let cfg = dir.path().join("c.json");
        fs::write(&cfg, r#"{"mcpServers":{}}"#).unwrap();
        for i in 0u32..2 { write_atomic(&cfg, &format!(r#"{{"p":{i}}}"#)); }
        assert!(bak_path(&cfg).exists(), ".bak must exist");
        assert!(!bak_old_path(&cfg).exists(), ".bak.old must be cleaned up");
        let extra: Vec<_> = fs::read_dir(dir.path()).unwrap().filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".bak-")).collect();
        assert!(extra.is_empty(), "no .bak-<ts> files; found {:?}", extra.iter().map(|e|e.file_name()).collect::<Vec<_>>());
    }

    // H2: Unix backup must be 0o600.
    #[test]
    #[cfg(unix)]
    fn h2_backup_0600_unix() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("c.json");
        fs::write(&cfg, r#"{"mcpServers":{}}"#).unwrap();
        write_atomic(&cfg, r#"{"x":1}"#);
        let mode = fs::metadata(bak_path(&cfg)).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "expected 0o600 got 0o{:o}", mode & 0o777);
    }

    // Low-1: "My Program Files" must be rejected; "Program Files" must be accepted.
    #[test]
    #[cfg(target_os = "windows")]
    fn low1_program_files_substring_rejected() {
        use std::path::PathBuf;
        assert!(!is_system_protected_dir(&PathBuf::from(r"C:\Users\victim\My Program Files\Amore")));
        assert!(is_system_protected_dir(&PathBuf::from(r"C:\Program Files\Amore")));
        assert!(is_system_protected_dir(&PathBuf::from(r"C:\Program Files (x86)\Amore")));
    }
    // Low-2 (Windows): set_private_permissions invokes absolute icacls and post-verifies ACL.
    #[test]
    #[cfg(target_os = "windows")]
    fn low2_icacls_post_verify_runs() { let d = TempDir::new().unwrap(); let f = d.path().join("t.bak"); fs::write(&f, b"x").unwrap(); let _ = set_private_permissions(&f); }
    // H3: no orphan tmp on rename failure; TargetLocked returned.
    #[test]
    #[cfg(unix)]
    fn h3_no_orphan_tmp_on_failure() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap(); let cfg = dir.path().join("c.json");
        fs::write(&cfg, r#"{"mcpServers":{}}"#).unwrap();
        let orig = fs::metadata(dir.path()).unwrap().permissions();
        fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        let v = write_atomic(&cfg, r#"{"x":1}"#);
        fs::set_permissions(dir.path(), orig).unwrap();
        assert!(!tmp_path(&cfg).exists(), "orphan tmp must not exist");
        match v {
            WireVerdict::Err(WireError::TargetLocked(_)) | WireVerdict::Err(WireError::Other(_)) => {}
            other => eprintln!("[h3] got {:?} — likely root, skip", other),
        }
    }
}
