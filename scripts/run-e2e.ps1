# scripts/run-e2e.ps1 — Run the amore live E2E integration suite.
#
# Topology: Qdrant via docker-compose.test.yml; Ollama NATIVE (not in compose).
# See docker-compose.test.yml header for the production-topology rationale.
#
# Prerequisites:
#   • Docker daemon running
#   • Ollama installed and serving natively (ollama serve + ollama pull nomic-embed-text)
#
# Exit codes:
#   0  All E2E tests passed
#   1  Any prerequisite check or test step failed; diagnostic printed to stderr

param(
    [switch]$SkipTeardown   # Keep compose stack running after the suite (useful for debugging)
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot     = (Resolve-Path "$PSScriptRoot\..").Path
$ComposeFile  = "$RepoRoot\docker-compose.test.yml"
$OllamaUrl    = "http://127.0.0.1:11434"
$ScriptStart  = Get-Date

function Write-Step([string]$Msg) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Msg"
}

function Test-Endpoint([string]$Url) {
    try {
        $r = Invoke-WebRequest -Uri $Url -TimeoutSec 3 -UseBasicParsing
        return $r.StatusCode -eq 200
    } catch { return $false }
}

function Poll-Until-Ready([string]$Url, [string]$Label, [int]$TimeoutSec) {
    for ($i = 0; $i -lt $TimeoutSec; $i++) {
        if (Test-Endpoint $Url) { Write-Step "  $Label up after ${i}s"; return $true }
        Start-Sleep -Seconds 1
    }
    return $false
}

# ─── Step 1: Verify Docker daemon ─────────────────────────────────────────────
Write-Step "E2E runner — checking Docker daemon..."
$dockerVer = & docker version --format "{{.Server.Version}}" 2>$null
if ($LASTEXITCODE -ne 0 -or -not $dockerVer -or $dockerVer.Trim() -eq "") {
    Write-Error "FATAL: Docker daemon not running. Start Docker Desktop and retry."
    exit 1
}
Write-Step "  Docker daemon OK (server $dockerVer)."

# ─── Step 2: Verify Ollama is reachable on :11434 ─────────────────────────────
Write-Step "Checking Ollama on $OllamaUrl ..."
if (-not (Test-Endpoint "$OllamaUrl/api/tags")) {
    Write-Error @"
FATAL: Ollama is not reachable at $OllamaUrl.
Ollama runs natively (not in Docker) for the production topology.
Fix: run 'ollama serve' in a separate terminal, then retry.
"@
    exit 1
}
Write-Step "  Ollama reachable."

# ─── Step 3: Bring up Qdrant via compose ──────────────────────────────────────
Write-Step "Starting Qdrant via docker compose -f docker-compose.test.yml ..."
& docker compose -f $ComposeFile up -d 2>&1 | ForEach-Object { Write-Step "  [compose] $_" }
if ($LASTEXITCODE -ne 0) {
    Write-Error "FATAL: docker compose up failed."
    exit 1
}

$qdrantReady = Poll-Until-Ready "http://localhost:6333/readyz" "qdrant" 30
if (-not $qdrantReady) {
    Write-Error "FATAL: Qdrant did not become healthy within 30s."
    & docker compose -f $ComposeFile logs qdrant 2>&1 | Select-Object -Last 20 | ForEach-Object { Write-Step "  [qdrant] $_" }
    & docker compose -f $ComposeFile down 2>$null
    exit 1
}
Write-Step "  Qdrant healthy."

# ─── Step 4: Run the E2E suite ────────────────────────────────────────────────
Write-Step "Running live E2E integration tests..."

$env:AMORE_TEST_MCP    = "1"
$env:AMORE_TEST_QDRANT = "1"
$env:AMORE_TEST_OLLAMA = "1"
$env:AMORE_TEST_E2E    = "1"

Push-Location $RepoRoot
try {
    & cargo test -p amore-mcp -p amore-core `
        --test mcp_handshake `
        --test qdrant_roundtrip `
        --test ollama_embed `
        --test hybrid_e2e `
        -- --ignored --test-threads=1 2>&1 | ForEach-Object { Write-Host $_ }
    $testExit = $LASTEXITCODE
} finally {
    Pop-Location
}

# ─── Step 5: Tear down compose ────────────────────────────────────────────────
if (-not $SkipTeardown) {
    Write-Step "Tearing down compose stack..."
    & docker compose -f $ComposeFile down 2>&1 | ForEach-Object { Write-Step "  [compose] $_" }
} else {
    Write-Step "  -SkipTeardown set — compose stack left running."
}

# ─── Result ───────────────────────────────────────────────────────────────────
$elapsed = [int]((Get-Date) - $ScriptStart).TotalSeconds
if ($testExit -ne 0) {
    Write-Error "FATAL: E2E suite failed (exit=$testExit) in ${elapsed}s."
    exit 1
}
Write-Step "E2E suite PASSED in ${elapsed}s."
exit 0
