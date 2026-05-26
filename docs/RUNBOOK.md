# Amore Runbook

stable: true
purpose: common ops procedures for single-node + cluster mode
audience: power users and (future) operators

## Daily ops (single-node, default deployment)

### Health check

```bash
amore doctor --json
```

Expected output: `{"status":"ok","ollama":"ok","qdrant":"ok","data_dir":"ok"}`.
Non-zero exit indicates at least one dep is unreachable.

### Inspect what Amore knows

```bash
amore recall "your query here" --top-k 5
```

Returns up to 5 hits with score + excerpt. If 0 hits: corpus is empty
or query doesn't match; not an error.

### Status of background services

- Windows: System tray icon (green/yellow/red) shows aggregate health.
- Right-click the tray → "Show status" opens a panel.
- CLI: `amore status` prints a one-line aggregate health summary.

## Start / stop

The first-run wizard registers `amore-mcp.exe` as a user-mode service
on install. To pause it:

- Tray icon → "Pause memory"
- CLI: `amore status --stop`
- CLI: `amore status --start` to resume

## Snapshot + restore

```bash
amore snapshot create ~/amore-backup-$(date +%Y%m%d).tar.gz
```

Writes a complete state dump (Qdrant collections + SQLite tables +
provenance chain + bundled models).

```bash
amore snapshot restore ~/amore-backup-20260526.tar.gz
```

Restores. Existing data at `%APPDATA%\Amore\` is moved to
`%APPDATA%\Amore.bak-<timestamp>` before the restore overwrites.

The snapshot CLI ships in v0.7.0 (Phase H). Until then, take a Kopia
snapshot of `%APPDATA%\Amore\` (per the Anto backup-stack doc) as the
manual equivalent.

## Diagnosing a degraded lane

If `amore recall` returns hits with `degraded: true` in the envelope,
one of Qdrant / Ollama is unreachable. Recall falls back to BM25-only.

1. Check the lane: `tail %APPDATA%\Amore\amore.log` for the most recent
   `WARN ... unreachable` line.
2. Restart the dep:
   - Ollama: re-run `ollama serve` (or use Ollama Tray's "Start" menu)
   - Qdrant: restart via the Amore tray icon
3. Recall returns to full hybrid on the next query.

## Provenance verification

```bash
amore provenance verify
```

Walks the SHA-256 chain across all observations. Returns the count of
verified-OK observations + first chain break (if any). A break means
SQLite tampering — restore from the most recent good snapshot.

## Rotating the bundled embedding model

```bash
amore models swap nomic-embed-text-v1.5
```

Downloads the named ONNX model, verifies SHA-256, re-embeds existing
observations in the background (rate-limited). The previous model file
is kept as `.bak` for one revision.

## Cluster mode ops (v0.7.0+, power users)

Initial deployment:
```bash
cd infra/qdrant-cluster
docker compose up -d
amore serve --grpc --bind unix:///run/amore.sock --qdrant-cluster
```

Verify the cluster:
```bash
curl http://localhost:6333/cluster
# expect: {"result":{"peers":[3]},...}
```

Loss of one node (RF=2 keeps the cluster healthy):
1. The remaining 2 nodes continue serving recall.
2. Restore the lost node:
   ```bash
   docker compose restart qdrant-node-1
   ```
3. Qdrant replicates the missing shards from peers.

## Logs

- `%APPDATA%\Amore\amore.log` — application log (tracing-subscriber).
- `%APPDATA%\Amore\security-baselines\<date>.json` — nightly audit.
- `%APPDATA%\Amore\install.log` — first-run installer events.

`OBELION_LOG=debug` (will be renamed `AMORE_LOG=debug` in v0.4.0) for
verbose output.

## Disaster recovery

If `%APPDATA%\Amore\amore.db` is corrupted:
1. Stop Amore (`amore status --stop`).
2. Restore from the most recent Kopia snapshot.
3. Start Amore.
4. Run `amore provenance verify` to confirm the chain is intact.

If the entire data dir is lost:
1. Re-install Amore.
2. The fresh install creates a clean data dir.
3. Optionally restore from Kopia snapshot of `%APPDATA%\Amore\`.
4. Re-run `amore init <ide>` for each IDE.

## Security ops

- Nightly `scripts/security-baseline.ps1` writes
  `%LOCALAPPDATA%\Amore\security-baselines\<date>.json`. Review weekly.
- A high-severity finding triggers an NTFY notification (if configured).
- Patch within the 30-day SLA per `SECURITY.md`.

## Performance triage

If recall latency exceeds the SLO (`docs/SLO.md`):
1. Check Qdrant collection size: `curl http://localhost:6333/collections/amore`.
2. Run `amore compact` (v0.7.0+) to dedup + evict per retention policy.
3. If still slow, opt into cluster mode (Phase H).

## Backup integration

The Anto backup stack (Hasleo + Kopia per `~/.claude/docs/backup-stack.md`)
covers `%APPDATA%\Amore\` natively. Daily Kopia snapshots = daily
Amore state snapshots. No separate Amore backup config needed.

## Out of scope here

- Threat model: `docs/THREAT-MODEL.md`
- Architecture: `docs/ARCHITECTURE.md`
- SLO targets: `docs/SLO.md`
- 100M-scale capacity math: `docs/SCALE-100M.md`
