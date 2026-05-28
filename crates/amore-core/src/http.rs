// amore_core::http — workspace HTTP client factory (A7, F14).
//
// All crates that need a reqwest::Client (async or blocking) MUST obtain it via
// this module.
//
// Corporate proxy support: reqwest 0.12 reads HTTP_PROXY / HTTPS_PROXY /
// ALL_PROXY / NO_PROXY environment variables **by default** (system proxies are
// enabled unless the caller explicitly calls ClientBuilder::no_proxy()). This
// factory never calls no_proxy(), so env-var proxy discovery is always active.
// See: https://docs.rs/reqwest/0.12/reqwest/index.html#proxies
//
// Prior-art: Adopt — reqwest::Client::builder().build() is the official reqwest
// API; system proxy env-vars are auto-honoured (state/prior-art-verdict.json
// 2026-05-28). No net-new proxy logic needed.

use anyhow::{Context, Result};
use std::time::Duration;

/// Build an async [`reqwest::Client`] that honours `HTTP_PROXY`, `HTTPS_PROXY`,
/// `ALL_PROXY`, and `NO_PROXY` environment variables (reqwest default behaviour).
///
/// `timeout_secs`: per-request timeout; pass `0` to inherit reqwest's default
/// (no per-request timeout). Returns `Err` only if the builder fails to
/// initialise TLS (should be infallible with the workspace `rustls-tls` feature).
pub fn build_client(timeout_secs: u64) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    if timeout_secs > 0 {
        builder = builder.timeout(Duration::from_secs(timeout_secs));
    }
    builder
        .build()
        .context("build_client: reqwest::Client::build()")
}

/// Convenience wrapper: builds with the given timeout and panics on builder failure
/// (appropriate for startup paths where TLS initialisation failure is unrecoverable).
///
/// Callers that need `Result` should call [`build_client`] directly.
pub fn build_client_or_panic(timeout_secs: u64) -> reqwest::Client {
    build_client(timeout_secs)
        .expect("reqwest async client build failed — TLS initialisation error")
}
