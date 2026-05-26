# Amore Toxiproxy Chaos Rig

stable: false — chaos-test infrastructure; not part of the standard amore stack
topic: toxiproxy chaos network-fault latency packet-loss circuit-breaker h12

## What this is

A [Toxiproxy](https://github.com/Shopify/toxiproxy) (Shopify, Apache-2.0) deployment
that sits between amore-mcp and its downstream services (Qdrant + Ollama) to inject
realistic fault conditions: 200 ms latency, jitter, and 30% packet loss.

Used by `tests/qa/h12_toxiproxy_chaos.ps1` to prove the elite-engineering
"no silent fail-open" principle under fault: the H.5 circuit breaker must
trip, amore must return a degraded-but-non-empty recall, and the breaker must
recover within 30 s once faults are removed.

## What this is NOT

- Not a production component. Bring it up for chaos tests only, then `docker compose down`.
- Not internet-exposed. All ports bind to `127.0.0.1` only.

## Proxy port map

| Toxiproxy port | Real service | Purpose |
|---|---|---|
| 127.0.0.1:8474 | — | Toxiproxy admin API (configure toxics) |
| 127.0.0.1:6433 | Qdrant REST :6333 | Proxied REST; chaos test sets `AMORE_QDRANT_URL=http://127.0.0.1:6433` |
| 127.0.0.1:6434 | Qdrant gRPC :6334 | Proxied gRPC |
| 127.0.0.1:11534 | Ollama :11434 | Proxied Ollama; chaos test sets `AMORE_OLLAMA_URL=http://127.0.0.1:11534` |

## Start / stop

```powershell
cd infra/toxiproxy
docker compose up -d
# verify
Invoke-RestMethod http://127.0.0.1:8474/version

# after chaos test
docker compose down
```

## Run the chaos test

```powershell
pwsh ./tests/qa/h12_toxiproxy_chaos.ps1 -DryRun   # deps check only
pwsh ./tests/qa/h12_toxiproxy_chaos.ps1            # full chaos run (Phase J)
```

## Reference

- ADR 0008: `docs/adr/0008-circuit-breaker.md` (H.5)
- Circuit breaker implementation: `crates/amore-core/src/circuit_breaker.rs`
- Chaos test: `tests/qa/h12_toxiproxy_chaos.ps1`
- Toxiproxy upstream: https://github.com/Shopify/toxiproxy
