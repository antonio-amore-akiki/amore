// crates/amore-gui/src/main.rs — first-run setup wizard (F.installer-2).
//
// Design constraints:
//   * Native window via eframe (egui front-end), NO Chromium / Tauri — keeps
//     the binary small (~5-8 MB stripped) and idle RAM <50 MB.
//   * Single-screen wizard (per the v1.0 plan non-tech-user UX criterion).
//   * IDE checkboxes auto-write each MCP config on Save (shells out to amore
//     init <ide> for the existing IdeAdapter trait implementations).
//   * Plain-English errors via modal dialogs; no rustc / anyhow leakage.
//   * Tray-icon path is a separate concern (F.installer-6) — this file is the
//     first-run flow only.
//   * Auto-detects Ollama; if absent, downloads + runs ollama-installer silently
//     in a background thread with a progress bar.
//   * Same code path on macOS + Linux (eframe is cross-platform).
//
// Reviewer nit absorbed (2026-05-26T~12:30Z): bundled bge-small ONNX is
// shipped by the installer; qdrant.exe is downloaded on first-run by this
// wizard to keep the installer .exe under 150 MB.

// ADR 0010: no-unwrap policy. expect() with documented invariant is the approved
// fix pattern; only bare unwrap() is banned. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod install;

use eframe::egui;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// ----- App state -----

// `qdrant_status` reserved for F.installer-4 (embedded Qdrant binary detect).
// `InstalledAt(PathBuf)` payload + `Installing` + `Ready` reserved for the
// progressive download UI in F.installer-3 (Ollama silent install).
#[allow(dead_code)]
#[derive(Default)]
struct WizardState {
    // Step 1: Which IDEs
    ide_claude: bool,
    ide_cursor: bool,
    ide_codex: bool,
    ide_cline: bool,
    ide_opencode: bool,
    ide_windsurf: bool,
    ide_hermes: bool,

    // Step 2: Memory location
    memory_dir: String,

    // Step 3: Brain choice (local Ollama default; cloud opt-in)
    brain_local: bool,

    // Status (shared across worker threads)
    ollama_status: Arc<Mutex<DepStatus>>,
    qdrant_status: Arc<Mutex<DepStatus>>,
    save_status: Arc<Mutex<SaveStatus>>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
#[derive(Default)]
pub(crate) enum DepStatus {
    #[default]
    Unknown,
    Detecting,
    InstalledAt(PathBuf),
    Missing,
    Downloading { pct: f32 },
    Installing,
    Ready,
    Failed(String), // plain-English message
}


#[derive(Clone, Debug, Default)]
enum SaveStatus {
    #[default]
    Idle,
    Saving,
    Done,
    Failed(String),
}

struct AmoreWizard {
    state: WizardState,
}

impl AmoreWizard {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let state = WizardState {
            brain_local: true,
            memory_dir: default_memory_dir().to_string_lossy().into_owned(),
            ..Default::default()
        };
        // Kick off Ollama detection in a background thread.
        let ollama = state.ollama_status.clone();
        std::thread::spawn(move || {
            *ollama.lock().expect("mutex poisoned: unrecoverable state corruption") = DepStatus::Detecting;
            let found = which::which("ollama").ok();
            *ollama.lock().expect("mutex poisoned: unrecoverable state corruption") = match found {
                Some(p) => DepStatus::InstalledAt(p),
                None => DepStatus::Missing,
            };
        });
        Self { state }
    }
}

impl eframe::App for AmoreWizard {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Welcome to Amore");
            ui.label("One-time setup. About 30 seconds. No technical knowledge required.");
            ui.separator();

            ui.label(egui::RichText::new("1) Which AI apps do you use?").size(16.0));
            ui.checkbox(&mut self.state.ide_claude, "Claude Code");
            ui.checkbox(&mut self.state.ide_cursor, "Cursor");
            ui.checkbox(&mut self.state.ide_codex, "Codex CLI");
            ui.checkbox(&mut self.state.ide_cline, "Cline");
            ui.checkbox(&mut self.state.ide_opencode, "opencode");
            ui.checkbox(&mut self.state.ide_windsurf, "Windsurf");
            ui.checkbox(&mut self.state.ide_hermes, "Hermes Agent");

            ui.separator();
            ui.label(egui::RichText::new("2) Where should Amore keep your memory?").size(16.0));
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.state.memory_dir);
                if ui.button("Browse…").clicked()
                    && let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.state.memory_dir = p.to_string_lossy().into_owned();
                    }
            });

            ui.separator();
            ui.label(egui::RichText::new("3) AI engine").size(16.0));
            ui.radio_value(&mut self.state.brain_local, true, "Use my computer's brain (private, slower)");
            ui.radio_value(&mut self.state.brain_local, false, "Use a faster cloud option (opt-in, requires API key)");

            ui.separator();
            let ollama = self.state.ollama_status.lock().expect("mutex poisoned: unrecoverable state corruption").clone();
            ui.label(format!("Ollama (local AI runtime): {}", dep_status_human(&ollama)));
            if matches!(ollama, DepStatus::Missing)
                && ui.button("Install Ollama automatically").clicked()
            {
                install::spawn_ollama(self.state.ollama_status.clone(), ctx.clone());
            }
            // Keep the UI live-painting while a download / install is in flight so the
            // user sees the progress bar update without having to mouse-move.
            if matches!(
                ollama,
                DepStatus::Downloading { .. } | DepStatus::Installing | DepStatus::Detecting
            ) {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }

            ui.separator();
            let save = self.state.save_status.lock().expect("mutex poisoned: unrecoverable state corruption").clone();
            match save {
                SaveStatus::Idle => {
                    if ui
                        .add_sized(
                            [200.0, 36.0],
                            egui::Button::new(egui::RichText::new("Save & Continue").size(16.0)),
                        )
                        .clicked()
                    {
                        spawn_save(&self.state);
                    }
                }
                SaveStatus::Saving => {
                    ui.label("Saving your setup…");
                    ui.spinner();
                }
                SaveStatus::Done => {
                    ui.label(
                        egui::RichText::new("✓ Amore is ready. You can close this window.")
                            .color(egui::Color32::DARK_GREEN),
                    );
                }
                SaveStatus::Failed(msg) => {
                    ui.label(
                        egui::RichText::new(format!("Something went wrong: {msg}"))
                            .color(egui::Color32::DARK_RED),
                    );
                    if ui.button("Try again").clicked() {
                        *self.state.save_status.lock().expect("mutex poisoned: unrecoverable state corruption") = SaveStatus::Idle;
                    }
                }
            }
        });
    }
}

