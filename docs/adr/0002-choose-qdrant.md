# 2. Choose Qdrant for the vector store

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Amore's recall pipeline fuses BM25 + vector search via RRF. The vector
store needs to scale from single-user (10k vectors) to power-user
cluster mode (100M+ vectors). It must self-host for free, run on the
user's machine by default, and offer a Rust-native client.

## Decision Drivers

* Free unlimited self-host (no paid cloud lock-in)
* Production scale to billions of vectors (cluster mode)
* Rust-native client (qdrant-client crate)
* Embeddable as a managed subprocess on first install
* gRPC + REST API
* Replication factor and shard count tunable per collection
* Persistent on-disk with snapshots

## Considered Options

* Qdrant (self-hosted; default single-node + opt-in cluster)
* Chroma
* Weaviate (self-hosted)
* Pinecone (paid only)
* Milvus
* pgvector (Postgres extension)

## Decision Outcome

Chosen option: **Qdrant**.

Default deployment is single-node embedded via `qdrant.exe` shipped in
the Windows installer (`installer/windows/amore.iss` stages it from
GitHub Releases). Cluster mode (3-node minimum, RF=2, 12 shards) lands
in v0.7.0 (Phase H) as opt-in for power users via the
`infra/qdrant-cluster/docker-compose.yml` reference deployment.

### Consequences

* Good: single binary, single-port, no daemon-of-daemons
* Good: cluster mode scales horizontally to billions of vectors
* Good: snapshot/restore API maps cleanly to Amore's
  `amore snapshot create/restore` CLI (v0.7.0)
* Good: qdrant-client is first-class Rust crate, maintained by upstream
* Bad: another moving part to manage in the installer
* Bad: cluster sharding rebalancing slow on v1.18+ (mitigated by sticky
  single-node default and only opting into cluster when needed)

## Pros and Cons of the Options

### Qdrant

* Good: production-grade, billion-vector scale proven
* Good: gRPC + REST, sharding + replication, snapshot API
* Good: Rust-native client; same language as Amore = simpler bindings
* Good: Apache-2.0 licence
* Bad: ~70 MB binary adds to installer size

### Chroma

* Good: simpler to embed
* Bad: not designed for production scale to 100M+ vectors
* Bad: Python-first; gRPC API is secondary

### Weaviate

* Good: production-grade
* Bad: Go runtime + Docker requirement
* Bad: heavier resource footprint per node

### Pinecone

* Bad: paid only; fails "free unlimited" mandate
* Bad: cloud-only; fails "local by default" mandate

### Milvus

* Good: production-grade, scales to billions
* Bad: heavy multi-component install (etcd + MinIO + Pulsar)
* Bad: not a single binary, fails the install-simplicity mandate

### pgvector

* Good: leverages existing Postgres install
* Bad: Postgres is not a default install on most user machines
* Bad: harder to scale horizontally than purpose-built vector DB

## More Information

* Pinning rationale: `qdrant-client = "1.15"` matches the bundled local
  Qdrant server version to avoid the v1.18 client / v1.15 server skew
  warning. Closed via Security Fix DG-F in v0.3.1.
* Cluster math + sizing for 100M corpus: `docs/SCALE-100M.md` (Phase H).
