// Prevents additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri_plugin_shell::ShellExt;

// Product catalog embedded at compile time.
// TODO(build-script): auto-sync from web/amore-app/content/products/amore.json via build.rs.
const PRODUCTS_JSON: &str = include_str!("../products.json");

/// Returns the product catalog as a JSON string.
#[tauri::command]
fn get_products() -> String {
    PRODUCTS_JSON.to_string()
}

/// Opens a URL in the system default browser.
/// Used by the launcher UI to open GH Releases asset download pages.
#[tauri::command]
async fn open_install_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.shell()
        .open(&url, None)
        .map_err(|e| format!("Failed to open URL: {e}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![get_products, open_install_url])
        .run(tauri::generate_context!())
        .expect("error while running Amore launcher");
}
