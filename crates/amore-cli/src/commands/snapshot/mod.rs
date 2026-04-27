// commands/snapshot/mod.rs — H.7 snapshot/restore CLI subcommands.
//
// `amore snapshot create <path>` bundles: Qdrant collection snapshots (REST API),
//   SQLite database (VACUUM INTO), manifest. Format: tar.gz + .sha256 sidecar.
// `amore snapshot restore <path>` verifies .sha256, untars, restores Qdrant
//   via upload API, and atomically replaces the SQLite database.
//
// Design: Adapt — composes workspace deps only (tar, flate2, reqwest,
//   sha2/hex, rusqlite VACUUM INTO). No new algorithm.
// ADR 0010: no-unwrap policy enforced at crate level.

mod archive;
mod qdrant;

use anyhow::{Context, Result};
use std::{fs, path::Path, time::Duration};

const SQLITE_INNER_NAME: &str = "amore.db";
const HTTP_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Create a snapshot at `out_path` (.tar.gz) and a .sha256 sidecar.
/// Reads the SQLite database from `data_dir/amore.db`.
pub async fn create(out_path: &Path, data_dir: &Path) -> Result<()> {
    let tmp_dir = temp_dir("amore-snapshot");
    fs::create_dir_all(&tmp_dir)
        .with_context(|| format!("create temp dir {}", tmp_dir.display()))?;

    let client = http_client()?;

    // 1. Discover and download all Qdrant collection snapshots.
    let collections = qdrant::list_collections(&client).await?;
    eprintln!(
        "[snapshot] found {} collection(s): {:?}",
        collections.len(),
        collections
    );
    for name in &collections {
        qdrant::download_collection_snapshot(&client, name, &tmp_dir).await?;
    }

    // 2. Backup SQLite via VACUUM INTO.
    let db_src = data_dir.join(SQLITE_INNER_NAME);
    let db_dst = tmp_dir.join(SQLITE_INNER_NAME);
    sqlite_vacuum_into(&db_src, &db_dst)?;

    // 3. Write provenance manifest.
    let manifest = build_manifest(&collections);
    fs::write(tmp_dir.join("manifest.json"), &manifest)
        .context("write manifest")?;

    // 4. Pack tmp_dir → tar.gz, write sidecar.
    archive::pack_tar_gz(&tmp_dir, out_path)?;
    let sidecar = archive::sidecar_path(out_path);
    archive::write_sha256_sidecar(out_path, &sidecar)?;

    fs::remove_dir_all(&tmp_dir)
        .with_context(|| format!("clean up temp dir {}", tmp_dir.display()))?;
    eprintln!(
        "[snapshot] created {} (sidecar: {})",
        out_path.display(),
        sidecar.display()
    );
    Ok(())
}

/// Restore from `in_path` (.tar.gz). Verifies .sha256, untars, uploads
/// Qdrant snapshots, and atomically replaces the SQLite database.
pub async fn restore(in_path: &Path, data_dir: &Path) -> Result<()> {
    let sidecar = archive::sidecar_path(in_path);
    archive::verify_sha256_sidecar(in_path, &sidecar)?;
    eprintln!("[snapshot] sha256 verified");

    let tmp_dir = temp_dir("amore-restore");
    fs::create_dir_all(&tmp_dir)
        .with_context(|| format!("create temp dir {}", tmp_dir.display()))?;

    archive::unpack_tar_gz(in_path, &tmp_dir)?;

    let collections = read_manifest_collections(&tmp_dir.join("manifest.json"))?;
    eprintln!("[snapshot] restoring {} collection(s)", collections.len());

    let client = http_client()?;
    for name in &collections {
        let snap = tmp_dir.join(qdrant::collection_snapshot_filename(name));
        qdrant::upload_collection_snapshot(&client, name, &snap).await?;
    }

    // Atomically replace SQLite database.
    let db_src = tmp_dir.join(SQLITE_INNER_NAME);
    let db_dst = data_dir.join(SQLITE_INNER_NAME);
    if db_src.exists() {
        if let Some(parent) = db_dst.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create data_dir {}", parent.display()))?;
        }
        let staging = db_dst.with_extension("db.restoring");
        fs::copy(&db_src, &staging)
            .with_context(|| format!("copy db to staging {}", staging.display()))?;
        fs::rename(&staging, &db_dst)
            .with_context(|| format!("atomic rename to {}", db_dst.display()))?;
        eprintln!("[snapshot] SQLite restored to {}", db_dst.display());
    } else {
        eprintln!("[snapshot] no SQLite db in archive — skipping db restore");
    }

    fs::remove_dir_all(&tmp_dir)
        .with_context(|| format!("clean up temp dir {}", tmp_dir.display()))?;
    eprintln!("[snapshot] restore complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()))
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .context("build reqwest client")
}

fn build_manifest(collections: &[String]) -> String {
    let items: Vec<String> = collections
        .iter()
        .map(|c| format!("\"{}\"", c.replace('"', "\\\"")))
        .collect();
    format!("{{\"collections\":[{}]}}", items.join(","))
}

fn read_manifest_collections(manifest_path: &Path) -> Result<Vec<String>> {
    let raw = fs::read_to_string(manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).context("parse manifest JSON")?;
    let arr = v["collections"]
        .as_array()
        .context("manifest missing 'collections' array")?;
    arr.iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .context("collection entry not a string")
        })
        .collect()
}

fn sqlite_vacuum_into(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        eprintln!(
            "[snapshot] SQLite db not found at {} — skipping",
            src.display()
        );
        return Ok(());
    }
    let conn = rusqlite::Connection::open(src)
        .with_context(|| format!("open SQLite {}", src.display()))?;
    conn.execute_batch(&format!("VACUUM INTO '{}'", dst.to_string_lossy()))
        .with_context(|| format!("VACUUM INTO {}", dst.display()))?;
    eprintln!("[snapshot] SQLite VACUUM INTO {}", dst.display());
    Ok(())
}
