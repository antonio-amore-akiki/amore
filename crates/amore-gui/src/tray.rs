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

fn check_for_updates() {
    let current = env!("CARGO_PKG_VERSION");
    std::thread::spawn(move || {
        let url = "https://api.github.com/repos/antonio-amore-akiki/amore/releases/latest";
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-Command",
                &format!(
                    "(Invoke-RestMethod -Uri '{url}' -Headers @{{Accept='application/vnd.github.v3+json';'User-Agent'='amore-tray/{current}'}}).tag_name"
                ),
            ])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                let latest = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if latest.is_empty() {
                    eprintln!("[amore-tray] update check: no tag returned");
                } else if latest != format!("v{current}") {
                    eprintln!("[amore-tray] update available: {latest} (running v{current})");
                } else {
                    eprintln!("[amore-tray] up to date (v{current})");
                }
            }
            _ => eprintln!("[amore-tray] update check failed (no network or GitHub unreachable)"),
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
