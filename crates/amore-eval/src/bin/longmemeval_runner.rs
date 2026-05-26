// crates/amore-eval/src/bin/longmemeval_runner.rs — LongMemEval evaluation harness.
//
// Runs Amore against the LongMemEval benchmark (xiaowu0162/LongMemEval, MIT).
// Dataset: https://huggingface.co/datasets/xiaowu0162/LongMemEval
//
// DATASET DOWNLOAD (one-time, ~100 MB):
//   pip install datasets
//   python -c "from datasets import load_dataset; \
//     ds = load_dataset('xiaowu0162/LongMemEval', split='test'); \
//     ds.to_json('~/.local/share/Amore/datasets/longmemeval/test.jsonl')"
//
// Then run:
//   amore-eval-longmemeval \
//     --dataset ~/.local/share/Amore/datasets/longmemeval/test.jsonl \
//     --output ./longmemeval-results.json
//
// Requires: AMORE_QDRANT_URL (default 127.0.0.1:6333) + AMORE_OLLAMA_URL.
// Without daemons: exits with status "skipped-no-daemon".
// Recall loop placeholder until Wave 3 wiring — R@K = 0 is expected until then.

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Parser)]
#[command(name = "amore-eval-longmemeval", about = "LongMemEval benchmark runner")]
struct Cli {
    #[arg(long)]
    dataset: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    max_sessions: usize,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    qdrant_url: Option<String>,
    #[arg(long)]
    ollama_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Session {
    #[allow(dead_code)] // used for logging; not yet printed
    session_id: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    history: Vec<Turn>,
    #[serde(default)]
    queries: Vec<EvalQuery>,
}

#[derive(Debug, Deserialize)]
struct Turn {
    #[allow(dead_code)]
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct EvalQuery {
    query: String,
    #[serde(default)]
    gold_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EvalReport {
    ts: String,
    dataset_path: String,
    sessions_evaluated: usize,
    queries_evaluated: usize,
    overall: RecallAtK,
    per_category: HashMap<String, RecallAtK>,
    status: String,
    notes: String,
}

#[derive(Debug, Serialize, Clone, Default)]
struct RecallAtK {
    r_at_1: f64,
    r_at_5: f64,
    r_at_10: f64,
    n_queries: usize,
}

fn now_ts() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (sec, min, hour, days) = (secs % 60, (secs / 60) % 60, (secs / 3600) % 24, secs / 86400);
    let (y, m, d) = epoch_days_to_ymd(days);
    format!("{y:04}{m:02}{d:02}T{hour:02}{min:02}{sec:02}Z")
}

fn epoch_days_to_ymd(z: u64) -> (u64, u64, u64) {
    let z = z + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn default_dataset() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Amore").join("datasets").join("longmemeval").join("test.jsonl")
}

fn default_output() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Amore").join("benchmarks");
    std::fs::create_dir_all(&base).context("create benchmarks dir")?;
    Ok(base.join(format!("{}-longmemeval.json", now_ts())))
}

fn daemon_reachable(addr: &str) -> bool {
    use std::net::TcpStream;
    let a = addr.trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/');
    a.parse().ok()
        .and_then(|sa| TcpStream::connect_timeout(&sa, Duration::from_secs(2)).ok())
        .is_some()
}

fn compute_recall(hits: &[String], gold: &[String]) -> (bool, bool, bool) {
    let gs: std::collections::HashSet<&str> = gold.iter().map(|s| s.as_str()).collect();
    let r1 = hits.first().map(|h| gs.contains(h.as_str())).unwrap_or(false);
    let r5 = hits.iter().take(5).any(|h| gs.contains(h.as_str()));
    let r10 = hits.iter().take(10).any(|h| gs.contains(h.as_str()));
    (r1, r5, r10)
}

fn run_eval(dataset: &PathBuf, max_sessions: usize, qdrant: &str, _ollama: &str) -> Result<EvalReport> {
    if !daemon_reachable(qdrant) {
        return Ok(EvalReport {
            ts: now_ts(), dataset_path: dataset.display().to_string(),
            sessions_evaluated: 0, queries_evaluated: 0,
            overall: RecallAtK::default(), per_category: HashMap::new(),
            status: "skipped-no-daemon".to_string(),
            notes: format!("Qdrant not reachable at {qdrant}. Start daemon then re-run."),
        });
    }
    if !dataset.exists() {
        bail!(
            "Dataset not found at {}.\nDownload: pip install datasets && python -c \
             \"from datasets import load_dataset; \
             load_dataset('xiaowu0162/LongMemEval', split='test').to_json('{}')\"",
            dataset.display(), dataset.display()
        );
    }
    let content = std::fs::read_to_string(dataset)
        .with_context(|| format!("read {}", dataset.display()))?;
    let mut sessions: Vec<Session> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() { continue; }
        match serde_json::from_str::<Session>(line) {
            Ok(s) => sessions.push(s),
            Err(e) => eprintln!("WARN: skipping malformed line {}: {e}", i + 1),
        }
    }
    if max_sessions > 0 { sessions.truncate(max_sessions); }

