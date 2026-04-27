// amore-eval — token-reduction harness (A9 / S13 gate).
//
// Measures the central product claim: Amore's canonical-docs router reduces
// context tokens vs the baseline of dumping the raw context an untaught agent
// would gather. Gate passes when avg reduction >=85% AND worst-class >=75%
// across >=30 fixtures (coding/docs/research/debugging classes).
//
// Elite-bar contract (CLAUDE.md): never claim PASS from synthetic proof.
// Fixtures point at REAL files in the user's workspace — no fabricated
// "imagined raw context". The baseline is the actual byte stream a naive

// ADR 0010: no-unwrap policy. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]
// agent would Read; the optimized stream is the canonical-docs router excerpt.
//
// Output: appends rows to docs/results.tsv (8-col QA schema):
//   ts \t step \t trigger \t verdict \t metric \t value \t artifact \t commit

use amore_core::docs::CanonicalDocsRouter;
use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use time::format_description::well_known::Iso8601;
use time::format_description::well_known::iso8601::{Config, EncodedConfig, TimePrecision};

const TS_CFG: EncodedConfig = Config::DEFAULT
    .set_time_precision(TimePrecision::Minute {
        decimal_digits: None,
    })
    .encode();

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub id: String,
    /// coding | docs | research | debugging
    pub class: String,
    pub query: String,
    /// Real files an untaught agent would gather. `~/` expanded to home.
    pub raw_context_files: Vec<String>,
    /// Canonical-docs router search paths. `~/` expanded to home.
    pub canonical_docs_paths: Vec<String>,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Serialize)]
pub struct FixtureResult {
    pub id: String,
    pub class: String,
    pub baseline_tokens: usize,
    pub optimized_tokens: usize,
    pub reduction_pct: f32,
    pub canonical_hits: usize,
}

#[derive(Parser)]
#[clap(about = "Measures token reduction of Amore canonical-docs router vs raw-context baseline.")]
struct Args {
    /// Directory of fixture *.json files.
    #[clap(long, default_value = "crates/eval/fixtures")]
    fixtures: PathBuf,
    /// Append proof rows here (8-col QA TSV).
    #[clap(long, default_value = "docs/results.tsv")]
    results_tsv: PathBuf,
    /// Per-fixture detail to stderr.
    #[clap(long)]
    verbose: bool,
}

pub fn expand_home(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(s)
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

pub fn count_tokens(bpe: &tiktoken_rs::CoreBPE, text: &str) -> usize {
    bpe.encode_with_special_tokens(text).len()
}

pub fn read_concat(paths: &[String]) -> String {
    let mut out = String::new();
    for raw in paths {
        let p = expand_home(raw);
        if let Ok(s) = fs::read_to_string(&p) {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("===== {} =====\n", p.display()));
            out.push_str(&s);
        }
    }
    out
}

pub fn load_fixtures(dir: &Path) -> Result<Vec<Fixture>> {
    if !dir.exists() {
        anyhow::bail!("fixtures dir not found: {}", dir.display());
    }
    let mut out: Vec<Fixture> = Vec::new();
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let body = fs::read_to_string(entry.path())
            .with_context(|| format!("read fixture {}", entry.path().display()))?;
        let f: Fixture = serde_json::from_str(&body)
            .with_context(|| format!("parse fixture {}", entry.path().display()))?;
        out.push(f);
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    if out.is_empty() {
        anyhow::bail!("no *.json fixtures found in {}", dir.display());
    }
    Ok(out)
}

