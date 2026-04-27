// Integration test — `amore init <ide> --dry-run` for each of the 7 supported
// IDEs. Each must exit 0 + print the dry-run header line that
// crates/amore-cli/src/main.rs emits:
//   `# dry-run: <ide> -> <config-path>`

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

fn assert_init_dry_run(ide: &str) {
    let bin = release_bin("amore");
    assert!(bin.exists(), "amore binary not built — run cargo build --release --workspace first (looked for {})", bin.display());

    let out = Command::new(&bin)
        .args(["init", ide, "--dry-run"])
        .output()
        .unwrap_or_else(|e| panic!("spawn amore init {ide} --dry-run: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        out.status.success(),
        "amore init {ide} --dry-run exited non-zero\nstdout: {stdout}\nstderr: {stderr}"
    );
    let header = format!("# dry-run: {ide} ->");
    assert!(
        stdout.contains(&header),
        "stdout missing dry-run header '{header}'\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test] fn init_dry_run_claude()    { assert_init_dry_run("claude"); }
#[test] fn init_dry_run_cursor()    { assert_init_dry_run("cursor"); }
#[test] fn init_dry_run_codex()     { assert_init_dry_run("codex"); }
#[test] fn init_dry_run_cline()     { assert_init_dry_run("cline"); }
#[test] fn init_dry_run_opencode()  { assert_init_dry_run("opencode"); }
#[test] fn init_dry_run_windsurf()  { assert_init_dry_run("windsurf"); }
#[test] fn init_dry_run_hermes()    { assert_init_dry_run("hermes"); }
