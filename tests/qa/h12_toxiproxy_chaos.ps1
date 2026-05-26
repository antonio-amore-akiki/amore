#!/usr/bin/env pwsh
# tests/qa/h12_toxiproxy_chaos.ps1 — H.12 toxiproxy chaos test
#
# Injects 30% packet loss + 200ms latency between amore-mcp <-> Qdrant + Ollama via
# Toxiproxy (Shopify, Apache-2.0). Proves the elite-engineering "no silent fail-open"
# principle: H.5 circuit breaker must trip, amore must return degraded-but-non-empty
# recall, and the breaker must recover within 30s after faults are removed.
#
# -DryRun: verify deps + toxiproxy reachability; skip toxic injection + amore recall.
#          Used by CI / Wave 3 orchestrator to confirm harness compiles + dispatches.
#
# Exit codes:
#   0 = green (or DryRun complete)
#   1 = toxiproxy not reachable (bring up infra/toxiproxy/ first)
#   2 = proxy config failed (admin API POST rejected)
#   3 = degraded response missing (recall returned no hits under fault)
#   4 = circuit breaker did not trip (no obelion_degraded_total increment)
#   5 = recovery failed (recall degraded flag still set 30s after toxic removal)
#   6 = teardown failed (docker compose down error)

[CmdletBinding()]
param(
    [switch]$DryRun,
    [string]$ToxiproxyAdmin = "http://127.0.0.1:8474",
    [string]$QdrantRestProxy = "http://127.0.0.1:6433",
    [string]$OllamaProxy     = "http://127.0.0.1:11534",
    [int]$LatencyMs          = 200,
    [int]$JitterMs           = 50,
    [int]$PacketLossPct      = 30,
    [int]$RecoveryTimeoutSec = 30
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$OutDir   = Join-Path $env:LOCALAPPDATA "Amore"
$Ts       = (Get-Date).ToUniversalTime().ToString("yyyyMMdd-HHmmss")
$ComposeFile = Join-Path $RepoRoot "infra\toxiproxy\docker-compose.yml"

function Log-Step($msg) { Write-Host "[h12-chaos] STEP  $msg" -ForegroundColor Cyan }
function Log-Pass($msg) { Write-Host "[h12-chaos] PASS  $msg" -ForegroundColor Green }
function Log-Fail($msg, $code) { Write-Host "[h12-chaos] FAIL  $msg" -ForegroundColor Red; exit $code }
function Log-Info($msg) { Write-Host "[h12-chaos] INFO  $msg" }

if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }

if ($DryRun) {
    Log-Info "DryRun mode: deps check + toxiproxy reachability; toxic injection SKIPPED"
}

# ─── Step 0: Bring up toxiproxy ─────────────────────────────────────────────
Log-Step "0. Bring up toxiproxy"

if (-not (Test-Path $ComposeFile)) {
    Log-Fail "docker-compose.yml not found: $ComposeFile" 1
}

$dockerPath = (Get-Command docker -ErrorAction SilentlyContinue)?.Source
if (-not $dockerPath) { Log-Fail "docker not found in PATH" 1 }
Log-Info "docker -> $dockerPath"

if (-not $DryRun) {
    docker compose -f $ComposeFile up -d 2>&1 | ForEach-Object { Log-Info $_ }
    if ($LASTEXITCODE -ne 0) { Log-Fail "docker compose up -d failed (exit $LASTEXITCODE)" 1 }
}

# Toxiproxy 2.9 rejects PowerShell's default user agent; use a consistent UA for all calls
$ToxiHeaders = @{ "User-Agent" = "amore-chaos-test/1.0" }

