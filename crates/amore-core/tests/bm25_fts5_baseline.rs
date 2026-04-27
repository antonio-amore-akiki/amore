// Characterization test: SQLite FTS5 BM25 lane — H.0 regression gate.
//
// PURPOSE: freeze the exact ranked output of bm25_search() on a fixed corpus
// so the Tantivy migration (H.1) must prove byte-identical parity.
//
// PATTERN:
//   First run (or AMORE_BM25_REBASE=1): ingest corpus -> run queries -> write
//   fixture JSON to tests/fixtures/bm25_baseline.json.
//   Subsequent runs: compare against that frozen fixture.
//
// CORPUS: 50 documents with deterministic IDs obs-000..obs-049 spanning
//   programming, prose, edge cases (empty, unicode, very long, single-token),
//   and adversarial content (FTS5 metacharacters, SQL-injection-shaped strings).
//
// QUERIES: 20 fixed queries spanning single-token, multi-token, unicode,
//   number-heavy, punctuation-heavy, empty/whitespace-only, and a ~200-token
//   long query.

#![allow(clippy::unwrap_used)]

use amore_core::sqlite_store::SqliteStore;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Corpus: 50 documents, deterministic IDs obs-000..obs-049
// Each tuple is (doc_id_suffix, source, text)
// ---------------------------------------------------------------------------
fn corpus() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // --- programming: Rust -------------------------------------------------
        ("obs-000", "rust_docs", "Rust ownership model prevents memory leaks by enforcing borrow checking at compile time without a garbage collector"),
        ("obs-001", "rust_docs", "Tokio is an asynchronous runtime for Rust providing async await syntax and the executor for futures"),
        ("obs-002", "rust_docs", "Cargo is the Rust package manager and build tool managing dependencies crates and workspaces"),
        ("obs-003", "rust_docs", "Rust lifetime annotations ensure references never outlive the data they point to avoiding dangling pointers"),
        ("obs-004", "rust_docs", "The Rust trait system enables polymorphism through interfaces that types can implement at compile time"),
        ("obs-005", "rust_docs", "Error handling in Rust uses the Result and Option enums rather than exceptions making failures explicit"),
        ("obs-006", "rust_docs", "Rust macros like println macro_rules and proc_macros extend syntax and reduce boilerplate at compile time"),
        ("obs-007", "rust_docs", "Unsafe Rust allows raw pointer dereferencing FFI calls and unsafe trait implementations bypassing the borrow checker"),
        // --- programming: Python -----------------------------------------------
        ("obs-008", "python_docs", "Python async await coroutines and the asyncio event loop enable concurrent IO without threads"),
        ("obs-009", "python_docs", "Python decorators wrap functions adding behavior before or after execution using the at sign syntax"),
        ("obs-010", "python_docs", "NumPy provides N-dimensional array operations for numerical Python computing with optimized C backend"),
        ("obs-011", "python_docs", "Python generators yield values lazily enabling memory efficient iteration over large datasets"),
        ("obs-012", "python_docs", "Type hints in Python allow static analysis with mypy and improve IDE code completion and documentation"),
        // --- programming: SQL --------------------------------------------------
        ("obs-013", "sql_docs", "SQL window functions like ROW_NUMBER RANK and DENSE_RANK compute values across a partition without collapsing rows"),
        ("obs-014", "sql_docs", "PostgreSQL full text search uses tsvector and tsquery with GIN indexes for BM25 style relevance ranking"),
        ("obs-015", "sql_docs", "SQLite FTS5 virtual tables support BM25 ranking through the bm25 auxiliary function returning negative scores"),
        ("obs-016", "sql_docs", "Database indexes speed up SELECT queries by maintaining a sorted copy of column values avoiding full table scans"),
        ("obs-017", "sql_docs", "Transactions in SQL ensure atomicity consistency isolation and durability through ACID properties"),
        // --- programming: JavaScript -------------------------------------------
        ("obs-018", "js_docs", "JavaScript promises and async await simplify asynchronous programming replacing callback hell patterns"),
        ("obs-019", "js_docs", "React hooks like useState and useEffect allow functional components to manage state and side effects"),
        ("obs-020", "js_docs", "Node.js event loop processes IO callbacks in a single thread using libuv for non-blocking operations"),
        ("obs-021", "js_docs", "TypeScript adds static types to JavaScript enabling better tooling refactoring and compile time error detection"),
        // --- general prose: weather --------------------------------------------
        ("obs-022", "weather", "The forecast calls for partly cloudy skies with a high of 24 degrees Celsius and light northwest winds"),
        ("obs-023", "weather", "Thunderstorms are expected in the afternoon with heavy rainfall and possible flash flooding in low lying areas"),
        ("obs-024", "weather", "A cold front will bring temperatures down by ten degrees over the next 48 hours with snow above 1500 meters"),
        // --- general prose: food -----------------------------------------------
        ("obs-025", "food", "Sourdough bread requires a live starter culture fed with flour and water to ferment naturally over 12 hours"),
        ("obs-026", "food", "Thai green curry balances coconut milk lemongrass galangal kaffir lime leaves and fresh green chilies"),
        ("obs-027", "food", "Chocolate tempering requires precise temperature control cycling between 50 27 and 31 degrees Celsius"),
        // --- general prose: travel ---------------------------------------------
        ("obs-028", "travel", "Tokyo Shinjuku district offers neon lit streets izakayas department stores and the famous Kabukicho entertainment area"),
        ("obs-029", "travel", "The Paris metro runs until 1am on weekdays and 2am on weekends connecting all 20 arrondissements"),
        ("obs-030", "travel", "Hiking the Inca Trail to Machu Picchu requires a permit booked months in advance and takes four days"),
        // --- edge cases: very long document ------------------------------------
        ("obs-031", "long_doc", "machine learning neural networks deep learning convolutional recurrent transformer attention mechanism gradient descent backpropagation stochastic gradient optimizer Adam SGD learning rate scheduler batch normalization dropout regularization overfitting underfitting bias variance tradeoff cross validation hyperparameter tuning feature engineering preprocessing normalization standardization one hot encoding embedding layer word2vec bert gpt llm fine tuning transfer learning zero shot few shot prompt engineering chain of thought retrieval augmented generation vector database embedding similarity cosine distance dot product faiss qdrant pinecone weaviate chroma milvus"),
        // --- edge cases: single token ------------------------------------------
        ("obs-032", "single", "rust"),
        ("obs-033", "single", "python"),
        ("obs-034", "single", "async"),
        // --- edge cases: unicode -----------------------------------------------
        ("obs-035", "unicode", "Tokyo is written as 東京 in Japanese and is the capital of Japan with a population of over 13 million"),
        ("obs-036", "unicode", "café au lait is a French coffee drink made with equal parts espresso and steamed milk"),
        ("obs-037", "unicode", "Привет мир means hello world in Russian and Cyrillic script is used across Eastern Europe"),
        // --- edge cases: numbers -----------------------------------------------
        ("obs-038", "network", "The server listens on port 6334 for gRPC and port 6333 for HTTP requests from Qdrant clients"),
        ("obs-039", "network", "IPv6 addresses use 128 bit notation like 2001 0db8 0000 0000 0000 0000 0000 0001 for unique host identification"),
        // --- edge cases: FTS5 metacharacters (sanitizer should handle) ----------
        ("obs-040", "adversarial", "SELECT star FROM table WHERE id EQUALS 1 OR 1 EQUALS 1 SEMICOLON DROP TABLE users SEMICOLON"),
        ("obs-041", "adversarial", "query AND NOT OR NEAR tokens MATCH FTS5 RESERVED WORDS should be sanitized before indexing"),
        ("obs-042", "adversarial", "caret dollar dot star plus question open_paren close_paren backslash regex metacharacters test"),
        // --- edge cases: SQL injection shaped ----------------------------------
        ("obs-043", "adversarial", "username equals admin APOSTROPHE OR APOSTROPHE 1 EQUALS APOSTROPHE 1 classic SQL injection pattern"),
        ("obs-044", "adversarial", "UNION SELECT password FROM users WHERE username EQUALS admin injection attempt for testing sanitizer"),
        // --- mixed programming + prose -----------------------------------------
        ("obs-045", "mixed", "Building a REST API with Rust and Axum framework requires defining route handlers serializing JSON responses"),
        ("obs-046", "mixed", "Python FastAPI uses Pydantic models for request validation and automatic OpenAPI documentation generation"),
        ("obs-047", "mixed", "Deploying containers with Docker Compose orchestrates multiple services with networking volumes and health checks"),
        // --- version / semver shaped -------------------------------------------
        ("obs-048", "versions", "Upgrading from amore v0.3.0 to v0.4.0 adds hybrid BM25 vector recall with RRF fusion at k equals 60"),
        // --- deliberately short ------------------------------------------------
        ("obs-049", "short", "hi"),
    ]
}