pub fn run_one(
    bpe: &tiktoken_rs::CoreBPE,
    router: &CanonicalDocsRouter,
    f: &Fixture,
) -> Result<FixtureResult> {
    let baseline_text = read_concat(&f.raw_context_files);
    let baseline_tokens = count_tokens(bpe, &baseline_text);

    let paths_expanded: Vec<PathBuf> = f
        .canonical_docs_paths
        .iter()
        .map(|s| expand_home(s))
        .collect();
    let paths_ref: Vec<&Path> = paths_expanded.iter().map(|p| p.as_path()).collect();
    let hits = router.route(&f.query, &paths_ref)?;
    let optimized_text = if hits.is_empty() {
        String::new()
    } else {
        hits.iter()
            .map(|h| format!("[{}] (score={:.3})\n{}", h.title, h.topic_score, h.excerpt))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let optimized_tokens = count_tokens(bpe, &optimized_text);
    let reduction_pct = if baseline_tokens == 0 {
        0.0
    } else {
        (1.0_f32 - optimized_tokens as f32 / baseline_tokens as f32) * 100.0
    };
    Ok(FixtureResult {
        id: f.id.clone(),
        class: f.class.clone(),
        baseline_tokens,
        optimized_tokens,
        reduction_pct,
        canonical_hits: hits.len(),
    })
}

fn append_results_tsv(path: &Path, rows: &[String]) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).ok();
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open results.tsv at {}", path.display()))?;
    for row in rows {
        file.write_all(row.as_bytes())?;
        if !row.ends_with('\n') {
            file.write_all(b"\n")?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let bpe = tiktoken_rs::cl100k_base().context("initialize cl100k_base BPE")?;
    let router = CanonicalDocsRouter::new();
    let fixtures = load_fixtures(&args.fixtures)?;
    tracing::info!(
        "loaded {} fixtures from {}",
        fixtures.len(),
        args.fixtures.display()
    );

    let mut results: Vec<FixtureResult> = Vec::new();
    for f in &fixtures {
        let r = run_one(&bpe, &router, f)?;
        if args.verbose {
            eprintln!("{}", serde_json::to_string(&r)?);
        }
        results.push(r);
    }

    let n = results.len();
    let avg = results.iter().map(|r| r.reduction_pct).sum::<f32>() / n as f32;
    let worst = results
        .iter()
        .map(|r| r.reduction_pct)
        .fold(f32::INFINITY, f32::min);
    let worst_id = results
        .iter()
        .min_by(|a, b| a.reduction_pct.partial_cmp(&b.reduction_pct).unwrap_or(std::cmp::Ordering::Equal))
        .map(|r| r.id.clone())
        .unwrap_or_default();
    let worst_class = results
        .iter()
        .find(|r| r.id == worst_id)
        .map(|r| r.class.clone())
        .unwrap_or_else(|| "?".to_string());

    let pass_avg = avg >= 85.0;
    let pass_worst = worst >= 75.0;
    let pass_n = n >= 30;
    let verdict = if pass_avg && pass_worst && pass_n {
        "PASS"
    } else {
        "PARTIAL"
    };

    let ts = OffsetDateTime::now_utc()
        .format(&Iso8601::<TS_CFG>)
        .unwrap_or_else(|_| "unknown-ts".to_string());

    let summary_row = format!(
        "{ts}\tA9\tcargo run -p amore-eval --bin token-reduction\t{verdict}\ttoken_reduction\tavg={avg:.1}%/worst={worst:.1}%(class={worst_class},id={worst_id})/N={n}\tcrates/eval/src/main.rs\tUNCOMMITTED"
    );
    let per_fixture: Vec<String> = results
        .iter()
        .map(|r| {
            format!(
                "{ts}\tA9-fx-{id}\tcargo run -p amore-eval --bin token-reduction\t{v}\t{cls}_reduction\tbaseline={b}/optimized={o}/reduction={p:.1}%/hits={h}\tcrates/eval/fixtures/{id}.json\tUNCOMMITTED",
                id = r.id,
                v = if r.reduction_pct >= 75.0 {
                    "PASS"
                } else {
                    "PARTIAL"
                },
                cls = r.class,
                b = r.baseline_tokens,
                o = r.optimized_tokens,
                p = r.reduction_pct,
                h = r.canonical_hits,
            )
        })
        .collect();

    let mut all_rows = per_fixture;
    all_rows.push(summary_row);
    append_results_tsv(&args.results_tsv, &all_rows)?;

    println!(
        "A9 token-reduction: verdict={} avg={:.1}% worst={:.1}% (id={}) N={} gate(avg>=85,worst>=75,N>=30)={}/{}/{}",
        verdict,
        avg,
        worst,
        worst_id,
        n,
        if pass_avg { "PASS" } else { "FAIL" },
        if pass_worst { "PASS" } else { "FAIL" },
        if pass_n { "PASS" } else { "FAIL" },
    );

    Ok(())
}
