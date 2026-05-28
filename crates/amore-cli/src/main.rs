// @file-size-exempt: CLI dispatcher — registry of all top-level subcommand handlers.
// amore — CLI entry point.
//
// Subcommands (v0.1.0+):
//   init <ide>          Patch the IDE's MCP config file to register amore.
//   recall <query>      Embed query, search Qdrant, print top hits as JSON.
//   serve               Launch amore-mcp inline (wraps the same code path).
//   status              Print resolved daemon URLs + version info.
//   doctor              Self-diagnose all dep states (Ollama, Qdrant, SQLite,
//                       data dir writable). Outputs machine-readable JSON.
//   snapshot create     Bundle Qdrant + SQLite into tar.gz + .sha256 sidecar.
//   snapshot restore    Verify sha256, untar, restore Qdrant + SQLite.
//   secrets set <name>  Store secret in OS keyring (no-echo prompt).
//   secrets get <name>  Retrieve secret from OS keyring.
//   flags list          Print compile-time features + runtime AMORE_FLAG_* flags.

// ADR 0010: no-unwrap policy. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod commands {
    pub mod flags;
    pub mod snapshot;
}
mod secrets;
mod update;

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
    /// Create or restore a point-in-time snapshot (tar.gz + .sha256 sidecar).
    Snapshot {
        #[command(subcommand)]
        action: SnapshotCommand,
    },
    /// Store and retrieve secrets via the OS keyring.
    Secrets {
        #[command(subcommand)]
        action: SecretsAction,
    },
    /// Print active compile-time Cargo features and runtime AMORE_FLAG_* flags.
    Flags(commands::flags::FlagsArgs),
    /// Check for and apply binary updates from GitHub releases.
    Update {
        #[command(subcommand)]
        action: UpdateAction,
    },
    /// Collect crash diagnostics into a portable bundle.
    Diag {
        #[command(subcommand)]
        action: DiagAction,
    },
}

#[derive(Subcommand)]
enum UpdateAction {
    /// Check whether a newer release is available (24h gate; AMORE_NO_AUTOUPDATE=1 skips).
    Check,
    /// Apply the latest release update (prompts before replacing binary).
    Apply,
}

#[derive(Subcommand)]
enum DiagAction {
    /// Bundle recent crash dumps into a tar.gz archive for sharing.
    Bundle {
        /// Output archive path (defaults to ./amore-diag.tar.gz).
        #[arg(long)]
        output: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum SecretsAction {
    /// Prompt for a secret and store it in the OS keyring (no-echo input).
    Set {
        /// Logical name for this secret (e.g. qdrant_api_key).
        name: String,
    },
    /// Retrieve a secret from the OS keyring and print it to stdout.
    Get {
        /// Logical name of the secret to retrieve.
        name: String,
    },
}

#[derive(Subcommand)]
enum SnapshotCommand {
    /// Bundle Qdrant collection snapshots + SQLite db into <path>.tar.gz
    /// with a <path>.tar.gz.sha256 sidecar.
    Create {
        /// Output archive path (e.g. /tmp/amore-snap.tar.gz).
        path: PathBuf,
        /// Directory containing amore.db (defaults to $AMORE_DATA_DIR or
        /// the platform config dir / Amore).
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    /// Verify the .sha256 sidecar, untar, restore Qdrant snapshots via upload
    /// API, and atomically replace the SQLite database.
    Restore {
        /// Archive path produced by `snapshot create`.
        path: PathBuf,
        /// Directory where amore.db should be written (defaults to same as create).
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install crash handler first so crashes during init are captured.
    amore_core::diag::install_crash_handler();

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
        Command::Snapshot { action } => cmd_snapshot(action).await,
        Command::Secrets { action } => cmd_secrets(action),
        Command::Flags(args) => commands::flags::run(args),
        Command::Update { action } => cmd_update(action).await,
        Command::Diag { action } => cmd_diag(action),
    }
}

fn cmd_diag(action: DiagAction) -> Result<()> {
    match action {
        DiagAction::Bundle { output } => {
            let out = output.unwrap_or_else(|| std::path::PathBuf::from("amore-diag.tar.gz"));
            let bundle = amore_core::diag::collect_diag_bundle(&out, 20)?;
            println!("Diagnostic bundle written to: {}", bundle.display());
        }
    }
    Ok(())
}

async fn cmd_update(action: UpdateAction) -> Result<()> {
    match action {
        UpdateAction::Check => {
            let status = update::check_for_update().await?;
            match &status {
                update::UpdateStatus::Disabled => println!("Auto-update disabled (AMORE_NO_AUTOUPDATE=1)."),
                update::UpdateStatus::TooSoon => println!("Last check was less than 24h ago. Run `amore update apply` to force."),
                update::UpdateStatus::UpToDate => println!("amore v{} is the latest release.", amore_core::VERSION),
                update::UpdateStatus::Available { version } => {
                    println!("Update available: v{version}. Run `amore update apply` to install.");
                }
            }
        }
        UpdateAction::Apply => {
            let status = update::check_for_update().await?;
            update::apply_update(status, false).await?;
        }
    }
    Ok(())
}

async fn cmd_snapshot(action: SnapshotCommand) -> Result<()> {
    match action {
        SnapshotCommand::Create { path, data_dir } => {
            let dd = resolve_data_dir(data_dir);
            commands::snapshot::create(&path, &dd).await
        }
        SnapshotCommand::Restore { path, data_dir } => {
            let dd = resolve_data_dir(data_dir);
            commands::snapshot::restore(&path, &dd).await
        }
    }
}

fn cmd_secrets(action: SecretsAction) -> Result<()> {
    match action {
        SecretsAction::Set { name } => secrets::set_password(&name),
        SecretsAction::Get { name } => {
            let val = secrets::get_password(&name)?;
            println!("{val}");
            Ok(())
        }
    }
}

fn resolve_data_dir(override_dir: Option<PathBuf>) -> PathBuf {
    if let Some(d) = override_dir {
        return d;
    }
    if let Ok(v) = std::env::var("AMORE_DATA_DIR").or_else(|_| std::env::var("OBELION_DATA_DIR")) {
        return PathBuf::from(v);
    }
    dirs::config_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Amore")
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
    // A7: use workspace http factory so HTTP_PROXY/HTTPS_PROXY/NO_PROXY are honoured.
    // Timeout kept at 3s (probe semantics: fast-fail, not default 30s).
    let client = match amore_core::http::build_client(3) {
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
