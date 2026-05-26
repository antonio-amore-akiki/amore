#!/usr/bin/env pwsh
# tests/qa/h7_snapshot_restore_smoke.ps1 - H.7 snapshot/restore smoke test.
#
# Steps:
#   1. Verify Qdrant reachable on 127.0.0.1:6333
#   2. Ingest 100 known docs into a test collection
#   3. amore snapshot create <path>
#   4. Delete the test collection (simulate wipe)
#   5. amore snapshot restore <path>
#   6. Re-run top-10 recall -- assert identical set
#   7. Write proof JSON to $env:LOCALAPPDATA\Amore\h7-snapshot-<ts>.json
#
# Exit codes:
#   0 = green
#   1 = create failed
#   2 = restore failed
#   3 = recall mismatch after restore
#
# Requires: Qdrant on 127.0.0.1:6333, amore.exe in PATH or ./target/release/.

[CmdletBinding()]
param(
    [string]$ExePath  = ".\target\release\amore.exe",
    [string]$SnapPath = "$env:TEMP\amore-snap-$([System.DateTime]::UtcNow.ToString('yyyyMMddHHmmss')).tar.gz",
    [string]$DataDir  = "$env:LOCALAPPDATA\Amore"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RestUrl    = "http://127.0.0.1:6333"
$Collection = "amore-h7-smoke"
$VecDim     = 768

function Log-Step($msg) { Write-Host "[h7-snapshot-smoke] $msg" -ForegroundColor Cyan }
function Log-Pass($msg) { Write-Host "[h7-snapshot-smoke] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg, $code) {
    Write-Host "[h7-snapshot-smoke] FAIL $msg" -ForegroundColor Red
    exit $code
}

function Invoke-Json($method, $url, $body = $null) {
    $params = @{ Uri = $url; Method = $method; UseBasicParsing = $true; TimeoutSec = 30 }
    if ($body) {
        $params.Body        = ($body | ConvertTo-Json -Depth 10)
        $params.ContentType = "application/json"
    }
    try   { return Invoke-RestMethod @params }
    catch { return $null }
}

# Step 1: Qdrant reachable
Log-Step "1. Checking Qdrant on $RestUrl"
$rdy = Invoke-Json GET "$RestUrl/readyz"
if ($null -eq $rdy) { Log-Fail "Qdrant not reachable on $RestUrl" 1 }
Log-Pass "Qdrant ready"

# Step 2: Ingest 100 known docs
Log-Step "2. Creating test collection '$Collection'"
$colBody = @{ vectors = @{ size = $VecDim; distance = "Cosine" } }
$col     = Invoke-Json PUT "$RestUrl/collections/$Collection" $colBody
if ($null -eq $col -or -not $col.result) {
    $existing = Invoke-Json GET "$RestUrl/collections/$Collection"
    if ($null -eq $existing) { Log-Fail "Could not create or find collection '$Collection'" 1 }
}
Log-Pass "collection ready"

Log-Step "2b. Ingesting 100 deterministic vectors"
$points = @()
for ($i = 0; $i -lt 100; $i++) {
    $vec = @()
    for ($d = 0; $d -lt $VecDim; $d++) {
        $vec += [double](( ($i -bxor $d) % 2 ) - 0.5) * 0.1
    }
    $points += @{ id = $i; vector = $vec; payload = @{ doc_id = $i } }
}
$upsert = Invoke-Json PUT "$RestUrl/collections/$Collection/points?wait=true" @{ points = $points }
if ($null -eq $upsert -or -not $upsert.result) { Log-Fail "upsert failed" 1 }

$cnt = Invoke-Json POST "$RestUrl/collections/$Collection/points/count" @{ exact = $true }
if ($cnt.result.count -ne 100) { Log-Fail "expected 100 points; got $($cnt.result.count)" 1 }
Log-Pass "100 docs ingested"

# Step 3: Capture top-10 pre-snapshot
Log-Step "3. Capturing top-10 recall IDs pre-snapshot"
$queryVec = @(0..($VecDim - 1) | ForEach-Object { 0.05 })
$preSrch  = Invoke-Json POST "$RestUrl/collections/$Collection/points/search" @{
    vector = $queryVec; limit = 10; with_payload = $false
}
if ($null -eq $preSrch -or $preSrch.result.Count -eq 0) {
    Log-Fail "pre-snapshot recall returned no hits" 1
}
$preIds = $preSrch.result | ForEach-Object { $_.id } | Sort-Object
Log-Pass "pre-snapshot top-10 IDs: $($preIds -join ',')"

# Step 4: amore snapshot create
Log-Step "4. Running: $ExePath snapshot create $SnapPath --data-dir $DataDir"
$createOut = & $ExePath snapshot create $SnapPath --data-dir $DataDir 2>&1
if ($LASTEXITCODE -ne 0) { Log-Fail "snapshot create exited $LASTEXITCODE: $createOut" 1 }
if (-not (Test-Path $SnapPath)) { Log-Fail "archive not found at $SnapPath" 1 }
$sidecar = "$SnapPath.sha256"
if (-not (Test-Path $sidecar)) { Log-Fail "sha256 sidecar not found at $sidecar" 1 }
Log-Pass "snapshot create OK; archive $([int](Get-Item $SnapPath).Length) bytes"

# Step 5: Wipe the test collection
Log-Step "5. Deleting collection '$Collection' (simulated wipe)"
$del = Invoke-Json DELETE "$RestUrl/collections/$Collection"
if ($null -eq $del) { Log-Fail "delete collection returned null" 2 }
Log-Pass "collection deleted"

# Step 6: amore snapshot restore
Log-Step "6. Running: $ExePath snapshot restore $SnapPath --data-dir $DataDir"
$restoreOut = & $ExePath snapshot restore $SnapPath --data-dir $DataDir 2>&1
if ($LASTEXITCODE -ne 0) { Log-Fail "snapshot restore exited $LASTEXITCODE: $restoreOut" 2 }
Log-Pass "snapshot restore OK"
Start-Sleep -Seconds 3

# Step 7: Re-run recall and assert top-10 identical
Log-Step "7. Re-running top-10 recall post-restore"
$postSrch = Invoke-Json POST "$RestUrl/collections/$Collection/points/search" @{
    vector = $queryVec; limit = 10; with_payload = $false
}
if ($null -eq $postSrch -or $postSrch.result.Count -eq 0) {
    Log-Fail "post-restore recall returned no hits" 3
}
$postIds = $postSrch.result | ForEach-Object { $_.id } | Sort-Object
$same    = (($preIds | Compare-Object $postIds) -eq $null) -and ($preIds.Count -eq $postIds.Count)
if (-not $same) {
    Log-Fail "top-10 mismatch. pre=[$($preIds -join ',')] post=[$($postIds -join ',')]" 3
}
Log-Pass "top-10 identical post-restore"

# Step 8: Write proof JSON
$ts       = [System.DateTime]::UtcNow.ToString("yyyyMMdd-HHmmss")
$proofDir = "$env:LOCALAPPDATA\Amore"
New-Item -ItemType Directory -Force -Path $proofDir | Out-Null
$proofPath = "$proofDir\h7-snapshot-$ts.json"
[PSCustomObject]@{
    timestamp_utc      = [System.DateTime]::UtcNow.ToString("o")
    collection         = $Collection
    points_ingested    = 100
    pre_snapshot_top10 = $preIds
    post_restore_top10 = $postIds
    top10_match        = $true
    archive_path       = $SnapPath
    archive_bytes      = ([int](Get-Item $SnapPath).Length)
    verdict            = "PASS"
} | ConvertTo-Json | Set-Content -Path $proofPath

Log-Pass "smoke green; proof written to $proofPath"
exit 0
