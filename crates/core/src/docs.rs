// Canonical-docs router.
//
// Walks `*.md` files in caller-supplied search paths, looks for a
// `stable: true` header within the first 10 lines, and scores topic
// match against the query by keyword overlap (filename + first
// `# Heading` + any `topic:` line in the header block).
//
// Per CLAUDE.md canonical-docs pattern: deterministic source-of-truth
// beats probabilistic recall for known domains. The router runs BEFORE
// hybrid recall so a topic-matched canonical doc gets injected as
// authoritative context; recall fills the long-tail.

use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

const HEADER_SCAN_LINES: usize = 10;
const EXCERPT_MAX_CHARS: usize = 800;

pub struct CanonicalDocsRouter {
    /// If true, only docs with `stable: true` header qualify. Default true.
    /// Tests / debug surfaces flip to false.
    pub require_stable: bool,
}

impl CanonicalDocsRouter {
    pub fn new() -> Self {
        Self {
            require_stable: true,
        }
    }

    /// Walk `search_paths` (each treated as a directory containing `*.md`
    /// files), return docs whose topic keywords overlap with the query.
    /// Result is sorted by `topic_score` descending. Non-existent paths
    /// are skipped without error (some agents have only the user dir,
    /// some only the workspace dir).
    pub fn route(&self, query: &str, search_paths: &[&Path]) -> Result<Vec<DocHit>> {
        let q_tokens = tokenize(query);
        if q_tokens.is_empty() {
            return Ok(vec![]);
        }
        let mut hits: Vec<DocHit> = Vec::new();
        for dir in search_paths {
            if !dir.exists() {
                continue;
            }
            let entries = match fs::read_dir(dir) {
                Ok(it) => it,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                if let Some(hit) = self.score_doc(&p, &q_tokens)? {
                    hits.push(hit);
                }
            }
        }
        hits.sort_by(|a, b| {
            b.topic_score
                .partial_cmp(&a.topic_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(hits)
    }

    fn score_doc(&self, path: &Path, q_tokens: &[String]) -> Result<Option<DocHit>> {
        let body = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        let header_lines: Vec<&str> = body.lines().take(HEADER_SCAN_LINES).collect();
        let header_text = header_lines.join("\n").to_lowercase();
        if self.require_stable && !header_text.contains("stable: true") {
            return Ok(None);
        }
        let title = extract_title(&body).unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });
        let topic_line = header_lines
            .iter()
            .find_map(|l| l.strip_prefix("topic:").map(|s| s.trim().to_string()))
            .unwrap_or_default();
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let haystack = format!(
            "{} {} {}",
            filename.to_lowercase(),
            title.to_lowercase(),
            topic_line.to_lowercase()
        );
        let matches: usize = q_tokens
            .iter()
            .filter(|t| haystack.contains(t.as_str()))
            .count();
        if matches == 0 {
            return Ok(None);
        }
        // Score = fraction of query tokens that hit. Range (0, 1].
        let topic_score = matches as f32 / q_tokens.len() as f32;
        let excerpt = extract_excerpt(&body);
        Ok(Some(DocHit {
            path: path.to_string_lossy().into_owned(),
            title,
            topic_score,
            excerpt,
        }))
    }
}

impl Default for CanonicalDocsRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocHit {
    pub path: String,
    pub title: String,
    pub topic_score: f32,
    pub excerpt: String,
}

/// Tokenize a query into lowercased alphanumeric words >=3 chars. Drops
/// stopwords-by-length (single chars + 2-letter junk) and dedupes.
fn tokenize(query: &str) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for raw in query.split(|c: char| !c.is_alphanumeric()) {
        let t = raw.to_lowercase();
        if t.len() < 3 {
            continue;
        }
        if seen.insert(t.clone()) {
            out.push(t);
        }
    }
    out
}

