// snapshot/qdrant.rs — Qdrant REST snapshot create/download/upload helpers.
// ADR 0010: no-unwrap policy — all Results propagated.

use anyhow::{Context, Result};
use reqwest::multipart;
use std::{fs, path::Path};

const QDRANT_REST_URL: &str = "http://127.0.0.1:6333";

/// Returns a stable filename for a collection's snapshot inside the archive.
pub fn collection_snapshot_filename(collection: &str) -> String {
    format!("qdrant-{collection}.snapshot")
}

/// List all Qdrant collection names via GET /collections.
pub async fn list_collections(client: &reqwest::Client) -> Result<Vec<String>> {
    let url = format!("{QDRANT_REST_URL}/collections");
    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .context("Qdrant /collections returned error")?
        .json()
        .await
        .context("parse /collections JSON")?;

    let names = resp["result"]["collections"]
        .as_array()
        .context("unexpected /collections shape")?
        .iter()
        .filter_map(|c| c["name"].as_str().map(str::to_owned))
        .collect();
    Ok(names)
}

/// POST /collections/<name>/snapshots, then GET the snapshot bytes and write
/// to `dest_dir/<collection_snapshot_filename(name)>`.
pub async fn download_collection_snapshot(
    client: &reqwest::Client,
    collection: &str,
    dest_dir: &Path,
) -> Result<()> {
    let create_url = format!("{QDRANT_REST_URL}/collections/{collection}/snapshots");
    let create_resp: serde_json::Value = client
        .post(&create_url)
        .send()
        .await
        .with_context(|| format!("POST {create_url}"))?
        .error_for_status()
        .with_context(|| format!("create snapshot for collection {collection}"))?
        .json()
        .await
        .context("parse snapshot create response")?;

    let snapshot_name = create_resp["result"]["name"]
        .as_str()
        .context("snapshot response missing result.name")?
        .to_owned();

    let dl_url = format!(
        "{QDRANT_REST_URL}/collections/{collection}/snapshots/{snapshot_name}"
    );
    let bytes = client
        .get(&dl_url)
        .send()
        .await
        .with_context(|| format!("GET {dl_url}"))?
        .error_for_status()
        .with_context(|| format!("download snapshot {snapshot_name}"))?
        .bytes()
        .await
        .context("read snapshot bytes")?;

    let dest = dest_dir.join(collection_snapshot_filename(collection));
    fs::write(&dest, &bytes)
        .with_context(|| format!("write snapshot to {}", dest.display()))?;
    eprintln!(
        "[snapshot] collection '{collection}' saved ({} bytes)",
        bytes.len()
    );
    Ok(())
}

/// Delete then re-create the collection (if needed), then POST
/// /collections/<name>/snapshots/upload?priority=snapshot with the file bytes.
pub async fn upload_collection_snapshot(
    client: &reqwest::Client,
    collection: &str,
    snap_file: &Path,
) -> Result<()> {
    // Delete the collection so the snapshot upload can fully reconstruct it.
    let col_url = format!("{QDRANT_REST_URL}/collections/{collection}");
    let _ = client.delete(&col_url).send().await; // ignore — may not exist

    let file_bytes = fs::read(snap_file)
        .with_context(|| format!("read snapshot {}", snap_file.display()))?;
    let filename = snap_file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "snapshot.snapshot".to_owned());

    let part = multipart::Part::bytes(file_bytes)
        .file_name(filename)
        .mime_str("application/octet-stream")
        .context("set MIME on multipart part")?;
    let form = multipart::Form::new().part("snapshot", part);

    let upload_url = format!(
        "{QDRANT_REST_URL}/collections/{collection}/snapshots/upload?priority=snapshot"
    );
    let resp = client
        .post(&upload_url)
        .multipart(form)
        .send()
        .await
        .with_context(|| format!("POST {upload_url}"))?
        .error_for_status()
        .with_context(|| format!("upload snapshot for collection {collection}"))?;

    eprintln!(
        "[snapshot] collection '{collection}' uploaded (HTTP {})",
        resp.status()
    );
    Ok(())
}
