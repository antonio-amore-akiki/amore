// obelion-mcp — MCP server entry point.
// Exposes tools: recall, canonical_doc_lookup, ensemble_decide, eig_question,
// observe, world_model_query, provenance_verify. Transport: stdio.

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("obelion-mcp v{} starting", obelion_core::VERSION);
    tracing::warn!("MCP server stub - not yet implemented");
    Ok(())
}
