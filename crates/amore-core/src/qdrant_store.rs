use anyhow::{Context, Result};
use bb8::Pool;
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CreateCollectionBuilder, DeletePointsBuilder, Distance, PointId, PointStruct,
    SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
    point_id::PointIdOptions,
};
use std::sync::Arc;
use crate::circuit_breaker::{BreakerError, CircuitBreaker};
use crate::qdrant_pool::QdrantConnectionManager;

const DEFAULT_VECTOR_SIZE: u64 = 768;

enum Conn { Direct(Qdrant), Pooled(Arc<Pool<QdrantConnectionManager>>) }

pub struct QdrantStore {
    conn: Conn, collection: String, vector_size: u64, breaker: Option<CircuitBreaker>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchHit { pub id: String, pub score: f32, pub payload: serde_json::Value }

impl QdrantStore {
    pub async fn new(url: &str, collection: &str, vector_size: u64) -> Result<Self> {
        let client = Qdrant::from_url(url).build()
            .with_context(|| format!("connecting to Qdrant at {url}"))?;
        let store = Self { conn: Conn::Direct(client), collection: collection.to_string(), vector_size, breaker: None };
        store.ensure_collection().await?;
        Ok(store)
    }
    pub async fn open(url: &str, collection: &str) -> Result<Self> { Self::new(url, collection, DEFAULT_VECTOR_SIZE).await }
    pub fn open_lazy(url: &str, collection: &str) -> Result<Self> { Self::new_lazy(url, collection, DEFAULT_VECTOR_SIZE) }
    pub fn new_lazy(url: &str, collection: &str, vector_size: u64) -> Result<Self> {
        let client = Qdrant::from_url(url).build()
            .with_context(|| format!("constructing Qdrant client for {url}"))?;
        Ok(Self { conn: Conn::Direct(client), collection: collection.to_string(), vector_size, breaker: None })
    }
    pub fn from_pool(pool: Arc<Pool<QdrantConnectionManager>>, collection: &str, vector_size: u64) -> Self {
        Self { conn: Conn::Pooled(pool), collection: collection.to_string(), vector_size, breaker: None }
    }
    pub fn with_breaker(mut self, breaker: CircuitBreaker) -> Self { self.breaker = Some(breaker); self }

