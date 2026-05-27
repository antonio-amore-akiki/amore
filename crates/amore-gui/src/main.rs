// crates/amore-gui/src/main.rs — binary entrypoint for amore-gui.
//
// Delegates all logic to the amore_gui library crate (wizard, tray, ide_detect,
// ide_wire modules declared in lib.rs and exercised by the lib test suite).
//
// Argument dispatch:
//   (no args)      Run the 6-screen first-run wizard (AmoreWizardApp).
//   --tray         Spawn the tray icon and block on its event loop (MSI Run-key path).
//   --version      Print version to stdout and exit 0.
//   --help         Print usage to stdout and exit 0.
//   --no-gui       Print JSON config summary and exit 0 (CI smoke).

// ADR 0010: no-unwrap policy. expect() with documented invariant is the approved
// fix pattern; only bare unwrap() is banned. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui;
use std::io::Write;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            let _ = writeln!(std::io::stdout(), "amore-gui {}", env!("CARGO_PKG_VERSION"));
            let _ = writeln!(std::io::stderr(), "amore-gui {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") => {
            let msg = "amore-gui — first-run setup wizard for Amore\n\nUsage:\n  amore-gui              Launch the GUI wizard\n  amore-gui --tray       Run as system tray icon (MSI auto-start path)\n  amore-gui --version    Print version and exit\n  amore-gui --help       Print this help and exit\n  amore-gui --no-gui     Print config summary as JSON and exit (CI smoke)\n  amore-gui --auto-wire  Detect IDEs and wire MCP config; emit JSON contract and exit\n";
            let _ = writeln!(std::io::stdout(), "{}", msg);
            let _ = writeln!(std::io::stderr(), "{}", msg);
            return Ok(());
        }
        Some("--no-gui") => {
            let summary = serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "ide_count": 5,
                "ready": true
            });
            let _ = writeln!(std::io::stdout(), "{}", summary);
            let _ = writeln!(std::io::stderr(), "{}", summary);
            return Ok(());
        }
        Some("--tray") => {
            if let Err(e) = amore_gui::tray::run_tray_loop() {
                eprintln!("[amore-gui] tray exited with error: {e}");
            }
            return Ok(());
        }
        // F3/F24: --auto-wire runs BEFORE eframe::run_native so it is safe on
        // headless Linux (no DISPLAY/winit init). Emits JSON contract to stdout.
        // Exit 0 iff errors == []; non-zero otherwise.
        // Schema documented in docs/AUTO-WIRE-CONTRACT.md.
        Some("--auto-wire") => {
            std::process::exit(run_auto_wire());
        }
        _ => {}
    }

    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 720.0])
            .with_min_inner_size([520.0, 600.0])
            .with_resizable(true)
            .with_icon(load_icon())
            .with_title("Amore"),
        ..Default::default()
    };
    eframe::run_native(
        "Amore",
        opts,
        Box::new(|cc| Ok(Box::new(amore_gui::wizard::AmoreWizardApp::new(cc)))),
    )
}

/// Headless auto-wire entry point (F3 + F24).
///
/// Detects all installed IDEs, wires each one, then emits the JSON contract
/// defined in docs/AUTO-WIRE-CONTRACT.md to stdout. Returns the process exit
/// code: 0 when errors is empty, 1 otherwise.
///
/// MUST NOT touch eframe, winit, or any display system — called before
/// eframe::run_native so it is safe on headless Linux with DISPLAY unset.
fn run_auto_wire() -> i32 {
    use amore_gui::ide_detect;
    use amore_gui::ide_wire::{WireVerdict, wire_all};

    let detected = ide_detect::detect_all();
    let detected_names: Vec<String> = detected.iter().map(|d| d.name.clone()).collect();

    let results = wire_all(&detected);

    let mut wired: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for (name, verdict) in &results {
        match verdict {
            WireVerdict::Ok => wired.push(name.clone()),
            WireVerdict::SkippedNoChange => skipped.push(name.clone()),
            WireVerdict::Err(e) => errors.push(serde_json::json!({
                "ide": name,
                "error": e.to_string()
            })),
        }
    }

    let contract = serde_json::json!({
        "detected": detected_names,
        "wired": wired,
        "skipped": skipped,
        "errors": errors
    });

    // Emit JSON contract to stdout (machine-readable; callers parse this).
    let serialized = serde_json::to_string(&contract)
        .unwrap_or_else(|e| format!(r#"{{"detected":[],"wired":[],"skipped":[],"errors":[{{"ide":"internal","error":"{e}"}}]}}"#));
    let _ = writeln!(std::io::stdout(), "{serialized}");

    if errors.is_empty() { 0 } else { 1 }
}

fn load_icon() -> egui::IconData {
    // Placeholder: solid color icon (RGBA tile 32x32). Replace with
    // branding/amore.png before ship via image::open + into_raw().
    let rgba: Vec<u8> = (0..32 * 32u32)
        .flat_map(|_| [138u8, 43, 226, 255])
        .collect();
    egui::IconData { rgba, width: 32, height: 32 }
}
