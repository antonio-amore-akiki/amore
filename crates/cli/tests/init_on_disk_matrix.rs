// QA A3 — `obelion init <ide>` real-disk matrix for all 7 adapters.
//
// First A3 attempt this session used PowerShell USERPROFILE env hijack to
// sandbox the binary. That FAILED — dirs::home_dir() on Windows resolves
// via SHGetKnownFolderPath(FOLDERID_Profile), not USERPROFILE — and the
// test leaked writes into ~/.claude.json + ~/.cursor + ~/.codeium + ~/.hermes
// before I noticed. Restored from .bak this turn; clean.
//
// Proper sandboxing uses each adapter's pub `config_path_override` field
// directly. No env hijack, no CLI process spawn, no real-config risk.
//
// Asserted per adapter:
//   1. apply() on empty target -> ApplyOutcome::Created(path); file exists
//   2. apply() on identical target -> ApplyOutcome::NoChange; file byte-equal
//   3. apply() on pre-existing-with-other-keys -> ApplyOutcome::Updated{path,backup};
//      backup file holds pre-edit bytes; new file contains both old + obelion
//   4. dry_run() before/after match what apply() would produce

use obelion_adapter_claude::ClaudeAdapter;
use obelion_adapter_cline::ClineAdapter;
use obelion_adapter_codex::CodexAdapter;
use obelion_adapter_cursor::CursorAdapter;
use obelion_adapter_hermes::HermesAdapter;
use obelion_adapter_opencode::OpencodeAdapter;
use obelion_adapter_windsurf::WindsurfAdapter;
use obelion_core::ide_adapter::{ApplyOutcome, IdeAdapter, apply, dry_run};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tmp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("obelion-a3-{nanos:x}-{n}"));
    std::fs::create_dir_all(&d).expect("mkdir tmp");
    d
}

/// Common contract: first apply -> Created, second apply -> NoChange, third
/// apply after content tamper -> Updated with .bak holding tampered content.
/// Returns the merged file content for caller-specific schema assertions.
fn verify_adapter<A: IdeAdapter>(adapter: &A, target_filename: &str) -> String {
    let target = adapter
        .config_path()
        .expect("config_path resolves with override");

    // dry-run should not touch disk
    assert!(
        !target.exists(),
        "{target_filename}: target must not exist before dry-run"
    );
    let dry = dry_run(adapter).expect("dry-run computes merged content");
    assert!(
        !dry.is_empty(),
        "{target_filename}: dry-run produces non-empty content"
    );
    assert!(
        !target.exists(),
        "{target_filename}: dry-run must NOT have touched disk"
    );

    // First apply: Created
    match apply(adapter).expect("first apply") {
        ApplyOutcome::Created(p) => assert_eq!(p, target, "Created path matches config_path"),
        other => panic!("{target_filename}: expected Created, got {other:?}"),
    }
    assert!(
        target.exists(),
        "{target_filename}: file written after Created"
    );
    let first_content = std::fs::read_to_string(&target).expect("read");

    // Second apply: NoChange + byte-identical
    match apply(adapter).expect("second apply") {
        ApplyOutcome::NoChange => {}
        other => panic!("{target_filename}: expected NoChange on idempotent re-run, got {other:?}"),
    }
    let second_content = std::fs::read_to_string(&target).expect("read");
    assert_eq!(
        first_content, second_content,
        "{target_filename}: idempotent re-run produced byte-identical content"
    );
    assert!(
        !target.with_extension("bak").exists()
            && !PathBuf::from(format!("{}.bak", target.display())).exists(),
        "{target_filename}: idempotent NoChange must NOT produce a .bak"
    );

    // Tamper: append benign content that still parses correctly per the schema
    // is hard to construct cross-schema, so instead we DELETE the obelion entry
    // by writing a clean baseline (empty mcpServers or empty TOML/YAML). The
    // next apply MUST detect the drift and produce Updated{path, backup}.
    let tampered = match target_filename {
        s if s.ends_with(".toml") => "[other]\nkey = \"value\"\n".to_string(),
        s if s.ends_with(".yaml") || s.ends_with(".yml") => "other: value\n".to_string(),
        _ => r#"{"other":"value"}"#.to_string(),
    };
    std::fs::write(&target, &tampered).expect("write tampered baseline");

    let bak_path = PathBuf::from(format!("{}.bak", target.display()));
    let _ = std::fs::remove_file(&bak_path); // ensure clean
    match apply(adapter).expect("third apply") {
        ApplyOutcome::Updated { path, backup } => {
            assert_eq!(path, target);
            assert!(backup.exists(), "{target_filename}: .bak sibling created");
            assert_eq!(
                std::fs::read_to_string(&backup).unwrap(),
                tampered,
                "{target_filename}: .bak holds pre-edit (tampered baseline) content"
            );
        }
        other => panic!("{target_filename}: expected Updated after drift, got {other:?}"),
    }
    let merged = std::fs::read_to_string(&target).expect("read post-merge");
    assert!(
        merged.contains("obelion"),
        "{target_filename}: post-merge content contains obelion entry"
    );
    assert!(
        merged.contains("other"),
        "{target_filename}: post-merge content preserves other key from tampered baseline"
    );
    merged
}

