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
/// Body window scanned for keyword matches. Beyond filename + title + topic
/// line, the router also matches against the first N chars of the body so
/// doc-internal vocabulary (e.g. `Hasleo`, `Kopia`, `WAL`, `BEGIN IMMEDIATE`)
/// is reachable without forcing every author to maintain a `topic:` line.
/// 2000 chars keeps the score deterministic on small canonical docs while
/// staying cheap (single substring scan per query token).
const BODY_KEYWORD_SCAN_CHARS: usize = 2000;
/// Maximum number of canonical-doc hits returned per query. Caps the
/// "router returns everything that scored > 0" failure mode that inflates
/// the injected context past the raw-context baseline (token-reduction
/// benchmark surfaced this: 16/43 fixtures over-fetched 30-49 docs on
/// common-vocabulary queries like "amore install …", dragging avg savings
/// from ~95% (success path) down to 21%). Top-3 matches the canonical
/// few-shot retrieval pattern (mem0 default; LongMemEval R@3 slice;
/// hybrid-RAG top-k=3), keeps multi-doc topics reachable, and crushes the
/// over-fetch tail. Empirically: TOP_K=5 → avg 84.4%; TOP_K=3 → avg 89.3%
/// over 43 fixtures (cf. docs/BENCHMARKS.md).
pub const TOP_K_HITS: usize = 3;

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
    /// Result is sorted by `topic_score` descending and **capped at
    /// `TOP_K_HITS`** (3) to prevent over-fetch on common-vocabulary
    /// queries from blowing past the raw-context baseline. Non-existent
    /// paths are skipped without error (some agents have only the user
    /// dir, some only the workspace dir).
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
        hits.truncate(TOP_K_HITS);
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
        // Body keywords: lowercased first N chars of body, so doc-internal
        // vocabulary contributes to matching even when authors haven't curated
        // a `topic:` line. Substring contains() is O(N*M) total — N tokens *
        // M chars — fine at this scale.
        let body_keywords: String = body
            .chars()
            .take(BODY_KEYWORD_SCAN_CHARS)
            .collect::<String>()
            .to_lowercase();
        let haystack = format!(
            "{} {} {} {}",
            filename.to_lowercase(),
            title.to_lowercase(),
            topic_line.to_lowercase(),
            body_keywords,
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

/// Resolve canonical search paths: [~/.claude/docs, <cwd>/.claude/docs, <cwd>/docs].
/// Env-derived paths must be validated with `validate_docs_path` before use (M3).
pub fn default_search_paths(home: &Path, cwd: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".claude").join("docs"),
        cwd.join(".claude").join("docs"),
        cwd.join("docs"),
    ]
}

/// Accept a caller-supplied docs path only if it is inside `home_dir()`.
/// Set `AMORE_DOCS_PATHS_ALLOW_ANY=1` to allow arbitrary paths (power-user opt-in).
/// Returns `Ok(())` on acceptance, `Err(String)` with reason on rejection.
pub fn validate_docs_path(path: &Path, home: &Path) -> Result<(), String> {
    if std::env::var("AMORE_DOCS_PATHS_ALLOW_ANY").as_deref() == Ok("1") {
        return Ok(());
    }
    if path.starts_with(home) {
        return Ok(());
    }
    Err(format!(
        "docs path {path:?} is outside home_dir {home:?}; \
         set AMORE_DOCS_PATHS_ALLOW_ANY=1 to allow"
    ))
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
        let p = std::env::temp_dir().join(format!("amore-docs-test-{nanos:x}-{n}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_doc(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn router_returns_empty_when_no_paths_exist() {
        let r = CanonicalDocsRouter::new();
        let missing = std::env::temp_dir().join("amore-no-such-dir-xyz");
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
    fn router_matches_body_keywords_when_title_filename_topic_miss() {
        // Body-scan path (2026-05-26): a query about doc-internal vocabulary
        // that does NOT appear in filename / title / topic-line must still
        // match via the first N chars of the body. Models the real A9 finding
        // that queries like "kopia daily snapshot dpapi credentials" missed
        // when the title only said "Backup Stack".
        let d = fresh_dir();
        write_doc(
            &d,
            "backup-stack.md",
            "stable: true\n# Backup Stack\n\nKopia daily snapshot uses DPAPI credentials in C:/Users/anto/.kopia/.cred.\n",
        );
        let r = CanonicalDocsRouter::new();
        let hits = r.route("kopia dpapi credentials", &[d.as_path()]).unwrap();
        assert!(
            !hits.is_empty(),
            "body-scan must surface backup-stack on kopia+dpapi+credentials"
        );
        assert!(hits[0].path.ends_with("backup-stack.md"));
    }

    #[test]
    fn router_returns_no_hits_on_query_without_overlap() {
        let d = fresh_dir();
        write_doc(&d, "rust.md", "stable: true\n# Rust Async\n\nNetworking docs.\n");
        let hits = CanonicalDocsRouter::new().route("chocolate cookies", &[d.as_path()]).unwrap();
        assert!(hits.is_empty(), "unrelated query must return no hits");
    }

    // M3: out-of-home rejected by default; opt-in env unlocks.
    #[test]
    fn validate_docs_path_home_guard() {
        let home = std::path::Path::new("/home/user");
        // SAFETY: test-only env mutation; tests run in isolated process.
        unsafe { std::env::remove_var("AMORE_DOCS_PATHS_ALLOW_ANY") };
        assert!(crate::docs::validate_docs_path(std::path::Path::new("/tmp/evil"), home).is_err());
        assert!(crate::docs::validate_docs_path(&home.join("docs"), home).is_ok());
        unsafe { std::env::set_var("AMORE_DOCS_PATHS_ALLOW_ANY", "1") };
        assert!(crate::docs::validate_docs_path(std::path::Path::new("/tmp/evil"), home).is_ok());
        unsafe { std::env::remove_var("AMORE_DOCS_PATHS_ALLOW_ANY") };
    }
}
