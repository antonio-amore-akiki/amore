# Amore Qdrant 3-node Cluster

stable: true
topic: amore qdrant cluster opt-in scale production 100M observations replication
purpose: power-user reference deployment to scale beyond single-node default

## What this is

A reference 3-node Qdrant cluster deployment for power users running Amore at scale.
Default Amore install uses a single embedded Qdrant — sized for grandma-mode and
fine through ~1M observations. Beyond ~1M, opt in to this cluster to get
horizontal sharding + replication.

## What this is NOT

- Not a production multi-tenant deployment. Amore is single-user single-machine
  per `SECURITY.md`. The cluster scales storage, not tenants.
- Not the default install path. Non-technical users never touch this file.
- Not internet-exposed. All ports bind to `127.0.0.1` only.

## When to opt in

Trigger the cluster when ANY of:

- Single-node Qdrant slows recall p95 past 500 ms on your corpus
- You exceed 1M observations
- You want 1-node-fail tolerance (single-node has zero redundancy)
- `amore doctor` reports memory pressure or qdrant_collection_size past 2 GB

## Configuration

| Setting | Value | Rationale |
|---|---|---|
| Nodes | 3 | Minimum for Raft quorum + 1-node-fail tolerance |
| Replication factor | 2 | Each shard lives on 2 of 3 nodes; one node can die without data loss |
| Shards per collection | 12 | ~4-8M obs/shard at 100M scale — sized in `docs/SCALE-100M.md` |
| Ports | 127.0.0.1:6333 (REST), 127.0.0.1:6334 (gRPC) | Loopback-only — never 0.0.0.0 |
| Persistence | named docker volumes | `amore-qdrant-node-{1,2,3}-data` |

## Start

```bash
cd infra/qdrant-cluster
docker compose up -d
docker compose ps      # expect 3 services 'healthy'
curl -s http://127.0.0.1:6333/cluster | jq '.result.peers | length'  # expected: 3
```

## Connect Amore to the cluster

```bash
export AMORE_QDRANT_URL=http://127.0.0.1:6333
export AMORE_QDRANT_GRPC_URL=http://127.0.0.1:6334
amore serve
```

The connection pool from `crates/amore-core/src/qdrant_pool.rs` (H.4) maintains
up to `min(num_cpus * 2, 16)` connections per process; the cluster handles
load-balancing across the 3 nodes.

## Create a collection with replication

```bash
curl -X PUT http://127.0.0.1:6333/collections/amore \
  -H 'Content-Type: application/json' \
  -d '{
    "vectors": { "size": 768, "distance": "Cosine" },
    "shard_number": 12,
    "replication_factor": 2,
    "write_consistency_factor": 1
  }'
```

`shard_number: 12` means each shard handles ~4-8M obs at the 100M tier.
`write_consistency_factor: 1` means writes ack as soon as 1 replica confirms;
raise to 2 for stricter durability at the cost of write latency.

## 1-node-fail smoke test

```bash
docker compose stop qdrant-node-2
sleep 10
curl -s http://127.0.0.1:6333/cluster | jq '.result.peers | length'  # expected: 2
# Recall MUST still return results — every shard lives on 2 nodes,
# so no shard goes dark when 1 of 3 nodes is down.
docker compose start qdrant-node-2  # restore
```

## Stop + clean up

```bash
docker compose down                 # stop containers, preserve volumes
docker compose down --volumes       # stop + WIPE data — irreversible
```

## Capacity math

| Corpus | RAM/node | Disk/node | Latency p95 |
|---|---|---|---|
| 1M | 2 GB | 5 GB | 50 ms |
| 10M | 6 GB | 50 GB | 200 ms |
| 100M | 32 GB | 500 GB | 5 s (extrapolated per `docs/SCALE-100M.md`) |

For 100M you need 96 GB RAM total across the cluster + 1.5 TB disk.

## Security notes

- All ports bind to `127.0.0.1` only — never 0.0.0.0. To expose externally
  (NOT recommended for the single-user threat model), edit `docker-compose.yml`
  AND set up Tailscale + mTLS first.
- TLS between Amore and Qdrant on loopback is disabled by design.
- Cluster consensus is Raft over loopback Docker network — no auth needed.

## Snapshot + restore

The `amore snapshot create` + `amore snapshot restore` CLI commands (H.7)
work with the cluster — they walk each shard's snapshot API in parallel and
produce a single archive. See `docs/RUNBOOK.md`.

## Troubleshooting

- "Peer count = 1 after `docker compose up`": wait 30s for Raft consensus to
  form. If still 1 after 1 minute, check
  `docker compose logs qdrant-node-2 qdrant-node-3` for connection errors.
- "Permission denied on volume": `docker compose down --volumes` and start
  fresh; named volumes may have stale permissions from a prior install.
- "Recall errors out with `not compatible`": the bundled Amore binary pins
  `qdrant-client = "1.15"` against Qdrant server 1.15.4. If you upgrade Qdrant
  past 1.15.x, also update `Cargo.toml` workspace dep.

## Reference

- ADR 0002: `docs/adr/0002-choose-qdrant.md`
- ADR 0007: `docs/adr/0007-cluster-opt-in.md`
- Capacity: `docs/SCALE-100M.md`
