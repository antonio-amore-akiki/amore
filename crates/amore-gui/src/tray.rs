// crates/amore-gui/src/tray.rs system tray icon.
//
// Uses tray-icon 0.24 (Tauri-maintained, Adopt per docs/prior-art-w8.5.md §6).
// Per prior-art note about winit standalone jank (winit #3835): uses
// tray-icon's own event loop pattern (event_loop::EventLoop from tray-icon).
//
// Menu items:
//   Open dashboard | Pause | Resume | Recent activity | Check for updates | Quit
//
// OS auto-start:
//   Windows: Run-registry-key (set by MSI installer; tray reads + offers disable)
//   macOS:   Login Items (manual; tray shows instruction on first launch)
//   Linux:   systemd-user unit amore-tray.service (tray offers enable if absent)

use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};
// reqwest::blocking is a workspace dependency (amore-gui/Cargo.toml line 30).
// serde_json and tracing are workspace deps already present in this crate.

/// Opaque handle that keeps the tray icon alive for the process lifetime.
pub struct TrayHandle {
    _icon: TrayIcon,
}

/// Menu item IDs for event dispatch.
pub mod item_id {
    pub const DASHBOARD: &str = "dashboard";
    pub const PAUSE: &str = "pause";
    pub const RESUME: &str = "resume";
    pub const RECENT: &str = "recent";
    pub const UPDATES: &str = "updates";
    pub const QUIT: &str = "quit";
}

/// Build and show the tray icon. Returns the handle; drop it to remove the icon.
///
/// On failure (e.g., compositor/GTK absent) returns an error string so the caller
/// can log and degrade gracefully rather than panic.
pub fn spawn_tray() -> Result<TrayHandle, String> {
    let menu = build_menu().map_err(|e| format!("menu build failed: {e}"))?;
    let icon = load_tray_icon();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Amore — local AI memory")
        .with_icon(icon)
        .build()
        .map_err(|e| format!("tray icon build failed: {e}"))?;

    check_autostart_state(&tray);

    Ok(TrayHandle { _icon: tray })
}

/// Spawn the tray and run a blocking event loop that processes MenuEvents
/// from `tray_icon::menu::MenuEvent::receiver()`. Returns when
/// `handle_menu_event` returns true (Quit was clicked).
///
/// Used by the binary entrypoint for `amore-gui --tray` (MSI HKCU Run autostart).
pub fn run_tray_loop() -> Result<(), String> {
    let _handle = spawn_tray()?;
    let menu_receiver = tray_icon::menu::MenuEvent::receiver();
    loop {
        if let Ok(event) = menu_receiver.try_recv()
            && handle_menu_event(event.id.0.as_str())
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
}

// ── Menu construction ─────────────────────────────────────────────────────────

fn build_menu() -> Result<Menu, tray_icon::menu::Error> {
    let menu = Menu::new();
    menu.append(&MenuItem::with_id(
        item_id::DASHBOARD,
        "Open dashboard",
        true,
        None,
    ))?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(item_id::PAUSE, "Pause", true, None))?;
    menu.append(&MenuItem::with_id(item_id::RESUME, "Resume", true, None))?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(
        item_id::RECENT,
        "Recent activity",
        true,
        None,
    ))?;
    menu.append(&MenuItem::with_id(
        item_id::UPDATES,
        "Check for updates",
        true,
        None,
    ))?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&MenuItem::with_id(item_id::QUIT, "Quit", true, None))?;
    Ok(menu)
}

// ── Event dispatch (call from your event loop) ────────────────────────────────

/// Handle a tray menu click by item id string. Returns true if the app should quit.
pub fn handle_menu_event(id: &str) -> bool {
    match id {
        item_id::DASHBOARD => {
            open_url("http://localhost:3111");
            false
        }
        item_id::PAUSE => {
            eprintln!("[amore-tray] pause requested");
            false
        }
        item_id::RESUME => {
            eprintln!("[amore-tray] resume requested");
            false
        }
        item_id::RECENT => {
            eprintln!("[amore-tray] recent activity requested");
            false
        }
        item_id::UPDATES => {
            check_for_updates();
            false
        }
        item_id::QUIT => true,
        _ => false,
    }
}

// ── Auto-start ────────────────────────────────────────────────────────────────

fn check_autostart_state(tray: &TrayIcon) {
    #[cfg(target_os = "windows")]
    {
        let _ = tray; // unused on windows
        windows_autostart_check();
    }

    #[cfg(target_os = "macos")]
    macos_autostart_hint(tray);

    #[cfg(target_os = "linux")]
    {
        let _ = tray; // unused on linux
        linux_autostart_check();
    }
}