// ---------------------------------------------------------------------------
// Queries: 20 fixed queries
// ---------------------------------------------------------------------------
fn queries() -> Vec<(&'static str, &'static str)> {
    vec![
        ("q01", "rust"),
        ("q02", "python async"),
        ("q03", "quick brown fox"),
        ("q04", "async await runtime"),
        ("q05", ""),                  // empty — should return no hits
        ("q06", "   \t  "),           // whitespace only — no hits
        ("q07", "東京"),              // unicode CJK token
        ("q08", "café"),              // unicode Latin extended
        ("q09", "port 6334"),         // number-heavy
        ("q10", "v0.4.0 --help"),     // punctuation-heavy (sanitizer strips dashes)
        ("q11", "SELECT FROM WHERE"), // SQL-shaped — sanitizer keeps tokens
        ("q12", "BM25 FTS5 ranking"),
        ("q13", "machine learning transformer attention"),
        ("q14", "docker container deployment"),
        ("q15", "thai curry coconut milk"),
        ("q16", "rust ownership borrow lifetime"),
        ("q17", "error handling result option"),
        ("q18", "neural network deep learning"),
        ("q19", "react hooks useState useEffect"),
        // long query (~200 tokens) — must not crash
        ("q20", "rust python sql javascript typescript async await future promise trait interface generics ownership lifetime borrow checker error handling result option enum struct impl method closure iterator filter map fold collect vec hashmap string slice reference pointer unsafe ffi tokio rayon serde json yaml toml axum actix rocket warp hyper tonic reqwest sqlx diesel sea_orm migration schema table column index transaction constraint join aggregate window function subquery view trigger procedure function stored procedure database postgres sqlite mysql redis kafka rabbitmq grpc rest graphql websocket http2 tls certificate authentication authorization oauth jwt session cookie csrf cors rate_limit middleware logging tracing metrics prometheus grafana docker kubernetes helm terraform ansible ci cd github actions jenkins pipeline artifact registry container image layer volume network bridge host port mapping healthcheck liveness readiness probe ingress service deployment statefulset daemonset configmap secret namespace label selector annotation rbac role binding serviceaccount")
    ]
}

