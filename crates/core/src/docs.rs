// Canonical-docs router.
//
// Reads docs/*.md files with `stable: true` header from:
//   - ~/.claude/docs/
//   - <cwd>/.claude/docs/ or <cwd>/docs/
//
// Topic-matches by keyword extraction from the query.
// Returns deterministic source-of-truth content BEFORE probabilistic recall.

use anyhow::Result;
use std::path::Path;

pub struct CanonicalDocsRouter {
    // TODO: doc index
}

impl CanonicalDocsRouter {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn route(&self, _query: &str, _search_paths: &[&Path]) -> Result<Vec<DocHit>> {
        // TODO: implement
        Ok(vec![])
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
