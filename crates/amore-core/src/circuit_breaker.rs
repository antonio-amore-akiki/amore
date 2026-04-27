// H.5 — In-house circuit breaker (ADR 0008: no failsafe-rs / no external dep).
//
// State machine:
//   Closed  -> failures >= threshold   -> Open(opened_at)
//   Open    -> reset_after elapsed     -> HalfOpen { successes: 0 }
//   HalfOpen-> success                 -> successes += 1 >= close_threshold -> Closed
//   HalfOpen-> failure                 -> Open (re-arm)
//
// Callers get a typed `BreakerError<E>` on circuit-open so the WARN log line
// and `degraded=true` response-envelope field are structured (no silent fail-open).

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use thiserror::Error;

// ── state ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BreakerState {
    /// Normal operation — calls pass through.
    Closed,
    /// Circuit open; stores when it was opened so callers can compute `open_for`.
    Open(Instant),
    /// Probing — first few successes close the breaker.
    HalfOpen { successes: u32 },
}

// ── error ──────────────────────────────────────────────────────────────────

/// Error returned by [`CircuitBreaker::call`].
///
/// `BreakerError::Open` carries the dependency name and how long the breaker
/// has been open so callers can emit a structured WARN log line and set
/// `degraded=true` in the response envelope.
#[derive(Debug, Error)]
pub enum BreakerError<E> {
    #[error("circuit breaker open for dep={dep} open_for={open_for:?}")]
    Open { dep: String, open_for: Duration },
    #[error("inner error: {0}")]
    Inner(E),
}

// ── breaker ────────────────────────────────────────────────────────────────

/// Minimal async circuit breaker.
///
/// Default thresholds (see [`CircuitBreaker::new`]):
///   - `threshold_failures = 5`  — consecutive Err calls that open the breaker
///   - `reset_after = 30s`       — how long the breaker stays Open before probing
///   - `close_after_successes = 3` — HalfOpen successes needed to close
///
/// Thread-safe: all state is behind `Arc<Mutex<_>>`.
#[derive(Clone)]
pub struct CircuitBreaker {
    dep: String,
    state: Arc<Mutex<BreakerState>>,
    consecutive_failures: Arc<Mutex<u32>>,
    threshold_failures: u32,
    reset_after: Duration,
    close_after_successes: u32,
}

impl CircuitBreaker {
    /// Create a breaker for the named dependency using the default thresholds.
    pub fn new(dep: impl Into<String>) -> Self {
        Self::with_config(dep, 5, Duration::from_secs(30), 3)
    }

    /// Create a breaker with explicit thresholds (useful for tests + tuning).
    pub fn with_config(
        dep: impl Into<String>,
        threshold_failures: u32,
        reset_after: Duration,
        close_after_successes: u32,
    ) -> Self {
        Self {
            dep: dep.into(),
            state: Arc::new(Mutex::new(BreakerState::Closed)),
            consecutive_failures: Arc::new(Mutex::new(0)),
            threshold_failures,
            reset_after,
            close_after_successes,
        }
    }

    /// Wrap an async call with circuit-breaker protection.
    ///
    /// - If the breaker is `Open` and `reset_after` has **not** elapsed:
    ///   returns `BreakerError::Open` immediately (inner future is **never polled**).
    /// - If the breaker is `Open` and `reset_after` **has** elapsed:
    ///   transitions to `HalfOpen`, then runs the inner call.
    /// - On success in `Closed`/`HalfOpen`: resets failure count / advances successes.
    /// - On failure in `Closed`: increments failure count; trips to `Open` at threshold.
    /// - On failure in `HalfOpen`: re-opens the breaker immediately.
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, BreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        // ── phase 1: inspect state, decide whether to run ───────────────────
        let run_inner = {
            let mut state = self.state.lock().expect("invariant: breaker mutex unpoisoned");
            match *state {
                BreakerState::Closed => true,
                BreakerState::Open(opened_at) => {
                    let open_for = opened_at.elapsed();
                    if open_for >= self.reset_after {
                        // Probe: transition to HalfOpen, allow this call through.
                        *state = BreakerState::HalfOpen { successes: 0 };
                        true
                    } else {
                        return Err(BreakerError::Open {
                            dep: self.dep.clone(),
                            open_for,
                        });
                    }
                }
                BreakerState::HalfOpen { .. } => true,
            }
        };

        debug_assert!(run_inner, "unreachable: non-run paths return early");

        // ── phase 2: run the inner future ────────────────────────────────────
        let outcome = f().await;

        // ── phase 3: update state ────────────────────────────────────────────
        {
            let mut state = self.state.lock().expect("invariant: breaker mutex unpoisoned");
            let mut failures =
                self.consecutive_failures.lock().expect("invariant: failures mutex unpoisoned");

            match outcome {
                Ok(v) => {
                    match *state {
                        BreakerState::HalfOpen { successes } => {
                            let next = successes + 1;
                            if next >= self.close_after_successes {
                                tracing::info!(
                                    target: "amore.circuit_breaker",
                                    dep = %self.dep,
                                    "circuit_breaker.closed — dep recovered after HalfOpen"
                                );
                                *state = BreakerState::Closed;
                            } else {
                                *state = BreakerState::HalfOpen { successes: next };
                            }
                        }
                        BreakerState::Closed => { /* normal success: stay Closed */ }
                        BreakerState::Open(_) => {
                            // Shouldn't happen, but if it does treat as success.
                            *state = BreakerState::Closed;
                        }
                    }
                    *failures = 0;
                    Ok(v)
                }
                Err(e) => {
                    match *state {
                        BreakerState::Closed => {
                            *failures += 1;
                            if *failures >= self.threshold_failures {
                                tracing::warn!(
                                    target: "amore.circuit_breaker",
                                    dep = %self.dep,
                                    failures = *failures,
                                    "circuit_breaker.open — threshold reached"
                                );
                                *state = BreakerState::Open(Instant::now());
                                *failures = 0;
                            }
                        }
                        BreakerState::HalfOpen { .. } => {
                            // Single failure in HalfOpen re-opens immediately.
                            tracing::warn!(
                                target: "amore.circuit_breaker",
                                dep = %self.dep,
                                "circuit_breaker.reopen — HalfOpen probe failed"
                            );
                            *state = BreakerState::Open(Instant::now());
                            *failures = 0;
                        }
                        BreakerState::Open(_) => { /* already open; don't double-trip */ }
                    }
                    Err(BreakerError::Inner(e))
                }
            }
        }
    }

    /// Dependency name (for logging + structured errors).
    pub fn dep(&self) -> &str {
        &self.dep
    }
}