#[cfg(target_os = "windows")]
fn windows_autostart_check() {
    // The MSI installer writes the Run registry key.
    // Tray reads it and logs; the user can disable via the Windows Startup manager.
    // We do not write the key here — that is the installer's job ().
    use std::process::Command;
    let out = Command::new("reg")
        .args([
            "query",
            r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "Amore",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            eprintln!("[amore-tray] auto-start is enabled (Run registry key present)");
        }
        _ => {
            eprintln!(
                "[amore-tray] auto-start not set (Run registry key absent — expected on first run; MSI sets it)"
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_autostart_hint(tray: &TrayIcon) {
    // macOS Login Items must be added by the user in System Settings > General > Login Items.
    // We surface a one-time hint rather than attempting programmatic insertion.
    const HINT: &str =
        "To start Amore at login: System Settings > General > Login Items > add Amore";

    // Structured log — captured by any tracing subscriber wired at startup.
    tracing::info!("[amore-tray] {}", HINT);

    // Set hint as tray tooltip so it's visible on hover.
    let _ = tray.set_tooltip(Some(HINT));

    // Write a one-shot first-run file so the hint is user-reachable even
    // without a tray tooltip (e.g., headless CI or unsupported compositor).
    if let Some(cfg_dir) = dirs::config_dir() {
        let first_run = cfg_dir.join("amore").join("first-run.txt");
        if !first_run.exists() {
            if let Some(parent) = first_run.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(
                &first_run,
                format!(
                    "Amore first-run note:\n{HINT}\n\n\
                     You can delete this file — it will not be recreated.\n"
                ),
            );
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_autostart_check() {
    // The .deb/.rpm installer drops amore-tray.service in ~/.config/systemd/user/.
    // Offer to enable if not already active.
    let out = std::process::Command::new("systemctl")
        .args(["--user", "is-active", "amore-tray.service"])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            eprintln!("[amore-tray] systemd unit amore-tray.service is active");
        }
        _ => {
            eprintln!(
                "[amore-tray] Run: systemctl --user enable --now amore-tray.service  to auto-start at login"
            );
        }
    }
}

// ── Update check ──────────────────────────────────────────────────────────────

/// Sanitize a GitHub release tag_name to `[a-zA-Z0-9._-]` only.
///
/// Returns `Some(sanitized)` when the input is non-empty and every character
/// passes the allow-list; returns `None` on empty input or any rejected character.
pub(crate) fn sanitize_tag_name(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return None;
    }
    let s: String = raw.chars().collect();
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
    {
        Some(s)
    } else {
        None
    }
}

fn check_for_updates() {
    // User-Agent interpolation is build-time only (env! macro) — safe.
    let user_agent = format!("amore-tray/{}", env!("CARGO_PKG_VERSION"));
    let current = env!("CARGO_PKG_VERSION");

    std::thread::spawn(move || {
        let url = "https://api.github.com/repos/antonio-amore-akiki/amore/releases/latest";

        let client = match reqwest::blocking::Client::builder()
            .user_agent(&user_agent)
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "update check: failed to build HTTP client");
                return;
            }
        };

        let resp = client
            .get(url)
            .header("Accept", "application/vnd.github.v3+json")
            .send();

        match resp {
            Ok(r) if r.status().is_success() => {
                match r.json::<serde_json::Value>() {
                    Ok(json) => {
                        let raw_tag = json
                            .get("tag_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        match sanitize_tag_name(raw_tag) {
                            None => {
                                tracing::warn!(
                                    raw_tag = raw_tag,
                                    "update check: tag_name failed sanitization — \
                                     contains non-alphanumeric characters or is empty"
                                );
                            }
                            Some(latest) => {
                                if latest != format!("v{current}") {
                                    tracing::info!(
                                        latest = %latest,
                                        current = %current,
                                        "update available"
                                    );
                                } else {
                                    tracing::info!(version = %current, "up to date");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "update check: failed to parse JSON response");
                    }
                }
            }
            Ok(r) => {
                tracing::warn!(
                    status = %r.status(),
                    "update check: GitHub API returned non-success status"
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "update check failed (no network or GitHub unreachable)");
            }
        }
    });
}

// ── Icon helper ───────────────────────────────────────────────────────────────

fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

fn load_tray_icon() -> tray_icon::Icon {
    // 16x16 RGBA solid-colour placeholder. Replace with branding asset before ship.
    let rgba: Vec<u8> = (0..16 * 16u32)
        .flat_map(|_| [138u8, 43, 226, 255])
        .collect();
    tray_icon::Icon::from_rgba(rgba, 16, 16).expect("static RGBA icon data is valid")
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::sanitize_tag_name;

    #[test]
    fn sanitize_tag_name_accepts_valid() {
        // Typical semver release tags — all must pass.
        assert_eq!(sanitize_tag_name("v1.2.3"), Some("v1.2.3".to_string()));
        assert_eq!(sanitize_tag_name("1.0.0"), Some("1.0.0".to_string()));
        assert_eq!(sanitize_tag_name("v0.4.0-rc.1"), Some("v0.4.0-rc.1".to_string()));
        assert_eq!(
            sanitize_tag_name("release_2025"),
            Some("release_2025".to_string())
        );
    }

    #[test]
    fn sanitize_tag_name_rejects_invalid() {
        // Any character outside [a-zA-Z0-9._-] must be rejected.
        assert_eq!(sanitize_tag_name(""), None, "empty input must return None");
        assert_eq!(
            sanitize_tag_name("v1.0.0; rm -rf /"),
            None,
            "shell metacharacters must be rejected"
        );
        assert_eq!(
            sanitize_tag_name("v1.0.0\n"),
            None,
            "newline must be rejected"
        );
        assert_eq!(
            sanitize_tag_name("v1.0.0<script>"),
            None,
            "angle brackets must be rejected"
        );
        assert_eq!(
            sanitize_tag_name("v1.0.0 beta"),
            None,
            "space must be rejected"
        );
    }
}
