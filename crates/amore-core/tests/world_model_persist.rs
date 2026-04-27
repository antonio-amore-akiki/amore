// QA A6 — file persistence across reopens for the WorldModel substrate.
//
// Proves the claim in CHANGELOG.md / README.md that the world model is
// "persistent" by exercising the actual disk path: open a file-backed DB,
// upsert representative rows for ALL three sub-graphs, drop the handle,
// reopen at the same path, assert every row is read back byte-equal.
//
// Also probes corruption tolerance: truncate the file post-close and reopen;
// the WorldModel must either refuse to open OR open clean — never silently
// return partial garbage. This is a soft assertion (SQLite's default integrity
// behaviour suffices) — the hard assertion is the roundtrip equality.
//
// Pattern adapted from crates/core/src/ide_adapter.rs `fresh_tmp_dir()` to
// avoid the tempfile crate dep (per CLAUDE.md keep-it-simple).

use amore_core::world_model::WorldModel;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_tmp_db_path() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("amore-wmpersist-{nanos:x}-{n}"));
    std::fs::create_dir_all(&dir).expect("mkdir tmp");
    dir.join("world_model.sqlite")
}

#[test]
fn file_roundtrip_preserves_projects_across_reopen() {
    let p = fresh_tmp_db_path();

    {
        let wm = WorldModel::open(&p).expect("open new file");
        wm.upsert_project("obelion", json!({"lang":"rust","version":"0.2.1"}))
            .expect("upsert obelion");
        wm.upsert_project("agentmemory", json!({"lang":"node","version":"3.x"}))
            .expect("upsert agentmemory");
        wm.upsert_project("docs-router", json!({"lang":"none"}))
            .expect("upsert docs-router");
    } // Drop closes the Connection; SQLite flushes on Drop.

    let wm2 = WorldModel::open(&p).expect("reopen existing file");
    let obelion = wm2
        .get_project("obelion")
        .expect("query")
        .expect("obelion row present after reopen");
    assert_eq!(obelion.name, "obelion");
    assert_eq!(obelion.payload["lang"], "rust");
    assert_eq!(obelion.payload["version"], "0.2.1");

    let agentmemory = wm2.get_project("agentmemory").unwrap().unwrap();
    assert_eq!(agentmemory.payload["lang"], "node");

    let docs = wm2.get_project("docs-router").unwrap().unwrap();
    assert_eq!(docs.payload["lang"], "none");

    assert!(
        wm2.get_project("nonexistent").unwrap().is_none(),
        "absent project must return None after reopen"
    );
}

#[test]
fn file_roundtrip_preserves_project_edges_across_reopen() {
    let p = fresh_tmp_db_path();

    {
        let wm = WorldModel::open(&p).unwrap();
        wm.upsert_project("a", json!({})).unwrap();
        wm.upsert_project("b", json!({})).unwrap();
        wm.upsert_project("c", json!({})).unwrap();
        wm.add_project_edge("a", "b", "depends_on", 0.9).unwrap();
        wm.add_project_edge("a", "c", "shares_dep", 0.4).unwrap();
        wm.add_project_edge("b", "c", "depends_on", 0.7).unwrap();
    }

    let wm2 = WorldModel::open(&p).unwrap();
    let nbrs_a = wm2.project_neighbors("a").unwrap();
    assert_eq!(nbrs_a.len(), 2, "edges from a survive reopen");
    // Ordered by weight DESC -> depends_on(0.9) before shares_dep(0.4)
    assert_eq!(nbrs_a[0].to, "b");
    assert_eq!(nbrs_a[0].edge_type, "depends_on");
    assert!((nbrs_a[0].weight - 0.9).abs() < 1e-9);
    assert_eq!(nbrs_a[1].to, "c");

    let nbrs_b = wm2.project_neighbors("b").unwrap();
    assert_eq!(nbrs_b.len(), 1);
    assert_eq!(nbrs_b[0].to, "c");
}

#[test]
fn file_roundtrip_preserves_tool_reliability_across_reopen() {
    let p = fresh_tmp_db_path();

    {
        let wm = WorldModel::open(&p).unwrap();
        for _ in 0..7 {
            wm.record_tool_outcome("Bash", "file_ops", true).unwrap();
        }
        for _ in 0..3 {
            wm.record_tool_outcome("Bash", "file_ops", false).unwrap();
        }
        wm.record_tool_outcome("Bash", "uninstall", false).unwrap();
    }

    let wm2 = WorldModel::open(&p).unwrap();
    let r1 = wm2.tool_reliability("Bash", "file_ops").unwrap().unwrap();
    assert_eq!(r1.success_count, 7);
    assert_eq!(r1.failure_count, 3);
    // Laplace-smoothed: (7+1)/(7+3+2) = 0.6666...
    assert!((r1.success_rate() - 8.0 / 12.0).abs() < 1e-9);

    let r2 = wm2.tool_reliability("Bash", "uninstall").unwrap().unwrap();
    assert_eq!(r2.success_count, 0);
    assert_eq!(r2.failure_count, 1);

    assert!(
        wm2.tool_reliability("Bash", "never_recorded")
            .unwrap()
            .is_none(),
        "missing class returns None after reopen"
    );
}

