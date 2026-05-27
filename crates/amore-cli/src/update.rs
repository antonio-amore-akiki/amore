// crates/amore-cli/src/update.rs — signed appcast auto-update via self_update.
//
// Public API:
//   check_for_update()   — poll GitHub releases API; respects AMORE_NO_AUTOUPDATE + 24h gate.
//   apply_update(status) — prompt via stdin then delegate to self_update to replace binary.
//
// Env overrides:
//   AMORE_NO_AUTOUPDATE=1   Skip check entirely; return UpdateStatus::Disabled.
//   AMORE_UPDATE_REPO       Override "antonio-amore-akiki/amore" (test hook).

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CHECK_INTERVAL_SECS: u64 = 60 * 60 * 24; // 24 hours
const DEFAULT_REPO_OWNER: &str = "antonio-amore-akiki";
const DEFAULT_REPO_NAME: &str = "amore";

/// Outcome of a release check.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Auto-update is disabled via AMORE_NO_AUTOUPDATE=1.
    Disabled,
    /// Binary is already at the latest released version.
    UpToDate,
    /// A newer version is available.
    Available { version: String },
    /// Check was skipped because 24h has not elapsed since the last check.
    TooSoon,
}

/// Return the platform path for the last-check timestamp file.
/// Windows: %LOCALAPPDATA%\Amore\.last-update-check
/// Other:   $XDG_CACHE_HOME/amore/.last-update-check (via dirs::cache_dir)
fn timestamp_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = dirs::data_local_dir() {
            return local.join("Amore").join(".last-update-check");
        }
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("amore")
        .join(".last-update-check")
}

/// Return true iff the 24h cooldown has elapsed (or the timestamp file is absent/corrupt).
fn cooldown_elapsed() -> bool {
    let path = timestamp_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            let stored: u64 = s.trim().parse().unwrap_or(0);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            now.saturating_sub(stored) >= CHECK_INTERVAL_SECS
        }
        Err(_) => true, // no file → first run
    }
}

/// Write the current unix timestamp to the cooldown file.
fn write_timestamp() {
    let path = timestamp_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let _ = std::fs::write(&path, now.to_string());
}

/// Resolve owner/repo; allow AMORE_UPDATE_REPO="owner/name" override (tests).
fn resolve_repo() -> (String, String) {
    if let Ok(env_repo) = std::env::var("AMORE_UPDATE_REPO") {
        let parts: Vec<&str> = env_repo.splitn(2, '/').collect();
        if parts.len() == 2 {
            return (parts[0].to_string(), parts[1].to_string());
        }
    }
    (DEFAULT_REPO_OWNER.to_string(), DEFAULT_REPO_NAME.to_string())
}

/// Check whether a newer release is available on GitHub.
///
/// Returns `UpdateStatus::Disabled` immediately when `AMORE_NO_AUTOUPDATE=1`.
/// Returns `UpdateStatus::TooSoon` when the 24h cooldown has not elapsed.
pub async fn check_for_update() -> Result<UpdateStatus> {
    if std::env::var("AMORE_NO_AUTOUPDATE").as_deref() == Ok("1") {
        return Ok(UpdateStatus::Disabled);
    }

    if !cooldown_elapsed() {
        return Ok(UpdateStatus::TooSoon);
    }

    let (owner, repo) = resolve_repo();
    let current_version = env!("CARGO_PKG_VERSION");

    // Spawn blocking self_update network call on a thread-pool thread.
    let status = tokio::task::spawn_blocking(move || -> Result<UpdateStatus> {
        let release = self_update::backends::github::Update::configure()
            .repo_owner(&owner)
            .repo_name(&repo)
            .bin_name("amore")
            .current_version(current_version)
            .build()
            .context("failed to build self_update config")?
            .get_latest_release()
            .context("failed to fetch latest GitHub release")?;

        let latest = release.version.trim_start_matches('v').to_string();
        if self_update::version::bump_is_greater(current_version, &latest)
            .unwrap_or(false)
        {
            Ok(UpdateStatus::Available { version: latest })
        } else {
            Ok(UpdateStatus::UpToDate)
        }
    })
    .await
    .context("spawn_blocking panicked")??;

    // Record timestamp regardless of outcome so noisy-network doesn't spam.
    write_timestamp();
    Ok(status)
}

/// Interactively prompt the user then apply the update.
///
/// In GUI mode (when `gui` is true) this function is a no-op stub — the GUI
/// tray should present its own notification. In CLI mode it reads from stdin.
pub async fn apply_update(status: UpdateStatus, gui: bool) -> Result<()> {
    let version = match status {
        UpdateStatus::Available { ref version } => version.clone(),
        _ => {
            println!("No update available.");
            return Ok(());
        }
    };

    if gui {
        // GUI tray owns update notifications; this code path is for CLI only.
        tracing::info!("update v{version} available — tray notification pending");
        return Ok(());
    }

    print!("A new version ({version}) is available. Apply now? [y/N] ");
    // Flush stdout before blocking read.
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Update skipped.");
        return Ok(());
    }

    let (owner, repo) = resolve_repo();
    let current_version = env!("CARGO_PKG_VERSION");

    tokio::task::spawn_blocking(move || -> Result<()> {
        self_update::backends::github::Update::configure()
            .repo_owner(&owner)
            .repo_name(&repo)
            .bin_name("amore")
            .current_version(current_version)
            .build()
            .context("failed to build self_update config")?
            .update()
            .context("self_update::update failed")?;
        Ok(())
    })
    .await
    .context("spawn_blocking panicked")??;

    println!("Updated to v{version}. Restart amore to use the new version.");
    Ok(())
}

/// Parse a GitHub release JSON fragment for version extraction (used in tests).
#[cfg(test)]
pub fn parse_release_version(json: &str) -> Option<String> {
    // Minimal parse: find `"tag_name":"v<ver>"` without pulling in full serde dep here.
    let key = "\"tag_name\":";
    let start = json.find(key)? + key.len();
    let rest = json[start..].trim_start_matches([' ', '"', 'v']);
    let end = rest.find('"').unwrap_or(rest.len());
    let ver = rest[..end].trim().to_string();
    if ver.is_empty() { None } else { Some(ver) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn update_respects_no_autoupdate_env() {
        // SAFETY: single-threaded test; no other thread reads AMORE_NO_AUTOUPDATE.
        unsafe { std::env::set_var("AMORE_NO_AUTOUPDATE", "1") };
        let result = check_for_update().await.expect("check_for_update failed");
        unsafe { std::env::remove_var("AMORE_NO_AUTOUPDATE") };
        assert_eq!(result, UpdateStatus::Disabled);
    }

    #[test]
    fn update_status_parses_release_json() {
        let json = r#"{"tag_name":"v1.2.3","name":"Release 1.2.3","body":""}"#;
        let version = parse_release_version(json).expect("expected a version");
        assert_eq!(version, "1.2.3");

        let json_with_space = r#"{"tag_name": "v2.0.0","name":"Release 2"}"#;
        let version2 = parse_release_version(json_with_space).expect("expected a version");
        assert_eq!(version2, "2.0.0");

        let bad = r#"{"name":"no tag here"}"#;
        assert!(parse_release_version(bad).is_none());
    }
}
