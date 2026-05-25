// SQLite store: observations, BM25 (FTS5), graph edges, ensemble votes, world-model.
// Single file db, portable, embeds in-process.
//
// The observations table is the source-of-truth for the provenance chain
// (see crate::provenance). Every insert seals an Envelope, persists the
// (id, ts, source, canonical_json, prev_hash, hash) tuple, and returns the
// envelope so callers can pin the new chain head.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

use crate::provenance::{Envelope, GENESIS_PREV_HASH, verify_chain};

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
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

    /// Insert a new observation. Seals the payload into the chain after the
    /// current head; returns the new envelope so the caller can keep tracking
    /// the chain without re-querying.
    pub fn insert_observation(
        &self,
        source: &str,
        payload: &serde_json::Value,
    ) -> Result<Envelope> {
        let prev_hash = self
            .last_observation_hash()?
            .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());
        let env = Envelope::seal(&prev_hash, payload)?;
        let ts = now_unix_ms();
        self.conn.execute(
            "INSERT INTO observations (id, ts, source, payload, prev_hash, hash) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![env.id, ts, source, env.canonical_json, env.prev_hash, env.hash],
        )?;
        Ok(env)
    }

    /// Return the hash of the most recent observation (by ts DESC), or None
    /// if the chain is empty.
    pub fn last_observation_hash(&self) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT hash FROM observations ORDER BY ts DESC, id DESC LIMIT 1")?;
        let hash: Option<String> = stmt.query_row([], |row| row.get(0)).optional()?;
        Ok(hash)
    }

    /// Walk the full chain in insertion order (ts ASC) and verify integrity +
    /// linkage end-to-end. Returns Err on the first broken link.
    pub fn verify_full_chain(&self) -> Result<()> {
        let mut stmt = self.conn.prepare(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store() -> SqliteStore {
        let conn = Connection::open_in_memory().unwrap();
        let store = SqliteStore { conn };
        store.init_schema().unwrap();
        store
    }

    #[test]
    fn first_observation_chains_off_genesis() {
        let store = temp_store();
        let env = store
            .insert_observation("test", &json!({"x": 1}))
            .unwrap();
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
            .execute(
                "UPDATE observations SET payload = ?1 WHERE id = ?2",
                params![r#"{"step":99}"#, env2.id],
            )
            .unwrap();
        let result = store.verify_full_chain();
        assert!(result.is_err(), "tampered payload must fail verify_full_chain");
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
        let env = store
            .insert_observation("test", &json!({"x": 1}))
            .unwrap();
        assert_eq!(store.last_observation_hash().unwrap(), Some(env.hash));
    }
}
