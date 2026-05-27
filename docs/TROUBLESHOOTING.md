stable: true

# Troubleshooting — Amore

## 1. Qdrant unreachable

Symptom: `amore doctor` reports `qdrant: unreachable`.

```bash
docker run -d -p 6333:6333 -p 6334:6334 --name amore-qdrant qdrant/qdrant:latest
docker ps --filter name=amore-qdrant
curl http://localhost:6333/healthz
```

## 2. sled corruption

Symptom: panic at startup containing `sled: ...`.
Fix: backup `~/.local/share/amore/sled` then delete it; Amore recreates on next start.

## 3. WAL replay stuck

Symptom: `amore serve` hangs at startup.
Fix: check `~/.local/share/amore/wal/` size; if >1GB, backup + delete + restart.

## 4. MSVC linker error (Windows build)

Symptom: `LNK2038: mismatch detected for 'RuntimeLibrary'`.
Fix: verify `ort` has `features = ["load-dynamic"]` (per docs/adr/0013-ort-load-dynamic.md). Set `ORT_DYLIB_PATH` to `vendor/onnxruntime/onnxruntime.dll`.

## 5. ort runtime missing

Symptom: `Unable to load library: ...onnxruntime.dll`.
Fix: ensure `vendor/onnxruntime/onnxruntime.dll` exists; download from `github.com/microsoft/onnxruntime/releases`.

## 6. Port conflicts (9090/9091)

Symptom: `Address already in use` for Prometheus or healthz endpoints.

```bash
export AMORE_HEALTH_BIND=127.0.0.1:9192
amore serve
```

Note: the default binds to loopback only. To expose the healthz endpoint on all interfaces
(e.g. in a container), set `AMORE_HEALTH_ALLOW_NETWORK=1` alongside a non-loopback bind address.

## 7. OOM during compaction

Symptom: process killed during background compaction.
Fix: lower batch size via `AMORE_COMPACTION_BATCH=100` (default 1000).

## 8. Slow recall

Symptom: recall latency >1s on small corpus.

```bash
amore doctor --json | jq '.cache_hit_ratio'
curl http://localhost:9090/metrics | grep amore_recall_latency
```

Fix: ensure reranker enabled (`AMORE_FLAG_RERANK_ONNX=on`); verify Qdrant gRPC port 6334 is reachable.

## 9. Missing API key

Symptom: `Secret 'qdrant_api_key' not found in keyring`.

```bash
amore secrets set qdrant_api_key
```

## 10. Feature flag not taking effect

Symptom: `AMORE_FLAG_X=on` ignored.
Fix: env vars must be set before `amore serve` starts (OnceLock pattern). Restart the daemon.

---

## Sources

- docs/SECRETS.md
- docs/FEATURE-FLAGS.md
- docs/OBSERVABILITY.md
