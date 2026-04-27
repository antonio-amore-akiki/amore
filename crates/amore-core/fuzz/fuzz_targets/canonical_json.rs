#![no_main]
// Fuzzes the canonical-JSON provenance path: arbitrary bytes -> serde_json parse
// -> Envelope::seal (calls to_canonical internally) -> Envelope::verify.
// Any panic is a bug; Err from bad JSON or bad canonical encoding is expected.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(s) {
            if let Ok(env) = amore_core::provenance::Envelope::seal(
                amore_core::provenance::GENESIS_PREV_HASH,
                &payload,
            ) {
                // verify() must never panic — true or false only
                let _ = env.verify();
            }
        }
    }
});
