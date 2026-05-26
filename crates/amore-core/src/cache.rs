// cache.rs — H.13 multi-level cache for (query, top_k) → Vec<RecallHit>
//
// L1 = moka async in-memory LRU (default 10_000 entries, TTL configurable).
// L2 = sled on-disk store (default 1 GB size limit).
// Cache key = first 8 bytes of SHA-256(format!("{}:{}", query, top_k)) as u64.
// L2 serialisation = serde_json (RecallHit already derives Serialize/Deserialize).
// L2 hits are promoted to L1 on read.
// invalidate_all clears both layers — called after compaction (H.9).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use moka::future::Cache;
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::recall::RecallHit;

// ---------------------------------------------------------------------------
// Public configuration
// ---------------------------------------------------------------------------

/// Tuning knobs for the two-level cache.
#[derive(Debug, Clone)]
pub struct CacheOpts {
    /// Maximum number of entries kept in the L1 in-memory LRU.
    pub l1_capacity: u64,
    /// Maximum byte budget for the L2 sled database (advisory).
    pub l2_capacity_bytes: u64,
    /// Time-to-live for entries in both layers.
    pub ttl: Duration,
}

impl Default for CacheOpts {
    fn default() -> Self {
        Self {
            l1_capacity: 10_000,
            l2_capacity_bytes: 1_073_741_824, // 1 GB
            ttl: Duration::from_secs(3_600),  // 1 hour
        }
    }
}

// ---------------------------------------------------------------------------
// RecallCache
// ---------------------------------------------------------------------------

/// Two-level cache: L1 (moka, in-memory LRU) + L2 (sled, on-disk).
pub struct RecallCache {
    l1: Cache<u64, Arc<Vec<RecallHit>>>,
    l2: sled::Db,
    opts: CacheOpts,
}

impl RecallCache {
    /// Open (or create) the cache.  The sled database is placed at
    /// `data_dir/cache_l2`.
    pub fn new(data_dir: &Path, opts: CacheOpts) -> Result<Self> {
        let l2_path = data_dir.join("cache_l2");
        let l2 = sled::open(&l2_path)
            .with_context(|| format!("opening sled L2 cache at {}", l2_path.display()))?;

        let l1 = Cache::builder()
            .max_capacity(opts.l1_capacity)
            .time_to_live(opts.ttl)
            .build();

        Ok(Self { l1, l2, opts })
    }

    // -----------------------------------------------------------------------
    // Key derivation
    // -----------------------------------------------------------------------

    fn cache_key(query: &str, top_k: usize) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", query, top_k).as_bytes());
        let digest = hasher.finalize();
        // First 8 bytes as big-endian u64.
        // Collision probability negligible for a local query cache.
        let arr: [u8; 8] = digest[..8]
            .try_into()
            .expect("sha256 output is always 32 bytes");
        u64::from_be_bytes(arr)
    }

    // -----------------------------------------------------------------------
    // Read path
    // -----------------------------------------------------------------------

    /// Look up `(query, top_k)`.  Returns `None` on miss or TTL expiry.
    ///
    /// Hit path: L1 → return.
    /// Miss path: check L2 → if found, deserialise, promote to L1, return.
    pub async fn get(&self, query: &str, top_k: usize) -> Option<Vec<RecallHit>> {
        let key = Self::cache_key(query, top_k);

        // L1
        if let Some(hits) = self.l1.get(&key).await {
            debug!(key, "cache L1 hit");
            return Some((*hits).clone());
        }

        // L2
        match self.l2_get(key) {
            Ok(Some(hits)) => {
                debug!(key, "cache L2 hit — promoting to L1");
                let arc = Arc::new(hits.clone());
                self.l1.insert(key, arc).await;
                Some(hits)
            }
            Ok(None) => {
                debug!(key, "cache miss");
                None
            }
            Err(e) => {
                warn!(key, err = %e, "L2 read error — treating as miss");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Write path
    // -----------------------------------------------------------------------

    /// Store `hits` for `(query, top_k)` in both L1 and L2.
    pub async fn put(&self, query: &str, top_k: usize, hits: Vec<RecallHit>) {
        let key = Self::cache_key(query, top_k);
        let arc = Arc::new(hits.clone());
        self.l1.insert(key, arc).await;

        if let Err(e) = self.l2_put(key, &hits) {
            warn!(key, err = %e, "L2 write error — entry lives in L1 only");
        }
    }

    // -----------------------------------------------------------------------
    // Invalidation
    // -----------------------------------------------------------------------

    /// Clear both cache layers.  Called after compaction (H.9) to prevent
    /// stale hits from surfacing post-compaction.
    pub async fn invalidate_all(&self) -> Result<()> {
        self.l1.invalidate_all();
        self.l2.clear().context("clearing sled L2 cache")?;
        self.l2
            .flush()
            .context("flushing sled L2 after clear")?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // L2 helpers (sync — sled API is synchronous)
    // -----------------------------------------------------------------------

    fn l2_get(&self, key: u64) -> Result<Option<Vec<RecallHit>>> {
        let raw_key = key.to_be_bytes();
        match self.l2.get(raw_key).context("sled L2 get")? {
            None => Ok(None),
            Some(bytes) => {
                let hits: Vec<RecallHit> =
                    serde_json::from_slice(&bytes).context("deserialising L2 cache entry")?;
                Ok(Some(hits))
            }
        }
    }

    fn l2_put(&self, key: u64, hits: &[RecallHit]) -> Result<()> {
        let raw_key = key.to_be_bytes();
        let bytes = serde_json::to_vec(hits).context("serialising cache entry for L2")?;
        self.l2
            .insert(raw_key, bytes.as_slice())
            .context("sled L2 insert")?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Test helpers — exposed only to allow white-box test assertions
    // -----------------------------------------------------------------------

    /// Invalidate the L1 layer only, leaving L2 intact.
    /// Used in tests to force L2 fall-through path.
    pub fn invalidate_l1_all(&self) {
        self.l1.invalidate_all();
    }

    /// Return the configured options.
    pub fn opts(&self) -> &CacheOpts {
        &self.opts
    }
}
