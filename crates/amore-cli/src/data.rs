// crates/amore-cli/src/data.rs — GDPR data erasure (A8, F22).
//
// `amore data erase` — dry-run (default): list all data stores Amore owns.
// `amore data erase --confirm` — destroy them all, log receipt to stderr.
//
// Erased surfaces:
//   1. SQLite store (amore.db + WAL siblings: amore.db-shm, amore.db-wal)
//   2. Qdrant storage dir (%LOCALAPPDATA%\Amore\qdrant\storage on Windows,
//      ~/.local/share/Amore/qdrant/storage on Linux,
//      ~/Library/Application Support/Amore/qdrant/storage on macOS)
//   3. WAL sled dir (AMORE_DATA_DIR/wal or default)
//   4. OS keyring entries: machine-key + key-fingerprint (keyring 3.x)
//   5. Crash dumps directory (AMORE_CRASH_DIR or default)
//   6. Windows-only: registry key HKCU\Software\Amore
//
// Without --confirm: prints a numbered list of what WOULD be deleted; exits 0.
// With --confirm: deletes, prints a receipt to stderr with counts + SHA-256
//                 fingerprint of the pre-deletion manifest. Exits 1 on any error.
//
// Design: Build — no existing implementation in amore-cli. Prior-art verdict:
// state/prior-art-verdict.json 2026-05-28.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Keyring constants — must match secrets.rs SERVICE name.
// ---------------------------------------------------------------------------

const KEYRING_SERVICE: &str = "amore";
const KEYRING_KEYS: &[&str] = &["machine-key", "key-fingerprint"];

// ---------------------------------------------------------------------------
// Entry type for the pre-deletion manifest
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) enum Target {
    File(PathBuf),
    Dir(PathBuf),
    KeyringEntry { service: String, account: String },
    RegistryKey(String),
}

impl Target {
    fn label(&self) -> String {
        match self {
            Target::File(p) => format!("file: {}", p.display()),
            Target::Dir(p) => format!("dir:  {}", p.display()),
            Target::KeyringEntry { service, account } => {
                format!("keyring: {service}/{account}")
            }
            Target::RegistryKey(k) => format!("registry: {k}"),
        }
    }

