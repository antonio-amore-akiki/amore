// crates/amore-eval/src/bin/longmemeval_runner.rs — LongMemEval evaluation harness.
//
// Runs Amore against the LongMemEval benchmark (xiaowu0162/LongMemEval, MIT).
// Dataset: https://huggingface.co/datasets/xiaowu0162/LongMemEval
//
// Two modes: --mock-deps (in-memory BM25 only, no daemons) and real (Qdrant+Ollama).
// Per-instance isolated ingestion: index only the instance's haystack, search, clear.
//
// Requires: AMORE_QDRANT_URL (default 127.0.0.1:6333) + AMORE_OLLAMA_URL.

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Duration;

use amore_core::sqlite_store::SqliteStore;

// ─── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(name = "amore-eval-longmemeval", about = "LongMemEval benchmark runner")]
struct Cli {
    #[arg(long)] dataset: Option<PathBuf>,
    /// Evaluate first N instances only (0 = all).
    #[arg(long, default_value_t = 0)] subset: usize,
    #[arg(long, default_value_t = 0)] max_sessions: usize, // legacy alias
    #[arg(long)] output: Option<PathBuf>,
    #[arg(long)] qdrant_url: Option<String>,
    #[arg(long)] ollama_url: Option<String>,
    /// Use in-memory BM25 only — no Qdrant/Ollama required.
    #[arg(long)] mock_deps: bool,
}

// ─── Dataset types ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Instance {
    #[serde(default)] question_id: String,
    #[serde(default)] question_type: String,
    #[serde(default)] question: String,
    #[serde(default)] answer_session_ids: Vec<String>,
    #[serde(default)] haystack_sessions: Vec<Vec<HaystackTurn>>,
    #[serde(default)] category: String,   // legacy converted format
    #[serde(default)] history: Vec<LegacyTurn>,
    #[serde(default)] queries: Vec<EvalQuery>,
}

#[derive(Debug, Deserialize)]
struct HaystackTurn {
    #[allow(dead_code)] role: String,
    content: String,
    #[serde(default)] session_id: String,
}

