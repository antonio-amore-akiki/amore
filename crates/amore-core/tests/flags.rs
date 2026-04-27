// Integration tests for amore_core::flags — runtime feature flag resolver.
//
// NOTE: Due to OnceLock + process-global static, these tests rely on distinct
// flag names that won't be set by other tests in the same binary. Run with
// `cargo test -p amore-core --test flags -- --test-threads=1` to avoid
// cross-test env var leakage if you add tests that modify the same flag names.
//
// Rust edition 2024: env::set_var / remove_var are unsafe — wrapped in unsafe block.

use amore_core::flags::Flags;
use std::env;

#[test]
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
