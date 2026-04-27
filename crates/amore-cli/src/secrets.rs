//! Secrets storage via OS keyring (Windows Credential Manager / Linux Secret Service).
//!
//! Prior-art: Adopt — thin adapter wiring `keyring` (docs.rs/keyring/3.x) and
//! `rpassword` (docs.rs/rpassword/7.x). No storage logic is implemented here.
//!
//! Fallback: `$config_dir/amore/secrets.toml` (mode 0600 on Linux) when keyring backend
//! is unavailable (e.g., headless Linux without Secret Service daemon).
//! Windows ACL check is pending — see docs/SECRETS.md.

use anyhow::{anyhow, Context, Result};
use keyring::Entry;
use std::fs;
use std::path::PathBuf;

const SERVICE: &str = "amore";

/// Prompt for a secret (no-echo) and persist it in the OS keyring.
pub fn set_password(name: &str) -> Result<()> {
    let secret = rpassword::prompt_password(format!("Enter secret for '{}': ", name))?;
    let entry = Entry::new(SERVICE, name).context("keyring entry create failed")?;
    entry.set_password(&secret).context("keyring write failed")?;
    println!("Secret '{}' stored in OS keyring.", name);
    Ok(())
}

/// Retrieve a secret from the OS keyring; falls back to secrets.toml when keyring fails.
pub fn get_password(name: &str) -> Result<String> {
    let entry = Entry::new(SERVICE, name).context("keyring entry create failed")?;
    match entry.get_password() {
        Ok(p) => Ok(p),
        Err(_) => get_from_file(name),
    }
}

fn secrets_file_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().ok_or_else(|| anyhow!("no config dir found"))?;
    Ok(dir.join("amore").join("secrets.toml"))
}

fn get_from_file(name: &str) -> Result<String> {
    let path = secrets_file_path()?;
    if !path.exists() {
        return Err(anyhow!(
            "Secret '{}' not found in keyring or {}",
            name,
            path.display()
        ));
    }

    // Permission check — Linux only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&path)?;
        if meta.permissions().mode() & 0o077 != 0 {
            eprintln!(
                "WARNING: {} has loose permissions (expected 0600). Tighten with `chmod 600 {}`.",
                path.display(),
                path.display()
            );
        }
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let table: toml::Value = toml::from_str(&content)
        .with_context(|| format!("parsing TOML from {}", path.display()))?;
    table
        .get(name)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("Key '{}' not found in {}", name, path.display()))
}