#[derive(Debug, Deserialize)]
struct LegacyTurn { #[allow(dead_code)] role: String, content: String }

#[derive(Debug, Deserialize)]
struct EvalQuery { query: String, #[serde(default)] gold_ids: Vec<String> }

// ─── Output types ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct EvalReport {
    ts: String, dataset_path: String,
    instances_evaluated: usize, queries_evaluated: usize,
    overall: RecallAtK, per_category: HashMap<String, RecallAtK>,
    mode: String, status: String, notes: String,
}

#[derive(Debug, Serialize, Clone, Default)]
struct RecallAtK {
    r_at_1: f64, r_at_5: f64, r_at_10: f64, mrr: f64, n_queries: usize,
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn now_ts() -> String {
    use std::time::SystemTime;
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_secs();
    let (sec, min, hour, days) = (secs%60, (secs/60)%60, (secs/3600)%24, secs/86400);
    let (y, m, d) = epoch_days_to_ymd(days);
    format!("{y:04}{m:02}{d:02}T{hour:02}{min:02}{sec:02}Z")
}

fn epoch_days_to_ymd(z: u64) -> (u64, u64, u64) {
    let z = z + 719_468; let era = z/146_097; let doe = z - era*146_097;
    let yoe = (doe - doe/1460 + doe/36524 - doe/146_096)/365;
    let y = yoe + era*400; let doy = doe - (365*yoe + yoe/4 - yoe/100);
    let mp = (5*doy+2)/153; let d = doy - (153*mp+2)/5 + 1;
    let m = if mp < 10 { mp+3 } else { mp-9 }; let y = if m <= 2 { y+1 } else { y };
    (y, m, d)
}

fn default_dataset() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."))
        .join("Amore").join("datasets").join("longmemeval").join("test.jsonl")
}

fn default_output() -> Result<PathBuf> {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."))
        .join("Amore").join("benchmarks");
    std::fs::create_dir_all(&base).context("create benchmarks dir")?;
    Ok(base.join(format!("{}-longmemeval.json", now_ts())))
}

fn daemon_reachable(addr: &str) -> bool {
    use std::net::TcpStream;
    let a = addr.trim_start_matches("http://").trim_start_matches("https://").trim_end_matches('/');
    a.parse().ok().and_then(|sa| TcpStream::connect_timeout(&sa, Duration::from_secs(2)).ok()).is_some()
}

fn str_to_u64(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new(); s.hash(&mut h); h.finish()
}

fn compute_recall(hit_sids: &[String], gold: &[String]) -> (bool, bool, bool, f64) {
    let gs: HashSet<&str> = gold.iter().map(|s| s.as_str()).collect();
    let r1  = hit_sids.first().map(|h| gs.contains(h.as_str())).unwrap_or(false);
    let r5  = hit_sids.iter().take(5).any(|h| gs.contains(h.as_str()));
    let r10 = hit_sids.iter().take(10).any(|h| gs.contains(h.as_str()));
    let mrr = hit_sids.iter().enumerate()
        .find_map(|(i, h)| if gs.contains(h.as_str()) { Some(1.0/(i+1) as f64) } else { None })
        .unwrap_or(0.0);
    (r1, r5, r10, mrr)
}

fn infer_sids(inst: &Instance) -> Vec<String> {
    inst.haystack_sessions.iter().enumerate().map(|(i, turns)| {
        turns.iter().find_map(|t| if t.session_id.is_empty() { None } else { Some(t.session_id.clone()) })
            .unwrap_or_else(|| format!("{}-haystack-{}", inst.question_id, i))
    }).collect()
}

fn effective_queries(inst: &Instance) -> Vec<(String, Vec<String>)> {
    if !inst.question.is_empty() { vec![(inst.question.clone(), inst.answer_session_ids.clone())] }
    else { inst.queries.iter().map(|q| (q.query.clone(), q.gold_ids.clone())).collect() }
}

type Acc = [usize; 5]; // [h1, h5, h10, mrr×1e6, n_queries]

fn accum(acc: &mut Acc, h1: usize, h5: usize, h10: usize, mrr_bits: u64, n: usize) {
    acc[0]+=h1; acc[1]+=h5; acc[2]+=h10; acc[3]+=mrr_bits as usize; acc[4]+=n;
}

fn from_acc(acc: &Acc) -> RecallAtK {
    let n = acc[4]; if n == 0 { return RecallAtK::default(); }
    RecallAtK { r_at_1: acc[0] as f64/n as f64, r_at_5: acc[1] as f64/n as f64,
        r_at_10: acc[2] as f64/n as f64, mrr: acc[3] as f64/(n as f64*1_000_000.0), n_queries: n }
}

// ─── Mock-deps instance eval ──────────────────────────────────────────────────

/// Strip English function-words so BM25 FTS5 AND-semantics can match on
/// content-bearing terms only.  FTS5 treats space-separated tokens as an
/// implicit AND; function-words like "what", "is", "for" rarely appear in
/// session content, causing zero matches even when the answer is present.
fn content_keywords(q: &str) -> String {
    const STOPWORDS: &[&str] = &[
        "a","an","the","is","are","was","were","be","been","being",
        "have","has","had","do","does","did","will","would","could","should",
        "may","might","shall","can","need","dare","ought","used",
        "i","me","my","we","our","you","your","he","she","it","its","they","their",
        "this","that","these","those","what","which","who","whom","whose",
        "when","where","why","how","all","each","every","both","few","more","most",
        "other","some","such","no","nor","not","only","own","same","so","than",
        "too","very","just","about","above","after","before","between","by",
        "during","for","from","in","into","of","on","or","and","at","to","with",
        "tell","me","please","did","does","mention","said","say","explain","describe",
        "know","remember","recall","find","get","show","talk","told",
        "use","uses","using",
    ];
    let tokens: Vec<&str> = q
        .split_whitespace()
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|t| {
            let low = t.to_lowercase();
            !t.is_empty() && !STOPWORDS.contains(&low.as_str())
        })
        .collect();
    // Fall back to full query if keyword stripping left nothing.
    if tokens.is_empty() { q.to_string() } else { tokens.join(" ") }
}