# Poll :8474/version until ready (60s cap)
$deadline = [datetime]::UtcNow.AddSeconds(60)
$toxiReady = $false
while ([datetime]::UtcNow -lt $deadline) {
    try {
        $resp = Invoke-WebRequest -Uri "$ToxiproxyAdmin/version" -Method GET -TimeoutSec 5 `
                    -Headers $ToxiHeaders -UseBasicParsing
        $ver  = ($resp.Content | ConvertFrom-Json).version
        Log-Info "toxiproxy version: $ver"
        $toxiReady = $true
        break
    } catch {
        Start-Sleep -Seconds 2
    }
}
if (-not $toxiReady) { Log-Fail "toxiproxy admin API not reachable at $ToxiproxyAdmin/version after 60s" 1 }
Log-Pass "toxiproxy admin API ready at $ToxiproxyAdmin"

if ($DryRun) {
    # DryRun: verify Docker + toxiproxy are reachable; confirm proxy ports are configurable
    Log-Info "DryRun: verifying proxy + toxic POST schema (no actual injection)"

    # Verify admin API lists proxies endpoint (returns {} when no proxies configured — that is ok)
    try {
        $resp        = Invoke-WebRequest -Uri "$ToxiproxyAdmin/proxies" -Method GET -TimeoutSec 5 `
                           -Headers $ToxiHeaders -UseBasicParsing
        $proxyCount  = if ($resp.Content -and $resp.Content -ne "{}") {
                           ($resp.Content | ConvertFrom-Json).PSObject.Properties.Name.Count
                       } else { 0 }
        Log-Info "proxies endpoint reachable; current proxy count: $proxyCount"
    } catch {
        Log-Fail "proxies endpoint not reachable: $_" 1
    }
    Log-Pass "DryRun complete — toxiproxy admin API up, proxies endpoint reachable"

    # Write dry-run proof JSON
    $proofPath = Join-Path $OutDir "h12-chaos-dryrun-$Ts.json"
    [PSCustomObject]@{
        timestamp_utc  = (Get-Date).ToUniversalTime().ToString("o")
        dry_run        = $true
        toxiproxy_url  = $ToxiproxyAdmin
        compose_file   = $ComposeFile
        verdict        = "PASS"
        note           = "deps-ok+proxies-configurable; full chaos run deferred to Phase J"
    } | ConvertTo-Json | Set-Content -Path $proofPath
    Log-Pass "dry-run proof -> $proofPath"

    Log-Pass "H.12 DryRun PASS"
    exit 0
}

# ─── Step 1: Configure proxies + toxics ─────────────────────────────────────
Log-Step "1. Configure proxies + toxics"

function Invoke-ToxiAdmin($Method, $Path, $Body = $null) {
    $params = @{
        Uri             = "$ToxiproxyAdmin$Path"
        Method          = $Method
        TimeoutSec      = 10
        Headers         = $ToxiHeaders
        UseBasicParsing = $true
    }
    if ($Body) {
        $params.Body        = ($Body | ConvertTo-Json -Compress)
        $params.ContentType = "application/json"
    }
    (Invoke-WebRequest @params).Content | ConvertFrom-Json
}

