// H.4 — Qdrant gRPC connection pool.
//
// Wraps `qdrant_client::Qdrant` with `bb8::Pool<QdrantConnectionManager>`.
// bb8::ManageConnection is declared with #[async_trait], so the impl must
// also carry #[async_trait] to satisfy the lifetime parameterization.
//
// Default max_size = min(available_parallelism * 2, 16) using stdlib only
// (no num_cpus dep). `is_valid()` performs a `health_check()` round-trip so
// the pool surfaces stale connections before they are handed to callers.
// `has_broken()` returns false — validity is the sole eviction signal.

use anyhow::Result;
use async_trait::async_trait;
use bb8::Pool;
use qdrant_client::Qdrant;
use std::sync::Arc;

/// bb8 connection manager for Qdrant gRPC connections.
pub struct QdrantConnectionManager {
    url: String,
}

impl QdrantConnectionManager {
    pub fn new(url: &str) -> Self {
        Self { url: url.to_owned() }
    }
}

#[async_trait]
impl bb8::ManageConnection for QdrantConnectionManager {
    type Connection = Qdrant;
    type Error = anyhow::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        Qdrant::from_url(&self.url)
            .build()
            .map_err(|e| anyhow::anyhow!("qdrant_pool: connect to {} failed: {:?}", self.url, e))
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.health_check()
            .await
            .map_err(|e| anyhow::anyhow!("qdrant_pool: health_check failed: {:?}", e))?;
        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        // Let is_valid() decide; don't eagerly evict on drop.
        false
    }
}

/// Build a `bb8::Pool` for the given Qdrant URL.
///
/// `max_size` caps the number of simultaneously open gRPC connections.
/// Call [`default_pool_size`] if you want the auto-tuned default.
pub async fn build_pool(
    url: &str,
    max_size: u32,
) -> Result<Arc<Pool<QdrantConnectionManager>>> {
    let pool = Pool::builder()
        .max_size(max_size)
        .build(QdrantConnectionManager::new(url))
        .await
        .map_err(|e| anyhow::anyhow!("qdrant_pool: pool build failed: {:?}", e))?;
    Ok(Arc::new(pool))
}

/// Returns `min(available_parallelism * 2, 16)` using only stdlib.
///
/// Rationale: two connections per CPU thread keeps the gRPC layer busy
/// during concurrent requests without thrashing the kernel scheduler.
/// The 16-cap prevents runaway connection counts on large machines.
pub fn default_pool_size() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() * 2)
        .unwrap_or(8)
        .min(16) as u32
}
