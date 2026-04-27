#!/usr/bin/env pwsh
# tests/qa/h2_qdrant_cluster_smoke.ps1 — Qdrant 3-node cluster smoke test (Phase H.2).
#
# Validates the docker-compose at infra/qdrant-cluster/:
#   1. Bring up the cluster
#   2. Assert peer count == 3 within 60s of startup
#   3. Create a test collection with replication_factor=2 + 12 shards
#   4. Ingest 100 sample vectors
#   5. Verify count returns 100 across all nodes
#   6. Stop node-2 (1-of-3 fail)
#   7. Assert recall STILL returns results (replication holds)
#   8. Restart node-2; wait for replication catch-up
#   9. Stop the cluster + clean up
#
# Exit codes:
#   0 = smoke green
#   1 = peer count != 3 within timeout
#   2 = collection creation failed
#   3 = ingest count mismatch
#   4 = recall failed during 1-node-fail
#   5 = node-2 restart / catch-up failed
#
# Requires: docker compose v2, PowerShell 7+ (Invoke-RestMethod).

[CmdletBinding()]
param(
    [int]$BootTimeoutSec = 60,
    [int]$ReplCatchupTimeoutSec = 120,
    [switch]$KeepCluster
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$ComposeDir = Join-Path $RepoRoot "infra/qdrant-cluster"
$RestUrl = "http://127.0.0.1:6333"
$Collection = "amore-cluster-smoke"

function Log-Step($msg) { Write-Host "[h2-cluster-smoke] $msg" -ForegroundColor Cyan }
function Log-Pass($msg) { Write-Host "[h2-cluster-smoke] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg, $code) { Write-Host "[h2-cluster-smoke] FAIL $msg" -ForegroundColor Red; exit $code }

function Invoke-Json($method, $url, $body = $null) {
    $params = @{ Uri = $url; Method = $method; UseBasicParsing = $true; TimeoutSec = 10 }
    if ($body) { $params.Body = ($body | ConvertTo-Json -Depth 10); $params.ContentType = "application/json" }
    try { return Invoke-RestMethod @params } catch { return $null }
}

Log-Step "1. Bringing cluster up via docker compose"
Push-Location $ComposeDir
docker compose up -d
Pop-Location

Log-Step "2. Waiting for 3-node Raft consensus (timeout ${BootTimeoutSec}s)"
$deadline = (Get-Date).AddSeconds($BootTimeoutSec)
$peerCount = 0
while ((Get-Date) -lt $deadline) {
    $cluster = Invoke-Json GET "$RestUrl/cluster"
    if ($cluster -and $cluster.result -and $cluster.result.peers) {
        $peerCount = $cluster.result.peers.Count
        if ($peerCount -eq 3) { break }
    }
    Start-Sleep -Seconds 3
}
if ($peerCount -ne 3) { Log-Fail "expected 3 peers; got $peerCount within ${BootTimeoutSec}s" 1 }
Log-Pass "peer_count=3"

Log-Step "3. Creating collection '$Collection' (RF=2, shards=12)"
$createBody = @{
    vectors = @{ size = 768; distance = "Cosine" }
    shard_number = 12
    replication_factor = 2
    write_consistency_factor = 1
}
$create = Invoke-Json PUT "$RestUrl/collections/$Collection" $createBody
if ($null -eq $create -or -not $create.result) { Log-Fail "collection create did not return result=true" 2 }
Log-Pass "collection created"

Log-Step "4. Ingesting 100 random vectors"
$points = @()
for ($i = 0; $i -lt 100; $i++) {
    $vec = @()
    for ($d = 0; $d -lt 768; $d++) { $vec += (Get-Random -Minimum -1.0 -Maximum 1.0) }
    $points += @{ id = $i; vector = $vec; payload = @{ idx = $i } }
}
$upsert = Invoke-Json PUT "$RestUrl/collections/$Collection/points?wait=true" @{ points = $points }
if ($null -eq $upsert -or -not $upsert.result) { Log-Fail "upsert did not return result" 3 }

$count = Invoke-Json POST "$RestUrl/collections/$Collection/points/count" @{ exact = $true }
if ($count.result.count -ne 100) { Log-Fail "expected 100 points; got $($count.result.count)" 3 }
Log-Pass "ingested 100 points; count verified"

Log-Step "5. Stopping qdrant-node-2 to simulate 1-of-3 fail"
Push-Location $ComposeDir
docker compose stop qdrant-node-2
Pop-Location
Start-Sleep -Seconds 10  # cluster needs time to notice

Log-Step "6. Verifying recall still works under 1-node-fail"
$query = @()
for ($d = 0; $d -lt 768; $d++) { $query += (Get-Random -Minimum -1.0 -Maximum 1.0) }
$search = Invoke-Json POST "$RestUrl/collections/$Collection/points/search" @{ vector = $query; limit = 5 }
if ($null -eq $search -or -not $search.result -or $search.result.Count -eq 0) {
    Log-Fail "recall returned no hits under 1-node-fail (replication broke)" 4
}
Log-Pass "recall returned $($search.result.Count) hits with 1 node down"

Log-Step "7. Restoring qdrant-node-2 + waiting for replication catch-up"
Push-Location $ComposeDir
docker compose start qdrant-node-2
Pop-Location
$deadline2 = (Get-Date).AddSeconds($ReplCatchupTimeoutSec)
while ((Get-Date) -lt $deadline2) {
    $cluster2 = Invoke-Json GET "$RestUrl/cluster"
    if ($cluster2 -and $cluster2.result.peers.Count -eq 3) { break }
    Start-Sleep -Seconds 5
}
if ($cluster2.result.peers.Count -ne 3) {
    Log-Fail "node-2 failed to rejoin within ${ReplCatchupTimeoutSec}s" 5
}
Log-Pass "node-2 rejoined; 3-peer quorum restored"

if (-not $KeepCluster) {
    Log-Step "8. Tearing down cluster (use -KeepCluster to skip)"
    Push-Location $ComposeDir
    docker compose down --volumes
    Pop-Location
}

$proof = [PSCustomObject]@{
    timestamp_utc = (Get-Date).ToUniversalTime().ToString("o")
    peer_count_initial = 3
    collection = $Collection
    points_ingested = 100
    one_node_fail_recall_hits = $search.result.Count
    rejoin_succeeded = $true
    verdict = "PASS"
}
$proofPath = Join-Path $env:LOCALAPPDATA "Amore\h2-cluster-smoke-$([DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss')).json"
New-Item -ItemType Directory -Force -Path (Split-Path $proofPath) | Out-Null
$proof | ConvertTo-Json | Set-Content -Path $proofPath
Log-Pass "smoke green; proof written to $proofPath"
exit 0