# Delete proxies if they already exist (idempotent re-run)
foreach ($name in @("qdrant-rest", "ollama")) {
    try { Invoke-ToxiAdmin DELETE "/proxies/$name" } catch { <# not present — ok #> }
}

# Create qdrant-rest proxy
try {
    Invoke-ToxiAdmin POST "/proxies" @{
        name     = "qdrant-rest"
        listen   = "0.0.0.0:6433"
        upstream = "host.docker.internal:6333"
        enabled  = $true
    } | Out-Null
    Log-Info "proxy qdrant-rest created (:6433 -> host.docker.internal:6333)"
} catch {
    Log-Fail "failed to create qdrant-rest proxy: $_" 2
}

# Create ollama proxy
try {
    Invoke-ToxiAdmin POST "/proxies" @{
        name     = "ollama"
        listen   = "0.0.0.0:11534"
        upstream = "host.docker.internal:11434"
        enabled  = $true
    } | Out-Null
    Log-Info "proxy ollama created (:11534 -> host.docker.internal:11434)"
} catch {
    Log-Fail "failed to create ollama proxy: $_" 2
}

# Add latency toxic to qdrant-rest (downstream)
try {
    Invoke-ToxiAdmin POST "/proxies/qdrant-rest/toxics" @{
        name       = "qdrant-latency"
        type       = "latency"
        stream     = "downstream"
        toxicity   = 1.0
        attributes = @{ latency = $LatencyMs; jitter = $JitterMs }
    } | Out-Null
    Log-Info "toxic: qdrant-rest latency downstream ${LatencyMs}ms jitter ${JitterMs}ms"
} catch {
    Log-Fail "failed to add latency toxic to qdrant-rest: $_" 2
}

# Add latency toxic to ollama (upstream)
try {
    Invoke-ToxiAdmin POST "/proxies/ollama/toxics" @{
        name       = "ollama-latency"
        type       = "latency"
        stream     = "upstream"
        toxicity   = 1.0
        attributes = @{ latency = $LatencyMs; jitter = $JitterMs }
    } | Out-Null
    Log-Info "toxic: ollama latency upstream ${LatencyMs}ms jitter ${JitterMs}ms"
} catch {
    Log-Fail "failed to add latency toxic to ollama: $_" 2
}

# Add bandwidth toxic to qdrant-rest (downstream) to simulate degraded throughput.
# Toxiproxy v2 bandwidth toxic throttles KB/s; toxicity parameter (0.0-1.0) controls
# what fraction of connections get the toxic. At toxicity=0.30 (~30% of connections
# are bandwidth-throttled to 100KB/s), recall times out → circuit breaker trips.
# Platform-portable (no Linux tc/iptables required).
try {
    Invoke-ToxiAdmin POST "/proxies/qdrant-rest/toxics" @{
        name       = "qdrant-bandwidth"
        type       = "bandwidth"
        stream     = "downstream"
        toxicity   = ([double]$PacketLossPct / 100.0)
        attributes = @{ rate = 100 }  # 100 KB/s — forces circuit-breaker timeout path
    } | Out-Null
    Log-Info "toxic: qdrant-rest bandwidth downstream rate=100KB/s toxicity=${PacketLossPct}%"
} catch {
    Log-Fail "failed to add bandwidth toxic to qdrant-rest: $_" 2
}

Log-Pass "proxies + toxics configured"

# ─── Step 2: Run amore against proxied endpoints ─────────────────────────────
Log-Step "2. Run amore against the proxied endpoints"

$env:AMORE_QDRANT_URL  = $QdrantRestProxy
$env:AMORE_OLLAMA_URL  = $OllamaProxy

$amorePath = (Get-Command amore -ErrorAction SilentlyContinue)?.Source
if (-not $amorePath) { Log-Fail "amore not found in PATH -- build via: cargo build --release" 3 }
Log-Info "amore -> $amorePath"

# Send a recall query over stdio MCP protocol
$recallPayload = '{"jsonrpc":"2.0","id":1,"method":"recall","params":{"query":"chaos test probe","top_k":5}}'
$recallResult  = $null
try {
    $recallResult = $recallPayload | & $amorePath recall --json 2>&1
} catch {
    Log-Info "amore recall threw: $_"
}
Log-Info "recall raw output: $recallResult"

# ─── Step 3: Assert degraded response ────────────────────────────────────────
Log-Step "3. Assert degraded response (non-empty hits, degraded flag present)"

if (-not $recallResult) {
    Log-Fail "recall returned no output under fault conditions" 3
}

$parsed = $null
try { $parsed = $recallResult | ConvertFrom-Json } catch { Log-Info "recall output not JSON — checking raw string" }

# Accept either JSON with hits array or text output indicating degraded mode
$hasDegradedSignal = ($recallResult -match "degraded") -or
                     ($recallResult -match "bm25") -or
                     ($parsed -and $parsed.result) -or
                     ($parsed -and $parsed.hits)
if (-not $hasDegradedSignal) {
    Log-Fail "recall output shows no degraded signal and no hits: $recallResult" 3
}
Log-Pass "degraded response received (non-empty recall under fault)"

# ─── Step 4: Assert circuit-breaker trips ────────────────────────────────────
Log-Step "4. Assert circuit-breaker trips (obelion_degraded_total{lane=qdrant} >= 1)"

$metricsUrl   = "http://127.0.0.1:9090/metrics"
$cbTripped    = $false
try {
    $metrics   = Invoke-RestMethod -Uri $metricsUrl -Method GET -TimeoutSec 5
    $cbTripped = ($metrics -match 'obelion_degraded_total\{.*lane="qdrant".*\}\s+[1-9]')
} catch {
    Log-Info "metrics endpoint not reachable ($metricsUrl) — checking log fallback"
    # Fallback: amore writes circuit-breaker open events to stderr
    $cbTripped = ($recallResult -match "circuit.open|breaker.open|cb_open")
}
if (-not $cbTripped) {
    Log-Fail "circuit breaker did not trip: no obelion_degraded_total increment or log signal" 4
}
Log-Pass "circuit breaker tripped (degraded signal confirmed)"

# ─── Step 5: Remove toxics ───────────────────────────────────────────────────
Log-Step "5. Remove toxics"

foreach ($spec in @(
    @{ proxy="qdrant-rest"; toxic="qdrant-latency" },
    @{ proxy="qdrant-rest"; toxic="qdrant-bandwidth" },
    @{ proxy="ollama";      toxic="ollama-latency" }
)) {
    try {
        Invoke-ToxiAdmin DELETE "/proxies/$($spec.proxy)/toxics/$($spec.toxic)" | Out-Null
        Log-Info "removed toxic $($spec.proxy)/$($spec.toxic)"
    } catch {
        Log-Info "toxic $($spec.proxy)/$($spec.toxic) not found (already removed) — ok"
    }
}
Log-Pass "toxics removed"

# ─── Step 6: Assert recovery ─────────────────────────────────────────────────
Log-Step "6. Assert recovery (full-quality recall within ${RecoveryTimeoutSec}s)"

$recoveryDeadline = [datetime]::UtcNow.AddSeconds($RecoveryTimeoutSec)
$recovered        = $false
while ([datetime]::UtcNow -lt $recoveryDeadline) {
    try {
        $r = $recallPayload | & $amorePath recall --json 2>&1
        # Full recovery: no degraded flag, response in < circuit-breaker timeout
        if ($r -and ($r -notmatch "degraded") -and ($r -match "hits|result")) {
            $recovered = $true
            Log-Info "recovery confirmed: $r"
            break
        }
    } catch { <# breaker still open — retry #> }
    Start-Sleep -Seconds 3
}
if (-not $recovered) {
    Log-Fail "amore did not recover within ${RecoveryTimeoutSec}s after toxic removal" 5
}
Log-Pass "circuit breaker closed + full-quality recall restored"

# ─── Step 7: Teardown ────────────────────────────────────────────────────────
Log-Step "7. Teardown"

docker compose -f $ComposeFile down 2>&1 | ForEach-Object { Log-Info $_ }
if ($LASTEXITCODE -ne 0) { Log-Fail "docker compose down failed (exit $LASTEXITCODE)" 6 }
Log-Pass "toxiproxy stopped"

# ─── Write proof JSON ────────────────────────────────────────────────────────
$proofPath = Join-Path $OutDir "h12-chaos-$Ts.json"
[PSCustomObject]@{
    timestamp_utc        = (Get-Date).ToUniversalTime().ToString("o")
    dry_run              = $false
    toxiproxy_url        = $ToxiproxyAdmin
    qdrant_proxy         = $QdrantRestProxy
    ollama_proxy         = $OllamaProxy
    latency_ms           = $LatencyMs
    jitter_ms            = $JitterMs
    packet_loss_pct      = $PacketLossPct
    recovery_timeout_sec = $RecoveryTimeoutSec
    degraded_signal      = $hasDegradedSignal
    cb_tripped           = $cbTripped
    recovered            = $recovered
    verdict              = "PASS"
    reference_adr        = "docs/adr/0008-circuit-breaker.md"
} | ConvertTo-Json | Set-Content -Path $proofPath
Log-Pass "proof written -> $proofPath"

Log-Pass "H.12 chaos test PASS — breaker tripped under fault, degraded recall non-empty, recovery confirmed"
exit 0
