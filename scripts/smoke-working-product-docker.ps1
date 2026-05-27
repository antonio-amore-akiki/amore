# scripts/smoke-working-product-docker.ps1 — Phase B full-stack working-product smoke.
#
# Prior-art: Adapt from scripts/run-longmemeval.ps1 (daemon startup + cargo run pattern),
# tests/qa/lib/ensure_daemons.ps1 (health polling helpers). See
# ~/.claude/state/prior-art-verdict.json entry smoke-working-product-docker.ps1 2026-05-27T20:14:33Z.
#
# Steps:
#   1. Ensure Docker daemon is live (calls recover-docker.ps1 if needed)
#   2. Start qdrant container (qdrant/qdrant:v1.18.0, idempotent — pinned to qdrant-client crate minor)
#   3. Start ollama natively (no-op if already running)
#   4. Pull nomic-embed-text model
#   5. Build amore-mcp if missing
#   6. Run cargo test --test working_product_docker (the Rust integration test)
#   7. Write state/working-product-smoke-docker.json with verdict + timings
#
# Exit codes:
#   0  All 3 services live + store+recall round-trip green
#   1  Any step failed; diagnostic printed to stderr

param(
    [switch]$SkipDockerRestart,   # Skip Step 1 if daemon already confirmed up
    [switch]$SkipBuild            # Skip Step 5 cargo build if binary already present
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot  = (Resolve-Path "$PSScriptRoot\..").Path
$StateDir  = "$RepoRoot\state"
$SmokeOut  = "$StateDir\working-product-smoke-docker.json"
$OllamaExe = "$env:USERPROFILE\AppData\Local\Programs\Ollama\ollama.exe"
$QdrantImg = "qdrant/qdrant:v1.18.0"  # pinned to match qdrant-client 1.18.0 in Cargo.toml — must stay within ±1 minor of the Rust client version, else gRPC operations error "Client version X is not compatible with server version Y"
$ContainerName = "amore-qdrant"
$ScriptStart = Get-Date

if (-not (Test-Path $StateDir)) { New-Item -ItemType Directory -Force $StateDir | Out-Null }

function Write-Step([string]$Msg) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Msg"
}

function Test-Endpoint([string]$Url) {
    try {
        $r = Invoke-WebRequest -Uri $Url -TimeoutSec 2 -UseBasicParsing
        return $r.StatusCode -eq 200
    } catch { return $false }
}

function Poll-Until-Ready([string]$Url, [string]$Label, [int]$TimeoutSec) {
    for ($i = 0; $i -lt $TimeoutSec; $i++) {
        if (Test-Endpoint $Url) { Write-Step "  $Label up after $i s"; return $true }
        Start-Sleep -Seconds 1
    }
    return $false
}

function Elapsed-Seconds { return [int]((Get-Date) - $ScriptStart).TotalSeconds }

$timings = @{}

# ─── Step 1: Ensure Docker daemon is live ─────────────────────────────────────
Write-Step "PHASE B: Working-product full-stack smoke"
Write-Step "Step 1 — Checking Docker daemon..."

$t0 = Get-Date
$ver = & docker version --format "{{.Server.Version}}" 2>$null
if (-not ($LASTEXITCODE -eq 0 -and $ver -and $ver.Trim() -ne "")) {
    if (-not $SkipDockerRestart) {
        Write-Step "  Docker daemon not ready. Running recover-docker.ps1..."
        & "$PSScriptRoot\recover-docker.ps1"
        if ($LASTEXITCODE -ne 0) {
            Write-Error "FATAL: Docker recovery failed. Cannot proceed."
            exit 1
        }
    } else {
        Write-Error "FATAL: Docker daemon not ready and -SkipDockerRestart set."
        exit 1
    }
}
$timings["docker_ready_s"] = [int]((Get-Date) - $t0).TotalSeconds
Write-Step "  Docker daemon confirmed. ($($timings['docker_ready_s'])s)"

# ─── Step 2: Start qdrant container (idempotent) ──────────────────────────────
Write-Step "Step 2 — Starting qdrant container ($QdrantImg)..."

$t0 = Get-Date
# Remove stale container (idempotent)
& docker rm -f $ContainerName 2>$null

