// crates/amore-core/src/diag.rs -- opt-in local crash diagnostics.
// Public API: install_crash_handler(), collect_diag_bundle()
// AMORE_NO_CRASH_DIAG=1 disables all crash dump writing.
// Zero telemetry: all data stays on disk; no network calls made here.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Return the directory where crash dumps are written.
/// Windows: %LOCALAPPDATA%\Amore\crashes  Other: $XDG_CACHE_HOME/amore/crashes
pub fn crash_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = dirs::data_local_dir() {
            return local.join("Amore").join("crashes");
        }
    }
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("amore")
        .join("crashes")
}

fn next_dump_path() -> PathBuf {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let suffix: u32 = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        secs.hash(&mut h);
        std::thread::current().id().hash(&mut h);
        h.finish() as u32
    };
    crash_dir().join(format!("{secs}-{suffix:08x}.dmp"))
}

/// Install crash handler. Returns immediately when AMORE_NO_CRASH_DIAG=1.
/// Silently no-ops on registration failure.
pub fn install_crash_handler() {
    if std::env::var("AMORE_NO_CRASH_DIAG").as_deref() == Ok("1") {
        tracing::debug!("AMORE_NO_CRASH_DIAG=1 -- crash diagnostics disabled");
        return;
    }
    let dir = crash_dir();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::debug!("diag: could not create crash dir {}: {e}", dir.display());
        return;
    }
    install_panic_hook();
    install_native_handler();
}

fn install_panic_hook() {
    let prior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write_panic_dump(info);
        prior(info);
    }));
}

fn write_panic_dump(info: &std::panic::PanicHookInfo<'_>) {
    let path = next_dump_path();
    let payload = info.to_string();
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".to_string());
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let content = format!(
        "amore panic dump\ntimestamp: {secs}\nlocation: {location}\npayload: {payload}\n"
    );
    if let Err(_e) = std::fs::write(&path, content.as_bytes()) {
        crash_handler::write_stderr("amore: could not write panic dump\n");
    } else {
        tracing::debug!("diag: panic dump written to {}", path.display());
    }
}

fn install_native_handler() {
    // SAFETY: closure uses only async-signal-safe OS operations.
    let event = unsafe {
        crash_handler::make_crash_event(move |_ctx| {
            let marker = next_dump_path();
            let _ = std::fs::write(&marker, b"amore native crash\n");
            // Handled(false) lets the default OS handler also run.
            crash_handler::CrashEventResult::Handled(false)
        })
    };
    match crash_handler::CrashHandler::attach(event) {
        Ok(handler) => {
            std::mem::forget(handler); // leak: must outlive main()
            tracing::debug!("diag: native crash handler installed");
        }
        Err(e) => {
            tracing::debug!("diag: native handler registration silently failed: {e}");
        }
    }
}

/// Collect the most recent `max_dumps` crash files into a tar.gz archive.
pub fn collect_diag_bundle(out_path: &Path, max_dumps: usize) -> Result<PathBuf> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let out = if out_path.extension().is_none() {
        out_path.with_extension("tar.gz")
    } else {
        out_path.to_path_buf()
    };
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).context("create output dir")?;
    }
    let gz = std::fs::File::create(&out).context("create bundle archive")?;
    let enc = GzEncoder::new(gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    let dumps = collect_recent_dumps(max_dumps);
    for dump_path in &dumps {
        let file_name = dump_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown.dmp".to_string());
        match std::fs::File::open(dump_path) {
            Ok(mut f) => {
                if let Err(e) = tar.append_file(&file_name, &mut f) {
                    tracing::warn!("diag: skipping {} in bundle: {e}", dump_path.display());
                }
            }
            Err(e) => {
                tracing::warn!("diag: could not open {} for bundle: {e}", dump_path.display());
            }
        }
    }
    // Explicitly finish gzip stream so trailer is flushed before the file is closed.
    let enc = tar.into_inner().context("tar into_inner")?;
    enc.finish().context("finalise gzip stream")?;
    tracing::info!("diag: bundle created at {} ({} dumps)", out.display(), dumps.len());
    Ok(out)
}

fn collect_recent_dumps(max_dumps: usize) -> Vec<PathBuf> {
    let dir = crash_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<(u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "dmp").unwrap_or(false))
        .filter_map(|e| {
            let mtime = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            Some((mtime, e.path()))
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().take(max_dumps).map(|(_, p)| p).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn crash_handler_writes_dump_on_panic() {
        let dir = tempdir().expect("tempdir");
        let dump_path = dir.path().join("test-panic.dmp");
        let dump_path_clone = dump_path.clone();
        std::panic::set_hook(Box::new(move |info| {
            let payload = info.to_string();
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "<unknown>".to_string());
            let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            let content = format!(
                "amore panic dump\ntimestamp: {secs}\nlocation: {location}\npayload: {payload}\n"
            );
            std::fs::write(&dump_path_clone, content.as_bytes()).ok();
        }));
        let _ = std::panic::catch_unwind(|| panic!("diag-test-panic"));
        let _ = std::panic::take_hook();
        assert!(dump_path.exists(), "dump file should exist after panic");
        let contents = std::fs::read_to_string(&dump_path).expect("read dump");
        assert!(
            contents.contains("diag-test-panic"),
            "dump should contain panic message; got: {contents}"
        );
        assert!(contents.contains("amore panic dump"), "dump should have amore header");
    }

    #[test]
    fn diag_bundle_creates_zip_with_dumps() {
        use flate2::read::GzDecoder;
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let dir = tempdir().expect("tempdir");
        let crashes = dir.path().join("crashes");
        std::fs::create_dir_all(&crashes).expect("create crashes dir");
        std::fs::write(crashes.join("1000000000-aabbccdd.dmp"), b"dump1\n").expect("write dump1");
        std::fs::write(crashes.join("1000000001-11223344.dmp"), b"dump2\n").expect("write dump2");
        let bundle_path = dir.path().join("test-bundle.tar.gz");
        let gz = std::fs::File::create(&bundle_path).expect("create bundle");
        let enc = GzEncoder::new(gz, Compression::default());
        let mut tar = tar::Builder::new(enc);
        for entry in std::fs::read_dir(&crashes).expect("read_dir") {
            let e = entry.expect("entry");
            if e.path().extension().map(|x| x == "dmp").unwrap_or(false) {
                let name = e.file_name().to_string_lossy().into_owned();
                let mut f = std::fs::File::open(e.path()).expect("open dump");
                tar.append_file(&name, &mut f).expect("append");
            }
        }
        let enc = tar.into_inner().expect("tar into_inner");
        enc.finish().expect("gzip finish");
        assert!(bundle_path.exists(), "bundle archive should exist");
        let meta = std::fs::metadata(&bundle_path).expect("stat bundle");
        assert!(meta.len() > 0, "bundle should not be empty");
        let f = std::fs::File::open(&bundle_path).expect("open bundle");
        let decoder = GzDecoder::new(f);
        let mut tar_reader = tar::Archive::new(decoder);
        let entries: Vec<_> = tar_reader
            .entries()
            .expect("tar entries")
            .map(|e| e.expect("entry").path().expect("path").to_path_buf())
            .collect();
        assert_eq!(entries.len(), 2, "bundle should contain 2 dump entries");
    }
}
