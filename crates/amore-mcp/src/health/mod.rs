// health/mod.rs — HTTP healthz/readyz sidecar (W2-2D).
pub mod http;
pub use http::{ReadyState, spawn_health_server};
