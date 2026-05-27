// crates/amore-gui/src/wizard/screens.rs per-screen render functions.
//
// Each function renders one screen within the wizard CentralPanel.
// State mutations (screen transitions, IDE detection) happen via &mut WizardState.

use eframe::egui;

use super::{ApplyStatus, Screen, WizardState, spawn_wire};
use crate::ide_detect::ConfigFormat;
use crate::ide_detect::detect_all;
use crate::ide_wire::amore_mcp_entry_object;

// ── Screen 1: Welcome + License ───────────────────────────────────────────────

const LICENSE_TEXT: &str = "Apache License, Version 2.0\n\n\
    Licensed under the Apache License, Version 2.0 (the \"License\");\n\
    you may not use this software except in compliance with the License.\n\
    You may obtain a copy of the License at:\n\n\
    https://www.apache.org/licenses/LICENSE-2.0\n\n\
    Unless required by applicable law or agreed to in writing, software\n\
    distributed under the License is distributed on an \"AS IS\" BASIS,\n\
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.";

pub fn render_welcome(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Amore").size(32.0).strong());
        ui.add_space(4.0);
        ui.label("Local-first persistent memory for every AI tool");
        ui.add_space(12.0);
    });
    ui.separator();
    ui.label(egui::RichText::new("License (Apache 2.0)").strong());
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .max_height(200.0)
        .id_salt("license_scroll")
        .show(ui, |ui| {
            ui.label(LICENSE_TEXT);
        });
    ui.add_space(8.0);
    ui.checkbox(
        &mut state.license_accepted,
        "I accept the Apache 2.0 license terms",
    );
    ui.add_space(12.0);
    nav_row(ui, state);
}

// ── Screen 2: Data dir picker ─────────────────────────────────────────────────

pub fn render_data_dir(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.heading("Where should Amore keep your memory?");
    ui.add_space(6.0);
    ui.label("Stores conversation history and embeddings.");
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.text_edit_singleline(&mut state.data_dir);
        if ui.button("Browse…").clicked()
            && let Some(p) = rfd::FileDialog::new().pick_folder()
        {
            state.data_dir = p.to_string_lossy().into_owned();
            state.free_bytes = None;
        }
    });
    if state.free_bytes.is_none() && !state.data_dir.is_empty() {
        state.free_bytes = free_space_bytes(&state.data_dir);
    }
    ui.add_space(4.0);
    match state.free_bytes {
        None => {
            ui.label("(disk space unknown)");
        }
        Some(b) if b < 500 * 1024 * 1024 => {
            ui.colored_label(
                egui::Color32::DARK_RED,
                format!(
                    "Warning: only {:.0} MB free — need ≥500 MB for AI model + data.",
                    b as f64 / 1_048_576.0
                ),
            );
        }
        Some(b) => {
            ui.colored_label(
                egui::Color32::DARK_GREEN,
                format!("{:.1} GB free", b as f64 / 1_073_741_824.0),
            );
        }
    }
    ui.add_space(12.0);
    nav_row(ui, state);
}

// ── Screen 3: Bundled deps ────────────────────────────────────────────────────

pub fn render_bundled_deps(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.heading("Bundled components");
    ui.add_space(6.0);
    ui.label("These are installed alongside Amore — no separate setup needed:");
    ui.add_space(6.0);
    egui::Grid::new("deps_grid")
        .num_columns(2)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Component").strong());
            ui.label(egui::RichText::new("Version").strong());
            ui.end_row();
            ui.label("Local AI model");
            ui.label("v0.3.x");
            ui.end_row();
            ui.label("Memory index");
            ui.label("v1.15.x");
            ui.end_row();
        });
    ui.add_space(4.0);
    ui.label("These run quietly in the background — you don't need to interact with them.");
    ui.add_space(4.0);
    ui.label("First-run disk usage: approximately 2–4 GB (AI model + data store).");
    ui.add_space(12.0);
    nav_row(ui, state);
}

// ── Screen 4: IDE auto-detect ─────────────────────────────────────────────────

pub fn render_ide_detect(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.heading("Detected AI tools");
    ui.add_space(4.0);
    ui.label("Amore will add itself to the tools you select below.");
    ui.add_space(6.0);
    if state.detected_ides.is_empty() && state.ide_checked.is_empty() {
        state.detected_ides = detect_all();
        state.ide_checked = vec![true; state.detected_ides.len()];
    }
    if state.detected_ides.is_empty() {
        ui.label("No supported AI tools detected.");
        ui.add_space(4.0);
        ui.hyperlink_to(
            "Learn how to connect manually",
            "https://github.com/antonio-amore-akiki/amore/blob/main/docs/IDE-AUTO-WIRE.md",
        );
    } else {
        egui::Grid::new("ide_grid")
            .num_columns(3)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Tool").strong());
                ui.label(egui::RichText::new("Config path").strong());
                ui.label(egui::RichText::new("Wire?").strong());
                ui.end_row();
                for (i, ide) in state.detected_ides.iter().enumerate() {
                    ui.label(&ide.name);
                    ui.label(ide.path.to_string_lossy().as_ref());
                    let checked = state.ide_checked.get_mut(i).expect("index aligned");
                    ui.checkbox(checked, "");
                    ui.end_row();
                }
            });
    }
    ui.add_space(12.0);
    nav_row(ui, state);
}

