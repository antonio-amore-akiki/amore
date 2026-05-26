// QA B6 — concurrent SQLite-store write contention.
//
// Multiple SqliteStore instances open the SAME file and hammer it with
// insert_observation calls from independent tokio tasks. The headline
// scenario this models: a single obelion-mcp instance multiplexing N IDE
// clients (or two obelion processes on the same machine — a CLI + an MCP
// server — sharing the data dir). SQLite WAL mode + busy_timeout must
// prevent "database is locked" panics; final count must match the
// promised N * K rows; no duplicate observation hashes; no torn writes.
//
// Constants: 4 writers, 250 rows each = 1000 total. Modest; the test runs
// fast and proves the contract without flaking. Tests `journal_mode=WAL`
// + `busy_timeout=5s` (PRAGMAs set by SqliteStore::init_schema).

use obelion_core::sqlite_store::SqliteStore;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

fn fresh_tmp_db_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("obelion_b6_{nanos:x}_{n}.db"))
}

const WRITERS: usize = 4;
const ROWS_PER_WRITER: usize = 250;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_multi_writer_no_panics_correct_total() {
    let path = fresh_tmp_db_path();

    let mut handles = Vec::new();
    for writer_id in 0..WRITERS {
        let path = path.clone();
        handles.push(tokio::task::spawn_blocking(move || {
            // Each "process" gets its own SqliteStore (its own Mutex<Connection>
            // pointing at the same file). Models multi-process contention even
            // though we stay in one OS process for test ergonomics.
            let store =
                SqliteStore::open(&path).unwrap_or_else(|e| panic!("writer {writer_id} open: {e}"));
            for row in 0..ROWS_PER_WRITER {
                // Payload distinct per (writer, row) so each insert seals a
                // unique envelope hash. Otherwise dedup at the PRIMARY KEY
                // would silently swallow rows.
                let payload = serde_json::json!({
                    "writer": writer_id,
                    "row": row,
                    "text": format!("observation w{writer_id}r{row}"),
                });
                store
                    .insert_observation("b6_test", &payload)
                    .unwrap_or_else(|e| panic!("writer {writer_id} insert row {row}: {e}"));
            }
            writer_id
        }));
    }
    for h in handles {
        let id = h.await.expect("task panicked");
        eprintln!("[b6] writer {id} finished");
    }

    // Final count + dedup check via a fresh handle (no shared mutex with the
    // writers above — pure file-state assertion).
    let reader = SqliteStore::open(&path).expect("reader open");
    let count = reader.count_observations().expect("count");
    let expected = WRITERS as u64 * ROWS_PER_WRITER as u64;
    assert_eq!(
        count, expected,
        "expected {expected} rows, got {count} — concurrent writers lost data"
    );

    // chain integrity must hold across all the concurrent writers
    reader.verify_full_chain().expect("chain integrity");

    // Cleanup the tmp DB + WAL + shm sidecar files.
    let wal = PathBuf::from(format!("{}-wal", path.display()));
    let shm = PathBuf::from(format!("{}-shm", path.display()));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&wal);
    let _ = std::fs::remove_file(&shm);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_during_write_no_lock_panic() {
    // Smaller scenario: while writers insert, a reader hammers bm25_search.
    // Proves WAL allows concurrent readers + writer without SQLITE_BUSY.
    let path = fresh_tmp_db_path();

    // Pre-seed one row so bm25_search has SOMETHING to find.
    {
        let warmup = SqliteStore::open(&path).expect("warmup open");
        warmup
            .insert_observation("b6_test", &serde_json::json!({"text": "seed"}))
            .expect("seed insert");
    }

    let writer_path = path.clone();
    let writer = tokio::task::spawn_blocking(move || {
        let store = SqliteStore::open(&writer_path).expect("writer open");
        for i in 0..100 {
            store
                .insert_observation(
                    "b6_writer",
                    &serde_json::json!({"text": format!("write {i}")}),
                )
                .expect("write");
        }
    });

    let reader_path = path.clone();
    let reader = tokio::task::spawn_blocking(move || {
        let store = SqliteStore::open(&reader_path).expect("reader open");
        for _ in 0..100 {
            // bm25_search runs while writer is hammering inserts.
            let _ = store.bm25_search("write", 5).expect("read");
        }
    });

    writer.await.expect("writer panicked");
    reader.await.expect("reader panicked");

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
}

#[tokio::test]
async fn wal_mode_is_actually_set_on_file_backed_store() {
    // Locks the assumption: every file-backed SqliteStore has WAL on. Without
    // this, B6 contention degrades back to the default "rollback journal"
    // which serializes ALL access — readers blocked while writer holds the
    // file lock. Production posture requires WAL.
    let path = fresh_tmp_db_path();
    let store = SqliteStore::open(&path).expect("open");
    let mode = store
        .journal_mode()
        .expect("journal_mode query")
        .to_lowercase();
    assert_eq!(mode, "wal", "expected journal_mode=wal, got {mode:?}");
    drop(store);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", path.display())));
    let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", path.display())));
    // Avoid unused-import warning if Arc not referenced elsewhere in the file.
    let _: Arc<()> = Arc::new(());
}
