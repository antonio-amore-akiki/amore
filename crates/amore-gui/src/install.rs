// crates/amore-gui/src/install.rs — F.installer-3 (Ollama silent install).
//
// Three phases —
//   Download  : chunked HTTPS GET https://ollama.com/download/OllamaSetup.exe,
//               written to %TEMP%/OllamaSetup.exe, periodic pct updates.
//   Install   : spawn the installer with /SILENT /SUPPRESSMSGBOXES.
//   Wait      : poll http://127.0.0.1:11434/api/version every 2 s up to 60 s.
//
// Every error path lands on DepStatus::Failed with a plain-English message —
// never leaks a reqwest::Error or anyhow chain to the GUI.

use crate::DepStatus;
use eframe::egui;
use sha2::{Sha256, Digest};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const URL: &str = "https://ollama.com/download/OllamaSetup.exe";
const PROBE_URL: &str = "http://127.0.0.1:11434/api/version";
const DOWNLOAD_TIMEOUT_SECS: u64 = 600;
const PROBE_TIMEOUT_SECS: u64 = 2;
const PROBE_INTERVAL_SECS: u64 = 2;
const PROBE_ATTEMPTS: u32 = 30; // 30 * 2 s = 60 s total
const READ_BUFFER_BYTES: usize = 65_536;

/// SHA-256 of OllamaSetup.exe vendored for v0.3.x. Bump this constant whenever a new
/// release pins a newer Ollama installer. Verified locally on 2026-05-26 via
/// `curl -sLo /tmp/OllamaSetup.exe https://ollama.com/download/OllamaSetup.exe && sha256sum /tmp/OllamaSetup.exe`.
/// Source: https://ollama.com/download/OllamaSetup.exe (~857 MB, Ollama bundles CUDA libs).
const OLLAMA_INSTALLER_SHA256: &str =
    "38ef4715a31b6fede8f37be840c5e1e1524150d2c637d1acca94227980daf300";

pub fn spawn_ollama(status: Arc<Mutex<DepStatus>>, ctx: egui::Context) {
    std::thread::spawn(move || run(status, ctx));
}

fn run(status: Arc<Mutex<DepStatus>>, ctx: egui::Context) {
    let set = |v: DepStatus| {
        *status.lock().expect("mutex poisoned: unrecoverable state corruption") = v;
        ctx.request_repaint();
    };
    set(DepStatus::Downloading { pct: 0.0 });

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            set(DepStatus::Failed(format!(
                "Couldn't start the downloader ({e}). Please check your internet connection."
            )));
            return;
        }
    };
    let temp_path = match download(&client, status.clone(), &ctx) {
        Some(p) => p,
        None => return, // download() already pushed a Failed status
    };

    set(DepStatus::Installing);
    if !run_installer(&temp_path, status.clone(), &ctx) {
        return;
    }

    if !wait_ready(&client) {
        set(DepStatus::Failed(
            "Ollama installed but didn't start within 60 seconds. Try opening Ollama from your Start menu.".into(),
        ));
        return;
    }
    set(DepStatus::Ready);
}

fn download(
    client: &reqwest::blocking::Client,
    status: Arc<Mutex<DepStatus>>,
    ctx: &egui::Context,
) -> Option<std::path::PathBuf> {
    let set = |v: DepStatus| {
        *status.lock().expect("mutex poisoned: unrecoverable state corruption") = v;
        ctx.request_repaint();
    };
    let mut resp = match client.get(URL).send() {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            set(DepStatus::Failed(format!(
                "Couldn't download Ollama (server returned {}). Try again in a minute.",
                r.status()
            )));
            return None;
        }
        Err(e) => {
            set(DepStatus::Failed(format!(
                "Couldn't reach ollama.com ({e}). Please check your internet connection."
            )));
            return None;
        }
    };
    let total = resp.content_length().unwrap_or(0);
    let temp_path = std::env::temp_dir().join("OllamaSetup.exe");
    let mut file = match std::fs::File::create(&temp_path) {
        Ok(f) => f,
        Err(e) => {
            set(DepStatus::Failed(format!(
                "Couldn't create the installer file at {} ({e}).",
                temp_path.display()
            )));
            return None;
        }
    };
    let mut buf = [0u8; READ_BUFFER_BYTES];
    let mut written: u64 = 0;
    let mut last_pct: f32 = -1.0;
    let mut hasher = Sha256::new();
    loop {
        let n = match resp.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                set(DepStatus::Failed(format!(
                    "Download interrupted ({e}). Please check your internet connection."
                )));
                return None;
            }
        };
        if n == 0 {
            break;
        }
        if let Err(e) = file.write_all(&buf[..n]) {
            set(DepStatus::Failed(format!(
                "Couldn't save the installer file ({e}). Is your disk full?"
            )));
            return None;
        }
        hasher.update(&buf[..n]);
        written += n as u64;
        if total > 0 {
            let pct = (written as f32 / total as f32).clamp(0.0, 1.0);
            // Push to UI only when pct moves >=1% — keeps lock contention low.
            if pct - last_pct >= 0.01 {
                last_pct = pct;
                set(DepStatus::Downloading { pct });
            }
        }
    }

    // Integrity check — fail-closed before exec (security finding 10a).
    let computed_hex = hex::encode(hasher.finalize());
    if !computed_hex.eq_ignore_ascii_case(OLLAMA_INSTALLER_SHA256) {
        set(DepStatus::Failed(
            "Ollama installer integrity check failed. The downloaded file doesn't match the \
             expected fingerprint. This could mean the download was corrupted, tampered with in \
             transit, or Ollama released a new version that Amore hasn't been updated for yet. \
             Please retry, or report this to amore.dev/security if it persists."
                .into(),
        ));
        return None;
    }

    Some(temp_path)
}

fn run_installer(temp_path: &std::path::Path, status: Arc<Mutex<DepStatus>>, ctx: &egui::Context) -> bool {
    let set = |v: DepStatus| {
        *status.lock().expect("mutex poisoned: unrecoverable state corruption") = v;
        ctx.request_repaint();
    };
    match std::process::Command::new(temp_path)
        .args(["/SILENT", "/SUPPRESSMSGBOXES"])
        .status()
    {
        Ok(s) if s.success() => true,
        Ok(s) => {
            set(DepStatus::Failed(format!(
                "The Ollama installer didn't finish cleanly (exit code {}).",
                s.code().unwrap_or(-1)
            )));
            false
        }
        Err(e) => {
            set(DepStatus::Failed(format!(
                "Couldn't run the Ollama installer ({e})."
            )));
            false
        }
    }
}

fn wait_ready(client: &reqwest::blocking::Client) -> bool {
    for _ in 0..PROBE_ATTEMPTS {
        if let Ok(r) = client
            .get(PROBE_URL)
            .timeout(Duration::from_secs(PROBE_TIMEOUT_SECS))
            .send()
            && r.status().is_success() {
                return true;
            }
        std::thread::sleep(Duration::from_secs(PROBE_INTERVAL_SECS));
    }
    false
}