    fn exists(&self) -> bool {
        match self {
            Target::File(p) => p.exists(),
            Target::Dir(p) => p.exists(),
            // Can't probe keyring/registry without attempting; treat as present.
            Target::KeyringEntry { .. } | Target::RegistryKey(_) => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Resolve data dir (mirrors amore-cli/src/main.rs)
// ---------------------------------------------------------------------------

fn resolve_data_dir() -> PathBuf {
    if let Ok(v) = std::env::var("AMORE_DATA_DIR").or_else(|_| std::env::var("OBELION_DATA_DIR")) {
        return PathBuf::from(v);
    }
    dirs::config_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Amore")
}

fn resolve_local_app_data() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public\AppData\Local"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        dirs::data_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn resolve_crash_dir() -> PathBuf {
    if let Ok(v) = std::env::var("AMORE_CRASH_DIR") {
        return PathBuf::from(v);
    }
    resolve_local_app_data().join("Amore").join("crashes")
}

// ---------------------------------------------------------------------------
// Build the manifest of everything Amore owns
// ---------------------------------------------------------------------------

fn build_targets() -> Vec<Target> {
    let data_dir = resolve_data_dir();
    let local_dir = resolve_local_app_data();

    let mut targets: Vec<Target> = Vec::new();

    // 1. SQLite store + WAL siblings
    let sqlite = data_dir.join("amore.db");
    targets.push(Target::File(sqlite.clone()));
    targets.push(Target::File(sqlite.with_extension("db-wal")));
    targets.push(Target::File(sqlite.with_extension("db-shm")));

    // 2. Qdrant embedded storage dir
    targets.push(Target::Dir(
        local_dir.join("Amore").join("qdrant").join("storage"),
    ));

    // 3. WAL sled dir (streaming_ingest.rs default path)
    targets.push(Target::Dir(data_dir.join("wal")));

    // 4. Entire data dir (catches sled DB + any future sub-paths)
    targets.push(Target::Dir(data_dir.clone()));

    // 5. OS keyring entries
    for &account in KEYRING_KEYS {
        targets.push(Target::KeyringEntry {
            service: KEYRING_SERVICE.to_string(),
            account: account.to_string(),
        });
    }

    // 6. Crash dumps directory
    targets.push(Target::Dir(resolve_crash_dir()));

    // 7. Windows registry key (no-op on non-Windows)
    #[cfg(target_os = "windows")]
    targets.push(Target::RegistryKey(
        r"HKCU\Software\Amore".to_string(),
    ));

    targets
}

// ---------------------------------------------------------------------------
// SHA-256 fingerprint of the pre-deletion manifest
// ---------------------------------------------------------------------------

fn manifest_fingerprint(targets: &[Target]) -> String {
    let mut h = Sha256::new();
    for t in targets {
        h.update(t.label().as_bytes());
        h.update(b"\n");
    }
    hex::encode(h.finalize())
}

// ---------------------------------------------------------------------------
// Dry run — list only
// ---------------------------------------------------------------------------

pub fn cmd_data_erase_dry(targets: &[Target]) {
    println!("Amore data-erasure dry run — would delete:");
    let mut n = 0usize;
    for t in targets {
        if t.exists() {
            println!("  [{n:>3}] {}", t.label());
            n += 1;
        }
    }
    if n == 0 {
        println!("  (nothing to delete — no Amore data found on this system)");
    } else {
        println!(
            "\nRun with --confirm to permanently erase {n} item(s). This action is IRREVERSIBLE."
        );
    }
}

// ---------------------------------------------------------------------------
// Erase — with --confirm
// ---------------------------------------------------------------------------

pub fn cmd_data_erase(targets: &[Target]) -> Result<()> {
    // Build pre-deletion fingerprint BEFORE touching anything.
    let fingerprint = manifest_fingerprint(targets);

    let mut files_deleted = 0usize;
    let mut dirs_deleted = 0usize;
    let mut keyring_deleted = 0usize;
    let mut registry_deleted = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for t in targets {
        match t {
            Target::File(p) => {
                if p.exists() {
                    match std::fs::remove_file(p)
                        .with_context(|| format!("remove file {}", p.display()))
                    {
                        Ok(()) => {
                            files_deleted += 1;
                            tracing::info!(path = %p.display(), "erased file");
                        }
                        Err(e) => {
                            errors.push(format!("{e:#}"));
                            tracing::warn!(path = %p.display(), error = %e, "erase file failed");
                        }
                    }
                }
            }
            Target::Dir(p) => {
                if p.exists() {
                    match std::fs::remove_dir_all(p)
                        .with_context(|| format!("remove dir {}", p.display()))
                    {
                        Ok(()) => {
                            dirs_deleted += 1;
                            tracing::info!(path = %p.display(), "erased dir");
                        }
                        Err(e) => {
                            errors.push(format!("{e:#}"));
                            tracing::warn!(path = %p.display(), error = %e, "erase dir failed");
                        }
                    }
                }
            }
            Target::KeyringEntry { service, account } => {
                match keyring::Entry::new(service, account)
                    .context("keyring entry create")
                    .and_then(|e| e.delete_credential().context("keyring delete"))
                {
                    Ok(()) => {
                        keyring_deleted += 1;
                        tracing::info!(service, account, "erased keyring entry");
                    }
                    Err(e) => {
                        // KeyringError::NoEntry is benign — nothing to delete.
                        let msg = e.to_string();
                        if !msg.contains("NoEntry") && !msg.contains("no entry") {
                            errors.push(format!("keyring {service}/{account}: {e:#}"));
                            tracing::warn!(service, account, error = %e, "erase keyring failed");
                        } else {
                            tracing::debug!(service, account, "keyring entry not found — skipped");
                        }
                    }
                }
            }
            Target::RegistryKey(key) => {
                #[cfg(target_os = "windows")]
                {
                    erase_registry_key(key, &mut registry_deleted, &mut errors);
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = key;
                }
            }
        }
    }

    // Emit receipt to stderr.
    eprintln!(
        "[amore data erase] GDPR erasure receipt:\n\
         files={files_deleted} dirs={dirs_deleted} keyring={keyring_deleted} \
         registry={registry_deleted} errors={}\n\
         pre-deletion manifest SHA-256: {fingerprint}",
        errors.len()
    );

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("[amore data erase] ERROR: {e}");
        }
        anyhow::bail!("data erase completed with {} error(s)", errors.len());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Windows registry erasure helper
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn erase_registry_key(
    key: &str,
    registry_deleted: &mut usize,
    errors: &mut Vec<String>,
) {
    // winreg is not in the workspace — use the raw Windows API via std::process::Command
    // to avoid adding a new dep. `reg delete` is available on all Windows versions.
    // This path is Windows-only so the compilation guard prevents it on other OS.
    let status = std::process::Command::new("reg")
        .args(["delete", key, "/f"])
        .output();
    match status {
        Ok(out) if out.status.success() => {
            *registry_deleted += 1;
            tracing::info!(key, "erased registry key");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // "The system cannot find the key" (exit 1) = benign, key already absent.
            if stderr.contains("system cannot find") || stderr.contains("ERROR: The system cannot find") {
                tracing::debug!(key, "registry key not found — skipped");
            } else {
                errors.push(format!("registry {key}: {stderr}"));
                tracing::warn!(key, stderr = %stderr, "erase registry key failed");
            }
        }
        Err(e) => {
            errors.push(format!("registry {key}: spawn reg.exe: {e}"));
            tracing::warn!(key, error = %e, "erase registry key: spawn failed");
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point — called from main.rs
// ---------------------------------------------------------------------------

/// Entry point for `amore data erase [--confirm]`.
pub fn run(confirm: bool) -> Result<()> {
    let targets = build_targets();
    if confirm {
        cmd_data_erase(&targets)
    } else {
        cmd_data_erase_dry(&targets);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_targets_returns_non_empty() {
        let targets = build_targets();
        assert!(!targets.is_empty(), "must have at least one erasure target");
    }

    #[test]
    fn manifest_fingerprint_is_deterministic() {
        let targets = build_targets();
        let fp1 = manifest_fingerprint(&targets);
        let fp2 = manifest_fingerprint(&targets);
        assert_eq!(fp1, fp2, "fingerprint must be deterministic");
        assert_eq!(fp1.len(), 64, "SHA-256 hex must be 64 chars");
    }

    #[test]
    fn dry_run_does_not_panic() {
        let targets = build_targets();
        // Should complete without panic; output goes to stdout (captured in test).
        cmd_data_erase_dry(&targets);
    }
}
