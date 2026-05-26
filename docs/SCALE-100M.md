# Amore 100M-Observation Capacity Plan

stable: true
purpose: cluster math + sizing + spot-validation procedure
target_tier: v0.7.0 architecture; v1.0.0 validation

## Goal

Demonstrate that Amore's architecture supports 100M observations under
real-world recall latency targets, without forcing every user into
that complexity by default.

## Default deployment is single-node

99% of Amore users will never have more than 100K observations on a
single machine. Single-node single-binary stays the default. Cluster
mode is an opt-in for the power-user 1%.

## Cluster topology @ 100M scale

```
+----------+         +----------+         +----------+
| Qdrant-1 |<------->| Qdrant-2 |<------->| Qdrant-3 |
|  RF=2    |         |  RF=2    |         |  RF=2    |
|  shards  |         |  shards  |         |  shards  |
|  0-3     |         |  4-7     |         |  8-11    |
+----------+         +----------+         +----------+
       \                   |                    /
        \                  |                   /
         +-----------------+-----------------+
                           |
                +-----------------------+
                | amore-mcp + amore-cli |
                | gRPC mode             |
                +-----------------------+
```

- 3-node Qdrant cluster
- Replication factor 2
- 12 shards per collection
- ~8.3 M vectors per shard
- ~5 GB RAM per Qdrant node (vectors quantized to int8)
- ~50 GB disk per node (~150 GB total + replication overhead)

## Math: from 10M baseline to 100M projection

Phase H load test target: 10M corpus on a single dev laptop. Once
green, extrapolate to 100M as follows.

### Vector lane (Qdrant)

- 100M × 768-dim float32 = ~300 GB raw
- int8 quantization → ~75 GB
- × RF=2 replication → ~150 GB total cluster disk
- HNSW index in-memory → ~3-5 GB RAM per shard
- 12 shards × ~4 GB → ~48 GB cluster RAM (with overhead, plan ~64 GB)
- Recall latency: HNSW shard scan ≤ 200 ms × 12-shard fan-out + merge
  ≤ 300 ms → ~500 ms p95 lane

### BM25 lane (Tantivy in Phase H)

- 100M docs × ~500 token avg → 50B tokens
- Tantivy postings + dictionary → ~50 GB disk
- 16 shards in 3-node cluster (Tantivy uses Amore's hash-range sharding
  not Qdrant's; see ADR 0006)
- ~3 GB RAM per shard for hot postings
- 16 × ~3 GB → ~48 GB cluster RAM (overlap with Qdrant)
- Shard fan-out latency: ≤ 100 ms p95 across 16 shards

### Cross-encoder reranker (Phase H)

- Top 50 candidates from RRF → reranker → top K
- bge-reranker-base int8 ONNX → ~28 MB on disk
- One forward pass on 50 query-doc pairs → ~50 ms p95
- Optional, opt-in via env var

### Total p95 budget for 100M corpus

| Stage | p95 |
|---|---|
| Network ingress (gRPC) | 5 ms |
| Bounds check + sanitize | 1 ms |
| Embed query (Ollama) | 100 ms |
| Vector lane fan-out + merge | 500 ms |
| BM25 lane fan-out + merge | 100 ms |
| RRF fusion | 5 ms |
| Cross-encoder rerank top-50 | 50 ms |
| Network egress | 5 ms |
| Total | ~770 ms p95 |

Slack margin to the SLO target of 10 s p95: very comfortable. The
binding constraint is cluster ops, not latency.

## Spot-validation procedure

Phase H acceptance gate:

1. Single-laptop **10M corpus** load test:
   - `oha`-based 100 QPS sustained recall for 1 hour
   - Gate: p95 ≤ 5 s, error rate < 0.1%
   - Resource: 64 GB free disk required
2. **1M corpus** spot validation with cluster scaling factor applied:
   - 3-node `docker-compose` cluster, single-laptop docker
   - Same 100 QPS sustained for 30 min
   - Gate: p95 ≤ 1.5 s × cluster overhead factor (target ≤ 2.5 s)
3. **100M projection** documented (this file) with extrapolation math.

If the 10M + 1M validations pass and the extrapolation math is
consistent, the 100M architecture is documented-good for v1.0
public-ship gate. Real 100M load test is post-v1.0 work — needs cloud
resources (estimated $50-100 one-time on AWS / Hetzner Cloud).

## Provisioning a 100M production cluster

(For Amore operators who want to run this themselves; v1.1+ goal.)

### Hardware (cheapest viable)

- 3 × Hetzner CCX23 (4 vCPU, 16 GB RAM, 240 GB NVMe) = ~€40/month total
- Or 3 × OVH B2-15 (4 vCPU, 15 GB RAM, 200 GB SSD) = similar pricing

### Deployment

```bash
git clone https://github.com/antonio-amore-akiki/amore.git
cd amore/infra/qdrant-cluster
# Edit docker-compose.yml to set peer hostnames
docker compose up -d
amore serve --grpc --qdrant-cluster
```

### Health monitoring

Prometheus metrics on opt-in `AMORE_METRICS_PORT` (Phase H deliverable).
Scrape from a local Grafana or push to Pushgateway.

## What we do NOT promise at 100M

- p99 < 1 s (we promise p99 ≤ 20 s)
- Single-machine deployment (you need a cluster)
- Zero-downtime updates without operator skill
- Multi-tenant isolation (Amore is single-tenant by design)

## Out of scope here

- Architecture diagrams: `docs/ARCHITECTURE.md`
- SLO tiers per corpus size: `docs/SLO.md`
- Runbook for cluster ops: `docs/RUNBOOK.md` "Cluster mode ops"
- Tantivy migration ADR: `docs/adr/0006-tantivy-vs-sqlite-fts5.md`
- Cluster opt-in ADR: `docs/adr/0007-cluster-opt-in.md`
