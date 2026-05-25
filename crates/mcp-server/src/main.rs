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
use obelion_core::ollama::OllamaClient;
use obelion_core::qdrant_store::QdrantStore;
use obelion_core::recall::HybridRecall;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::transport::stdio;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router,
};
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

/// The obelion MCP server state. Holds an `Arc<HybridRecall>` so tool methods
/// can clone cheaply across concurrent JSON-RPC calls.
#[derive(Clone)]
pub struct ObelionServer {
    recall: Arc<HybridRecall>,
    // Macro-populated field. The `#[tool_router]` macro reads this via
    // `Self::tool_router()` at attach time, but the read is invisible to
    // dead-code analysis (proc-macro expansion is opaque to the lint).
    #[allow(dead_code)]
    tool_router: ToolRouter<ObelionServer>,
}

#[tool_router]
impl ObelionServer {
    pub fn new(recall: HybridRecall) -> Self {
        Self {
            recall: Arc::new(recall),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Hybrid recall over the obelion observation store. Embeds the query via Ollama (nomic-embed-text, 768-dim) and searches Qdrant by cosine similarity. Returns a JSON array of hits (id, score, text, source, payload) ranked by relevance."
    )]
    async fn recall(
        &self,
        Parameters(params): Parameters<RecallParams>,
    ) -> Result<CallToolResult, McpError> {
        let hits = self
            .recall
            .search(&params.query, params.top_k)
            .await
            .map_err(|e| McpError::internal_error(format!("recall failed: {e}"), None))?;
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
                "obelion-mcp: production-grade hybrid retrieval over Ollama embeddings + Qdrant cosine search. v0.1.0 path is pure vector; S8 adds BM25 + RRF fusion. Tool: recall(query, top_k=5)."
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
    let recall = HybridRecall::new(ollama, qdrant);
    let server = ObelionServer::new(recall);

    let service = server
        .serve(stdio())
        .await
        .inspect_err(|e| tracing::error!("MCP serve error: {e:?}"))?;
    service.waiting().await?;
    Ok(())
}
