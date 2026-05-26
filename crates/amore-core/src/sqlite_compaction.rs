// sqlite_compaction.rs — compaction-only helper methods on SqliteStore (H.9).
//
// Placed in a separate file to respect the sqlite_store.rs 400-line cap.
// All methods are `pub(crate)` — only `compaction.rs` uses them.

use anyhow::Result;
use rusqlite::params;
use std::time::Duration;

use crate::sqlite_store::SqliteStore;

impl SqliteStore {
    /// Return doc_ids of stale duplicate observations (same payload hash)
    /// inserted within `window_secs` of now, excluding the newest per group.
    pub(crate) fn compaction_find_stale_duplicates(
        &self,
        window_secs: u64,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable");
        let cutoff_ms: i64 = now_unix_ms() - (window_secs as i64 * 1000);
        let mut stmt = conn.prepare(
            "SELECT o.id \
             FROM observations o \
             WHERE o.ts >= ?1 \
             AND o.hash IN ( \
                 SELECT hash FROM observations \
                 WHERE ts >= ?1 \
                 GROUP BY hash \
                 HAVING COUNT(*) > 1 \
             ) \
             AND o.ts < ( \
                 SELECT MAX(ts) FROM observations o2 \
                 WHERE o2.hash = o.hash \
             )",
        )?;
        let ids: Vec<String> = stmt
            .query_map(params![cutoff_ms], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }

    /// Return doc_ids of observations older than `max_age`.
    pub(crate) fn compaction_find_aged_rows(&self, max_age: Duration) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable");
        let cutoff_ms: i64 = now_unix_ms() - (max_age.as_millis() as i64);
        let mut stmt = conn.prepare("SELECT id FROM observations WHERE ts < ?1")?;
        let ids: Vec<String> = stmt
            .query_map(params![cutoff_ms], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }

    /// Delete observations (and their FTS shadows) by id list.
    pub(crate) fn compaction_delete_by_ids(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("mutex poisoned: unrecoverable");
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM observations_fts WHERE id = ?1", params![id])?;
            tx.execute("DELETE FROM observations WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Run `PRAGMA incremental_vacuum(N)` and return approximate bytes freed.
    pub(crate) fn compaction_incremental_vacuum(&self, pages: u64) -> Result<u64> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable");
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        let freelist_before: i64 =
            conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        conn.execute_batch(&format!("PRAGMA incremental_vacuum({pages})"))?;
        let freelist_after: i64 =
            conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        let reclaimed = (freelist_before - freelist_after).max(0);
        Ok((reclaimed * page_size) as u64)
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
