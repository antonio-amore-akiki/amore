// Integration test — `amore --help` must list every advertised subcommand.
//
// Spawns the release binary at ../../target/release/amore.exe. CI must build
// the workspace in release mode before running this test (`cargo build
// --release --workspace`).

use std::path::PathBuf;
use std::process::Command;

fn release_bin(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut p = manifest_dir.parent().unwrap().parent().unwrap().to_path_buf();
    p.push("target");
    p.push("release");
    p.push(if cfg!(windows) { format!("{name}.exe") } else { name.to_string() });
    p
}

#[test]
fn amore_help_lists_subcommands() {
    let bin = release_bin("amore");
    assert!(bin.exists(), "release binary not built — run cargo build --release --workspace first (looked for {})", bin.display());

    let out = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("spawn amore --help");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let merged = format!("{stdout}\n{stderr}");

    assert!(out.status.success(), "amore --help exited non-zero\nstdout: {stdout}\nstderr: {stderr}");
    assert!(merged.contains("Usage"), "stdout missing 'Usage'\nfull output:\n{merged}");

    for sub in ["init", "serve", "recall", "status", "doctor"] {
        assert!(
            merged.contains(sub),
            "stdout missing subcommand '{sub}'\nfull output:\n{merged}"
        );
    }
}
