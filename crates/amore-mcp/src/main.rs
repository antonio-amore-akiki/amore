// amore-mcp — MCP server exposing amore-core's hybrid recall over stdio.
//
// v0.1.0: single tool `recall(query, top_k)` -> JSON array of {id, score, text,
// source, payload}. Embeds query via Ollama (nomic-embed-text), searches
// Qdrant by cosine, returns ranked hits. S8 adds BM25 fusion (FTS5) +
// canonical-doc lookup; subsequent steps add ensemble_decide / eig_question /
// observe / world_model_query / provenance_verify.
//
// Transport: stdio (the universal contract every MCP client supports).
// Daemons required at startup: Qdrant on AMORE_QDRANT_URL (gRPC, default
// http://127.0.0.1:6334) + Ollama on AMORE_OLLAMA_URL (default
// http://127.0.0.1:11434). Collection name via AMORE_COLLECTION
// (default "amore").
//
// Adopt: official Anthropic rust-sdk (rmcp 1.7.0) for protocol; HybridRecall
// from amore-core for retrieval. No hand-rolled JSON-RPC; the SDK is the
// primary path.

// ADR 0010: no-unwrap policy. expect() with documented invariant is the approved
// fix pattern; only bare unwrap() is banned. Test modules exempted via cfg_attr.
#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used))]

mod health;
mod middleware;
mod observability;
mod register;
mod shutdown;

use amore_core::docs::CanonicalDocsRouter;
use amore_core::ollama::OllamaClient;
use amore_core::qdrant_store::QdrantStore;
use amore_core::recall::HybridRecall;
use amore_core::sqlite_store::SqliteStore;
use anyhow::{Context, Result};
use clap::Parser;
use health::ReadyState;
use middleware::{SessionId, build_rate_limiter, check_rate_limit};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::service::ServerInitializeError;
use rmcp::transport::stdio;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router,
};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// ---------------------------------------------------------------------------
// CLI — registration flags (A5 + A6).
//
// These flags exit BEFORE heavy async startup (OTel/Prometheus/Qdrant/Ollama).
// Intended for installer scripts and the CI cert script.
// ---------------------------------------------------------------------------

/// amore-mcp — MCP memory server + IDE registration helper.
#[derive(Parser, Debug, Default)]
#[command(name = "amore-mcp", about = "Amore MCP memory server")]
struct Cli {
    /// Register amore-mcp as a Claude Code MCP server.
    #[arg(long)]
    register_claude_code: bool,
    /// Register amore-mcp as a Claude Desktop MCP server (always direct-write).
    #[arg(long)]
    register_claude_desktop: bool,
    /// Register amore-mcp as a Cursor MCP server (always direct-write).
    #[arg(long)]
    register_cursor: bool,
    /// Register amore-mcp as a Cline MCP server (always direct-write).
    #[arg(long)]
    register_cline: bool,
    /// Register amore-mcp for Continue (always direct-write via config.json).
    #[arg(long)]
    register_continue: bool,
    /// Skip the `claude` CLI; write directly to IDE config file.
    /// Installer default — does not require `claude` CLI on PATH (closes F12).
    #[arg(long)]
    self_contained: bool,
}

/// Run any --register-* flags. Returns true if at least one flag was handled.
fn run_registration_flags(cli: &Cli) -> bool {
    let sc = cli.self_contained;
    let mut any = false;

    macro_rules! run_reg {
        ($flag:expr, $fn:expr, $name:literal) => {
            if $flag {
                any = true;
                match $fn(sc) {
                    Ok(report) => println!(
                        "[amore-mcp] {} registered via {} → {}",
                        report.target, report.method, report.config_path.display()
                    ),
                    Err(e) => {
                        eprintln!("[amore-mcp] {} registration failed: {e:#}", $name);
                        std::process::exit(1);
                    }
                }
            }
        };
    }

    run_reg!(cli.register_claude_code,    register::register_claude_code,    "Claude Code");
    run_reg!(cli.register_claude_desktop, register::register_claude_desktop, "Claude Desktop");
    run_reg!(cli.register_cursor,         register::register_cursor,         "Cursor");
    run_reg!(cli.register_cline,          register::register_cline,          "Cline");
    run_reg!(cli.register_continue,       register::register_continue,       "Continue");

    any
}

// ---------------------------------------------------------------------------
// MainError — plain-English error type for user-facing process exit messages.
// No Rust-internal strings (anyhow chains, rmcp variant names) may leak here.
// ---------------------------------------------------------------------------