fn run_mock(inst: &Instance, top_k: usize) -> Result<(usize, usize, usize, u64, usize)> {
    let store = SqliteStore::open_in_memory().context("in-memory SQLite")?;
    for (sid, turns) in infer_sids(inst).iter().zip(inst.haystack_sessions.iter()) {
        let text: String = turns.iter().map(|t| t.content.as_str()).collect::<Vec<_>>().join(" ");
        // sid passed as source → Bm25Hit.source == session_id (no extra payload lookup needed).
        store.insert_observation(sid, &serde_json::json!({"session_id": sid, "text": text}))
            .with_context(|| format!("indexing session {sid}"))?;
    }
    if inst.haystack_sessions.is_empty() && !inst.history.is_empty() {
        let text: String = inst.history.iter().map(|t| t.content.as_str()).collect::<Vec<_>>().join(" ");
        let sid = inst.question_id.as_str();
        store.insert_observation(sid, &serde_json::json!({"session_id": sid, "text": text}))
            .context("indexing legacy history")?;
    }
    let (mut h1, mut h5, mut h10) = (0, 0, 0);
    let mut mrr_sum = 0.0f64; let mut n = 0usize;
    for (q, gold) in &effective_queries(inst) {
        // Strip function-words: FTS5 AND-semantics fails if stopwords are
        // included since they rarely appear verbatim in session transcripts.
        let kw = content_keywords(q);
        let hits = store.bm25_search(&kw, (top_k*4).max(10) as u64)
            .with_context(|| format!("bm25 search: {kw}"))?;
        let sids: Vec<String> = hits.iter().map(|h| h.source.clone()).take(top_k).collect();
        let (r1, r5, r10, mrr) = compute_recall(&sids, gold);
        if r1 { h1+=1; }
        if r5 { h5+=1; }
        if r10 { h10+=1; }
        mrr_sum += mrr; n += 1;
    }
    Ok((h1, h5, h10, (mrr_sum*1_000_000.0) as u64, n))
}

// ─── Real-daemon instance eval ────────────────────────────────────────────────

async fn run_real(
    inst: &Instance, qdrant_url: &str, ollama_url: &str, top_k: usize, coll: &str,
) -> Result<(usize, usize, usize, u64, usize)> {
    use amore_core::{ollama::OllamaClient, qdrant_store::QdrantStore, recall::HybridRecall};
    let qdrant = QdrantStore::new(&format!("http://{qdrant_url}"), coll, 768)
        .await.with_context(|| format!("connect Qdrant at {qdrant_url}"))?;
    let recall = HybridRecall::new(OllamaClient::new(ollama_url), qdrant);
    for (sid, turns) in infer_sids(inst).iter().zip(inst.haystack_sessions.iter()) {
        let text: String = turns.iter().map(|t| t.content.as_str()).collect::<Vec<_>>().join(" ");
        recall.index(str_to_u64(sid), "longmemeval", &text, Some(serde_json::json!({"session_id": sid})))
            .await.with_context(|| format!("index session {sid}"))?;
    }
    if inst.haystack_sessions.is_empty() && !inst.history.is_empty() {
        let text: String = inst.history.iter().map(|t| t.content.as_str()).collect::<Vec<_>>().join(" ");
        let sid = &inst.question_id;
        recall.index(str_to_u64(sid), "longmemeval", &text, Some(serde_json::json!({"session_id": sid})))
            .await.context("index legacy")?;
    }
    let (mut h1, mut h5, mut h10) = (0, 0, 0);
    let mut mrr_sum = 0.0f64; let mut n = 0usize;
    for (q, gold) in &effective_queries(inst) {
        let env = recall.search(q, top_k).await.with_context(|| format!("search: {q}"))?;
        let sids: Vec<String> = env.hits.iter()
            .filter_map(|h| h.payload.get("session_id")?.as_str().map(|s| s.to_string()))
            .take(top_k).collect();
        let (r1, r5, r10, mrr) = compute_recall(&sids, gold);
        if r1 { h1+=1; }
        if r5 { h5+=1; }
        if r10 { h10+=1; }
        mrr_sum += mrr; n += 1;
    }
    // Per-instance isolation: drop collection (ignore errors from partial create).
    if let Ok(q2) = QdrantStore::open_lazy(&format!("http://{qdrant_url}"), coll) {
        let _ = q2.drop_collection().await;
    }
    Ok((h1, h5, h10, (mrr_sum*1_000_000.0) as u64, n))
}