    async fn coll_exists(&self) -> Result<bool> {
        match &self.conn {
            Conn::Direct(c) => c.collection_exists(&self.collection).await
                .with_context(|| format!("checking {}", self.collection)),
            Conn::Pooled(pool) => {
                let conn = pool.get().await.map_err(|e| anyhow::anyhow!("pool.get: {:?}", e))?;
                conn.collection_exists(&self.collection).await
                    .with_context(|| format!("checking {}", self.collection))
            }
        }
    }
    async fn ensure_collection(&self) -> Result<()> {
        if self.coll_exists().await? { return Ok(()); }
        let params = CreateCollectionBuilder::new(&self.collection)
            .vectors_config(VectorParamsBuilder::new(self.vector_size, Distance::Cosine));
        match &self.conn {
            Conn::Direct(c) => c.create_collection(params).await
                .with_context(|| format!("creating {}", self.collection))?,
            Conn::Pooled(pool) => {
                let conn = pool.get().await.map_err(|e| anyhow::anyhow!("pool.get: {:?}", e))?;
                conn.create_collection(params).await
                    .with_context(|| format!("creating {}", self.collection))?
            }
        };
        Ok(())
    }
    pub async fn upsert(&self, id: u64, vector: Vec<f32>, payload: serde_json::Value) -> Result<()> {
        let qpayload: Payload = payload.try_into()
            .with_context(|| "converting serde_json::Value into Qdrant Payload")?;
        let point = PointStruct::new(id, vector, qpayload);
        if let Some(breaker) = &self.breaker {
            return breaker.call(|| async { self.upsert_raw(point).await }).await
                .map_err(|e| match e {
                    BreakerError::Open { dep, open_for } => {
                        tracing::warn!(target: "amore.qdrant", dep = %dep, open_for = ?open_for, "qdrant.upsert breaker open");
                        anyhow::anyhow!("circuit breaker open dep={dep} open_for={open_for:?}")
                    }
                    BreakerError::Inner(inner) => inner,
                });
        }
        self.upsert_raw(point).await
    }
    async fn upsert_raw(&self, point: PointStruct) -> Result<()> {
        let req = UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true);
        match &self.conn {
            Conn::Direct(c) => c.upsert_points(req).await
                .with_context(|| format!("upsert into {}", self.collection)).map(|_| ()),
            Conn::Pooled(pool) => {
                let conn = pool.get().await.map_err(|e| anyhow::anyhow!("pool.get: {:?}", e))?;
                conn.upsert_points(req).await
                    .with_context(|| format!("upsert into {}", self.collection)).map(|_| ())
            }
        }
    }
    pub async fn search(&self, query: Vec<f32>, top_k: u64) -> Result<Vec<SearchHit>> {
        if let Some(breaker) = &self.breaker {
            return breaker.call(|| async { self.search_raw(query, top_k).await }).await
                .map_err(|e| match e {
                    BreakerError::Open { dep, open_for } => {
                        tracing::warn!(target: "amore.qdrant", dep = %dep, open_for = ?open_for, "qdrant.search breaker open");
                        anyhow::anyhow!("circuit breaker open dep={dep} open_for={open_for:?}")
                    }
                    BreakerError::Inner(inner) => inner,
                });
        }
        self.search_raw(query, top_k).await
    }
    async fn search_raw(&self, query: Vec<f32>, top_k: u64) -> Result<Vec<SearchHit>> {
        let req = SearchPointsBuilder::new(&self.collection, query, top_k).with_payload(true);
        let response = match &self.conn {
            Conn::Direct(c) => c.search_points(req).await
                .with_context(|| format!("search in {}", self.collection))?,
            Conn::Pooled(pool) => {
                let conn = pool.get().await.map_err(|e| anyhow::anyhow!("pool.get: {:?}", e))?;
                conn.search_points(req).await
                    .with_context(|| format!("search in {}", self.collection))?
            }
        };
        Ok(response.result.into_iter().map(|p| {
            let id = p.id.as_ref().map(|pid| format!("{pid:?}")).unwrap_or_default();
            let payload_json = serde_json::to_value(&p.payload).unwrap_or(serde_json::Value::Null);
            SearchHit { id, score: p.score, payload: payload_json }
        }).collect())
    }
    pub async fn drop_collection(&self) -> Result<()> {
        match &self.conn {
            Conn::Direct(c) => c.delete_collection(&self.collection).await
                .with_context(|| format!("dropping {}", self.collection))?,
            Conn::Pooled(pool) => {
                let conn = pool.get().await.map_err(|e| anyhow::anyhow!("pool.get: {:?}", e))?;
                conn.delete_collection(&self.collection).await
                    .with_context(|| format!("dropping {}", self.collection))?
            }
        };
        Ok(())
    }
    pub fn collection_name(&self) -> &str { &self.collection }
    pub fn vector_size(&self) -> u64 { self.vector_size }

    // ─── Compaction helper (H.9) ──────────────────────────────────────────────

    /// Delete points by string ids (UUID or numeric string) — compaction use only.
    /// Ids that do not exist in the collection are silently ignored by Qdrant.
    pub async fn delete_by_ids(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let point_ids: Vec<PointId> = ids
            .iter()
            .map(|s| {
                // Try numeric first, fall back to UUID string.
                let opts = if let Ok(n) = s.parse::<u64>() {
                    PointIdOptions::Num(n)
                } else {
                    PointIdOptions::Uuid(s.clone())
                };
                PointId { point_id_options: Some(opts) }
            })
            .collect();
        let req = DeletePointsBuilder::new(&self.collection)
            .points(point_ids)
            .wait(true);
        match &self.conn {
            Conn::Direct(c) => c.delete_points(req).await
                .with_context(|| format!("delete_by_ids from {}", self.collection))
                .map(|_| ()),
            Conn::Pooled(pool) => {
                let conn = pool.get().await.map_err(|e| anyhow::anyhow!("pool.get: {:?}", e))?;
                conn.delete_points(req).await
                    .with_context(|| format!("delete_by_ids from {}", self.collection))
                    .map(|_| ())
            }
        }
    }
}