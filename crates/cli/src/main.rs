// obelion — CLI entry point.
//
// Subcommands (v0.1.0):
//   init claude         Patch ~/.claude.json (Claude Code) to register obelion MCP server.
//   init cursor         Patch ~/.cursor/mcp.json (Cursor) to register obelion MCP server.
//   recall <query>      Embed query, search Qdrant, print top hits as JSON.
//   serve               Launch obelion-mcp inline (wraps the same code path).
//   status              Print resolved daemon URLs + version info.
//
// Init contract: atomic-write (tmp + rename), .bak sibling on overwrite,
// byte-identical on repeat (idempotent). --dry-run prints proposed content
// without touching disk.

use anyhow::Result;
use clap::{Parser, Subcommand};
use obelion_adapter_claude::ClaudeAdapter;
use obelion_adapter_cline::ClineAdapter;
use obelion_adapter_codex::CodexAdapter;
use obelion_adapter_cursor::CursorAdapter;
use obelion_adapter_hermes::HermesAdapter;
use obelion_adapter_opencode::OpencodeAdapter;
use obelion_adapter_windsurf::WindsurfAdapter;
use obelion_core::ide_adapter::{ApplyOutcome, IdeAdapter, apply, dry_run};

#[derive(Parser)]
#[command(
    name = "obelion",
    version,
    about = "Universal MCP agent memory backbone"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Register the obelion MCP server in an IDE's config file.
    Init {
        /// Which IDE to wire up. Supported: claude, cursor, codex, cline, opencode, windsurf, hermes.
        ide: String,
        /// Print the proposed merged config without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Run the obelion-mcp server inline (alias for the obelion-mcp binary).
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
}

fn main() -> Result<()> {
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
                "[{}] no change — obelion already registered in {}",
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
        "`obelion serve` is reserved for v0.2.0. Use the dedicated obelion-mcp binary today: \
         build with `cargo build -p obelion-mcp` then run obelion-mcp from your IDE's MCP config."
    );
}

fn cmd_recall(_query: String, _top_k: usize) -> Result<()> {
    anyhow::bail!(
        "`obelion recall` is reserved for v0.2.0. Today, call the MCP server via your IDE \
         (run `obelion init <ide>` first), or test directly with the `mcp_handshake` integration test."
    );
}

fn cmd_status() -> Result<()> {
    let qdrant = std::env::var("OBELION_QDRANT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:6334 (default)".to_string());
    let ollama = std::env::var("OBELION_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434 (default)".to_string());
    let collection =
        std::env::var("OBELION_COLLECTION").unwrap_or_else(|_| "obelion (default)".to_string());
    println!("obelion v{}", obelion_core::VERSION);
    println!("  qdrant     {qdrant}");
    println!("  ollama     {ollama}");
    println!("  collection {collection}");
    Ok(())
}
