// tantivy_parity.rs — H.1 integration test: Tantivy BM25 rank-order parity with SQLite FTS5 baseline.
//
// Loads tests/fixtures/bm25_baseline.json (frozen FTS5 fixture).
// Indexes the same 50-doc corpus through TantivyIndex.
// For each of the 20 fixture queries, asserts:
//   1. Top-N rank order (doc_id sequence) is identical to the FTS5 baseline.
//   2. BM25 scores are positive and descending (sanity check for Tantivy output).
//
// Score NUMERIC values are NOT compared to FTS5 scores — different BM25 implementations
// produce different numbers for equivalent rankings (SQLite's bm25() returns negative values
// that sqlite_store.rs sign-flips; tantivy is always positive; both use k1=1.2, b=0.75).
// The parity assertion is rank ORDER only, which is the production-relevant invariant.
//
// Doc ID mapping: fixture uses string labels "obs-NNN"; TantivyIndex uses u64.
// We assign u64 = NNN (0..=49) deterministically by parsing the suffix.

#![allow(clippy::unwrap_used)]

use amore_core::tantivy_index::TantivyIndex;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Corpus — identical to bm25_fts5_baseline.rs to ensure same documents are indexed.
// Tuple: (logical_id, source, text)
// ---------------------------------------------------------------------------
fn corpus() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // --- programming: Rust -------------------------------------------------
        (
            "obs-000",
            "rust_docs",
            "Rust ownership model prevents memory leaks by enforcing borrow checking at compile time without a garbage collector",
        ),
        (
            "obs-001",
            "rust_docs",
            "Tokio is an asynchronous runtime for Rust providing async await syntax and the executor for futures",
        ),
        (
            "obs-002",
            "rust_docs",
            "Cargo is the Rust package manager and build tool managing dependencies crates and workspaces",
        ),
        (
            "obs-003",
            "rust_docs",
            "Rust lifetime annotations ensure references never outlive the data they point to avoiding dangling pointers",
        ),
        (
            "obs-004",
            "rust_docs",
            "The Rust trait system enables polymorphism through interfaces that types can implement at compile time",
        ),
        (
            "obs-005",
            "rust_docs",
            "Error handling in Rust uses the Result and Option enums rather than exceptions making failures explicit",
        ),
        (
            "obs-006",
            "rust_docs",
            "Rust macros like println macro_rules and proc_macros extend syntax and reduce boilerplate at compile time",
        ),
        (
            "obs-007",
            "rust_docs",
            "Unsafe Rust allows raw pointer dereferencing FFI calls and unsafe trait implementations bypassing the borrow checker",
        ),
        // --- programming: Python -----------------------------------------------
        (
            "obs-008",
            "python_docs",
            "Python async await coroutines and the asyncio event loop enable concurrent IO without threads",
        ),
        (
            "obs-009",
            "python_docs",
            "Python decorators wrap functions adding behavior before or after execution using the at sign syntax",
        ),
        (
            "obs-010",
            "python_docs",
            "NumPy provides N-dimensional array operations for numerical Python computing with optimized C backend",
        ),
        (
            "obs-011",
            "python_docs",
            "Python generators yield values lazily enabling memory efficient iteration over large datasets",
        ),
        (
            "obs-012",
            "python_docs",
            "Type hints in Python allow static analysis with mypy and improve IDE code completion and documentation",
        ),
        // --- programming: SQL --------------------------------------------------
        (
            "obs-013",
            "sql_docs",
            "SQL window functions like ROW_NUMBER RANK and DENSE_RANK compute values across a partition without collapsing rows",
        ),
        (
            "obs-014",
            "sql_docs",
            "PostgreSQL full text search uses tsvector and tsquery with GIN indexes for BM25 style relevance ranking",
        ),
        (
            "obs-015",
            "sql_docs",
            "SQLite FTS5 virtual tables support BM25 ranking through the bm25 auxiliary function returning negative scores",
        ),
        (
            "obs-016",
            "sql_docs",
            "Database indexes speed up SELECT queries by maintaining a sorted copy of column values avoiding full table scans",
        ),
        (
            "obs-017",
            "sql_docs",
            "Transactions in SQL ensure atomicity consistency isolation and durability through ACID properties",
        ),
        // --- programming: JavaScript -------------------------------------------
        (
            "obs-018",
            "js_docs",
            "JavaScript promises and async await simplify asynchronous programming replacing callback hell patterns",
        ),
        (
            "obs-019",
            "js_docs",
            "React hooks like useState and useEffect allow functional components to manage state and side effects",
        ),
        (
            "obs-020",
            "js_docs",
            "Node.js event loop processes IO callbacks in a single thread using libuv for non-blocking operations",
        ),
        (
            "obs-021",
            "js_docs",
            "TypeScript adds static types to JavaScript enabling better tooling refactoring and compile time error detection",
        ),
        // --- general prose: weather --------------------------------------------
        (
            "obs-022",
            "weather",
            "The forecast calls for partly cloudy skies with a high of 24 degrees Celsius and light northwest winds",
        ),
        (
            "obs-023",
            "weather",
            "Thunderstorms are expected in the afternoon with heavy rainfall and possible flash flooding in low lying areas",
        ),
        (
            "obs-024",
            "weather",
            "A cold front will bring temperatures down by ten degrees over the next 48 hours with snow above 1500 meters",
        ),
        // --- general prose: food -----------------------------------------------
        (
            "obs-025",
            "food",
            "Sourdough bread requires a live starter culture fed with flour and water to ferment naturally over 12 hours",
        ),
        (
            "obs-026",
            "food",
            "Thai green curry balances coconut milk lemongrass galangal kaffir lime leaves and fresh green chilies",
        ),
        (
            "obs-027",
            "food",
            "Chocolate tempering requires precise temperature control cycling between 50 27 and 31 degrees Celsius",
        ),
        // --- general prose: travel ---------------------------------------------
        (
            "obs-028",
            "travel",
            "Tokyo Shinjuku district offers neon lit streets izakayas department stores and the famous Kabukicho entertainment area",
        ),
        (
            "obs-029",
            "travel",
            "The Paris metro runs until 1am on weekdays and 2am on weekends connecting all 20 arrondissements",
        ),
        (
            "obs-030",
            "travel",
            "Hiking the Inca Trail to Machu Picchu requires a permit booked months in advance and takes four days",
        ),
        // --- edge cases: very long document ------------------------------------
        (
            "obs-031",
            "long_doc",
            "machine learning neural networks deep learning convolutional recurrent transformer attention mechanism gradient descent backpropagation stochastic gradient optimizer Adam SGD learning rate scheduler batch normalization dropout regularization overfitting underfitting bias variance tradeoff cross validation hyperparameter tuning feature engineering preprocessing normalization standardization one hot encoding embedding layer word2vec bert gpt llm fine tuning transfer learning zero shot few shot prompt engineering chain of thought retrieval augmented generation vector database embedding similarity cosine distance dot product faiss qdrant pinecone weaviate chroma milvus",
        ),
        // --- edge cases: single token ------------------------------------------
        ("obs-032", "single", "rust"),
        ("obs-033", "single", "python"),
        ("obs-034", "single", "async"),
        // --- edge cases: unicode -----------------------------------------------
        (
            "obs-035",
            "unicode",
            "Tokyo is written as 東京 in Japanese and is the capital of Japan with a population of over 13 million",
        ),
        (
            "obs-036",
            "unicode",
            "café au lait is a French coffee drink made with equal parts espresso and steamed milk",
        ),
        (
            "obs-037",
            "unicode",
            "Привет мир means hello world in Russian and Cyrillic script is used across Eastern Europe",
        ),
        // --- edge cases: numbers -----------------------------------------------
        (
            "obs-038",
            "network",
            "The server listens on port 6334 for gRPC and port 6333 for HTTP requests from Qdrant clients",
        ),
        (
            "obs-039",
            "network",
            "IPv6 addresses use 128 bit notation like 2001 0db8 0000 0000 0000 0000 0000 0001 for unique host identification",
        ),
        // --- edge cases: FTS5 metacharacters (sanitizer should handle) ----------
        (
            "obs-040",
            "adversarial",
            "SELECT star FROM table WHERE id EQUALS 1 OR 1 EQUALS 1 SEMICOLON DROP TABLE users SEMICOLON",
        ),
        (
            "obs-041",
            "adversarial",
            "query AND NOT OR NEAR tokens MATCH FTS5 RESERVED WORDS should be sanitized before indexing",
        ),
        (
            "obs-042",
            "adversarial",
            "caret dollar dot star plus question open_paren close_paren backslash regex metacharacters test",
        ),
        // --- edge cases: SQL injection shaped ----------------------------------
        (
            "obs-043",
            "adversarial",
            "username equals admin APOSTROPHE OR APOSTROPHE 1 EQUALS APOSTROPHE 1 classic SQL injection pattern",
        ),
        (
            "obs-044",
            "adversarial",
            "UNION SELECT password FROM users WHERE username EQUALS admin injection attempt for testing sanitizer",
        ),
        // --- mixed programming + prose -----------------------------------------
        (
            "obs-045",
            "mixed",
            "Building a REST API with Rust and Axum framework requires defining route handlers serializing JSON responses",
        ),
        (
            "obs-046",
            "mixed",
            "Python FastAPI uses Pydantic models for request validation and automatic OpenAPI documentation generation",
        ),
        (
            "obs-047",
            "mixed",
            "Deploying containers with Docker Compose orchestrates multiple services with networking volumes and health checks",
        ),
        // --- version / semver shaped -------------------------------------------
        (
            "obs-048",
            "versions",
            "Upgrading from amore v0.3.0 to v0.4.0 adds hybrid BM25 vector recall with RRF fusion at k equals 60",
        ),
        // --- deliberately short ------------------------------------------------
        ("obs-049", "short", "hi"),
    ]
}

