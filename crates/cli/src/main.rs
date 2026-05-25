// obelion — CLI entry point.
// Commands: serve, recall, init <ide>, status, setup.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    println!("obelion v{} (stub)", obelion_core::VERSION);
    Ok(())
}