// ─── Orchestrator ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn run_eval(
    dataset: &PathBuf, subset: usize, qdrant: &str, ollama: &str, mock_deps: bool,
) -> Result<EvalReport> {
    if !mock_deps && !daemon_reachable(qdrant) {
        return Ok(EvalReport {
            ts: now_ts(), dataset_path: dataset.display().to_string(),
            instances_evaluated: 0, queries_evaluated: 0,
            overall: RecallAtK::default(), per_category: HashMap::new(),
            mode: "real-daemons-required".to_string(), status: "skipped-no-daemon".to_string(),
            notes: format!("Qdrant not reachable at {qdrant}. Start daemon or pass --mock-deps."),
        });
    }
    if !dataset.exists() {
        anyhow::bail!(
            "Dataset not found at {}. Download: pip install datasets && python -c \
             \"from datasets import load_dataset; \
             load_dataset('xiaowu0162/LongMemEval', split='test').to_json('{}')\"",
            dataset.display(), dataset.display()
        );
    }
    let content = std::fs::read_to_string(dataset).with_context(|| format!("read {}", dataset.display()))?;
    let mut instances: Vec<Instance> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim(); if line.is_empty() { continue; }
        match serde_json::from_str::<Instance>(line) {
            Ok(s) => instances.push(s),
            Err(e) => eprintln!("WARN: skipping malformed line {}: {e}", i+1),
        }
    }
    if subset > 0 { instances.truncate(subset); }
    if instances.is_empty() { anyhow::bail!("Dataset has 0 instances"); }

    let mode = if mock_deps { "mock-deps-bm25-only" } else { "real-daemons-hybrid" };
    eprintln!("INFO: evaluating {} instances in {mode} mode", instances.len());

    let mut cat_acc: HashMap<String, Acc> = HashMap::new();
    let mut ov = [0usize; 5];

    for (idx, inst) in instances.iter().enumerate() {
        let cat = if !inst.question_type.is_empty() { inst.question_type.clone() }
            else if !inst.category.is_empty() { inst.category.clone() }
            else { "unknown".to_string() };
        let (h1, h5, h10, mrr_bits, n) = if mock_deps {
            run_mock(inst, 10).with_context(|| format!("instance {} ({})", idx, inst.question_id))?
        } else {
            let coll = format!("lme-{}-{idx}", now_ts());
            run_real(inst, qdrant, ollama, 10, &coll).await
                .with_context(|| format!("instance {} ({})", idx, inst.question_id))?
        };
        accum(cat_acc.entry(cat).or_insert([0;5]), h1, h5, h10, mrr_bits, n);
        accum(&mut ov, h1, h5, h10, mrr_bits, n);
        if (idx+1) % 5 == 0 || idx+1 == instances.len() {
            let r = from_acc(&ov);
            eprintln!("  [{}/{}] R@1={:.1}% R@5={:.1}% R@10={:.1}% MRR={:.3}",
                idx+1, instances.len(), r.r_at_1*100.0, r.r_at_5*100.0, r.r_at_10*100.0, r.mrr);
        }
    }

    let per_category: HashMap<String, RecallAtK> = cat_acc.iter()
        .filter(|(_, a)| a[4] > 0).map(|(k, a)| (k.clone(), from_acc(a))).collect();
    let overall = from_acc(&ov);

    println!("\n=== LongMemEval  instances={}  queries={} ===", instances.len(), ov[4]);
    println!("{:<35} {:>8} {:>8} {:>8} {:>8}", "Category", "R@1", "R@5", "R@10", "MRR");
    println!("{}", "-".repeat(67));
    for (cat, r) in &per_category {
        println!("{:<35} {:>7.1}% {:>7.1}% {:>7.1}% {:>7.3}", cat,
            r.r_at_1*100.0, r.r_at_5*100.0, r.r_at_10*100.0, r.mrr);
    }
    println!("{}", "-".repeat(67));
    println!("{:<35} {:>7.1}% {:>7.1}% {:>7.1}% {:>7.3}", "OVERALL",
        overall.r_at_1*100.0, overall.r_at_5*100.0, overall.r_at_10*100.0, overall.mrr);
    let gate = if overall.r_at_5 >= 0.85 && overall.r_at_10 >= 0.90 { "PASS" } else { "FAIL" };
    println!("\nGATE: R@5={:.4} (target ≥0.85) R@10={:.4} (target ≥0.90) → {gate}",
        overall.r_at_5, overall.r_at_10);
    println!("Mode: {mode}");

    let status = if gate == "PASS" { "pass" } else { "fail" }.to_string();
    let notes = format!("R@5={:.4} R@10={:.4} MRR={:.4}. Mode={mode}. \
        Instances={} Queries={}. GATE={gate}. Target: R@5≥0.85 AND R@10≥0.90.",
        overall.r_at_5, overall.r_at_10, overall.mrr, instances.len(), ov[4]);
    Ok(EvalReport { ts: now_ts(), dataset_path: dataset.display().to_string(),
        instances_evaluated: instances.len(), queries_evaluated: ov[4],
        overall, per_category, mode: mode.to_string(), status, notes })
}

