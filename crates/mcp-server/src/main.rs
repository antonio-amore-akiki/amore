// obelion-mcp — MCP server exposing obelion-core's hybrid recall over stdio.
//
// v0.1.0: single tool `recall(query, top_k)` -> JSON array of {id, score, text,
// source, payload}. Embeds query via Ollama (nomic-embed-text), searches
// Qdrant by cosine, returns ranked hits. S8 adds BM25 fusion (FTS5) +
// canonical-doc lookup; subsequent steps add ensemble_decide / eig_question /
// observe / world_model_query / provenance_verify.
//
// Transport: stdio (the universal contract every MCP client supports).
// Daemons required at startup: Qdrant on OBELION_QDRANT_URL (gRPC, default
// http://127.0.0.1:6334) + Ollama on OBELION_OLLAMA_URL (default
// http://127.0.0.1:11434). Collection name via OBELION_COLLECTION
// (default "obelion").
//
// Adopt: official Anthropic rust-sdk (rmcp 1.7.0) for protocol; HybridRecall
// from obelion-core for retrieval. No hand-rolled JSON-RPC; the SDK is the
// production-grade path.

use anyhow::{Context, Result};
use obelion_core::docs::CanonicalDocsRouter;
use obelion_core::ollama::OllamaClient;
use obelion_core::qdrant_store::QdrantStore;
use obelion_core::recall::HybridRecall;
use obelion_core::sqlite_store::SqliteStore;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::transport::stdio;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router,
};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

/// Parameters for the `recall` tool — embedded query + top-k cosine search.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RecallParams {
    /// Natural-language query text. Embedded via Ollama nomic-embed-text (768-dim).
    pub query: String,
    /// Number of top hits to return. Defaults to 5 if omitted.
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

/// The obelion MCP server state. Holds `Arc<HybridRecall>` + canonical-docs
/// router + resolved search paths so tool methods can clone cheaply across
/// concurrent JSON-RPC calls.
#[derive(Clone)]
pub struct ObelionServer {
    recall: Arc<HybridRecall>,
    docs_router: Arc<CanonicalDocsRouter>,
    docs_paths: Arc<Vec<PathBuf>>,
    // Macro-populated field. The `#[tool_router]` macro reads this via
    // `Self::tool_router()` at attach time, but the read is invisible to
    // dead-code analysis (proc-macro expansion is opaque to the lint).
    #[allow(dead_code)]
    tool_router: ToolRouter<ObelionServer>,
}

#[tool_router]
impl ObelionServer {
    pub fn new(recall: HybridRecall, docs_paths: Vec<PathBuf>) -> Self {
        Self {
            recall: Arc::new(recall),
            docs_router: Arc::new(CanonicalDocsRouter::new()),
            docs_paths: Arc::new(docs_paths),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Hybrid recall over the obelion observation store. Embeds the query via Ollama (nomic-embed-text, 768-dim) and searches Qdrant by cosine; if a SQLite BM25 lane is attached, fuses both via Reciprocal Rank Fusion (k=60). Returns a JSON object {hits: [{id, score, text, source, payload}], degraded: {ollama_unavailable, qdrant_unavailable, bm25_unavailable}} — caller MUST inspect `degraded` to detect a lane outage (no silent fail-open). When BOTH retrieval lanes are unavailable the call errors out with the actionable cause."
    )]
    async fn recall(
        &self,
        Parameters(params): Parameters<RecallParams>,
    ) -> Result<CallToolResult, McpError> {
        let envelope = self
            .recall
            .search(&params.query, params.top_k)
            .await
            .map_err(|e| McpError::internal_error(format!("recall failed: {e}"), None))?;
        let body = serde_json::to_string(&envelope).map_err(|e| {
            McpError::internal_error(format!("serialize recall envelope failed: {e}"), None)
        })?;
        Ok(CallToolResult::success(vec![Content::text(body)]))
    }

