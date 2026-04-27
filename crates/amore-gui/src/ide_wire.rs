// crates/amore-gui/src/ide_wire.rs — W8.5D IDE config wire-up.
//
// Merges an `amore` MCP server entry into each detected IDE's config file.
//
// CRITICAL schema differences (source: docs/prior-art-w8.5.md §7):
//   Claude Desktop / Claude Code / Cursor / Cline — mcpServers is an OBJECT:
//     { "mcpServers": { "amore": { "command": "...", "args": [...], "env": {...} } } }
//   Continue — mcpServers is an ARRAY:
//     mcpServers: [{ name: "amore", command: "...", args: [...], env: {...} }]
//
// Wire steps per IDE:
//   1. Read existing config file
//   2. Parse (JSON or YAML)
//   3. Save backup: <config>.bak-<ISO-ts>
//   4. Merge amore entry (warns + overwrites if amore already exists)
//   5. Write to tmp file in same dir
//   6. Atomic rename tmp -> original
//   7. Verify post-write by re-parsing

use crate::ide_detect::{ConfigFormat, DetectedIde};
use std::path::PathBuf;

/// Outcome of a wire-up operation for one IDE.
#[derive(Debug)]
pub enum WireVerdict {
    /// Config updated successfully.
    Ok,
    /// Config already contained an amore entry identical to what we would write; no change made.
    SkippedNoChange,
    /// An error occurred.
    Err(String),
}

/// The MCP server entry we inject.
///
/// All OS: command is `amore-mcp` on the PATH (installed alongside amore-gui).
pub fn amore_mcp_entry_object() -> serde_json::Value {
    serde_json::json!({
        "command": "amore-mcp",
        "args": ["--stdio"],
        "env": {}
    })
}

/// Wire all IDEs. Returns (ide_name, verdict) pairs.
pub fn wire_all(ides: &[DetectedIde]) -> Vec<(String, WireVerdict)> {
    ides.iter()
        .map(|ide| (ide.name.clone(), wire_one(ide)))
        .collect()
}

/// Wire a single IDE config file.
pub fn wire_one(ide: &DetectedIde) -> WireVerdict {
    match ide.config_format {
        ConfigFormat::Json => wire_json(ide),
        ConfigFormat::Yaml => wire_yaml(ide),
    }
}

// ── JSON wire (Claude Desktop, Claude Code, Cursor, Cline) ───────────────────

fn wire_json(ide: &DetectedIde) -> WireVerdict {
    let raw = match std::fs::read_to_string(&ide.path) {
        Ok(s) => s,
        Err(e) => return WireVerdict::Err(format!("read failed: {e}")),
    };

    let mut root: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return WireVerdict::Err(format!("JSON parse failed: {e}")),
    };

    let servers = root
        .as_object_mut()
        .and_then(|obj| {
            if !obj.contains_key("mcpServers") {
                obj.insert("mcpServers".to_string(), serde_json::json!({}));
            }
            obj.get_mut("mcpServers")?.as_object_mut()
        });

    let servers = match servers {
        Some(s) => s,
        None => return WireVerdict::Err("mcpServers is not a JSON object".to_string()),
    };

    let entry = amore_mcp_entry_object();

    if servers.get("amore") == Some(&entry) {
        return WireVerdict::SkippedNoChange;
    }

    if servers.contains_key("amore") {
        // Overwrite with updated entry; log is surfaced by caller.
        eprintln!("[amore-wire] {} already had an amore entry — overwriting", ide.name);
    }

    servers.insert("amore".to_string(), entry);

    let updated = match serde_json::to_string_pretty(&root) {
        Ok(s) => s,
        Err(e) => return WireVerdict::Err(format!("JSON serialise failed: {e}")),
    };

    write_atomic(&ide.path, &updated)
}

// ── YAML wire (Continue) ──────────────────────────────────────────────────────

