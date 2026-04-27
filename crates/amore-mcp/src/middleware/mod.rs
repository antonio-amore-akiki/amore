// middleware/mod.rs — MCP server middleware (W3-3B).
pub mod rate_limit;
pub use rate_limit::{SessionId, build_rate_limiter, check_rate_limit};