/// First Markdown H1/H2 heading, or None if no heading found.
fn extract_title(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// First chunk of body content (up to EXCERPT_MAX_CHARS), skipping any
/// initial header block before the first blank line.
fn extract_excerpt(body: &str) -> String {
    let mut after_header = false;
    let mut buf = String::new();
    for line in body.lines() {
        if !after_header {
            if line.trim().is_empty() {
                after_header = true;
            }
            continue;
        }
        if buf.len() + line.len() + 1 > EXCERPT_MAX_CHARS {
            break;
        }
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
    }
    if buf.is_empty() {
        // No header/body split — fall back to first N chars.
        buf = body.chars().take(EXCERPT_MAX_CHARS).collect();
    }
    buf
}

/// Convenience: resolve the canonical search paths for the current user.
/// Returns [~/.claude/docs, <cwd>/.claude/docs, <cwd>/docs] in that order;
/// callers filter the non-existent ones during `route()`.
pub fn default_search_paths(home: &Path, cwd: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".claude").join("docs"),
        cwd.join(".claude").join("docs"),
        cwd.join("docs"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    fn fresh_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("obelion-docs-test-{nanos:x}-{n}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_doc(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn router_returns_empty_when_no_paths_exist() {
        let r = CanonicalDocsRouter::new();
        let missing = std::env::temp_dir().join("obelion-no-such-dir-xyz");
        let hits = r.route("anything", &[missing.as_path()]).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn router_finds_stable_doc_by_keyword() {
        let d = fresh_dir();
        write_doc(
            &d,
            "backup-stack.md",
            "stable: true\n# Backup Stack\n\nHasleo + Kopia recipe.\n",
        );
        let r = CanonicalDocsRouter::new();
        let hits = r.route("backup hasleo", &[d.as_path()]).unwrap();
        assert!(!hits.is_empty(), "expected match on backup keyword");
        assert!(hits[0].path.ends_with("backup-stack.md"));
        assert_eq!(hits[0].title, "Backup Stack");
        assert!(hits[0].topic_score > 0.0);
        assert!(hits[0].excerpt.contains("Hasleo"));
    }

    #[test]
    fn router_skips_non_stable_docs_by_default() {
        let d = fresh_dir();
        write_doc(
            &d,
            "draft.md",
            "stable: false\n# Draft Notes\n\nNot for canonical lookup.\n",
        );
        let r = CanonicalDocsRouter::new();
        let hits = r.route("draft notes", &[d.as_path()]).unwrap();
        assert!(hits.is_empty(), "non-stable doc must not be returned");
    }

    #[test]
    fn router_can_relax_stable_requirement_for_debug() {
        let d = fresh_dir();
        write_doc(&d, "draft.md", "# Draft Notes\n\nAuxiliary context.\n");
        let mut r = CanonicalDocsRouter::new();
        r.require_stable = false;
        let hits = r.route("draft notes", &[d.as_path()]).unwrap();
        assert!(!hits.is_empty(), "unstable doc must surface in debug mode");
    }

    #[test]
    fn router_ranks_better_matches_higher() {
        let d = fresh_dir();
        write_doc(
            &d,
            "rust-async.md",
            "stable: true\n# Rust Async Networking\n\nTokio + futures.\n",
        );
        write_doc(
            &d,
            "cookies.md",
            "stable: true\n# Cookies\n\nBaking instructions.\n",
        );
        let r = CanonicalDocsRouter::new();
        let hits = r.route("rust async tokio", &[d.as_path()]).unwrap();
        assert!(!hits.is_empty());
        assert!(
            hits[0].path.ends_with("rust-async.md"),
            "rust-async doc must outrank cookies doc on rust+async+tokio query"
        );
    }

    #[test]
    fn router_uses_topic_header_line_for_matching() {
        let d = fresh_dir();
        write_doc(
            &d,
            "esoteric-name.md",
            "stable: true\ntopic: backup hasleo kopia\n# Esoteric Title\n\nBody.\n",
        );
        let r = CanonicalDocsRouter::new();
        let hits = r.route("hasleo", &[d.as_path()]).unwrap();
        assert!(!hits.is_empty(), "topic: line must drive matching");
        assert!(hits[0].path.ends_with("esoteric-name.md"));
    }

    #[test]
    fn router_returns_no_hits_on_query_without_overlap() {
        let d = fresh_dir();
        write_doc(
            &d,
            "rust.md",
            "stable: true\n# Rust Async\n\nNetworking docs.\n",
        );
        let r = CanonicalDocsRouter::new();
        let hits = r.route("chocolate cookies", &[d.as_path()]).unwrap();
        assert!(hits.is_empty(), "unrelated query must return no hits");
    }
}
