//! Runtime feature flag resolver — Meta Gatekeeper analog for Amore.
//!
//! Source: engineering.fb.com/2017/08/31/web/rapid-release-at-massive-scale/ —
//! "rollback is a toggle, not a revert". Capability rollouts ship gated by flag;
//! flag flip rolls back without binary swap.
//!
//! Resolution order (highest priority first):
//! 1. Process env: `AMORE_FLAG_<NAME>=on|off` (e.g., `AMORE_FLAG_RERANKER_V2=on`)
//! 2. File: `$AMORE_FLAGS_FILE` JSON map `{"reranker_v2": "on", ...}`
//! 3. Compile-time default (always `off` for runtime flags; Cargo features handle compile-time)

use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::OnceLock;

/// Runtime feature flag state loaded once at startup.
pub struct Flags {
    runtime: HashMap<String, bool>,
}

static FLAGS: OnceLock<Flags> = OnceLock::new();

impl Flags {
    fn load() -> Self {
        let mut runtime: HashMap<String, bool> = HashMap::new();

        // Layer 2: file (lower priority — env overrides below)
        if let Ok(path) = env::var("AMORE_FLAGS_FILE")
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content)
        {
            for (k, v) in map {
                runtime.insert(k.to_lowercase(), v == "on" || v == "true");
            }
        }

        // Layer 1: env vars (highest priority — overrides file)
        for (k, v) in env::vars() {
            if let Some(name) = k.strip_prefix("AMORE_FLAG_") {
                runtime.insert(name.to_lowercase(), v == "on" || v == "true");
            }
        }

        Flags { runtime }
    }

    /// Returns `true` when the named flag is explicitly set to `on` or `true`.
    /// Unknown flags default to `false` (fail-closed).
    pub fn is_enabled(name: &str) -> bool {
        let flags = FLAGS.get_or_init(Self::load);
        flags
            .runtime
            .get(&name.to_lowercase())
            .copied()
            .unwrap_or(false)
    }

    /// Returns all runtime flags, sorted by name.
    pub fn list() -> Vec<(String, bool)> {
        let flags = FLAGS.get_or_init(Self::load);
        let mut v: Vec<(String, bool)> =
            flags.runtime.iter().map(|(k, v)| (k.clone(), *v)).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }
}

/// Compile-time feature state as (name, enabled) pairs.
/// Consumers (e.g. amore-cli) call this instead of `cfg!(feature = "...")`,
/// which only works within the crate that declares the feature.
pub fn compile_time_features() -> [(&'static str, bool); 5] {
    [
        ("rerank-onnx", cfg!(feature = "rerank-onnx")),
        ("tantivy-bm25", cfg!(feature = "tantivy-bm25")),
        ("compaction-worker", cfg!(feature = "compaction-worker")),
        ("wal-sync", cfg!(feature = "wal-sync")),
        ("metrics-exporter", cfg!(feature = "metrics-exporter")),
    ]
}
