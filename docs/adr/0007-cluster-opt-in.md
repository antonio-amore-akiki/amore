# 7. Cluster mode is opt-in, not the default

stable: true
status: accepted
date: 2026-05-25
deciders: Antonio

## Context and Problem Statement

Qdrant supports a distributed cluster mode: multiple nodes, replication
factor, shard re-balancing, peer discovery. This is the right
architecture for 100M-vector corpora. However, running a 3-node minimum
cluster requires Docker Compose, a static IP or DNS name for each peer,
and non-trivial operational knowledge to maintain.

Should Amore default to cluster mode, opt-in cluster mode, or
single-node only?

## Decision Drivers

* 99% of target users will never exceed 10k vectors (typical agent
  session corpus for a single developer or knowledge worker)
* Cluster mode requires Docker Compose: violates the one-click install
  mandate for the default path
* Operational overhead (peer discovery, shard rebalancing, snapshot
  quorum) is non-trivial to expose in a non-technical-user UI
* Power users (team deployments, shared RAG corpora) DO need cluster
  mode; it must not be removed
* Default path must stay "it just works" on a laptop with no Docker

## Considered Options

* Cluster mode always (every install is a 3-node cluster)
* Cluster mode opt-in (power-user manual config)
* Cluster mode never (single-node forever)

## Decision Outcome

Chosen option: **cluster mode OPT-IN via
`infra/qdrant-cluster/docker-compose.yml`**.

The default install is single-node Qdrant managed as a subprocess
(see ADR-0005). To enable cluster mode, the operator:

1. Installs Docker Desktop (power-user prerequisite; documented in
   `docs/CLUSTER.md`).
2. Runs `amore cluster init --nodes 3 --rf 2 --shards 12`.
3. Amore writes a resolved `docker-compose.yml` under
   `infra/qdrant-cluster/` and stops the subprocess Qdrant.
4. Operator runs `docker compose -f infra/qdrant-cluster/docker-compose.yml up -d`.
5. Amore detects the cluster endpoint and switches the qdrant-client
   config from `localhost:6334` to the load-balanced cluster endpoint.

Cluster mode lands as the reference deployment in v0.7.0 (Phase H).
The `infra/qdrant-cluster/` reference is already committed with
placeholder node addresses.

### Consequences

* Good: default install remains laptop-friendly; no Docker required
* Good: cluster mode is fully supported for power users who need it
* Good: separation of concerns: Amore daemon is stateless with respect
  to the Qdrant topology; switching from single-node to cluster is a
  config change, not a code change
* Bad: cluster mode activation is a multi-step manual process; a
  non-technical user who stumbles into it may break their install
* Bad: cluster endpoint config is currently stored in
  `~/.amore/config.toml`; a misconfiguration silently falls back to
  single-node (logged, not silent — see CLAUDE.md no-silent-fail-open)

## Pros and Cons of the Options

### Cluster always

* Good: simplest production scaling story
* Bad: requires Docker Desktop on every installation
* Bad: 3-node minimum = 3x resource overhead on a laptop
* Bad: peer discovery failure at startup = total recall failure;
  unacceptable for a non-technical-user default

### Cluster opt-in (CHOSEN)

* Good: default path has zero cluster overhead
* Good: power users get full horizontal scale
* Good: clean separation between single-node and cluster config paths
* Bad: two code paths to maintain (subprocess watchdog vs cluster client)

### Cluster never

* Good: simplest architecture; one path
* Bad: forecloses on the 100M-vector scale target entirely
* Bad: team deployments (shared RAG corpus) are impossible
* Bad: violates the "100M users scalable" mandate

## More Information

* Reference deployment: `infra/qdrant-cluster/docker-compose.yml`
* Cluster sizing for 100M corpus: `docs/SCALE-100M.md` (Phase H)
* Qdrant cluster docs: https://qdrant.tech/documentation/guides/distributed_deployment/
* The 3-node minimum with RF=2 and 12 shards is the Phase H recommended
  starting point for a team deployment on 3 × 16 GB RAM nodes