# Start fresh
$dockerOut = & docker run -d --name $ContainerName `
    -p 6333:6333 -p 6334:6334 `
    -v amore-qdrant-data:/qdrant/storage `
    $QdrantImg 2>&1
Write-Step "  Container ID: $($dockerOut | Select-Object -First 1)"

# Poll HTTP health
$qdrantReady = Poll-Until-Ready "http://localhost:6333/healthz" "qdrant" 30
if (-not $qdrantReady) {
    Write-Error "FATAL: Qdrant container did not become healthy within 30s."
    & docker logs $ContainerName 2>&1 | Select-Object -Last 10 | ForEach-Object { Write-Step "  [qdrant] $_" }
    exit 1
}
$timings["qdrant_ready_s"] = [int]((Get-Date) - $t0).TotalSeconds
Write-Step "  Qdrant healthy. ($($timings['qdrant_ready_s'])s)"

# ─── Step 3: Start ollama natively ────────────────────────────────────────────
Write-Step "Step 3 — Starting ollama natively..."

$t0 = Get-Date
if (-not (Test-Path $OllamaExe)) {
    Write-Error "FATAL: Ollama not found at $OllamaExe. Install via https://ollama.com/download"
    exit 1
}

if (-not (Test-Endpoint "http://127.0.0.1:11434/api/tags")) {
    Write-Step "  Ollama not running. Starting..."
    Start-Process -FilePath $OllamaExe -ArgumentList "serve" -WindowStyle Hidden `
        -RedirectStandardOutput "$env:TEMP\ollama-smoke.out" `
        -RedirectStandardError  "$env:TEMP\ollama-smoke.err"

    $ollamaReady = Poll-Until-Ready "http://127.0.0.1:11434/api/tags" "ollama" 30
    if (-not $ollamaReady) {
        Write-Error "FATAL: Ollama did not come up within 30s."
        Get-Content "$env:TEMP\ollama-smoke.err" -ErrorAction SilentlyContinue | Select-Object -Last 10
        exit 1
    }
} else {
    Write-Step "  Ollama already running."
}
$timings["ollama_ready_s"] = [int]((Get-Date) - $t0).TotalSeconds
Write-Step "  Ollama ready. ($($timings['ollama_ready_s'])s)"

# ─── Step 4: Pull nomic-embed-text ────────────────────────────────────────────
Write-Step "Step 4 — Pulling nomic-embed-text (cap 90s)..."

$t0 = Get-Date
$pullJob = Start-Job -ScriptBlock {
    param($exe)
    & $exe pull nomic-embed-text 2>&1
} -ArgumentList $OllamaExe

$pulled = $false
for ($i = 0; $i -lt 90; $i++) {
    if ($pullJob.State -ne "Running") { $pulled = $true; break }
    Start-Sleep -Seconds 1
}
$pullOut = Receive-Job $pullJob
Remove-Job $pullJob -Force

if (-not $pulled) {
    Write-Error "FATAL: ollama pull nomic-embed-text timed out after 90s."
    exit 1
}
$timings["model_pull_s"] = [int]((Get-Date) - $t0).TotalSeconds
Write-Step "  nomic-embed-text ready. ($($timings['model_pull_s'])s)"

# ─── Step 5: Build amore-mcp if missing ───────────────────────────────────────
Write-Step "Step 5 — Checking amore-mcp binary..."

$t0 = Get-Date
$amcpBin = "$RepoRoot\target\release\amore-mcp.exe"
if (-not (Test-Path $amcpBin) -and -not $SkipBuild) {
    Write-Step "  Binary missing. Building..."
    Push-Location $RepoRoot
    try {
        & cargo build --release -p amore-mcp 2>&1 | ForEach-Object { Write-Step "  $_" }
        if ($LASTEXITCODE -ne 0) { Write-Error "FATAL: cargo build amore-mcp failed."; exit 1 }
    } finally { Pop-Location }
} elseif (Test-Path $amcpBin) {
    Write-Step "  Binary present: $amcpBin"
} else {
    Write-Error "FATAL: amore-mcp.exe not found and -SkipBuild set."
    exit 1
}
$timings["build_s"] = [int]((Get-Date) - $t0).TotalSeconds

# ─── Step 6: Run Rust integration test ────────────────────────────────────────
Write-Step "Step 6 — Running cargo test --test working_product_docker..."

$t0 = Get-Date
Push-Location $RepoRoot
try {
    $testOut = & cargo test --release --test working_product_docker -- --nocapture 2>&1
    $testExit = $LASTEXITCODE
    $testOut | ForEach-Object { Write-Step "  $_" }
} finally { Pop-Location }
$timings["test_s"] = [int]((Get-Date) - $t0).TotalSeconds

if ($testExit -ne 0) {
    Write-Error "FATAL: working_product_docker test failed (exit=$testExit)."
    exit 1
}
Write-Step "  Integration test: PASS. ($($timings['test_s'])s)"

# ─── Step 7: Write verdict JSON ───────────────────────────────────────────────
$totalElapsed = Elapsed-Seconds
$verdict = @{
    ts                  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    phase               = "B"
    overall_verdict     = "PASS"
    services            = @{
        qdrant = @{ status = "live"; url = "http://127.0.0.1:6333"; container = $QdrantImg }
        ollama = @{ status = "live"; url = "http://127.0.0.1:11434" }
        amore_mcp = @{ status = "live"; binary = $amcpBin }
    }
    timings_s           = $timings
    total_elapsed_s     = $totalElapsed
    note                = "store+recall round-trip green via BM25 lane; protocolVersion+tools/list+recall all asserted"
}

$verdict | ConvertTo-Json -Depth 5 | Set-Content $SmokeOut
Write-Step "PHASE B COMPLETE: all 3 services live + store+recall green in ${totalElapsed}s."
Write-Step "Verdict written: $SmokeOut"
exit 0