// ---------------------------------------------------------------------------
// Fixture path (sibling of this file in tests/fixtures/)
// ---------------------------------------------------------------------------
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("bm25_baseline.json")
}

// ---------------------------------------------------------------------------
// Data types for the fixture
// ---------------------------------------------------------------------------
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RankEntry {
    doc_id: String,
    score: f32,
    rank: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct QueryResult {
    id: String,
    query: String,
    expected_ranks: Vec<RankEntry>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Fixture {
    schema_version: u32,
    queries: Vec<QueryResult>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Open a fresh in-memory store, ingest the full corpus, and return both the
/// store AND an envelope_id -> logical_doc_id map built from the same
/// insertions. Using the same store ensures the envelope IDs in the map match
/// those returned by bm25_search() — they're the same DB connection.
fn build_store_and_id_map() -> (SqliteStore, HashMap<String, String>) {
    let store = SqliteStore::open_in_memory().expect("in-memory store failed");
    let mut id_map = HashMap::new();
    for (logical_id, source, text) in corpus() {
        let env = store
            .insert_observation(source, &json!({"text": text}))
            .expect("insert failed");
        id_map.insert(env.id, logical_id.to_string());
    }
    (store, id_map)
}

/// Convenience wrapper for tests that only need a populated store.
fn build_store() -> SqliteStore {
    build_store_and_id_map().0
}

fn run_queries(store: &SqliteStore, id_map: &HashMap<String, String>) -> Vec<QueryResult> {
    let mut results = Vec::new();
    for (qid, query) in queries() {
        let hits = store
            .bm25_search(query, 20)
            .expect("bm25_search failed");
        let expected_ranks: Vec<RankEntry> = hits
            .into_iter()
            .enumerate()
            .map(|(i, hit)| {
                let logical_id = id_map
                    .get(&hit.id)
                    .cloned()
                    .unwrap_or_else(|| hit.id.clone());
                RankEntry {
                    doc_id: logical_id,
                    score: hit.score,
                    rank: i + 1,
                }
            })
            .collect();
        results.push(QueryResult {
            id: qid.to_string(),
            query: query.to_string(),
            expected_ranks,
        });
    }
    results
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------
#[test]
fn bm25_fts5_baseline() {
    let rebase = std::env::var("AMORE_BM25_REBASE")
        .or_else(|_| std::env::var("OBELION_BM25_REBASE"))
        .map(|v| v == "1")
        .unwrap_or(false);

    // build_store_and_id_map() inserts the corpus into one store and records
    // the envelope IDs it gets back, so the id_map always aligns with the
    // store's envelope IDs regardless of timestamp-based SHA variation.
    let (store, id_map) = build_store_and_id_map();

    let query_results = run_queries(&store, &id_map);

    let fixture_path = fixture_path();

    if rebase || !fixture_path.exists() {
        let fixture = Fixture {
            schema_version: 1,
            queries: query_results,
        };
        let json_str = serde_json::to_string_pretty(&fixture).expect("serialize fixture");
        std::fs::create_dir_all(fixture_path.parent().unwrap()).expect("create fixture dir");
        std::fs::write(&fixture_path, &json_str).expect("write fixture");
        println!("BM25 baseline fixture written to {}", fixture_path.display());
        return;
    }

    // Compare mode
    let raw = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("fixture not found at {}; run with AMORE_BM25_REBASE=1 to generate", fixture_path.display()));
    let fixture: Fixture = serde_json::from_str(&raw).expect("parse fixture JSON");

    assert_eq!(
        fixture.queries.len(),
        query_results.len(),
        "query count mismatch"
    );

    for (expected, actual) in fixture.queries.iter().zip(query_results.iter()) {
        assert_eq!(expected.id, actual.id, "query id mismatch");
        assert_eq!(expected.query, actual.query, "query text mismatch for {}", expected.id);
        assert_eq!(
            expected.expected_ranks.len(),
            actual.expected_ranks.len(),
            "hit count mismatch for query {} ({:?})",
            expected.id,
            expected.query
        );
        for (e, a) in expected.expected_ranks.iter().zip(actual.expected_ranks.iter()) {
            assert_eq!(
                e.doc_id, a.doc_id,
                "rank {} doc_id mismatch for query {} ({:?}): expected {} got {}",
                e.rank, expected.id, expected.query, e.doc_id, a.doc_id
            );
            assert_eq!(
                e.rank, a.rank,
                "rank position mismatch for doc {} in query {}",
                e.doc_id, expected.id
            );
            assert!(
                (e.score - a.score).abs() < 1e-4,
                "score drift for doc {} in query {}: expected {:.6} got {:.6}",
                e.doc_id, expected.id, e.score, a.score
            );
        }
    }

    println!(
        "BM25 baseline: {} queries verified against {}",
        query_results.len(),
        fixture_path.display()
    );
}

// Sanity tests that don't require the fixture file — always run.

#[test]
fn corpus_has_50_docs() {
    assert_eq!(corpus().len(), 50, "corpus must have exactly 50 docs");
}

#[test]
fn queries_has_20_entries() {
    assert_eq!(queries().len(), 20, "must have exactly 20 fixed queries");
}

#[test]
fn empty_query_returns_no_hits() {
    let store = build_store();
    let hits = store.bm25_search("", 10).unwrap();
    assert!(hits.is_empty(), "empty query must return no hits");
}

#[test]
fn whitespace_query_returns_no_hits() {
    let store = build_store();
    let hits = store.bm25_search("   \t  ", 10).unwrap();
    assert!(hits.is_empty(), "whitespace-only query must return no hits");
}

#[test]
fn adversarial_fts5_metachar_query_does_not_panic() {
    let store = build_store();
    // These would crash FTS5 if not sanitized; should return hits or no hits,
    // never panic / Err.
    let result = store.bm25_search("SELECT * FROM users; DROP TABLE--", 5);
    assert!(result.is_ok(), "adversarial query must not return Err");
}

#[test]
fn rust_query_hits_rust_docs() {
    let (store, id_map) = build_store_and_id_map();
    let hits = store.bm25_search("rust ownership borrow", 5).unwrap();
    assert!(!hits.is_empty(), "rust query must return at least one hit");
    // Top hit should be a rust_docs document
    let top_logical = id_map.get(&hits[0].id).map(|s| s.as_str()).unwrap_or("");
    assert!(
        top_logical.starts_with("obs-00") || hits[0].text.to_lowercase().contains("rust"),
        "top BM25 hit for rust query should be a rust doc; got logical_id={} text={}",
        top_logical,
        hits[0].text
    );
}

#[test]
fn scores_are_positive_and_ordered_descending() {
    let store = build_store();
    let hits = store.bm25_search("machine learning neural network", 10).unwrap();
    if hits.len() < 2 {
        return; // corpus may legitimately return <2 hits for this query
    }
    for h in &hits {
        assert!(h.score > 0.0, "all BM25 scores must be positive (flip applied)");
    }
    for pair in hits.windows(2) {
        assert!(
            pair[0].score >= pair[1].score,
            "BM25 results must be ordered score descending: {} < {}",
            pair[0].score,
            pair[1].score
        );
    }
}