#[test]
fn file_roundtrip_preserves_revealed_preferences_across_reopen() {
    let p = fresh_tmp_db_path();
    let final_p1;

    {
        let wm = WorldModel::open(&p).unwrap();
        wm.update_preference("elite-engineering", "s1", 1.0)
            .unwrap();
        let p1 = wm
            .update_preference("elite-engineering", "s2", 1.0)
            .unwrap();
        wm.update_preference("verbose-output", "s3", -2.0).unwrap();
        final_p1 = p1;
    }

    let wm2 = WorldModel::open(&p).unwrap();
    let elite = wm2.get_preference("elite-engineering").unwrap().unwrap();
    assert_eq!(elite.evidence_count, 2);
    assert_eq!(elite.last_evidence.as_deref(), Some("s2"));
    assert!((elite.probability - final_p1).abs() < 1e-9);

    let verbose = wm2.get_preference("verbose-output").unwrap().unwrap();
    assert!(verbose.probability < 0.5);

    let top = wm2.top_preferences(10).unwrap();
    assert!(top.len() >= 2);
    assert_eq!(top[0].key, "elite-engineering");
    assert!(top[0].probability > top.last().unwrap().probability);
}

#[test]
fn re_upserting_existing_project_after_reopen_overwrites_payload() {
    let p = fresh_tmp_db_path();

    {
        let wm = WorldModel::open(&p).unwrap();
        wm.upsert_project("obelion", json!({"v":"0.1.0"})).unwrap();
    }
    {
        let wm = WorldModel::open(&p).unwrap();
        let before = wm.get_project("obelion").unwrap().unwrap();
        assert_eq!(before.payload["v"], "0.1.0");
        wm.upsert_project("obelion", json!({"v":"0.2.1"})).unwrap();
    }
    let wm3 = WorldModel::open(&p).unwrap();
    assert_eq!(
        wm3.get_project("obelion").unwrap().unwrap().payload["v"],
        "0.2.1",
        "second reopen sees the post-upsert payload"
    );
}

#[test]
fn truncated_file_either_refuses_or_opens_clean_never_silent_garbage() {
    let p = fresh_tmp_db_path();

    {
        let wm = WorldModel::open(&p).unwrap();
        wm.upsert_project("obelion", json!({"v":"0.2.1"})).unwrap();
        wm.record_tool_outcome("Bash", "file_ops", true).unwrap();
    }

    // Truncate the file to 16 bytes — destroys the SQLite header. SQLite will
    // refuse to open it; that's the GREEN outcome here. A successful open with
    // garbage data would be a RED finding (silent corruption).
    let original = std::fs::read(&p).expect("read DB file");
    assert!(original.len() > 16, "DB file must have written real bytes");
    std::fs::write(&p, &original[..16]).expect("truncate");

    match WorldModel::open(&p) {
        Err(_) => {
            // RED→GREEN: open refused. Expected — corrupted header.
        }
        Ok(wm) => {
            // If SQLite accepted the file, it MUST report empty/clean state,
            // never the stale rows. Probe both tables.
            let proj = wm.get_project("obelion").unwrap_or(None);
            let tool = wm.tool_reliability("Bash", "file_ops").unwrap_or(None);
            assert!(
                proj.is_none() && tool.is_none(),
                "truncated file must not produce silent-garbage hits — found {proj:?} / {tool:?}"
            );
        }
    }
}

#[test]
fn many_writes_across_reopens_remain_consistent() {
    let p = fresh_tmp_db_path();

    // 5 reopen cycles, each adds 4 projects + 2 edges + 2 tool outcomes.
    for cycle in 0..5 {
        let wm = WorldModel::open(&p).unwrap();
        for k in 0..4 {
            wm.upsert_project(&format!("p-{cycle}-{k}"), json!({"cycle":cycle,"k":k}))
                .unwrap();
        }
        wm.add_project_edge(
            &format!("p-{cycle}-0"),
            &format!("p-{cycle}-1"),
            "depends_on",
            1.0,
        )
        .unwrap();
        wm.add_project_edge(
            &format!("p-{cycle}-0"),
            &format!("p-{cycle}-2"),
            "shares_dep",
            0.5,
        )
        .unwrap();
        wm.record_tool_outcome("X", &format!("class-{cycle}"), true)
            .unwrap();
        wm.record_tool_outcome("X", &format!("class-{cycle}"), false)
            .unwrap();
    }

    // Final reopen: confirm all 20 projects, 10 edges, 5 distinct tool-class rows present.
    let wm = WorldModel::open(&p).unwrap();
    for cycle in 0..5 {
        for k in 0..4 {
            let pj = wm
                .get_project(&format!("p-{cycle}-{k}"))
                .unwrap()
                .unwrap_or_else(|| panic!("project p-{cycle}-{k} missing after all reopens"));
            assert_eq!(pj.payload["cycle"], cycle);
            assert_eq!(pj.payload["k"], k);
        }
        let nbrs = wm.project_neighbors(&format!("p-{cycle}-0")).unwrap();
        assert_eq!(nbrs.len(), 2, "cycle {cycle} produced 2 outbound edges");
        let r = wm
            .tool_reliability("X", &format!("class-{cycle}"))
            .unwrap()
            .unwrap();
        assert_eq!(r.success_count, 1);
        assert_eq!(r.failure_count, 1);
    }
}
