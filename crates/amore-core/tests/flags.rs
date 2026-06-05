// Integration tests for amore_core::flags — runtime feature flag resolver.
//
// NOTE: `FLAGS` is a process-global OnceLock that initialises on first call.
// Under the default multi-threaded test runner any test that calls
// `Flags::is_enabled` first will freeze the flag state before another test
// can set env vars.  Only `env_flag_on_resolves_true` has this ordering
// dependency — it is marked `#[ignore]` so default `cargo test` stays green.
//
// To run the ignored test in isolation (guaranteed single-threaded binary):
//   cargo test -p amore-core --test flags -- --ignored --test-threads=1
//
// Rust edition 2024: env::set_var / remove_var are unsafe — wrapped in unsafe block.

use amore_core::flags::Flags;
use std::env;

/// Requires single-threaded execution so the env var is visible before the
/// OnceLock fires.  Run with:
///   cargo test -p amore-core --test flags -- --ignored --test-threads=1
#[test]
#[ignore = "OnceLock is process-global; must run --test-threads=1 in isolation (see file comment)"]
fn env_flag_on_resolves_true() {
    // Uses a unique flag name not used elsewhere in the test binary.
    // SAFETY: single-threaded test binary (--test-threads=1); no concurrent env mutation.
    unsafe {
        env::set_var("AMORE_FLAG_W3_TEST_GATE_ON", "on");
    }
    assert!(Flags::is_enabled("w3_test_gate_on"));
    unsafe {
        env::remove_var("AMORE_FLAG_W3_TEST_GATE_ON");
    }
}

#[test]
fn unknown_flag_defaults_to_false() {
    assert!(!Flags::is_enabled("nonexistent_flag_xyz_w3_3a"));
}