// ─── Entry point ──────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dataset = cli.dataset.unwrap_or_else(default_dataset);
    let qdrant = cli.qdrant_url.or_else(|| std::env::var("AMORE_QDRANT_URL").ok())
        .unwrap_or_else(|| "127.0.0.1:6333".to_string());
    let ollama = cli.ollama_url.or_else(|| std::env::var("AMORE_OLLAMA_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    let subset = if cli.subset > 0 { cli.subset } else { cli.max_sessions };
    let report = run_eval(&dataset, subset, &qdrant, &ollama, cli.mock_deps)?;
    let out = cli.output.map(Ok).unwrap_or_else(default_output)?;
    std::fs::write(&out, serde_json::to_string_pretty(&report).context("serialize")?)
        .with_context(|| format!("write {}", out.display()))?;
    println!("Report: {}", out.display());
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_hit_top1() {
        let (r1, r5, r10, mrr) = compute_recall(&["s_a".to_string()], &["s_a".to_string()]);
        assert!(r1 && r5 && r10); assert!((mrr - 1.0).abs() < 1e-9);
    }
    #[test]
    fn recall_miss_top1_hit_top5() {
        let hits: Vec<String> = (0..6).map(|i| format!("s{i}")).collect();
        let (r1, r5, r10, mrr) = compute_recall(&hits, &["s4".to_string()]);
        assert!(!r1 && r5 && r10); assert!((mrr - 0.2).abs() < 1e-9);
    }
    #[test]
    fn recall_all_miss() {
        let (r1, r5, r10, mrr) = compute_recall(&["sx".to_string()], &["sy".to_string()]);
        assert!(!r1 && !r5 && !r10 && mrr == 0.0);
    }
    #[test]
    fn recall_empty_hits() {
        let (r1, r5, r10, mrr) = compute_recall(&[], &["s_a".to_string()]);
        assert!(!r1 && !r5 && !r10 && mrr == 0.0);
    }
    #[test]
    fn skipped_when_no_daemon() {
        let r = run_eval(&PathBuf::from("x.jsonl"), 0, "192.0.2.1:9999",
            "http://192.0.2.1:11434", false).unwrap();
        assert_eq!(r.status, "skipped-no-daemon");
    }
    #[test]
    fn str_to_u64_stable() {
        assert_eq!(str_to_u64("abc"), str_to_u64("abc"));
        assert_ne!(str_to_u64("abc"), str_to_u64("xyz"));
    }
    #[test]
    fn mock_eval_bm25_hits() {
        let inst = Instance {
            question_id: "q1".to_string(), question_type: "single_session".to_string(),
            question: "who uses rust systems programming".to_string(),
            answer_session_ids: vec!["q1-haystack-0".to_string()],
            haystack_sessions: vec![vec![HaystackTurn {
                role: "user".to_string(),
                content: "rust systems programming language safety".to_string(),
                session_id: "q1-haystack-0".to_string(),
            }]],
            category: "".to_string(), history: vec![], queries: vec![],
        };
        let (h1, h5, h10, _mrr, n) = run_mock(&inst, 10).unwrap();
        assert_eq!(n, 1);
        assert!(h5 == 1, "BM25 should surface the single session; h1={h1} h5={h5} h10={h10}");
    }
}
