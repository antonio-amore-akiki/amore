// amore — CLI entry point.
//
// Subcommands (v0.1.0+):
//   init <ide>          Patch the IDE's MCP config file to register amore.
//   recall <query>      Embed query, search Qdrant, print top hits as JSON.
//   serve               Launch amore-mcp inline (wraps the same code path).
//   status              Print resolved daemon URLs + version info.
//   doctor              Self-diagnose all dep states (Ollama, Qdrant, SQLite,
//                       data dir writable). Outputs machine-readable JSON.

// ADR 0010: no-unwrap policy. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

use amore_adapter_claude::ClaudeAdapter;
use amore_adapter_cline::ClineAdapter;
use amore_adapter_codex::CodexAdapter;
use amore_adapter_cursor::CursorAdapter;
use amore_adapter_hermes::HermesAdapter;
use amore_adapter_opencode::OpencodeAdapter;
use amore_adapter_windsurf::WindsurfAdapter;
use amore_core::ide_adapter::{ApplyOutcome, IdeAdapter, apply, dry_run};
use amore_core::sqlite_store::SqliteStore;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "amore", version, about = "Universal MCP agent memory backbone")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Register the Amore MCP server in an IDE's config file.
    Init {
        /// Which IDE to wire up. Supported: claude, cursor, codex, cline, opencode, windsurf, hermes.
        ide: String,
        /// Print the proposed merged config without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Run the amore-mcp server inline (alias for the amore-mcp binary).
    Serve,
    /// Embed a query and print top hits from the configured Qdrant store.
    Recall {
        /// Natural-language query.
        query: String,
        /// Number of hits to return.
        #[arg(long, default_value_t = 5)]
        top_k: usize,
    },
    /// Print resolved daemon endpoints + version info.
    Status,
    /// Self-diagnose all dep states. Outputs JSON; exits non-zero if any
    /// FAIL check is detected. Use this to triage "why isn't Amore
    /// working?" without scrolling MCP server logs.
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Init { ide, dry_run: dry } => cmd_init(&ide, dry),
        Command::Serve => cmd_serve(),
        Command::Recall { query, top_k } => cmd_recall(query, top_k),
        Command::Status => cmd_status(),
        Command::Doctor => cmd_doctor().await,
    }
}

fn cmd_init(ide: &str, dry: bool) -> Result<()> {
    let adapter: Box<dyn IdeAdapter> = match ide {
        "claude" => Box::new(ClaudeAdapter::new()),
        "cursor" => Box::new(CursorAdapter::new()),
        "codex" => Box::new(CodexAdapter::new()),
        "cline" => Box::new(ClineAdapter::new()),
        "opencode" => Box::new(OpencodeAdapter::new()),
        "windsurf" => Box::new(WindsurfAdapter::new()),
        "hermes" => Box::new(HermesAdapter::new()),
        other => anyhow::bail!(
            "unknown IDE '{other}'. Supported: claude, cursor, codex, cline, opencode, windsurf, hermes."
        ),
    };
    if dry {
        let merged = dry_run(adapter.as_ref())?;
        let path = adapter.config_path()?;
        println!("# dry-run: {} -> {}", adapter.name(), path.display());
        print!("{merged}");
        if !merged.ends_with('\n') {
            println!();
        }
        return Ok(());
    }
    match apply(adapter.as_ref())? {
        ApplyOutcome::NoChange => {
            println!(
                "[{}] no change — amore already registered in {}",
                adapter.name(),
                adapter.config_path()?.display()
            );
        }
        ApplyOutcome::Created(p) => {
            println!("[{}] created {}", adapter.name(), p.display());
        }
        ApplyOutcome::Updated { path, backup } => {
            println!(
                "[{}] updated {} (backup at {})",
                adapter.name(),
                path.display(),
                backup.display()
            );
        }
    }
    Ok(())
}

fn cmd_serve() -> Result<()> {
    anyhow::bail!(
        "`amore serve` is reserved for v0.2.0. Use the dedicated amore-mcp binary today: \
         build with `cargo build -p amore-mcp` then run amore-mcp from your IDE's MCP config."
    );
}

fn cmd_recall(_query: String, _top_k: usize) -> Result<()> {
    anyhow::bail!(
        "`amore recall` is reserved for v0.2.0. Today, call the MCP server via your IDE \
         (run `amore init <ide>` first), or test directly with the `mcp_handshake` integration test."
    );
}

/// Read an env var with legacy OBELION_* alias support.
fn env_or_legacy(amore_key: &str, legacy_key: &str, default: &str) -> String {
    if let Ok(v) = std::env::var(amore_key) {
        return v;
    }
    if let Ok(v) = std::env::var(legacy_key) {
        eprintln!(
            "warning: deprecated env var {} — use {} instead (removed in v0.4.0)",
            legacy_key, amore_key
        );
        return v;
    }
    default.to_string()
}

