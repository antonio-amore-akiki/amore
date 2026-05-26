// QA A8 — Adversarial-test mining against real ~/.claude/state/edit-rationale-ledger.jsonl.
//
// Gated by OBELION_TEST_MINING=1. Run with:
//     $env:OBELION_TEST_MINING=1
//     cargo test -p obelion-core --test mining_real_ledger -- --ignored --nocapture
//
// Proves: the pure-function mining module shipped in S15 actually processes
// the real on-disk ledger format produced by the ~/.claude SessionEnd hooks
// without panicking, with malformed-line tolerance, returning a sane count.
//
// Why no LLM dep: mining is pattern-extraction over JSONL. The model-driven
// adversarial-test elaboration is downstream (later phase). A8 only proves
// the parser + filter pipeline survives real corpus.

use obelion_core::mining::{mine_from_jsonl, parse_ledger};

fn enabled() -> bool {
    std::env::var("OBELION_TEST_MINING").ok().as_deref() == Some("1")
}

fn ledger_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var("OBELION_TEST_MINING_LEDGER") {
        return std::path::PathBuf::from(p);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".claude")
        .join("state")
        .join("edit-rationale-ledger.jsonl")
}

#[test]
#[ignore = "requires OBELION_TEST_MINING=1 + ~/.claude/state/edit-rationale-ledger.jsonl"]
fn mining_processes_real_ledger_without_panic() {
    if !enabled() {
        eprintln!("OBELION_TEST_MINING not set; skipping");
        return;
    }
    let path = ledger_path();
    eprintln!("[A8] reading real ledger: {}", path.display());
    if !path.exists() {
        eprintln!(
            "[A8] ledger not present; treating as empty corpus. (this is fine — proves the empty path.)"
        );
        let mined = mine_from_jsonl("");
        assert!(mined.is_empty());
        return;
    }
    let body = std::fs::read_to_string(&path).expect("read ledger");
    eprintln!("[A8] ledger size: {} bytes", body.len());
    let line_count = body.lines().count();
    let parsed = parse_ledger(&body);
    eprintln!(
        "[A8] {} raw lines -> {} parsed entries ({} skipped as malformed)",
        line_count,
        parsed.len(),
        line_count.saturating_sub(parsed.len())
    );
    // The parser must not panic on real data; we don't enforce a min count
    // because user's ledger may be empty / just-rotated. We DO enforce that
    // mine_from_jsonl returns a valid Vec (no panic mid-iteration).
    let mined = mine_from_jsonl(&body);
    eprintln!(
        "[A8] mined {} adversarial-test stubs from real corpus",
        mined.len()
    );
    // Sanity-check the first mined entry (if any).
    if let Some(first) = mined.first() {
        assert!(!first.context.is_empty(), "mined context must not be empty");
        assert!(
            !first.forbidden_output.is_empty(),
            "mined forbidden_output must not be empty"
        );
        assert!(
            matches!(first.kind.as_str(), "failure" | "corrected"),
            "kind must be failure|corrected, got {:?}",
            first.kind
        );
        eprintln!("[A8] sample mined entry: {first:?}");
    }
}

#[test]
#[ignore = "requires OBELION_TEST_MINING=1"]
fn mining_tolerates_synthetic_malformed_corpus_alongside_real() {
    if !enabled() {
        eprintln!("OBELION_TEST_MINING not set; skipping");
        return;
    }
    // Mix malformed + valid lines to verify the parser keeps going past errors
    // and produces a sane count. Models the situation where the SessionEnd
    // hook crashed mid-write and left a partial line.
    let jsonl = r#"
        {"ts":1,"session_id":"valid-1","payload":{"context":"agent tried X","forbidden":"X(stale-arg)"},"failure":true}
        not json at all -- ignore me
        {"ts":2,"session_id":"valid-2","payload":{"prompt":"do Y","corrected_from":"Y(old)","corrected_to":"Y(new)"},"corrected":true}
        {"ts":3,"session_id":"
        {"ts":4,"session_id":"valid-3","payload":{},"failure":false}
        {"ts":5,"session_id":"valid-4","payload":{"action_summary":"act Z","error_text":"Z failed"},"failure":true}
    "#;
    let mined = mine_from_jsonl(jsonl);
    assert_eq!(
        mined.len(),
        3,
        "expected 3 mined (valid-1 + valid-2 + valid-4), got: {:?}",
        mined
            .iter()
            .map(|m| &m.source_session_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(mined[0].source_session_id, "valid-1");
    assert_eq!(mined[1].source_session_id, "valid-2");
    assert_eq!(mined[1].kind, "corrected");
    assert_eq!(mined[1].desired_output.as_deref(), Some("Y(new)"));
    assert_eq!(mined[2].source_session_id, "valid-4");
}