// ---------------------------------------------------------------------------
// Fixture types
// ---------------------------------------------------------------------------
#[derive(Debug, serde::Deserialize)]
struct RankEntry {
    doc_id: String,
    #[allow(dead_code)]
    score: f32,
    #[allow(dead_code)]
    rank: usize,
}

#[derive(Debug, serde::Deserialize)]
struct QueryResult {
    id: String,
    query: String,
    expected_ranks: Vec<RankEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct Fixture {
    #[allow(dead_code)]
    schema_version: u32,
    queries: Vec<QueryResult>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("bm25_baseline.json")
}

/// Build a RAM-backed TantivyIndex loaded with the 50-doc corpus.
/// u64 doc_id = NNN from "obs-NNN".
fn build_index() -> TantivyIndex {
    let mut idx =
        TantivyIndex::new(std::path::Path::new(":memory:")).expect("create in-memory TantivyIndex");
    for (logical_id, _source, text) in corpus() {
        let n: u64 = logical_id
            .split('-')
            .next_back()
            .expect("corpus id has dash separator")
            .parse()
            .expect("corpus id numeric suffix is valid u64");
        idx.add(n, text).expect("add doc to TantivyIndex");
    }
    idx.commit().expect("commit TantivyIndex");
    idx
}

fn id_to_label(id: u64) -> String {
    format!("obs-{:03}", id)
}

// ---------------------------------------------------------------------------
// Main parity test
// ---------------------------------------------------------------------------

#[test]
fn tantivy_rank_order_matches_fts5_baseline() {
    let fixture_path = fixture_path();
    let raw = std::fs::read_to_string(&fixture_path).unwrap_or_else(|_| {
        panic!(
            "fixture not found at {}; regenerate with AMORE_BM25_REBASE=1",
            fixture_path.display()
        )
    });
    let fixture: Fixture = serde_json::from_str(&raw).expect("parse bm25_baseline.json");

    let idx = build_index();

    let mut passed = 0usize;
    let total = fixture.queries.len();

    for qr in &fixture.queries {
        let expected_ids: Vec<&str> = qr
            .expected_ranks
            .iter()
            .map(|e| e.doc_id.as_str())
            .collect();
        let top_k = expected_ids.len().max(10);

        let hits = idx
            .search(&qr.query, top_k)
            .unwrap_or_else(|e| panic!("search failed for query {}: {e}", qr.id));

        if expected_ids.is_empty() {
            // Fixture expects zero hits — Tantivy must also return zero.
            assert!(
                hits.is_empty(),
                "query {} ({:?}): FTS5 baseline expects 0 hits; Tantivy returned {} hit(s): {:?}",
                qr.id,
                qr.query,
                hits.len(),
                hits.iter()
                    .map(|(id, s)| (id_to_label(*id), s))
                    .collect::<Vec<_>>()
            );
            passed += 1;
            continue;
        }

        // Non-empty: verify rank order for the fixture's top-N results.
        let got_labels: Vec<String> = hits
            .iter()
            .take(expected_ids.len())
            .map(|(id, _)| id_to_label(*id))
            .collect();

        assert_eq!(
            got_labels, expected_ids,
            "query {} ({:?}): rank order mismatch\n  FTS5 expected: {:?}\n  Tantivy got:   {:?}",
            qr.id, qr.query, expected_ids, got_labels
        );

        // Tantivy scores must be positive and descending.
        for pair in hits.windows(2) {
            assert!(
                pair[0].1 >= pair[1].1,
                "query {}: scores not descending: {} vs {}",
                qr.id,
                pair[0].1,
                pair[1].1
            );
        }
        for (_, score) in &hits {
            assert!(
                *score > 0.0,
                "query {}: tantivy BM25 score must be positive, got {}",
                qr.id,
                score
            );
        }

        passed += 1;
    }

    println!(
        "tantivy_parity: {}/{} queries passed rank-order parity check",
        passed, total
    );
    assert_eq!(
        passed, total,
        "not all queries passed rank-order parity check"
    );
}

// ---------------------------------------------------------------------------
// Sanity tests (no fixture required)
// ---------------------------------------------------------------------------

#[test]
fn corpus_has_50_docs() {
    assert_eq!(corpus().len(), 50);
}

#[test]
fn empty_query_returns_no_hits() {
    let idx = build_index();
    let hits = idx.search("", 10).unwrap();
    assert!(hits.is_empty(), "empty query must return no hits");
}

#[test]
fn whitespace_query_returns_no_hits() {
    let idx = build_index();
    let hits = idx.search("   \t  ", 10).unwrap();
    assert!(hits.is_empty(), "whitespace-only query must return no hits");
}

#[test]
fn adversarial_query_does_not_panic() {
    let idx = build_index();
    let result = idx.search("SELECT * FROM users; DROP TABLE--", 5);
    assert!(result.is_ok(), "adversarial query must not return Err");
}

#[test]
fn single_token_rust_top_hit_is_single_token_doc() {
    let idx = build_index();
    let hits = idx.search("rust", 10).unwrap();
    assert!(
        !hits.is_empty(),
        "'rust' query must return at least one hit"
    );
    // obs-032 is the single-token "rust" doc — highest IDF density, must rank first.
    assert_eq!(
        id_to_label(hits[0].0),
        "obs-032",
        "single-token 'rust' doc (obs-032) should rank first for query 'rust'"
    );
}