// ── Screen 5: Wire confirmation ────────────────────────────────────────────────

pub fn render_wire_confirm(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.heading("Review changes");
    ui.add_space(4.0);
    ui.label("The following will be added to each selected tool's config:");
    ui.add_space(6.0);

    let selected: Vec<_> = state
        .detected_ides
        .iter()
        .enumerate()
        .filter_map(|(i, ide)| {
            state
                .ide_checked
                .get(i)
                .copied()
                .unwrap_or(false)
                .then_some(ide)
        })
        .collect();

    if selected.is_empty() {
        ui.label("No tools selected. Go back and check at least one.");
    } else {
        for ide in &selected {
            ui.horizontal(|ui| {
                ui.label("Amore will add a memory link to ");
                ui.label(egui::RichText::new(&ide.name).strong());
                ui.label(". Your existing settings are backed up automatically before any change.");
            });
            ui.collapsing("Show technical details", |ui| {
                ui.code(preview_for_ide(ide));
            });
            ui.add_space(4.0);
        }
        ui.add_space(8.0);

        let status = state
            .apply_status
            .lock()
            .expect("apply_status poisoned")
            .clone();
        match status {
            ApplyStatus::Idle => {
                if ui.button("Apply").clicked() {
                    let owned = selected.into_iter().cloned().collect();
                    spawn_wire(owned, state.apply_status.clone());
                }
            }
            ApplyStatus::Applying => {
                ui.label("Applying…");
                ui.spinner();
            }
            ApplyStatus::Done => {
                ui.colored_label(egui::Color32::DARK_GREEN, "Done — click Next to finish.");
                if ui.button("Next").clicked() {
                    state.screen = Screen::Done;
                }
            }
            ApplyStatus::Failed(msg) => {
                ui.colored_label(egui::Color32::DARK_RED, format!("Error: {msg}"));
                if ui.button("Retry").clicked() {
                    *state.apply_status.lock().expect("apply_status poisoned") = ApplyStatus::Idle;
                }
            }
        }
    }
    ui.add_space(8.0);
    if let Some(prev) = state.screen.prev()
        && ui.button("Back").clicked()
    {
        state.screen = prev;
    }
}

// ── Screen 6: Done ────────────────────────────────────────────────────────────

pub fn render_done(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.vertical_centered(|ui| {
        ui.add_space(24.0);
        ui.label(
            egui::RichText::new("Amore is ready!")
                .size(24.0)
                .color(egui::Color32::DARK_GREEN),
        );
        ui.add_space(12.0);
        ui.label("Your AI tools are wired. Amore will remember context across sessions.");
        ui.add_space(20.0);
        if ui
            .add_sized([240.0, 36.0], egui::Button::new("Open dashboard"))
            .clicked()
        {
            state.open_dashboard_clicked = true;
            open_url("http://localhost:3111");
        }
        ui.add_space(8.0);
        if ui
            .add_sized([240.0, 36.0], egui::Button::new("Keep running quietly in the background"))
            .clicked()
        {
            state.run_in_tray_clicked = true;
            // Caller (main.rs) polls this flag and spawns the tray.
        }
    });
}

// ── Nav row ───────────────────────────────────────────────────────────────────

pub fn nav_row(ui: &mut egui::Ui, state: &mut WizardState) {
    ui.horizontal(|ui| {
        if let Some(prev) = state.screen.prev() {
            if ui.button("Back").clicked() {
                state.screen = prev;
            }
        } else {
            ui.add_space(56.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(state.can_advance(), egui::Button::new("Next"))
                .clicked()
                && let Some(next) = state.screen.next()
            {
                state.screen = next;
            }
        });
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn preview_for_ide(ide: &crate::ide_detect::DetectedIde) -> String {
    match ide.config_format {
        ConfigFormat::Json => {
            let entry = amore_mcp_entry_object()
                .unwrap_or_else(|e| serde_json::json!({"command": format!("<{e}>"), "args": ["--stdio"], "env": {}}));
            format!(
                "{{\n  \"mcpServers\": {{\n    \"amore\": {}\n  }}\n}}",
                serde_json::to_string_pretty(&entry).unwrap_or_default()
            )
        }
        ConfigFormat::Yaml => {
            "mcpServers:\n  - name: amore\n    command: amore-mcp\n    args: [--stdio]\n    env: {}"
                .to_string()
        }
    }
}

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

fn free_space_bytes(path: &str) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        let cmd = format!(
            "(Get-PSDrive -Name ((Split-Path -Qualifier '{path}') -replace ':','') -ErrorAction SilentlyContinue)?.Free"
        );
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &cmd])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<u64>()
            .ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let out = std::process::Command::new("df")
            .args(["-B1", "--output=avail", path])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .nth(1)?
            .trim()
            .parse::<u64>()
            .ok()
    }
}
