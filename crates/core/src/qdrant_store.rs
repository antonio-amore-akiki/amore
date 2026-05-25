// Qdrant store wrapper: collection lifecycle + point upsert + vector search.
//
// Vector size matches the embedding model (default 768 for nomic-embed-text).
// Distance metric: Cosine — standard for sentence embeddings.
//
// Connects to a running Qdrant daemon (default http://127.0.0.1:6333).
// Subprocess auto-launch deferred to S?/F10 (CLI lifecycle management).

use anyhow::{Context, Result};
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, SearchPointsBuilder, UpsertPointsBuilder,
    VectorParamsBuilder,
};

const DEFAULT_VECTOR_SIZE: u64 = 768;

pub struct QdrantStore {
    client: Qdrant,
    collection: String,
    vector_size: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub score: f32,
    pub payload: serde_json::Value,
}

impl QdrantStore {
    /// Connect to a Qdrant daemon and ensure the named collection exists.
    /// Idempotent: re-running with the same args is a no-op if the collection
    /// is already configured with the matching vector size + Cosine distance.
    pub async fn new(url: &str, collection: &str, vector_size: u64) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .build()
            .with_context(|| format!("connecting to Qdrant at {url}"))?;
        let store = Self {
            client,
            collection: collection.to_string(),
            vector_size,
        };
        store.ensure_collection().await?;
        Ok(store)
    }

    /// Convenience: connect with the default vector size (768 for nomic-embed-text).
    pub async fn open(url: &str, collection: &str) -> Result<Self> {
        Self::new(url, collection, DEFAULT_VECTOR_SIZE).await
    }

    async fn ensure_collection(&self) -> Result<()> {
        let exists = self
            .client
            .collection_exists(&self.collection)
            .await
            .with_context(|| format!("checking collection {}", self.collection))?;
        if exists {
            return Ok(());
        }
        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection)
                    .vectors_config(VectorParamsBuilder::new(self.vector_size, Distance::Cosine)),
            )
            .await
            .with_context(|| format!("creating collection {}", self.collection))?;
        Ok(())
    }

    /// Upsert a single point. Numeric IDs avoid the Qdrant string-id limit
    /// (must be Uuid for string ids); higher layers map observation envelopes
    /// to u64 via stable hashing.
    pub async fn upsert(&self, id: u64, vector: Vec<f32>, payload: serde_json::Value) -> Result<()> {
        let qpayload: Payload = payload
            .try_into()
            .with_context(|| "converting serde_json::Value into Qdrant Payload")?;
        let point = PointStruct::new(id, vector, qpayload);
        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await
            .with_context(|| format!("upsert into {}", self.collection))?;
        Ok(())
    }

    /// Vector search returning up to `top_k` scored hits with payload.
    pub async fn search(&self, query: Vec<f32>, top_k: u64) -> Result<Vec<SearchHit>> {
        let response = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection, query, top_k).with_payload(true),
            )
            .await
            .with_context(|| format!("search in {}", self.collection))?;

        let hits = response
            .result
            .into_iter()
            .map(|p| {
                let id = p
                    .id
                    .as_ref()
                    .map(|pid| format!("{pid:?}"))
                    .unwrap_or_default();
                let payload_json = serde_json::to_value(&p.payload).unwrap_or(serde_json::Value::Null);
                SearchHit {
                    id,
                    score: p.score,
                    payload: payload_json,
                }
            })
            .collect();
        Ok(hits)
    }

    /// Delete the collection. Useful for tests; not exposed at MCP layer.
    pub async fn drop_collection(&self) -> Result<()> {
        self.client
            .delete_collection(&self.collection)
            .await
            .with_context(|| format!("dropping {}", self.collection))?;
        Ok(())
    }

    pub fn collection_name(&self) -> &str {
        &self.collection
    }

    pub fn vector_size(&self) -> u64 {
        self.vector_size
    }
}