    let mut cat_acc: HashMap<String, [usize; 4]> = HashMap::new();
    let mut total = 0usize;

    for s in &sessions {
        let cat = if s.category.is_empty() { "unknown".to_string() } else { s.category.clone() };
        let acc = cat_acc.entry(cat).or_insert([0usize; 4]);
        // Seed placeholder: Wave 3 wires recall.index(turn.content) here.
        for t in &s.history { let _ = &t.content; }
        for q in &s.queries {
            // Placeholder: let hits = recall.search(&q.query, 10)?;
            let hits: Vec<String> = Vec::new();
            let (r1, r5, r10) = compute_recall(&hits, &q.gold_ids);
            if r1 { acc[0] += 1; }
            if r5 { acc[1] += 1; }
            if r10 { acc[2] += 1; }
            acc[3] += 1;
            total += 1;
            let _ = &q.query; // silence unused warning until Wave 3
        }
    }

    let mut per_category: HashMap<String, RecallAtK> = HashMap::new();
    let mut ov = [0usize; 4];
    for (cat, acc) in &cat_acc {
        let n = acc[3]; if n == 0 { continue; }
        per_category.insert(cat.clone(), RecallAtK {
            r_at_1: acc[0] as f64 / n as f64,
            r_at_5: acc[1] as f64 / n as f64,
            r_at_10: acc[2] as f64 / n as f64, n_queries: n,
        });
        for i in 0..4 { ov[i] += acc[i]; }
    }
    let overall = if ov[3] > 0 {
        RecallAtK { r_at_1: ov[0] as f64 / ov[3] as f64, r_at_5: ov[1] as f64 / ov[3] as f64,
            r_at_10: ov[2] as f64 / ov[3] as f64, n_queries: ov[3] }
    } else { RecallAtK::default() };

    println!("\n=== LongMemEval  sessions={}  queries={total} ===", sessions.len());
    println!("{:<30} {:>8} {:>8} {:>8}", "Category", "R@1", "R@5", "R@10");
    println!("{}", "-".repeat(54));
    for (cat, r) in &per_category {
        println!("{:<30} {:>7.1}% {:>7.1}% {:>7.1}%", cat, r.r_at_1*100.0, r.r_at_5*100.0, r.r_at_10*100.0);
    }
    println!("{}", "-".repeat(54));
    println!("{:<30} {:>7.1}% {:>7.1}% {:>7.1}%", "OVERALL",
             overall.r_at_1*100.0, overall.r_at_5*100.0, overall.r_at_10*100.0);
    println!("SOTA target (mem0): R@5 = 95.2% (https://github.com/mem0ai/mem0, 2025)");
    println!("NOTE: R@K = 0 until Wave 3 wires the recall loop.");

    Ok(EvalReport { ts: now_ts(), dataset_path: dataset.display().to_string(),
        sessions_evaluated: sessions.len(), queries_evaluated: total,
        overall, per_category, status: "pass-structure-only".to_string(),
        notes: "Recall loop placeholder; R@K = 0 expected until Wave 3 wiring.".to_string(),
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dataset = cli.dataset.unwrap_or_else(default_dataset);
    let qdrant = cli.qdrant_url.or_else(|| std::env::var("AMORE_QDRANT_URL").ok())
        .unwrap_or_else(|| "127.0.0.1:6333".to_string());
    let ollama = cli.ollama_url.or_else(|| std::env::var("AMORE_OLLAMA_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    let report = run_eval(&dataset, cli.max_sessions, &qdrant, &ollama)?;
    let out = cli.output.map(Ok).unwrap_or_else(default_output)?;
    let json = serde_json::to_string_pretty(&report).context("serialize")?;
    std::fs::write(&out, &json).with_context(|| format!("write {}", out.display()))?;
    println!("Report: {}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_hit_top1() {
        let (r1, r5, r10) = compute_recall(&["a".to_string()], &["a".to_string()]);
        assert!(r1 && r5 && r10);
    }

    #[test]
    fn recall_miss_top1_hit_top5() {
        let hits: Vec<String> = (0..6).map(|i| i.to_string()).collect();
        let (r1, r5, r10) = compute_recall(&hits, &["4".to_string()]);
        assert!(!r1 && r5 && r10);
    }

    #[test]
    fn recall_all_miss() {
        let (r1, r5, r10) = compute_recall(&["x".to_string()], &["z".to_string()]);
        assert!(!r1 && !r5 && !r10);
    }

    #[test]
    fn recall_empty_hits() {
        let (r1, r5, r10) = compute_recall(&[], &["a".to_string()]);
        assert!(!r1 && !r5 && !r10);
    }

    #[test]
    fn skipped_when_no_daemon() {
        let r = run_eval(&PathBuf::from("x.jsonl"), 0, "192.0.2.1:9999", "http://192.0.2.1:11434").unwrap();
        assert_eq!(r.status, "skipped-no-daemon");
    }
}