/// Actionable error classes for the amore-mcp process. Each variant maps to a
/// single plain-English message — no Rust-internal detail on stderr.
#[derive(Debug)]
pub enum MainError {
    /// rmcp closed the connection before receiving an `initialize` JSON-RPC
    /// request. Happens when the IDE adapter starts amore-mcp before it is
    /// ready to write on the pipe (DG-D / DG-E).
    IdeDisconnected,
    /// Ollama or Qdrant was not reachable during the first startup probe.
    DepUnreachable(String),
    /// env / CLI argument validation failed.
    ConfigInvalid(String),
    /// Any other start-up error not covered by a specific variant.
    Other(String),
}

impl fmt::Display for MainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MainError::IdeDisconnected => write!(
                f,
                "Waiting for your IDE — start the editor and connect via MCP."
            ),
            MainError::DepUnreachable(_) => write!(
                f,
                "Couldn't reach a required service (Ollama or Qdrant). \
                 Run `amore doctor` to diagnose."
            ),
            MainError::ConfigInvalid(_) => write!(
                f,
                "Amore couldn't read its configuration. \
                 Check AMORE_DATA_DIR + AMORE_BRAIN env vars."
            ),
            MainError::Other(_) => write!(
                f,
                "Amore couldn't start. Run `amore doctor` to see details."
            ),
        }
    }
}

impl std::error::Error for MainError {}

// ---------------------------------------------------------------------------
// Input-validation limits for the `recall` tool (Security fix 6a).
//
// ### Limits
// - `MAX_TOP_K`: ceiling on the number of hits returned. Values above this are
//   silently clamped. In release profile `usize::MAX * 4` wraps to 0 and
//   propagates junk fetch-counts to Qdrant; the clamp is the only safe guard.
// - `MAX_QUERY_BYTES`: ceiling on the raw UTF-8 byte length of the query.
//   Multi-MB queries trigger Ollama embedding with unbounded memory pressure
//   and latency; requests above this threshold are rejected with a clean
//   JSON-RPC error before any network call is made.
// ---------------------------------------------------------------------------
const MAX_TOP_K: usize = 100;
const MAX_QUERY_BYTES: usize = 16 * 1024; // 16 KiB

/// Parameters for the `recall` tool — embedded query + top-k cosine search.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecallParams {
    /// Natural-language query text. Embedded via Ollama nomic-embed-text (768-dim).
    /// Maximum: 16 KiB (16 384 bytes). Requests exceeding this limit are rejected.
    pub query: String,
    /// Number of top hits to return. Defaults to 5 if omitted. Clamped to [1, 100].
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    5
}

/// Parameters for the `canonical_doc_lookup` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CanonicalLookupParams {
    /// Natural-language query. Topic-matched against `*.md` files with
    /// `stable: true` headers in the configured search paths.
    pub query: String,
}

/// The Amore MCP server state. Holds `Arc<HybridRecall>` + canonical-docs
/// router + resolved search paths so tool methods can clone cheaply across
/// concurrent JSON-RPC calls.
#[derive(Clone)]
pub struct AmoreServer {
    recall: Arc<HybridRecall>,
    docs_router: Arc<CanonicalDocsRouter>,
    docs_paths: Arc<Vec<PathBuf>>,
    /// Per-session rate limiter (W3-3B). Arc so the field is Clone.
    rate_limiter: Arc<governor::DefaultKeyedRateLimiter<SessionId>>,
    // Macro-populated field. The `#[tool_router]` macro reads this via
    // `Self::tool_router()` at attach time, but the read is invisible to
    // dead-code analysis (proc-macro expansion is opaque to the lint).
    #[allow(dead_code)]
    tool_router: ToolRouter<AmoreServer>,
}

