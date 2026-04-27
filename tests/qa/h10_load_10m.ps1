#!/usr/bin/env pwsh
# tests/qa/h10_load_10m.ps1 -- H.10 10M-corpus sustained load test (100 QPS, 1 hour).
#
# Proves the production scale claim in docs/SCALE-100M.md: 10M observations,
# 100 QPS sustained recall, p95 <= 5 s, error rate <= 0.1% per docs/SLO.md.
#
# -DryRun: verify deps + run seeder with --count 100 only; skip the 1-hour oha run.
#          Used by CI / Wave 2 orchestrator to confirm harness compiles and dispatches.
#
# Exit codes:
#   0 = green (or DryRun complete)
#   1 = missing dependency (oha / amore / qdrant cluster)
#   2 = p95 over 5 s budget
#   3 = error rate over 0.1%
#   4 = corpus seeder failed

[CmdletBinding()]
param(
    [switch]$DryRun,
    [int]$CorpusSize = 10000000,
    [int]$DurationSec = 3600,
    [int]$TargetQps = 100,
    [string]$Endpoint = "http://127.0.0.1:6333"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$OutDir   = Join-Path $env:LOCALAPPDATA "Amore"
$Ts       = (Get-Date).ToUniversalTime().ToString("yyyyMMdd-HHmmss")

function Log-Step($msg) { Write-Host "[h10-load] STEP  $msg" -ForegroundColor Cyan }
function Log-Pass($msg) { Write-Host "[h10-load] PASS  $msg" -ForegroundColor Green }
function Log-Fail($msg, $code) { Write-Host "[h10-load] FAIL  $msg" -ForegroundColor Red; exit $code }
function Log-Info($msg) { Write-Host "[h10-load] INFO  $msg" }

if ($DryRun) { Log-Info "DryRun mode: deps check + 100-obs seeder sanity; 1-hour oha run SKIPPED" }

# Step 0: Verify deps
Log-Step "0. Verify deps: oha + amore + qdrant cluster at $Endpoint"

$ohaPath = (where.exe oha 2>$null) | Select-Object -First 1
if (-not $ohaPath) {
    Log-Fail "oha not found in PATH -- install via: cargo install oha" 1
}
Log-Info "oha    -> $ohaPath"

$amorePath = (where.exe amore 2>$null) | Select-Object -First 1
if (-not $amorePath) {
    Log-Fail "amore not found in PATH -- build via: cargo build --release" 1
}
Log-Info "amore  -> $amorePath"

try {
    $cluster = Invoke-RestMethod -Uri "$Endpoint/cluster" -Method GET -TimeoutSec 10
    $peerCount = if ($cluster.result -and $cluster.result.peers) { $cluster.result.peers.Count } else { 0 }
} catch {
    Log-Fail "Qdrant cluster not reachable at $Endpoint/cluster -- run H.2 smoke first" 1
}
if ($peerCount -lt 3) {
    Log-Fail "Qdrant cluster has $peerCount peers (need >= 3) -- run: ./tests/qa/h2_qdrant_cluster_smoke.ps1" 1
}
Log-Pass "deps verified: oha present, amore present, qdrant peers=$peerCount"

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# Step 1: Seed corpus
Log-Step "1. Seed corpus: $CorpusSize observations (DryRun uses 100)"

$seedCount = if ($DryRun) { 100 } else { $CorpusSize }
$seedArgs  = @(
    "run", "--release", "-p", "amore-eval", "--bin", "seed_load_test_corpus",
    "--",
    "--count", "$seedCount",
    "--endpoint", $Endpoint
)
Push-Location $RepoRoot
try {
    & cargo @seedArgs
    if ($LASTEXITCODE -ne 0) {
        Pop-Location
        Log-Fail "seed_load_test_corpus exited $LASTEXITCODE (count=$seedCount)" 4
    }
} catch {
    Pop-Location
    Log-Fail "seed_load_test_corpus threw: $_" 4
}
Pop-Location
Log-Pass "corpus seeded: $seedCount observations"

if ($DryRun) {
    Log-Pass "DryRun complete -- seeder 100-obs sanity green; 1-hour oha skipped"
    Write-Host ""
    Write-Host "To run the full 1-hour load test:"
    Write-Host "  pwsh ./tests/qa/h10_load_10m.ps1"
    exit 0
}

# Step 2: Sustained load
Log-Step "2. Sustained load: $TargetQps QPS for $DurationSec s via oha"

$ohaOut  = Join-Path $OutDir "h10-load-$Ts.json"
$body    = '{"query":"test","top_k":10}'
$ohaArgs = @(
    "-n", "unlimited",
    "-z", "${DurationSec}s",
    "-q", "$TargetQps",
    "-m", "POST",
    "-T", "application/json",
    "-d", $body,
    "--json",
    "$Endpoint/recall"
)

& oha @ohaArgs | Out-File -FilePath $ohaOut -Encoding utf8
if ($LASTEXITCODE -ne 0) {
    Log-Fail "oha exited $LASTEXITCODE -- see $ohaOut" 2
}
Log-Info "oha raw output -> $ohaOut"

# Step 3: Parse oha summary
Log-Step "3. Parse oha summary from $ohaOut"

$ohaJson = Get-Content $ohaOut -Raw | ConvertFrom-Json

# oha --json emits latency_percentiles.{p50,p95,p99,p99_9} in seconds
$p50Ms  = [math]::Round($ohaJson.latency_percentiles.p50   * 1000, 1)
$p95Ms  = [math]::Round($ohaJson.latency_percentiles.p95   * 1000, 1)
$p99Ms  = [math]::Round($ohaJson.latency_percentiles.p99   * 1000, 1)
$p999Ms = [math]::Round($ohaJson.latency_percentiles.p99_9 * 1000, 1)

$totalReqs    = $ohaJson.summary.total
$errorReqs    = $ohaJson.summary.errors
$errorRate    = if ($totalReqs -gt 0) { ($errorReqs / $totalReqs) * 100 } else { 0 }
$errorRateFmt = [math]::Round($errorRate, 4)
$achievedQps  = [math]::Round($ohaJson.summary.requests_per_sec, 1)

Log-Info "p50=${p50Ms}ms  p95=${p95Ms}ms  p99=${p99Ms}ms  p99.9=${p999Ms}ms"
Log-Info "total_reqs=$totalReqs errors=$errorReqs error_rate=${errorRateFmt}% achieved_qps=$achievedQps"

$P95LimitMs     = 5000
$ErrorRateLimit = 0.1
$minQps         = $TargetQps * 0.95

if ($p95Ms -gt $P95LimitMs) {
    Log-Fail "p95=${p95Ms}ms exceeds SLO limit of ${P95LimitMs}ms (docs/SLO.md: 1M-10M tier p95<=5s)" 2
}
Log-Pass "p95=${p95Ms}ms <= ${P95LimitMs}ms gate"

if ($errorRate -gt $ErrorRateLimit) {
    Log-Fail "error_rate=${errorRateFmt}% exceeds limit of ${ErrorRateLimit}% (docs/SLO.md availability error budget)" 3
}
Log-Pass "error_rate=${errorRateFmt}% <= ${ErrorRateLimit}% gate"

if ($achievedQps -lt $minQps) {
    Log-Fail "achieved QPS=$achievedQps < ${minQps} (95% of TargetQps=$TargetQps)" 3
}
Log-Pass "achieved_qps=$achievedQps >= ${minQps} gate"

# Step 4: Write proof JSON
Log-Step "4. Write proof JSON"

$proofPath = Join-Path $OutDir "h10-load-result-$Ts.json"
$proof = [PSCustomObject]@{
    timestamp_utc   = (Get-Date).ToUniversalTime().ToString("o")
    corpus_size     = $CorpusSize
    duration_sec    = $DurationSec
    target_qps      = $TargetQps
    achieved_qps    = $achievedQps
    p50_ms          = $p50Ms
    p95_ms          = $p95Ms
    p99_ms          = $p99Ms
    p99_9_ms        = $p999Ms
    total_requests  = $totalReqs
    error_count     = $errorReqs
    error_rate_pct  = $errorRateFmt
    p95_gate_ms     = $P95LimitMs
    p95_gate_pass   = ($p95Ms -le $P95LimitMs)
    error_rate_gate = $ErrorRateLimit
    error_rate_pass = ($errorRate -le $ErrorRateLimit)
    qps_gate_pass   = ($achievedQps -ge $minQps)
    verdict         = "PASS"
    oha_raw_output  = $ohaOut
    slo_reference   = "docs/SLO.md"
    scale_reference = "docs/SCALE-100M.md"
}
$proof | ConvertTo-Json -Depth 5 | Set-Content -Path $proofPath
Log-Pass "proof written -> $proofPath"

Log-Pass "H.10 load test PASS -- p95=${p95Ms}ms, error_rate=${errorRateFmt}%, qps=$achievedQps"
exit 0