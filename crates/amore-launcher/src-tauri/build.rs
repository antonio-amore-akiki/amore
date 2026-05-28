// Adopt verdict (no rejected alternatives): Tauri 2 canonical 1-line build.rs ships with
// every `cargo create-tauri-app` scaffold (upstream docs.tauri.app/start/create-project/).
// Source: github.com/tauri-apps/tauri tauri-build crate — invokes WiX/MSIX/dpkg-buildpackage
// resource embedding via tauri.conf.json. No reasonable replacement exists.
fn main() {
    tauri_build::build()
}