#[tool_router]
impl AmoreServer {
    pub fn new(
        recall: HybridRecall,
        docs_paths: Vec<PathBuf>,
        rate_limiter: Arc<governor::DefaultKeyedRateLimiter<SessionId>>,
    ) -> Self {
        Self {
            recall: Arc::new(recall),
            docs_router: Arc::new(CanonicalDocsRouter::new()),
            docs_paths: Arc::new(docs_paths),
            rate_limiter,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Hybrid recall over the Amore observation store. Embeds the query via Ollama (nomic-embed-text, 768-dim) and searches Qdrant by cosine; if a SQLite BM25 lane is attached, fuses both via Reciprocal Rank Fusion (k=60). Returns a JSON object {hits: [{id, score, text, source, payload}], degraded: {ollama_unavailable, qdrant_unavailable, bm25_unavailable}} — caller MUST inspect `degraded` to detect a lane outage (no silent fail-open). When BOTH retrieval lanes are unavailable the call errors out with the actionable cause."
    )]
    async fn recall(
        &self,
        Parameters(params): Parameters<RecallParams>,
    ) -> Result<CallToolResult, McpError> {
        // Rate-limit check (W3-3B): keyed by a default session. Full per-session
        // keying requires rmcp session metadata; "default" makes the path live.
        let session = SessionId("default".to_string());
        check_rate_limit(&self.rate_limiter, &session)?;

        // Security fix 6a: reject oversized queries before hitting Ollama.
        if params.query.len() > MAX_QUERY_BYTES {
            return Err(McpError::invalid_params(
                format!(
                    "query exceeds {MAX_QUERY_BYTES} bytes (got {})",
                    params.query.len()
                ),
                None,
            ));
        }
        // Clamp top_k: release-profile overflow in `top_k * 4` wraps to 0 and
        // sends a junk fetch-count to Qdrant; clamp is the only safe guard.
        let top_k = params.top_k.clamp(1, MAX_TOP_K);
        let envelope = self
            .recall
            .search(&params.query, top_k)
            .await
            .map_err(|e| McpError::internal_error(format!("recall failed: {e}"), None))?;
        let body = serde_json::to_string(&envelope).map_err(|e| {
            McpError::internal_error(format!("serialize recall envelope failed: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(body)]))
    }

    #[tool(
        description = "Canonical-docs lookup: topic-matches the query against *.md files with `stable: true` headers in the configured search paths (default: ~/.claude/docs, <cwd>/.claude/docs, <cwd>/docs). Deterministic source-of-truth surface for known domains — beats probabilistic recall when an authoritative doc exists. Returns a JSON array of {path, title, topic_score, excerpt}, capped at TOP_K_HITS = 3 results."
    )]
    async fn canonical_doc_lookup(
        &self,
        Parameters(params): Parameters<CanonicalLookupParams>,
    ) -> Result<CallToolResult, McpError> {
        let paths: Vec<&std::path::Path> = self.docs_paths.iter().map(|p| p.as_path()).collect();
        let hits = self
            .docs_router
            .route(&params.query, &paths)
            .map_err(|e| McpError::internal_error(format!("canonical lookup failed: {e}"), None))?;
        let body = serde_json::to_string(&hits)
            .map_err(|e| McpError::internal_error(format!("serialize hits failed: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(body)]))
    }
}

#[tool_handler]
impl ServerHandler for AmoreServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "amore-mcp: production-grade hybrid retrieval. recall(query, top_k=5) returns vector + (optional) BM25 RRF-fused hits over the observation store. canonical_doc_lookup(query) topic-matches the query against `stable: true` *.md docs in the configured search paths for deterministic source-of-truth context."
                    .to_string(),
            )
    }
}

/// Read an env var, with a legacy OBELION_* alias that emits a deprecation
/// warning when used. Returns `Some(value)` if either name is set; prefers
/// the AMORE_* key over the legacy key.
fn env_with_legacy(amore_key: &str, legacy_key: &str) -> Option<String> {
    if let Ok(v) = std::env::var(amore_key) {
        return Some(v);
    }
    if let Ok(v) = std::env::var(legacy_key) {
        tracing::warn!(
            "deprecated: {} — use {} instead (OBELION_* env vars are removed in v0.4.0)",
            legacy_key,
            amore_key
        );
        return Some(v);
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    // Step 1: Parse CLI — registration flags exit before heavy startup (A5+A6).
    let cli = Cli::parse();
    if run_registration_flags(&cli) {
        return Ok(());
    }

    // Step 2: Init OTel tracer (if endpoint set) — must happen before logging
    // because the tracer provider is passed into the logging subscriber.
    let otel_provider = observability::tracing::init_otel_tracer()
        .map_err(|e| MainError::ConfigInvalid(format!("OTel init failed: {e:#}")))?;

    // Step 3: Init logging layer FIRST (with OTel layer if active).
    observability::logging::install_logging_subscriber(otel_provider.as_ref());

    // Step 4: Init Prometheus exporter.
    observability::metrics::install_prometheus_exporter()
        .map_err(|e| MainError::ConfigInvalid(format!("Prometheus init failed: {e:#}")))?;
    observability::metrics::describe_metrics();

    let qdrant_url = env_with_legacy("AMORE_QDRANT_URL", "OBELION_QDRANT_URL")
        .unwrap_or_else(|| "http://127.0.0.1:6334".to_string());
    let ollama_url = env_with_legacy("AMORE_OLLAMA_URL", "OBELION_OLLAMA_URL")
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    let collection = env_with_legacy("AMORE_COLLECTION", "OBELION_COLLECTION")
        .unwrap_or_else(|| "amore".to_string());

    tracing::info!(
        version = amore_core::VERSION,
        qdrant = %qdrant_url,
        ollama = %ollama_url,
        collection = %collection,
        "amore-mcp starting"
    );

    // Step 5: Spawn axum healthz/readyz sidecar.
    let ready = Arc::new(ReadyState::new());
    let health_handle = health::spawn_health_server(Arc::clone(&ready))
        .await
        .map_err(|e| MainError::ConfigInvalid(format!("health sidecar failed: {e:#}")))?;

    // Step 6: Build rate limiter.
    let rate_limiter = build_rate_limiter()
        .map_err(|e| MainError::ConfigInvalid(format!("rate limiter config error: {e:#}")))?;

    // Step 7: Build Qdrant store (pool env-tuning lives in amore-core/qdrant_pool.rs).
    let ollama = OllamaClient::new(&ollama_url);
    let qdrant = QdrantStore::open(&qdrant_url, &collection)
        .await
        .with_context(|| format!("opening Qdrant at {qdrant_url} collection={collection}"))
        .map_err(|e| MainError::DepUnreachable(format!("{e:#}")))?;

    // BM25 lane via SQLite — required for graceful degradation when Qdrant or
    // Ollama is down (B1/B2 QA gates). Path resolves from AMORE_DATA_DIR or
    // defaults to <data_dir>/Amore/amore.db. Schema is created idempotently.
    let sqlite_path = resolve_sqlite_path()
        .map_err(|e| MainError::ConfigInvalid(format!("{e:#}")))?;
    if let Some(parent) = sqlite_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    // Migration: if old obelion dir exists and new Amore dir doesn't, copy the DB.
    maybe_migrate_data_dir(&sqlite_path);
    let sqlite = Arc::new(
        SqliteStore::open(&sqlite_path)
            .with_context(|| format!("opening SQLite store at {}", sqlite_path.display()))
            .map_err(|e| MainError::ConfigInvalid(format!("{e:#}")))?,
    );
    tracing::info!(
        path = %sqlite_path.display(),
        observations = sqlite.count_observations().unwrap_or(0),
        "SQLite BM25 lane attached"
    );

    // Step 8: WAL replay complete — flip wal_replayed.
    ready.wal_replayed.store(true, Ordering::Release);

    // Step 9: First warmup probe would go here. We flip warmed_up immediately
    // since the warmup is optional and we don't block startup on it.
    ready.warmed_up.store(true, Ordering::Release);
    tracing::info!("readyz: all gates passed — serving traffic");

    // Step 10: Build MCP server with rate-limit middleware wrapping handler.
    let recall = HybridRecall::new(ollama, qdrant).with_sqlite(sqlite);
    let docs_paths = resolve_docs_paths();
    let server = AmoreServer::new(recall, docs_paths, rate_limiter);

    // Handle ConnectionClosed: rmcp closes before receiving `initialize` when
    // the IDE adapter hasn't written to stdin yet (empty-stdin race, DG-D/DG-E).
    // Exit 0 with a plain-English INFO message — not an error from the OS view.
    let service = match server.serve(stdio()).await {
        Ok(svc) => svc,
        Err(ServerInitializeError::ConnectionClosed(_)) => {
            tracing::info!(
                "Waiting for your IDE — start the editor and connect via MCP."
            );
            health_handle.abort();
            return Ok(());
        }
        Err(e) => {
            let detail = format!("{e}");
            tracing::error!("MCP serve error: {detail}");
            health_handle.abort();
            return Err(MainError::Other(detail));
        }
    };

    // Step 11: Install shutdown handler.
    let (shutdown_tx, shutdown_rx) = shutdown::shutdown_channel();
    let service_handle = tokio::spawn(async move {
        let _ = service.waiting().await;
        // Signal shutdown when the MCP service exits.
        let _ = shutdown_tx.send(());
    });

    shutdown::wait_for_shutdown(Some(shutdown_rx)).await;

    // Drain phase: abort health sidecar, wait for MCP service.
    health_handle.abort();
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        service_handle,
    )
    .await;

    // Flush OTel if active. TracerProvider::shutdown() is a method on the
    // concrete opentelemetry_sdk::trace::TracerProvider type directly.
    if let Some(provider) = otel_provider {
        let _ = provider.shutdown();
    }

    tracing::info!("amore-mcp shutdown complete");
    Ok(())
}

/// Resolve the SQLite BM25 store path. `AMORE_DATA_DIR` env var (single
/// directory) overrides (legacy `OBELION_DATA_DIR` also accepted with a
/// deprecation warning); default = `<config_dir>/Amore/amore.db` per
/// XDG / Windows AppData conventions. Returns Err only if the system has no
/// home directory at all (extreme corner case).
fn resolve_sqlite_path() -> anyhow::Result<PathBuf> {
    if let Some(dir) = env_with_legacy("AMORE_DATA_DIR", "OBELION_DATA_DIR") {
        return Ok(PathBuf::from(dir).join("amore.db"));
    }
    let base = dirs::config_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .with_context(|| "no config/data/home dir resolvable for SQLite store path")?;
    Ok(base.join("Amore").join("amore.db"))
}

/// One-time data directory migration: if `<base>/obelion/` exists (old layout)
/// and `<base>/Amore/` does not, copy `obelion.db` -> `Amore/amore.db` and
/// write a `migrated-from-obelion.txt` marker. Best-effort: failures are logged
/// as warnings rather than fatal errors so a fresh install always continues.
fn maybe_migrate_data_dir(amore_db_path: &PathBuf) {
    let amore_dir = match amore_db_path.parent() {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    if amore_dir.exists() {
        return; // Already migrated or fresh install — nothing to do.
    }
    // Locate legacy dir by replacing the Amore segment with obelion.
    let legacy_dir = amore_dir.parent().map(|p| p.join("obelion"));
    let legacy_db = legacy_dir.as_ref().map(|d| d.join("obelion.db"));
    let (Some(legacy_dir), Some(legacy_db)) = (legacy_dir, legacy_db) else {
        return;
    };
    if !legacy_dir.exists() || !legacy_db.exists() {
        return; // No old installation found.
    }
    if let Err(e) = std::fs::create_dir_all(&amore_dir) {
        tracing::warn!("migration: could not create Amore dir: {e}");
        return;
    }
    match std::fs::copy(&legacy_db, amore_db_path) {
        Err(e) => {
            tracing::warn!("migration: could not copy obelion.db -> amore.db: {e}");
        }
        Ok(_) => {
            let marker = amore_dir.join("migrated-from-obelion.txt");
            let _ = std::fs::write(
                &marker,
                format!(
                    "Migrated from {} on startup.\n\
                     Source: {}\nDest: {}\n",
                    legacy_dir.display(),
                    legacy_db.display(),
                    amore_db_path.display()
                ),
            );
            tracing::info!(
                "migration: copied obelion.db -> {} and wrote marker",
                amore_db_path.display()
            );
        }
    }
}

/// Resolve canonical-docs search paths. AMORE_DOCS_PATHS env var (colon- or
/// semicolon-separated) overrides (legacy OBELION_DOCS_PATHS also accepted);
/// default is [~/.claude/docs, <cwd>/.claude/docs, <cwd>/docs]. Non-existent
/// paths are kept (the router skips them at route time).
///
/// M3: env-derived paths are validated against `home_dir()` via
/// `amore_core::docs::validate_docs_path`. Paths that fail validation are
/// logged and dropped unless `AMORE_DOCS_PATHS_ALLOW_ANY=1` is set.
fn resolve_docs_paths() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    if let Some(val) = env_with_legacy("AMORE_DOCS_PATHS", "OBELION_DOCS_PATHS") {
        let sep = if val.contains(';') { ';' } else { ':' };
        return val
            .split(sep)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .filter(|p| {
                match amore_core::docs::validate_docs_path(p, &home) {
                    Ok(()) => true,
                    Err(reason) => {
                        tracing::warn!("docs path rejected: {reason}");
                        false
                    }
                }
            })
            .collect();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    vec![
        home.join(".claude").join("docs"),
        cwd.join(".claude").join("docs"),
        cwd.join("docs"),
    ]
}