fn cmd_status() -> Result<()> {
    let qdrant = env_or_legacy(
        "AMORE_QDRANT_URL",
        "OBELION_QDRANT_URL",
        "http://127.0.0.1:6334 (default)",
    );
    let ollama = env_or_legacy(
        "AMORE_OLLAMA_URL",
        "OBELION_OLLAMA_URL",
        "http://127.0.0.1:11434 (default)",
    );
    let collection = env_or_legacy("AMORE_COLLECTION", "OBELION_COLLECTION", "amore (default)");
    println!("amore v{}", amore_core::VERSION);
    println!("  qdrant     {qdrant}");
    println!("  ollama     {ollama}");
    println!("  collection {collection}");
    Ok(())
}

#[derive(serde::Serialize)]
struct DoctorCheck {
    name: &'static str,
    verdict: &'static str, // "PASS" | "FAIL" | "WARN"
    detail: String,
}

#[derive(serde::Serialize)]
struct DoctorReport {
    version: &'static str,
    checks: Vec<DoctorCheck>,
    all_pass: bool,
}

fn resolve_doctor_sqlite_path() -> PathBuf {
    if let Ok(dir) = std::env::var("AMORE_DATA_DIR").or_else(|_| {
        std::env::var("OBELION_DATA_DIR").inspect(|_| {
            eprintln!("warning: deprecated OBELION_DATA_DIR — use AMORE_DATA_DIR (removed v0.4.0)");
        })
    }) {
        return PathBuf::from(dir).join("amore.db");
    }
    let base = dirs::config_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Amore").join("amore.db")
}

async fn probe_http(url: &str) -> DoctorCheck {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return DoctorCheck {
                name: "http_client_init",
                verdict: "FAIL",
                detail: format!("reqwest builder: {e}"),
            };
        }
    };
    match client.get(url).send().await {
        Ok(r) if r.status().is_success() => DoctorCheck {
            name: "http_probe",
            verdict: "PASS",
            detail: format!("GET {url} -> {}", r.status()),
        },
        Ok(r) => DoctorCheck {
            name: "http_probe",
            verdict: "FAIL",
            detail: format!("GET {url} -> {}", r.status()),
        },
        Err(e) => DoctorCheck {
            name: "http_probe",
            verdict: "FAIL",
            detail: format!("GET {url}: {e}"),
        },
    }
}

async fn cmd_doctor() -> Result<()> {
    let ollama_url = env_or_legacy(
        "AMORE_OLLAMA_URL",
        "OBELION_OLLAMA_URL",
        "http://127.0.0.1:11434",
    );
    let qdrant_http_url = env_or_legacy(
        "AMORE_QDRANT_HTTP_URL",
        "OBELION_QDRANT_HTTP_URL",
        "http://127.0.0.1:6333",
    );

    let mut checks: Vec<DoctorCheck> = Vec::new();

    // 1. Ollama reachable + responding to /api/version
    let mut c = probe_http(&format!("{ollama_url}/api/version")).await;
    c.name = "ollama_api_version";
    if c.verdict == "FAIL" {
        c.detail = format!("{} (remediation: `ollama serve` to start)", c.detail);
    }
    checks.push(c);

    // 2. Qdrant HTTP /readyz
    let mut c = probe_http(&format!("{qdrant_http_url}/readyz")).await;
    c.name = "qdrant_http_readyz";
    if c.verdict == "FAIL" {
        c.detail = format!(
            "{} (remediation: `docker run -d -p 6333:6333 -p 6334:6334 qdrant/qdrant:v1.15.4`)",
            c.detail
        );
    }
    checks.push(c);

    // 3. SQLite store path + WAL mode + count
    let sqlite_path = resolve_doctor_sqlite_path();
    if let Some(parent) = sqlite_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let sqlite_check = match SqliteStore::open(&sqlite_path) {
        Ok(store) => {
            let mode = store.journal_mode().unwrap_or_else(|e| format!("err: {e}"));
            let count = store.count_observations().unwrap_or(0);
            let ok = mode.eq_ignore_ascii_case("wal");
            DoctorCheck {
                name: "sqlite_store",
                verdict: if ok { "PASS" } else { "WARN" },
                detail: format!(
                    "path={} journal_mode={} observations={}",
                    sqlite_path.display(),
                    mode,
                    count
                ),
            }
        }
        Err(e) => DoctorCheck {
            name: "sqlite_store",
            verdict: "FAIL",
            detail: format!("open {}: {e}", sqlite_path.display()),
        },
    };
    checks.push(sqlite_check);

    // 4. Data dir writable
    let probe_file = sqlite_path
        .parent()
        .map(|p| p.join(".amore-doctor-probe"))
        .unwrap_or_else(|| PathBuf::from(".amore-doctor-probe"));
    let write_check = match std::fs::write(&probe_file, b"amore-doctor") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe_file);
            DoctorCheck {
                name: "data_dir_writable",
                verdict: "PASS",
                detail: format!("{} writable", probe_file.parent().expect("invariant: probe_file path always has a parent directory").display()),
            }
        }
        Err(e) => DoctorCheck {
            name: "data_dir_writable",
            verdict: "FAIL",
            detail: format!("write probe at {}: {e}", probe_file.display()),
        },
    };
    checks.push(write_check);

    let all_pass = checks.iter().all(|c| c.verdict == "PASS");
    let report = DoctorReport {
        version: amore_core::VERSION,
        checks,
        all_pass,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !all_pass {
        std::process::exit(1);
    }
    Ok(())
}
