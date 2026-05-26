// Adversarial-test mining (S15). Parses an edit-rationale-ledger JSONL stream,
// filters failure+corrected entries, generates AdversarialTest stubs that the
// ~/.claude SessionEnd hook appends to policy/test.mjs A-block. Pure-function
// — tests use inline JSONL strings; no file I/O at this layer.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    #[serde(default)]
    pub ts: i64,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub failure: bool,
    #[serde(default)]
    pub corrected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdversarialTest {
    pub context: String,
    pub forbidden_output: String,
    pub desired_output: Option<String>,
    pub source_session_id: String,
    pub source_ts: i64,
    pub kind: String,
}

/// Parse an edit-rationale-ledger JSONL string. One entry per line; malformed
/// lines silently skipped (callers can compare result.len() vs input line count
/// to detect drops). Blank lines + leading whitespace tolerated.
pub fn parse_ledger(jsonl: &str) -> Vec<LedgerEntry> {
    jsonl
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<LedgerEntry>(l).ok())
        .collect()
}

/// Filter entries marked failure OR corrected, map to AdversarialTest stubs.
/// The context + forbidden_output are extracted from the payload via heuristic
/// (see derive_*). Entries without enough payload signal are dropped.
pub fn mine_adversarials(entries: &[LedgerEntry]) -> Vec<AdversarialTest> {
    entries
        .iter()
        .filter(|e| e.failure || e.corrected)
        .filter_map(adversarial_from_entry)
        .collect()
}

/// Convenience: parse + mine in one pass.
pub fn mine_from_jsonl(jsonl: &str) -> Vec<AdversarialTest> {
    let entries = parse_ledger(jsonl);
    mine_adversarials(&entries)
}

fn adversarial_from_entry(e: &LedgerEntry) -> Option<AdversarialTest> {
    let kind = if e.corrected {
        "corrected"
    } else if e.failure {
        "failure"
    } else {
        return None;
    };
    let context = derive_context(&e.payload)?;
    let forbidden = derive_forbidden(&e.payload)?;
    let desired = derive_desired(&e.payload);
    Some(AdversarialTest {
        context,
        forbidden_output: forbidden,
        desired_output: desired,
        source_session_id: e.session_id.clone(),
        source_ts: e.ts,
        kind: kind.to_string(),
    })
}

fn derive_context(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("context")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("prompt").and_then(|v| v.as_str()))
        .or_else(|| payload.get("action_summary").and_then(|v| v.as_str()))
        .or_else(|| payload.get("trigger").and_then(|v| v.as_str()))
        .map(str::to_string)
}

fn derive_forbidden(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("forbidden")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("corrected_from").and_then(|v| v.as_str()))
        .or_else(|| payload.get("error_text").and_then(|v| v.as_str()))
        .or_else(|| payload.get("bad_output").and_then(|v| v.as_str()))
        .map(str::to_string)
}

fn derive_desired(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("corrected_to")
        .and_then(|v| v.as_str())
        .or_else(|| payload.get("desired").and_then(|v| v.as_str()))
        .or_else(|| payload.get("good_output").and_then(|v| v.as_str()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_no_entries() {
        assert!(parse_ledger("").is_empty());
        assert!(parse_ledger("   \n  \n  ").is_empty());
    }

    #[test]
    fn parse_skips_malformed_lines_keeps_good() {
        let jsonl = r#"
            {"ts":1,"session_id":"s1","source":"edit","payload":{"context":"c","forbidden":"f"},"failure":true}
            not json at all
            {"ts":2,"session_id":"s2","source":"edit","payload":{},"failure":false}
        "#;
        let parsed = parse_ledger(jsonl);
        assert_eq!(parsed.len(), 2, "malformed line skipped, 2 valid kept");
        assert_eq!(parsed[0].session_id, "s1");
        assert_eq!(parsed[1].session_id, "s2");
    }

    #[test]
    fn mine_keeps_only_failure_or_corrected() {
        let entries = vec![
            LedgerEntry {
                ts: 1,
                session_id: "s1".into(),
                source: "e".into(),
                payload: serde_json::json!({"context":"c","forbidden":"f"}),
                failure: true,
                corrected: false,
            },
            LedgerEntry {
                ts: 2,
                session_id: "s2".into(),
                source: "e".into(),
                payload: serde_json::json!({"context":"c2","forbidden":"f2"}),
                failure: false,
                corrected: false,
            },
            LedgerEntry {
                ts: 3,
                session_id: "s3".into(),
                source: "e".into(),
                payload: serde_json::json!({"context":"c3","corrected_from":"old","corrected_to":"new"}),
                failure: false,
                corrected: true,
            },
        ];
        let mined = mine_adversarials(&entries);
        assert_eq!(mined.len(), 2);
        assert_eq!(mined[0].kind, "failure");
        assert_eq!(mined[0].source_session_id, "s1");
        assert_eq!(mined[1].kind, "corrected");
        assert_eq!(mined[1].desired_output.as_deref(), Some("new"));
    }

    #[test]
    fn mine_drops_failures_without_required_payload_fields() {
        let entries = vec![LedgerEntry {
            ts: 1,
            session_id: "s1".into(),
            source: "e".into(),
            payload: serde_json::json!({"unrelated":"x"}),
            failure: true,
            corrected: false,
        }];
        let mined = mine_adversarials(&entries);
        assert!(
            mined.is_empty(),
            "failure with no context/forbidden is dropped"
        );
    }

    #[test]
    fn mine_uses_alternate_field_names_for_context() {
        let entries = vec![
            LedgerEntry {
                ts: 1,
                session_id: "s1".into(),
                source: "e".into(),
                payload: serde_json::json!({"prompt":"do X","forbidden":"f"}),
                failure: true,
                corrected: false,
            },
            LedgerEntry {
                ts: 2,
                session_id: "s2".into(),
                source: "e".into(),
                payload: serde_json::json!({"action_summary":"do Y","error_text":"e2"}),
                failure: true,
                corrected: false,
            },
        ];
        let mined = mine_adversarials(&entries);
        assert_eq!(mined.len(), 2);
        assert_eq!(mined[0].context, "do X");
        assert_eq!(mined[1].context, "do Y");
        assert_eq!(mined[1].forbidden_output, "e2");
    }

    #[test]
    fn mine_from_jsonl_end_to_end() {
        let jsonl = r#"
            {"ts":1,"session_id":"s1","payload":{"context":"agent attempted X","forbidden":"X(stale-arg)"},"failure":true}
            {"ts":2,"session_id":"s2","payload":{"context":"clean attempt"},"failure":false}
            {"ts":3,"session_id":"s3","payload":{"context":"corrected case","corrected_from":"bad","corrected_to":"good"},"corrected":true}
        "#;
        let mined = mine_from_jsonl(jsonl);
        assert_eq!(mined.len(), 2);
        assert_eq!(mined[0].source_session_id, "s1");
        assert_eq!(mined[0].forbidden_output, "X(stale-arg)");
        assert_eq!(mined[1].source_session_id, "s3");
        assert_eq!(mined[1].desired_output.as_deref(), Some("good"));
    }

    #[test]
    fn adversarial_test_is_round_trippable_via_serde() {
        let t = AdversarialTest {
            context: "c".into(),
            forbidden_output: "f".into(),
            desired_output: Some("d".into()),
            source_session_id: "s".into(),
            source_ts: 42,
            kind: "failure".into(),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: AdversarialTest = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }
}
