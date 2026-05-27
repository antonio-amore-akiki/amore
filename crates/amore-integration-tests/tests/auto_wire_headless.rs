// Integration test — `amore-gui --auto-wire` must not panic on headless Linux
// and must emit a parseable JSON contract to stdout.
//
// Prior-art: Adapt from crates/amore-integration-tests/tests/cli_help.rs
//   (identical spawn-and-assert pattern; adapted to test --auto-wire + JSON shape).
//
// Spawns the release binary with DISPLAY="" so no X11/winit display init occurs.
// This proves F3 (headless safety) and F24 (JSON contract defined).
//
// Run after: cargo build --release --workspace
// The test is safe to run on Windows too — DISPLAY is simply ignored there.

use std::path::PathBuf;
use std::process::Command;

fn release_bin(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut p = manifest_dir
        .parent()
        .expect("crates/ parent")
        .parent()
        .expect("repo root")
        .to_path_buf();
    p.push("target");
    p.push("release");
    p.push(if cfg!(windows) { format!("{name}.exe") } else { name.to_string() });
    p
}

#[test]
fn auto_wire_headless_no_panic_and_valid_json() {
    let bin = release_bin("amore-gui");
    assert!(
        bin.exists(),
        "release binary not built — run `cargo build --release --workspace` first (looked for {})",
        bin.display()
    );

    let mut cmd = Command::new(&bin);
    cmd.arg("--auto-wire");
    // Headless: clear DISPLAY so X11 init would fail if it were ever reached.
    // On Windows this env var is unused; the test still exercises the JSON path.
    cmd.env("DISPLAY", "");
    // Allow binary to resolve the sibling amore-mcp from a non-system dir in tests.
    cmd.env("AMORE_ALLOW_USER_INSTALL", "1");

    let output = cmd.output().expect("spawn amore-gui --auto-wire");

    // Must not have crashed (exit code 0 or 1 — only panic causes 101/SIGABRT).
    let code = output.status.code().unwrap_or(0);
    assert!(
        code == 0 || code == 1,
        "amore-gui --auto-wire exited with unexpected code {code} — likely a panic\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Stdout must contain exactly one JSON object on the last non-empty line.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .expect("stdout must have at least one non-empty line");

    let contract: serde_json::Value = serde_json::from_str(json_line).unwrap_or_else(|e| {
        panic!(
            "stdout last line is not valid JSON: {e}\nline: {json_line}\nfull stdout:\n{stdout}"
        )
    });

    // Contract shape: all four keys must exist and be arrays.
    for key in ["detected", "wired", "skipped", "errors"] {
        assert!(
            contract.get(key).and_then(|v| v.as_array()).is_some(),
            "JSON contract missing array field '{key}'\ncontract: {contract}"
        );
    }

    // Exit code semantics: 0 iff errors == [].
    let errors_empty = contract["errors"].as_array().expect("errors is array").is_empty();
    if errors_empty {
        assert_eq!(code, 0, "exit 0 expected when errors == []");
    } else {
        assert_eq!(code, 1, "exit 1 expected when errors is non-empty");
    }
}