fn wire_yaml(ide: &DetectedIde) -> WireVerdict {
    let raw = match std::fs::read_to_string(&ide.path) {
        Ok(s) => s,
        Err(e) => return WireVerdict::Err(format!("read failed: {e}")),
    };

    let mut root: serde_yaml::Value = match serde_yaml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => return WireVerdict::Err(format!("YAML parse failed: {e}")),
    };

    // Continue mcpServers is an ARRAY. Ensure the key exists.
    let servers = root
        .as_mapping_mut()
        .and_then(|m| {
            let key = serde_yaml::Value::String("mcpServers".to_string());
            if !m.contains_key(&key) {
                m.insert(key.clone(), serde_yaml::Value::Sequence(vec![]));
            }
            m.get_mut(&key)?.as_sequence_mut()
        });

    let servers = match servers {
        Some(s) => s,
        None => return WireVerdict::Err("mcpServers is not a YAML sequence".to_string()),
    };

    // Build the entry to insert.
    let entry = serde_yaml::to_value(serde_json::json!({
        "name": "amore",
        "command": "amore-mcp",
        "args": ["--stdio"],
        "env": {}
    }))
    .expect("static JSON->YAML conversion is infallible");

    // Check if an entry with name "amore" already exists.
    let name_key = serde_yaml::Value::String("name".to_string());
    let amore_str = serde_yaml::Value::String("amore".to_string());
    let existing_pos = servers.iter().position(|v| {
        v.as_mapping()
            .and_then(|m| m.get(&name_key))
            .map(|n| n == &amore_str)
            .unwrap_or(false)
    });

    match existing_pos {
        Some(pos) if servers[pos] == entry => return WireVerdict::SkippedNoChange,
        Some(pos) => {
            eprintln!("[amore-wire] {} already had an amore entry — overwriting", ide.name);
            servers[pos] = entry;
        }
        None => servers.push(entry),
    }

    let updated = match serde_yaml::to_string(&root) {
        Ok(s) => s,
        Err(e) => return WireVerdict::Err(format!("YAML serialise failed: {e}")),
    };

    write_atomic(&ide.path, &updated)
}

// ── Atomic write helper ───────────────────────────────────────────────────────

fn write_atomic(target: &std::path::Path, content: &str) -> WireVerdict {
    // Step 3: backup.
    let ts = chrono_iso();
    let backup = target.with_extension(format!(
        "{}.bak-{}",
        target.extension().and_then(|e| e.to_str()).unwrap_or(""),
        ts
    ));
    if let Err(e) = std::fs::copy(target, &backup) {
        return WireVerdict::Err(format!("backup failed: {e}"));
    }

    // Step 5: write to tmp in same dir.
    let tmp = tmp_path(target);
    if let Err(e) = std::fs::write(&tmp, content) {
        return WireVerdict::Err(format!("tmp write failed: {e}"));
    }

    // Step 6: atomic rename.
    if let Err(e) = std::fs::rename(&tmp, target) {
        // Clean up tmp on failure.
        let _ = std::fs::remove_file(&tmp);
        return WireVerdict::Err(format!("atomic rename failed: {e}"));
    }

    // Step 7: verify by re-parsing.
    match verify_parseable(target) {
        Ok(()) => WireVerdict::Ok,
        Err(e) => WireVerdict::Err(format!("post-write verify failed: {e}")),
    }
}

fn tmp_path(original: &std::path::Path) -> PathBuf {
    let stem = original
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config");
    let dir = original.parent().unwrap_or(std::path::Path::new("."));
    dir.join(format!(".{stem}.amore-tmp"))
}

fn verify_parseable(path: &std::path::Path) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "json" => serde_json::from_str::<serde_json::Value>(&raw)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        "yaml" | "yml" => serde_yaml::from_str::<serde_yaml::Value>(&raw)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        _ => Ok(()), // Unknown extension; skip validation.
    }
}

fn chrono_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as YYYYMMDDTHHMMSSz without external time dep.
    let s = secs;
    let (y, mo, d, h, mi, sec) = epoch_to_ymd(s);
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{sec:02}Z")
}

fn epoch_to_ymd(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    let min = ((secs / 60) % 60) as u32;
    let hour = ((secs / 3600) % 24) as u32;
    let days = secs / 86400;
    // Gregorian calendar calculation from days since Unix epoch.
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u32, m as u32, d as u32, hour, min, sec)
}
