// snapshot/archive.rs — tar.gz pack/unpack + SHA-256 sidecar helpers.
// ADR 0010: no-unwrap policy — all Results propagated.

use anyhow::{Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use hex::ToHex;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

/// Path of the .sha256 sidecar for a given archive path.
pub fn sidecar_path(archive: &Path) -> PathBuf {
    let ext = archive
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut p = archive.to_path_buf();
    p.set_extension(format!("{ext}.sha256"));
    p
}

/// Pack the contents of `src_dir` into a gzip-compressed tar at `out_path`.
pub fn pack_tar_gz(src_dir: &Path, out_path: &Path) -> Result<()> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create output dir {}", parent.display()))?;
    }
    let out_file = fs::File::create(out_path)
        .with_context(|| format!("create archive {}", out_path.display()))?;
    let gz = GzEncoder::new(out_file, Compression::best());
    let mut tar = tar::Builder::new(gz);
    tar.append_dir_all(".", src_dir)
        .with_context(|| format!("append {} to archive", src_dir.display()))?;
    tar.finish().context("finalize tar archive")?;
    Ok(())
}

/// Unpack a gzip-compressed tar `archive` into `dest_dir`.
pub fn unpack_tar_gz(archive: &Path, dest_dir: &Path) -> Result<()> {
    let file = fs::File::open(archive)
        .with_context(|| format!("open archive {}", archive.display()))?;
    let gz = GzDecoder::new(file);
    let mut tar_archive = tar::Archive::new(gz);
    tar_archive
        .unpack(dest_dir)
        .with_context(|| format!("unpack archive to {}", dest_dir.display()))?;
    Ok(())
}

/// Compute SHA-256 of `path` and write a `<hex>  <filename>` sidecar.
pub fn write_sha256_sidecar(archive: &Path, sidecar: &Path) -> Result<()> {
    let digest = sha256_file(archive)?;
    let name = archive
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "archive".to_owned());
    fs::write(sidecar, format!("{digest}  {name}\n"))
        .with_context(|| format!("write sidecar {}", sidecar.display()))?;
    Ok(())
}

/// Verify the sidecar checksum matches the archive; bail if missing or wrong.
pub fn verify_sha256_sidecar(archive: &Path, sidecar: &Path) -> Result<()> {
    if !sidecar.exists() {
        anyhow::bail!(
            "sha256 sidecar not found: {}. Cannot verify archive integrity.",
            sidecar.display()
        );
    }
    let content = fs::read_to_string(sidecar)
        .with_context(|| format!("read sidecar {}", sidecar.display()))?;
    let expected = content
        .split_whitespace()
        .next()
        .context("sidecar file is empty or malformed")?
        .to_owned();
    let actual = sha256_file(archive)?;
    if actual != expected {
        anyhow::bail!(
            "sha256 mismatch for {}: expected {expected}, got {actual}",
            archive.display()
        );
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("open {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 65536];
    loop {
        let n = file
            .read(&mut buf)
            .with_context(|| format!("read {} for hashing", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().encode_hex())
}