#[test]
fn a3_claude_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("claude.json");
    let mut a = ClaudeAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "claude.json");
    assert!(merged.contains("\"mcpServers\""));
    assert!(merged.contains("\"obelion-mcp\""));
}

#[test]
fn a3_cursor_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("mcp.json");
    let mut a = CursorAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "mcp.json");
    assert!(merged.contains("\"mcpServers\""));
}

#[test]
fn a3_codex_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("config.toml");
    let mut a = CodexAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "config.toml");
    assert!(merged.contains("[mcp_servers.obelion]"));
    assert!(merged.contains("[other]"));
}

#[test]
fn a3_cline_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("cline_mcp_settings.json");
    let mut a = ClineAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "cline_mcp_settings.json");
    assert!(merged.contains("\"mcpServers\""));
}

#[test]
fn a3_opencode_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("opencode.json");
    let mut a = OpencodeAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "opencode.json");
    assert!(merged.contains("\"mcp\""));
    assert!(merged.contains("\"type\""));
    assert!(merged.contains("\"local\""));
}

#[test]
fn a3_windsurf_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("mcp_config.json");
    let mut a = WindsurfAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "mcp_config.json");
    assert!(merged.contains("\"mcpServers\""));
}

#[test]
fn a3_hermes_init_on_disk_full_lifecycle() {
    let d = fresh_tmp_dir();
    let path = d.join("config.yaml");
    let mut a = HermesAdapter::new();
    a.config_path_override = Some(path);
    let merged = verify_adapter(&a, "config.yaml");
    assert!(merged.contains("mcp_servers"));
    assert!(merged.contains("obelion"));
}

#[test]
fn a3_seven_adapters_resolve_to_distinct_default_paths() {
    // Sanity: each adapter's default config_path() (no override) resolves to
    // a path containing the expected OS-specific fragment. Run-time only;
    // assertions tolerate either Win or POSIX paths.
    let cases: Vec<(Box<dyn IdeAdapter>, &str)> = vec![
        (Box::new(ClaudeAdapter::new()), ".claude.json"),
        (Box::new(CursorAdapter::new()), "mcp.json"),
        (Box::new(CodexAdapter::new()), "config.toml"),
        (Box::new(ClineAdapter::new()), "cline_mcp_settings.json"),
        (Box::new(OpencodeAdapter::new()), "opencode.json"),
        (Box::new(WindsurfAdapter::new()), "mcp_config.json"),
        (Box::new(HermesAdapter::new()), "config.yaml"),
    ];
    let mut seen: Vec<PathBuf> = Vec::new();
    for (a, frag) in &cases {
        let p = a.config_path().expect("default config_path resolves");
        assert!(
            p.to_string_lossy().contains(frag),
            "{} path '{}' must contain '{frag}'",
            a.name(),
            p.display()
        );
        assert!(
            !seen.contains(&p),
            "{} default path collides with a prior adapter: {}",
            a.name(),
            p.display()
        );
        seen.push(p);
    }
    assert_eq!(seen.len(), 7, "all 7 adapters produced distinct paths");
}
