// SQLite store: observations, BM25 (FTS5), graph edges, ensemble votes, world-model.
// observations_fts uses porter+unicode61 tokenizer; bm25() returns negative
// scores (we flip in bm25_search so higher=better, matching cosine).
// Connection is Mutex-wrapped so SqliteStore is Sync for rmcp Arc-handlers.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

use crate::provenance::{Envelope, GENESIS_PREV_HASH, verify_chain};

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct Bm25Hit {
    pub id: String,
    pub score: f32,
    pub source: String,
    pub text: String,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_conn(Connection::open(path)?)
    }
    pub fn open_in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }
    fn from_conn(conn: Connection) -> Result<Self> {
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // WAL + NORMAL sync + 5s busy_timeout = production-grade
        // multi-process write contention behaviour (B6). In-memory DBs
        // reject journal_mode; that error is benign and ignored.
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS observations (
                id TEXT PRIMARY KEY,
                ts INTEGER NOT NULL,
                source TEXT NOT NULL,
                payload TEXT NOT NULL,
                prev_hash TEXT,
                hash TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_observations_ts ON observations(ts);

            CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
                id UNINDEXED,
                content,
                tokenize = 'porter unicode61'
            );

            CREATE TABLE IF NOT EXISTS ensemble_votes (
                ts INTEGER NOT NULL,
                decision_id TEXT NOT NULL,
                agent TEXT NOT NULL,
                position TEXT NOT NULL,
                rationale TEXT,
                weight REAL NOT NULL
            );

            CREATE TABLE IF NOT EXISTS world_model_kv (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                updated_ts INTEGER NOT NULL,
                PRIMARY KEY (namespace, key)
            );
            "#,
        )?;
        Ok(())
    }

    /// Insert a new observation: seals the payload into the chain after the
    /// current head, persists it, populates observations_fts for BM25.
    /// Indexable text comes from payload.text (if a string) else the canonical
    /// JSON itself.
    pub fn insert_observation(
        &self,
        source: &str,
        payload: &serde_json::Value,
    ) -> Result<Envelope> {
        // Read-head + write must be atomic across concurrent writers (B6) or
        // the chain forks. Hold the SQLite write lock from the start
        // (BEGIN IMMEDIATE), read the current head inside the same tx, then
        // seal + insert + commit. WAL + busy_timeout (set in init_schema)
        // ride on top for cross-process contention.
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let prev_hash: String = tx
            .query_row(
                "SELECT hash FROM observations ORDER BY ts DESC, id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());
        let env = Envelope::seal(&prev_hash, payload)?;
        let ts = now_unix_ms();
        let indexable = extract_indexable_text(payload, &env.canonical_json);
        tx.execute(
            "INSERT INTO observations (id, ts, source, payload, prev_hash, hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                env.id,
                ts,
                source,
                env.canonical_json,
                env.prev_hash,
                env.hash
            ],
        )?;
        tx.execute(
            "INSERT INTO observations_fts (id, content) VALUES (?1, ?2)",
            params![env.id, indexable],
        )?;
        tx.commit()?;
        Ok(env)
    }

    /// BM25 search over observations_fts. Returns up to `top_k` hits ranked
    /// by relevance (sign-flipped so higher = better, mirroring Qdrant cosine
    /// — keeps RRF math uniform across lanes). Empty/unsanitizable query -> no hits.
    pub fn bm25_search(&self, query: &str, top_k: u64) -> Result<Vec<Bm25Hit>> {
        // Sanitize: keep alphanumeric tokens only, AND-by-default (FTS5 treats
        // bare tokens conjunctively). Drops every FTS5 special char.
        let sanitized: String = query
            .split_whitespace()
            .map(|t| {
                t.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
            })
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if sanitized.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT o.id, o.source, f.content, bm25(observations_fts) AS rank \
             FROM observations_fts f \
             JOIN observations o ON o.id = f.id \
             WHERE observations_fts MATCH ?1 \
             ORDER BY rank \
             LIMIT ?2",
        )?;
        let hits = stmt
            .query_map(params![sanitized, top_k as i64], |row| {
                let raw_score: f64 = row.get(3)?;
                Ok(Bm25Hit {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    text: row.get(2)?,
                    // FTS5 returns negative; flip so higher=better.
                    score: (-raw_score) as f32,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(hits)
    }

    /// Return the active SQLite journal_mode (e.g. "wal", "memory"). Used by
    /// B6 + `amore doctor` to confirm WAL was negotiated.
    pub fn journal_mode(&self) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        Ok(mode)
    }

    /// Number of observations currently persisted. Cheap; useful for tests
    /// + status reporting.
    pub fn count_observations(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM observations", [], |row| row.get(0))?;
        Ok(n as u64)
    }

    /// Return the hash of the most recent observation (by ts DESC), or None
    /// if the chain is empty.
    pub fn last_observation_hash(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT hash FROM observations ORDER BY ts DESC, id DESC LIMIT 1")?;
        let hash: Option<String> = stmt.query_row([], |row| row.get(0)).optional()?;
        Ok(hash)
    }

    /// Walk the full chain in insertion order (ts ASC) and verify integrity +
    /// linkage end-to-end. Returns Err on the first broken link.
    pub fn verify_full_chain(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, prev_hash, payload, hash FROM observations ORDER BY ts ASC, id ASC",
        )?;
        let envelopes: Vec<Envelope> = stmt
            .query_map([], |row| {
                Ok(Envelope {
                    id: row.get(0)?,
                    prev_hash: row.get(1)?,
                    canonical_json: row.get(2)?,
                    hash: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        verify_chain(&envelopes)
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Extract the text payload that goes into the FTS5 index. Prefers an explicit
/// `text` string field on the payload object; falls back to the canonical_json
/// representation so even untyped payloads remain searchable by some token.
fn extract_indexable_text(payload: &serde_json::Value, canonical_json: &str) -> String {
    if let Some(t) = payload.get("text").and_then(|v| v.as_str()) {
        return t.to_string();
    }
    canonical_json.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store() -> SqliteStore {
        SqliteStore::open_in_memory().unwrap()
    }

    #[test]
    fn first_observation_chains_off_genesis() {
        let store = temp_store();
        let env = store.insert_observation("test", &json!({"x": 1})).unwrap();
        assert_eq!(env.prev_hash, GENESIS_PREV_HASH);
        store
            .verify_full_chain()
            .expect("single-entry chain must verify");
    }

    #[test]
    fn three_observations_link_correctly() {
        let store = temp_store();
        store
            .insert_observation("test", &json!({"step": 1}))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        store
            .insert_observation("test", &json!({"step": 2}))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        store
            .insert_observation("test", &json!({"step": 3}))
            .unwrap();
        store
            .verify_full_chain()
            .expect("three-entry chain must verify");
    }

    #[test]
    fn tamper_with_payload_breaks_chain() {
        let store = temp_store();
        store
            .insert_observation("test", &json!({"step": 1}))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let env2 = store
            .insert_observation("test", &json!({"step": 2}))
            .unwrap();
        // Tamper with the persisted payload of env2 directly.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE observations SET payload = ?1 WHERE id = ?2",
                params![r#"{"step":99}"#, env2.id],
            )
            .unwrap();
        let result = store.verify_full_chain();
        assert!(
            result.is_err(),
            "tampered payload must fail verify_full_chain"
        );
    }

    #[test]
    fn empty_store_verifies() {
        let store = temp_store();
        store
            .verify_full_chain()
            .expect("empty chain is vacuously valid");
    }

    #[test]
    fn last_hash_tracks_head() {
        let store = temp_store();
        assert!(store.last_observation_hash().unwrap().is_none());
        let env = store.insert_observation("test", &json!({"x": 1})).unwrap();
        assert_eq!(store.last_observation_hash().unwrap(), Some(env.hash));
    }

    #[test]
    fn bm25_empty_index_returns_no_hits() {
        let store = temp_store();
        let hits = store.bm25_search("rust async", 5).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn bm25_indexes_observation_text_field() {
        let store = temp_store();
        store
            .insert_observation(
                "user_prompt",
                &json!({"text":"Rust tokio async runtime and the await keyword for futures"}),
            )
            .unwrap();
        store
            .insert_observation(
                "user_prompt",
                &json!({"text":"Baking chocolate chip cookies needs flour, butter, sugar, eggs"}),
            )
            .unwrap();
        let hits = store.bm25_search("rust async", 5).unwrap();
        assert!(!hits.is_empty(), "expected at least one BM25 hit");
        let top = &hits[0];
        assert!(
            top.text.contains("Rust") || top.text.contains("async"),
            "BM25 top hit should mention rust/async, got: {}",
            top.text
        );
        assert!(top.score > 0.0, "flipped BM25 score must be positive");
    }

    #[test]
    fn bm25_ranks_more_relevant_higher() {
        let store = temp_store();
        store
            .insert_observation("doc", &json!({"text":"rust rust rust async networking"}))
            .unwrap();
        store
            .insert_observation(
                "doc",
                &json!({"text":"javascript node npm package management"}),
            )
            .unwrap();
        let hits = store.bm25_search("rust async networking", 5).unwrap();
        assert!(!hits.is_empty());
        let top = &hits[0];
        assert!(
            top.text.contains("rust") && top.text.contains("async"),
            "top BM25 hit must be the rust/async doc, got: {}",
            top.text
        );
    }

    #[test]
    fn bm25_returns_indexed_text_and_source() {
        let store = temp_store();
        store
            .insert_observation(
                "edit_log",
                &json!({"text":"refactor authentication module"}),
            )
            .unwrap();
        let hits = store.bm25_search("authentication", 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].source, "edit_log");
        assert!(hits[0].text.contains("authentication"));
    }

    #[test]
    fn count_observations_tracks_inserts() {
        let store = temp_store();
        assert_eq!(store.count_observations().unwrap(), 0);
        store.insert_observation("t", &json!({"text":"a"})).unwrap();
        store.insert_observation("t", &json!({"text":"b"})).unwrap();
        assert_eq!(store.count_observations().unwrap(), 2);
    }
}