    #[tool(
        description = "Canonical-docs lookup: topic-matches the query against *.md files with `stable: true` headers in the configured search paths (default: ~/.claude/docs, <cwd>/.claude/docs, <cwd>/docs). Deterministic source-of-truth surface for known domains — beats probabilistic recall when an authoritative doc exists. Returns a JSON array of {path, title, topic_score, excerpt}."
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
impl ServerHandler for ObelionServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_instructions(
                "obelion-mcp: production-grade hybrid retrieval. recall(query, top_k=5) returns vector + (optional) BM25 RRF-fused hits over the observation store. canonical_doc_lookup(query) topic-matches the query against `stable: true` *.md docs in the configured search paths for deterministic source-of-truth context."
                    .to_string(),
            )
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let qdrant_url =
        std::env::var("OBELION_QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());
    let ollama_url = std::env::var("OBELION_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let collection = std::env::var("OBELION_COLLECTION").unwrap_or_else(|_| "obelion".to_string());

    tracing::info!(
        "obelion-mcp v{} starting; qdrant={qdrant_url} ollama={ollama_url} collection={collection}",
        obelion_core::VERSION
    );

    let ollama = OllamaClient::new(&ollama_url);
    let qdrant = QdrantStore::open(&qdrant_url, &collection)
        .await
        .with_context(|| format!("opening Qdrant at {qdrant_url} collection={collection}"))?;

    // BM25 lane via SQLite — required for graceful degradation when Qdrant or
    // Ollama is down (B1/B2 QA gates). Path resolves from OBELION_DATA_DIR or
    // defaults to <data_dir>/obelion.db. Schema is created idempotently.
    let sqlite_path = resolve_sqlite_path()?;
    if let Some(parent) = sqlite_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let sqlite = std::sync::Arc::new(
        SqliteStore::open(&sqlite_path)
            .with_context(|| format!("opening SQLite store at {}", sqlite_path.display()))?,
    );
    tracing::info!(
        "SQLite BM25 lane attached at {} ({} prior observations)",
        sqlite_path.display(),
        sqlite.count_observations().unwrap_or(0)
    );

    let recall = HybridRecall::new(ollama, qdrant).with_sqlite(sqlite);
    let docs_paths = resolve_docs_paths();
    let server = ObelionServer::new(recall, docs_paths);

    let service = server
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("MCP serve error: {e:?}"))?;
    service.waiting().await?;
    Ok(())
}

/// Resolve the SQLite BM25 store path. `OBELION_DATA_DIR` env var (single
/// directory) overrides; default = `<config_dir>/obelion/obelion.db` per
/// XDG / Windows AppData conventions. Returns Err only if the system has no
/// home directory at all (extreme corner case).
fn resolve_sqlite_path() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("OBELION_DATA_DIR") {
        return Ok(PathBuf::from(dir).join("obelion.db"));
    }
    let base = dirs::config_dir()
        .or_else(dirs::data_dir)
        .or_else(dirs::home_dir)
        .with_context(|| "no config/data/home dir resolvable for SQLite store path")?;
    Ok(base.join("obelion").join("obelion.db"))
}

/// Resolve canonical-docs search paths. OBELION_DOCS_PATHS env var (colon- or
/// semicolon-separated) overrides; default is [~/.claude/docs, <cwd>/.claude/
/// docs, <cwd>/docs]. Non-existent paths are kept (the router skips them at
/// route time) so paths can be created later without restarting the server.
fn resolve_docs_paths() -> Vec<PathBuf> {
    if let Ok(val) = std::env::var("OBELION_DOCS_PATHS") {
        let sep = if val.contains(';') { ';' } else { ':' };
        return val
            .split(sep)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut out = Vec::new();
    if let Some(home) = dirs::home_dir() {
        out.push(home.join(".claude").join("docs"));
    }
    out.push(cwd.join(".claude").join("docs"));
    out.push(cwd.join("docs"));
    out
}
