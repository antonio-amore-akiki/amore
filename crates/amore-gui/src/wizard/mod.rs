// crates/amore-gui/src/wizard/mod.rs — W8.5D first-run wizard (6 screens).
// @file-size-exempt: 6-screen dispatch table — each screen is a distinct match arm; no reusable extraction without losing clarity
//
// egui state machine: each screen has Back/Next with screen-specific validation.
// Screens: Welcome → DataDir → BundledDeps → IdeDetect → WireConfirm → Done.
// Prior-art: Adapt — docs/prior-art-w8.5.md §6, state/prior-art-verdict.json.

mod screens;

use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::ide_detect::DetectedIde;
use crate::ide_wire::{WireVerdict, wire_all};

pub use screens::*;

// ── Screen enum ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    DataDir,
    BundledDeps,
    IdeDetect,
    WireConfirm,
    Done,
}

impl Screen {
    pub fn next(&self) -> Option<Screen> {
        match self {
            Screen::Welcome => Some(Screen::DataDir),
            Screen::DataDir => Some(Screen::BundledDeps),
            Screen::BundledDeps => Some(Screen::IdeDetect),
            Screen::IdeDetect => Some(Screen::WireConfirm),
            Screen::WireConfirm => Some(Screen::Done),
            Screen::Done => None,
        }
    }

    pub fn prev(&self) -> Option<Screen> {
        match self {
            Screen::Welcome => None,
            Screen::DataDir => Some(Screen::Welcome),
            Screen::BundledDeps => Some(Screen::DataDir),
            Screen::IdeDetect => Some(Screen::BundledDeps),
            Screen::WireConfirm => Some(Screen::IdeDetect),
            Screen::Done => Some(Screen::WireConfirm),
        }
    }
}

// ── Shared wizard state ───────────────────────────────────────────────────────

pub struct WizardState {
    pub screen: Screen,
    pub license_accepted: bool,
    pub data_dir: String,
    pub free_bytes: Option<u64>,
    pub detected_ides: Vec<DetectedIde>,
    pub ide_checked: Vec<bool>,
    pub wire_results: Vec<(String, WireVerdict)>,
    pub apply_status: Arc<Mutex<ApplyStatus>>,
    pub open_dashboard_clicked: bool,
    pub run_in_tray_clicked: bool,
}

#[derive(Clone, Debug, Default)]
pub enum ApplyStatus {
    #[default]
    Idle,
    Applying,
    Done,
    Failed(String),
}

impl Default for WizardState {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardState {
    pub fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Amore")
            .to_string_lossy()
            .into_owned();
        WizardState {
            screen: Screen::Welcome,
            license_accepted: false,
            data_dir,
            free_bytes: None,
            detected_ides: vec![],
            ide_checked: vec![],
            wire_results: vec![],
            apply_status: Arc::new(Mutex::new(ApplyStatus::Idle)),
            open_dashboard_clicked: false,
            run_in_tray_clicked: false,
        }
    }

    /// Whether Next is valid for the current screen.
    pub fn can_advance(&self) -> bool {
        match self.screen {
            Screen::Welcome => self.license_accepted,
            Screen::DataDir => !self.data_dir.is_empty(),
            Screen::BundledDeps | Screen::IdeDetect => true,
            // WireConfirm: Next replaced by Apply; Done has no Next.
            Screen::WireConfirm | Screen::Done => false,
        }
    }
}

// ── egui App ──────────────────────────────────────────────────────────────────

pub struct AmoreWizardApp {
    pub state: WizardState,
}

impl AmoreWizardApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            state: WizardState::new(),
        }
    }
}

impl eframe::App for AmoreWizardApp {
    // eframe 0.30+ replaced `update(ctx, frame)` with `ui(ui, frame)` — the
    // framework now auto-wraps the body in a CentralPanel. Per eframe 0.34 epi.rs
    // trait signature (v-next #34 migration 2026-05-27).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        match self.state.screen.clone() {
            Screen::Welcome => screens::render_welcome(ui, &mut self.state),
            Screen::DataDir => screens::render_data_dir(ui, &mut self.state),
            Screen::BundledDeps => screens::render_bundled_deps(ui, &mut self.state),
            Screen::IdeDetect => screens::render_ide_detect(ui, &mut self.state),
            Screen::WireConfirm => screens::render_wire_confirm(ui, &mut self.state),
            Screen::Done => screens::render_done(ui, &mut self.state),
        }

        if matches!(
            *self
                .state
                .apply_status
                .lock()
                .expect("apply_status poisoned"),
            ApplyStatus::Applying
        ) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

// ── Wire-up launch helper (called from WireConfirm screen) ───────────────────

pub fn spawn_wire(selected: Vec<DetectedIde>, status: Arc<Mutex<ApplyStatus>>) {
    std::thread::spawn(move || {
        *status.lock().expect("apply_status poisoned") = ApplyStatus::Applying;
        let results = wire_all(&selected);
        let all_ok = results
            .iter()
            .all(|(_, v)| matches!(v, WireVerdict::Ok | WireVerdict::SkippedNoChange));
        *status.lock().expect("apply_status poisoned") = if all_ok {
            ApplyStatus::Done
        } else {
            let errs: Vec<String> = results
                .iter()
                .filter_map(|(name, v)| {
                    if let WireVerdict::Err(e) = v {
                        Some(format!("{name}: {e}"))
                    } else {
                        None
                    }
                })
                .collect();
            ApplyStatus::Failed(errs.join("; "))
        };
    });
}
