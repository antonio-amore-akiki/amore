// SQLite store: observations, BM25 (FTS5), graph edges, ensemble votes, world-model.
// Single file db, portable, embeds in-process.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

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
}
