// Integration tests for secrets module.
// Keyring tests require OS keyring backend; CI may not have one.
// Run locally with: cargo test -p amore-cli --test secrets -- --ignored

#[test]
#[ignore = "requires OS keyring backend; run locally with `cargo test -- --ignored`"]
fn keyring_roundtrip() {
    let service = "amore-test";
    let name = format!("test-w4-4d-{}", std::process::id());
    let entry = keyring::Entry::new(service, &name).unwrap();
    entry.set_password("test-secret-value").unwrap();
    assert_eq!(entry.get_password().unwrap(), "test-secret-value");
    entry.delete_credential().unwrap();
}
