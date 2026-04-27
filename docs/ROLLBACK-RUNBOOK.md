---
ef006_bypass: AMORE_BIGTECH_BAR_BYPASS=local-housekeeping
stable: true
---

# Amore Rollback Runbook

topic: rollback-runbook
purpose: step-by-step downgrade procedure for binary swap, tap downgrade, and config schema
stable: true
owner: Antonio Amore AKIKI

This runbook covers the full rollback path: diagnosis → snapshot → binary swap →
tap downgrade → config schema check → verify → postmortem trigger.
For single-feature regressions, prefer the feature-flag toggle path (§8) over binary rollback.

---

## Step 1 — Diagnosis

Identify what broke before taking any rollback action.

1. Check which SLO is breached:
   ```bash
   amore doctor --json
   ```
   Expected fields: `status`, `ollama`, `qdrant`, `data_dir`. Any non-`"ok"` field narrows the lane.

2. Check logs for the failure signature:
   - Windows: `%APPDATA%\Amore\amore.log`
   - Look for `ERROR` or `WARN slow_lane=` lines with timestamp.

3. Record:
   - **Current version:** `amore --version`
   - **Previous stable version:** check GitHub Releases page or `brew info amore` output
   - **SLO breached:** availability / latency / throughput / durability (docs/SLO.md)
   - **First-failure timestamp** (from log)

Only proceed to Step 2 once the breach is confirmed and scoped. Rollback without diagnosis
is forbidden per docs/POSTMORTEM-TEMPLATE.md §root-cause.

---

## Step 2 — Pre-rollback snapshot

Snapshot current state before any binary or data change. This preserves evidence for
the postmortem and enables forward-recovery if rollback introduces regression.

```powershell
# Windows — PowerShell
$ts = (Get-Date -Format "yyyyMMddTHHmmss")
$src = "$env:APPDATA\Amore"
$dst = "$env:APPDATA\Amore-snapshot-$ts"
Copy-Item -Recurse -Path $src -Destination $dst
Write-Host "Snapshot at: $dst"
```

Snapshot includes:
- sled L2 cache data directory (`$APPDATA\Amore\cache\`)
- SQLite provenance + conversation DB (`$APPDATA\Amore\amore.db`)
- Qdrant collection data (if local embedded: `$APPDATA\Amore\qdrant\`)
- WAL files (`$APPDATA\Amore\wal\`)

**Do not delete the snapshot** until the postmortem is closed and the fix is confirmed stable.

---

## Step 3 — Identify previous stable version

```bash
# Check GitHub Releases for last stable tag
gh release list --repo antonio-amore-akiki/amore --limit 5

# Or inspect local brew tap history
brew info amore | grep "stable"
```

Confirm the previous version does not have a known regression that triggered this rollback.
Cross-reference docs/RELEASE-NOTES-v*.md for the target version's known issues section.

---

## Step 4 — Binary swap (direct download path)

If Homebrew tap is unavailable, download the previous-version binary directly.

```bash
# Download previous release binary (example: v0.5.0 → rollback to v0.4.x)
VERSION="v0.4.0"  # replace with actual previous stable
ARCH="x86_64-pc-windows-msvc"

gh release download "$VERSION" \
  --repo antonio-amore-akiki/amore \
  --pattern "amore-${ARCH}.zip" \
  --dir /tmp/amore-rollback/

# Verify SHA256 signature against published checksums
gh release download "$VERSION" \
  --repo antonio-amore-akiki/amore \
  --pattern "sha256sums.txt" \
  --dir /tmp/amore-rollback/

cd /tmp/amore-rollback
sha256sum --check sha256sums.txt
# Must print: amore-<arch>.zip: OK
# Any FAILED line = abort; do not deploy the binary

# Replace running binary (stop service first)
amore service stop
cp /tmp/amore-rollback/amore.exe "$(which amore)"
```

---

## Step 5 — Tap downgrade (Homebrew path)

If Homebrew tap is available and binary swap is not needed:

```bash
brew tap antonio-amore-akiki/tap
brew install amore@<prev-version>
# e.g.: brew install amore@0.4.0

# Switch default
brew unlink amore && brew link amore@0.4.0
```

Verify:
```bash
amore --version
# Must print the target rollback version
```

---

## Step 6 — Config schema check

Between versions, config schema may have breaking changes. Check before starting the service.

1. Compare config schema versions:
   ```bash
   amore config validate --schema-version
   ```

2. If breaking change detected (schema version mismatch):
   - Locate migration script in `docs/UPGRADING.md` for the version pair.
   - Run the documented downgrade migration (schema changes are listed per version in UPGRADING.md).
   - If no downgrade migration exists: restore from snapshot (Step 2 snapshot) to preserve config.

3. State directory compatibility: sled and SQLite files are forward-compatible within a minor
   version series but may require migration across major versions. Confirm in UPGRADING.md.

---

## Step 7 — Verify rollback succeeded

```bash
# 1. Confirm version
amore --version

# 2. Health check — all lanes green
amore doctor --json
# Expected: {"status":"ok","ollama":"ok","qdrant":"ok","data_dir":"ok"}

# 3. Smoke recall against canary corpus
amore recall "test query for rollback verification" --top-k 3
# Expected: 3 results with non-zero scores; non-empty excerpts

# 4. (optional) Run eval baseline comparison
cargo run -p amore-eval -- --corpus canary --compare-baseline
# Expected: NDCG within 5% of rollback-target version's baseline
```

If any check fails: do NOT mark rollback complete. Restore from Step 2 snapshot and
escalate to GitHub Issues with full log output.

---

## Step 8 — Feature flag toggle alternative

**Prefer this path for single-feature regressions** — avoids full binary rollback risk.

If the regression is isolated to one feature, use the `AMORE_FLAG_*` env vars (W3-3A):

| Feature | Flag | Effect when off |
|---|---|---|
| Vector recall | `AMORE_FLAG_VECTOR_RECALL=off` | BM25-only mode |
| ONNX reranker | `AMORE_FLAG_RERANKER=off` | vector-similarity ranking only |
| L2 sled cache | `AMORE_FLAG_L2_CACHE=off` | L1 in-memory cache only |
| Embed pipeline | `AMORE_FLAG_EMBED=off` | ingestion disabled; recall continues |

To apply:
```bash
# Windows — set for current session
$env:AMORE_FLAG_RERANKER = "off"
amore service restart

# Or set permanently in user config
amore config set feature.reranker false
```

Document which flag was toggled, at what timestamp, and which SLO breach it was in response to.
This is an incident record for the postmortem.

**When to prefer flag over binary rollback:**
- Flag toggle takes effect in < 1 minute; binary rollback takes 10–30 minutes.
- Flag preserves all other features; binary rollback may regress unrelated behavior.
- Flag is reversible without snapshot restore.

---

## Step 9 — Postmortem trigger

After any rollback (binary or flag):

1. Open postmortem from template: docs/POSTMORTEM-TEMPLATE.md
2. File as GitHub Issue titled: `[POSTMORTEM] <date> — <brief description>`
3. Severity classification per docs/ON-CALL.md §postmortem-trigger
4. Minimum content before closing:
   - Timeline with first-failure timestamp from logs
   - Root cause (not hypothesis — evidence from logs/metrics)
   - Action items with owners and due dates
   - SLO impact (budget consumed; docs/ERROR-BUDGET-POLICY.md)

---

Source: docs/CANARY-RUNBOOK-v0.5.1.md; docs/POSTMORTEM-TEMPLATE.md; docs/ON-CALL.md;
docs/UPGRADING.md; sre.google/workbook/incident-response
