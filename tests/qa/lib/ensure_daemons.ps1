# tests/qa/lib/ensure_daemons.ps1
#
# CLASS FIX (2026-05-26): never skip a daemon-gated QA gate. If Ollama or
# Qdrant is down, start it. Per user directive: "u start them not me. and fix
# the class so u always start if smthg not running."
#
# Probes:
#   Ollama  http://127.0.0.1:11434/api/version    -> spawns ollama.exe serve
#   Qdrant  http://127.0.0.1:6333/readyz          -> docker run qdrant/qdrant:v1.15.4
#
# Exit 0 -> both up + reachable, fully ready. Caller proceeds with QA gate.
# Exit non-zero -> daemon failed to come up after timeout; caller MUST surface
# the cause, never silently skip.

[CmdletBinding()]
param(
    [int]$OllamaTimeout = 15,
    [int]$QdrantTimeout = 30,
    [string]$QdrantImage = "qdrant/qdrant:v1.15.4",
    [string]$QdrantContainerName = "qdrant",
    [string]$QdrantVolume = "qdrant_storage"
)

$ErrorActionPreference = "Continue"

function Test-Daemon($url, $label) {
    try {
        $r = Invoke-WebRequest -Uri $url -TimeoutSec 2 -UseBasicParsing
        if ($r.StatusCode -eq 200) { return $true }
    } catch {}
    return $false
}

function Start-Ollama() {
    $exe = "$env:USERPROFILE\AppData\Local\Programs\Ollama\ollama.exe"
    if (-not (Test-Path $exe)) {
        Write-Error "Ollama not installed at $exe. Install via https://ollama.com/download"
        return $false
    }
    Write-Output "[ensure_daemons] starting Ollama..."
    $proc = Start-Process -FilePath $exe -ArgumentList "serve" -PassThru -WindowStyle Hidden -RedirectStandardOutput "$env:TEMP\ollama.out" -RedirectStandardError "$env:TEMP\ollama.err"
    Write-Output "[ensure_daemons] Ollama PID: $($proc.Id)"
    return $true
}

function Start-Qdrant() {
    # Try docker first
    docker version 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        # Docker not up. Try Docker Desktop.
        $dd = "$env:ProgramFiles\Docker\Docker\Docker Desktop.exe"
        if (Test-Path $dd) {
            Write-Output "[ensure_daemons] Docker not running; starting Docker Desktop..."
            Start-Process -FilePath $dd -WindowStyle Hidden
            $i = 0
            while ($i -lt 60) {
                $info = docker info --format "{{.ServerVersion}}" 2>$null
                if ($LASTEXITCODE -eq 0 -and $info) { break }
                Start-Sleep -Seconds 2; $i += 2
            }
            $info = docker info --format "{{.ServerVersion}}" 2>$null
            if (-not $info) { Write-Error "Docker Desktop failed to come up"; return $false }
        } else {
            Write-Error "Docker not installed; cannot start Qdrant"
            return $false
        }
    }
    # Check existing container
    $existing = docker ps -a --filter "name=$QdrantContainerName" --format "{{.Names}}" 2>&1 | Out-String
    if ($existing.Trim() -eq $QdrantContainerName) {
        $running = docker ps --filter "name=$QdrantContainerName" --format "{{.Names}}" 2>&1 | Out-String
        if ($running.Trim() -ne $QdrantContainerName) {
            Write-Output "[ensure_daemons] starting existing qdrant container..."
            docker start $QdrantContainerName 2>&1 | Out-Null
        } else {
            Write-Output "[ensure_daemons] qdrant container already running"
        }
    } else {
        Write-Output "[ensure_daemons] running new qdrant container..."
        docker run -d --name $QdrantContainerName `
            -p 6333:6333 -p 6334:6334 `
            -v "${QdrantVolume}:/qdrant/storage" $QdrantImage 2>&1 | Out-Null
    }
    return $true
}

function Poll-Until-Ready($url, $label, $timeoutSec) {
    for ($i = 0; $i -lt $timeoutSec; $i++) {
        if (Test-Daemon $url $label) { Write-Output "[ensure_daemons] $label up after $i s"; return $true }
        Start-Sleep -Seconds 1
    }
    return $false
}

# ----------------- main -----------------
Write-Output "[ensure_daemons] probing Ollama..."
if (-not (Test-Daemon "http://127.0.0.1:11434/api/version" "ollama")) {
    if (-not (Start-Ollama)) { exit 2 }
    if (-not (Poll-Until-Ready "http://127.0.0.1:11434/api/version" "ollama" $OllamaTimeout)) {
        Write-Error "Ollama failed to come up after $OllamaTimeout s. Logs:"
        Get-Content "$env:TEMP\ollama.err" -ErrorAction SilentlyContinue | Select-Object -First 10
        exit 3
    }
} else {
    Write-Output "[ensure_daemons] Ollama already up"
}

Write-Output "[ensure_daemons] probing Qdrant..."
if (-not (Test-Daemon "http://127.0.0.1:6333/readyz" "qdrant")) {
    if (-not (Start-Qdrant)) { exit 4 }
    if (-not (Poll-Until-Ready "http://127.0.0.1:6333/readyz" "qdrant" $QdrantTimeout)) {
        Write-Error "Qdrant failed to come up after $QdrantTimeout s. Container logs:"
        docker logs $QdrantContainerName 2>&1 | Select-Object -Last 15
        exit 5
    }
} else {
    Write-Output "[ensure_daemons] Qdrant already up"
}

# Final port reachability check for gRPC (BM25 only needs HTTP, but vector
# search uses gRPC 6334 via qdrant-client crate).
$grpc = Test-NetConnection -ComputerName 127.0.0.1 -Port 6334 -InformationLevel Quiet 2>&1
if (-not $grpc) { Write-Error "Qdrant gRPC :6334 not reachable"; exit 6 }
Write-Output "[ensure_daemons] both daemons ready (ollama:11434, qdrant:6333+6334)"
exit 0
