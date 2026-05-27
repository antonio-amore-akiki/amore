// Integration test: qdrant_storage_writable
// Prior-art: Adapt from tests/working_product_docker.rs (skip-if-service-absent)
// Test #[ignore] by default — requires running qdrant + QDRANT_URL + QDRANT_STORAGE_PATH.
// cargo test -p amore-integration-tests qdrant_storage_writable -- --ignored

use std::env;

fn qdrant_url() -> String {
    env::var("QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6333".to_string())
}
fn qdrant_storage_path() -> Option<std::path::PathBuf> {
    env::var("QDRANT_STORAGE_PATH").ok().map(std::path::PathBuf::from)
}
fn qdrant_reachable(url: &str) -> bool {
    std::process::Command::new("curl")
        .args(["--silent", "--max-time", "2", "--fail", &format!("{url}/healthz")])
        .status().map(|s| s.success()).unwrap_or(false)
}
const PROBE_COLLECTION: &str = "amore-storage-probe-test";

#[test]
#[ignore = "requires running qdrant; set QDRANT_URL + QDRANT_STORAGE_PATH and pass --ignored"]
fn qdrant_storage_dir_is_user_writable() {
    let url = qdrant_url();
    if !qdrant_reachable(&url) { eprintln!("[skip] qdrant not reachable at {url}"); return; }
    let storage = qdrant_storage_path()
        .expect("QDRANT_STORAGE_PATH must be set to configured qdrant storage.storage_path value");
    // 1. Create probe collection.
    let body = serde_json::json!({"vectors":{"size":4,"distance":"Dot"}});
    let create_url = format!("{url}/collections/{PROBE_COLLECTION}");
    let status = std::process::Command::new("curl")
        .args(["--silent","--fail","-X","PUT","-H","Content-Type: application/json",
               "-d",&body.to_string(),&create_url])
        .status().expect("curl PUT collection");
    assert!(status.success(), "qdrant PUT collection failed");
    // 2. Verify file landed under configured storage path.
    let collection_dir = storage.join("collections").join(PROBE_COLLECTION);
    assert!(collection_dir.exists(),
        "qdrant collection dir not found at {}
         qdrant may be writing to Program Files (read-only). Check          packaging/installer/qdrant/config.yaml storage.storage_path.",
        collection_dir.display());
    // 3. Cleanup.
    let _ = std::process::Command::new("curl")
        .args(["--silent","-X","DELETE",&format!("{url}/collections/{PROBE_COLLECTION}")])
        .status();
}
