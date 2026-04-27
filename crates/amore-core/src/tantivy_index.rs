// tantivy_index.rs — H.1 Tantivy BM25 lane for amore-core.
//
// Thin wrapper around a Tantivy Index providing add/commit/search.
// Tokeniser: custom `porter1` (see porter1.rs) = SimpleTokenizer + LowerCaser +
// Porter1Filter.  Matches SQLite FTS5's `porter unicode61` stemmer exactly.
// Scoring: Tantivy default BM25 (k1=1.2, b=0.75); scores differ from FTS5
// numerically but rank order is identical (parity test asserts order, not values).

use std::path::Path;

use anyhow::{Context, Result};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::schema::{
    Field, IndexRecordOption, Schema, SchemaBuilder, TextFieldIndexing, TextOptions, STORED,
};
use tantivy::tokenizer::{LowerCaser, SimpleTokenizer, TextAnalyzer};
use tantivy::{Index, IndexWriter, ReloadPolicy, TantivyDocument};

use crate::porter1::Porter1Filter;

const PORTER1_TOKENIZER: &str = "porter1";

/// Thin wrapper around a Tantivy `Index` providing add/commit/search.
pub struct TantivyIndex {
    index: Index,
    writer: IndexWriter,
    field_doc_id: Field,
    field_body: Field,
}

impl TantivyIndex {
    /// Open or create a Tantivy index at `path`.
    /// Pass `Path::new(":memory:")` for a RAM-backed index (tests).
    pub fn new(path: &Path) -> Result<Self> {
        let schema = build_schema();
        let field_doc_id = schema
            .get_field("doc_id")
            .context("invariant: doc_id field must exist in schema")?;
        let field_body = schema
            .get_field("body")
            .context("invariant: body field must exist in schema")?;

        let index = if path.to_str() == Some(":memory:") {
            Index::create_in_ram(schema)
        } else {
            std::fs::create_dir_all(path)
                .with_context(|| format!("create tantivy dir at {}", path.display()))?;
            let dir = tantivy::directory::MmapDirectory::open(path)
                .with_context(|| format!("open mmap dir at {}", path.display()))?;
            Index::open_or_create(dir, schema)
                .with_context(|| format!("open_or_create tantivy index at {}", path.display()))?
        };

        register_porter1(&index);

        let writer: IndexWriter = index
            .writer(50_000_000)
            .context("create tantivy IndexWriter")?;

        Ok(Self { index, writer, field_doc_id, field_body })
    }

    /// Buffer `(doc_id, text)` into the writer.  Call [`commit`] to make searchable.
    pub fn add(&mut self, doc_id: u64, text: &str) -> Result<()> {
        let mut doc = TantivyDocument::default();
        doc.add_u64(self.field_doc_id, doc_id);
        doc.add_text(self.field_body, text);
        self.writer
            .add_document(doc)
            .context("add document to tantivy writer")?;
        Ok(())
    }

    /// Flush buffered documents to the index.
    pub fn commit(&mut self) -> Result<()> {
        self.writer.commit().context("tantivy writer commit")?;
        Ok(())
    }

    /// BM25 search — returns up to `top_k` `(doc_id, score)` pairs descending.
    ///
    /// Empty/whitespace-only queries return `[]`.  Sanitisation mirrors
    /// `sqlite_store::bm25_search`: keep only alphanumeric tokens.
    /// Multi-token queries are conjunctive (AND) to match FTS5 default.
    pub fn search(&self, query: &str, top_k: usize) -> Result<Vec<(u64, f32)>> {
        let sanitized: String = query
            .split_whitespace()
            .map(|t| t.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if sanitized.is_empty() {
            return Ok(vec![]);
        }

        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .context("build tantivy IndexReader")?;

        let searcher = reader.searcher();
        let mut qp = QueryParser::for_index(&self.index, vec![self.field_body]);
        qp.set_conjunction_by_default();

        let parsed = match qp.parse_query(&sanitized) {
            Ok(q) => q,
            Err(_) => return Ok(vec![]),
        };

        // tantivy 0.26 (v-next #34): TopDocs no longer impls Collector directly;
        // must chain .order_by_score() to get Vec<(Score, DocAddress)>.
        let top_docs = searcher
            .search(&parsed, &TopDocs::with_limit(top_k).order_by_score())
            .context("tantivy search")?;

        let mut results = Vec::with_capacity(top_docs.len());
        for (score, addr) in top_docs {
            let doc: TantivyDocument = searcher.doc(addr).context("retrieve tantivy doc")?;
            let id = doc
                .get_first(self.field_doc_id)
                .and_then(|v| v.as_u64())
                .context("invariant: every indexed doc has a u64 doc_id")?;
            results.push((id, score));
        }
        // Stable secondary sort by doc_id ascending within equal-score groups —
        // mirrors FTS5 rowid-based tie-breaking (insertion order = doc_id order).
        results.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        Ok(results)
    }
}

fn register_porter1(index: &Index) {
    let analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .filter(Porter1Filter)
        .build();
    index.tokenizers().register(PORTER1_TOKENIZER, analyzer);
}

fn build_schema() -> Schema {
    let mut builder: SchemaBuilder = Schema::builder();
    builder.add_u64_field("doc_id", STORED);
    let text_opts = TextOptions::default()
        .set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(PORTER1_TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        )
        .set_stored();
    builder.add_text_field("body", text_opts);
    builder.build()
}