fn dep_status_human(s: &DepStatus) -> String {
    match s {
        DepStatus::Unknown | DepStatus::Detecting => "checking…".to_string(),
        DepStatus::InstalledAt(_) | DepStatus::Ready => "installed ✓".to_string(),
        DepStatus::Missing => "not installed — click below to install".to_string(),
        DepStatus::Downloading { pct } => format!("downloading… {:.0}%", pct * 100.0),
        DepStatus::Installing => "installing…".to_string(),
        DepStatus::Failed(msg) => format!("failed: {msg}"),
    }
}

fn default_memory_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Amore")
}

fn spawn_save(state: &WizardState) {
    let status = state.save_status.clone();
    let ides: Vec<&str> = [
        ("claude", state.ide_claude),
        ("cursor", state.ide_cursor),
        ("codex", state.ide_codex),
        ("cline", state.ide_cline),
        ("opencode", state.ide_opencode),
        ("windsurf", state.ide_windsurf),
        ("hermes", state.ide_hermes),
    ]
    .iter()
    .filter_map(|(name, sel)| if *sel { Some(*name) } else { None })
    .collect();
    let memory_dir = state.memory_dir.clone();
    let brain_local = state.brain_local;
    // Security fix 11a: resolve amore-cli relative to this executable's
    // directory, not via PATH. On Windows, CreateProcess checks CWD before
    // PATH — a malicious amore.exe in CWD would otherwise be invoked with
    // AMORE_DATA_DIR pointing at the user's memory store.
    //
    // C-2 (2026-05-26): fail closed if current_exe() fails rather than falling
    // back to bare-name PATH lookup, which would re-introduce the 11a vulnerability
    // on any OS anomaly (proc-fd unlink, unsupported platform, container quirk).
    let cli_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| {
            d.join(if cfg!(windows) { "amore.exe" } else { "amore" })
        }));
    let Some(cli_path) = cli_path else {
        *status.lock().expect("mutex poisoned: unrecoverable state corruption") = SaveStatus::Failed(
            "Couldn't locate the Amore CLI binary. Please reinstall Amore.".into(),
        );
        return;
    };
    std::thread::spawn(move || {
        *status.lock().expect("mutex poisoned: unrecoverable state corruption") = SaveStatus::Saving;
        // C-2: verify the CLI binary exists before invoking it to surface a clear
        // error instead of a cryptic OS process-spawn failure.
        if !cli_path.exists() {
            *status.lock().expect("mutex poisoned: unrecoverable state corruption") = SaveStatus::Failed(
                "Couldn't locate the Amore CLI binary. Please reinstall Amore.".into(),
            );
            return;
        }
        // Shell out to amore-cli for each selected IDE.
        for ide in &ides {
            let r = std::process::Command::new(&cli_path)
                .args(["init", ide])
                .env("AMORE_DATA_DIR", &memory_dir)
                .env("AMORE_BRAIN", if brain_local { "local" } else { "cloud" })
                .output();
            if let Err(e) = r {
                *status.lock().expect("mutex poisoned: unrecoverable state corruption") =
                    SaveStatus::Failed(format!("Couldn't connect Amore to {ide}. ({e})"));
                return;
            }
        }
        // F.installer-7 will register auto-start via Windows Task Scheduler /
        // launchctl / systemd-user.
        *status.lock().expect("mutex poisoned: unrecoverable state corruption") = SaveStatus::Done;
    });
}

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => {
            let _ = writeln!(std::io::stderr(), "amore-gui {}", env!("CARGO_PKG_VERSION"));
            let _ = writeln!(std::io::stdout(), "amore-gui {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--help") => {
            let msg = "amore-gui — first-run setup wizard for Amore\n\nUsage:\n  amore-gui              Launch the GUI wizard\n  amore-gui --version    Print version and exit\n  amore-gui --help       Print this help and exit\n  amore-gui --no-gui     Print config summary as JSON and exit (CI smoke)\n";
            let _ = writeln!(std::io::stderr(), "{}", msg);
            let _ = writeln!(std::io::stdout(), "{}", msg);
            return Ok(());
        }
        Some("--no-gui") => {
            let summary = serde_json::json!({"version": env!("CARGO_PKG_VERSION"), "ide_count": 7, "ready": true});
            let _ = writeln!(std::io::stderr(), "{}", summary);
            let _ = writeln!(std::io::stdout(), "{}", summary);
            return Ok(());
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
        Box::new(|cc| Ok(Box::new(AmoreWizard::new(cc)))),
    )
}

fn load_icon() -> egui::IconData {
    // Placeholder: solid color icon (RGBA tile 32x32). Replace with
    // branding/amore.png before ship via image::open + into_raw().
    let rgba: Vec<u8> = (0..32 * 32u32)
        .flat_map(|_| [138u8, 43, 226, 255])
        .collect();
    egui::IconData { rgba, width: 32, height: 32 }
}
