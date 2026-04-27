# scripts/run-longmemeval.ps1 — Reproducible LongMemEval eval runner.
#
# Usage:
#   .\scripts\run-longmemeval.ps1 [-Subset 20] [-MockDeps] [-KeepDaemons]
#
# -Subset N      Evaluate first N instances only (default 20).
# -MockDeps      Skip Docker; use in-memory BM25 only. Overrides daemon startup.
# -KeepDaemons   Leave Docker services running after eval.
#
# Without -MockDeps: tries to start docker-compose.eval.yml; if Docker
# unavailable or health check fails within 120s, falls through to -MockDeps
# with a WARNING logged.

param(
    [int]$Subset = 20,
    [switch]$MockDeps,
    [switch]$KeepDaemons
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot  = (Resolve-Path "$PSScriptRoot\..").Path
$LogFile   = "$RepoRoot\state\w1-longmemeval-run.log"
$OutputDir = "$RepoRoot\state"

if (-not (Test-Path $OutputDir)) { New-Item -ItemType Directory -Force $OutputDir | Out-Null }

function Write-Log([string]$Msg) {
    $line = "[$(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ')] $Msg"
    Write-Host $line
    Add-Content -Path $LogFile -Value $line
}

# ─── Daemon startup ───────────────────────────────────────────────────────────

$UseMock = [bool]$MockDeps
$DaemonsStarted = $false

if (-not $UseMock) {
    Write-Log "Checking Docker availability..."
    $dockerOk = $false
    try {
        $null = & docker compose version 2>&1
        if ($LASTEXITCODE -eq 0) { $dockerOk = $true }
    } catch { $dockerOk = $false }

    if ($dockerOk) {
        Write-Log "Starting eval daemons (docker-compose.eval.yml)..."
        $composeFile = "$RepoRoot\docker-compose.eval.yml"
        $startJob = Start-Job -ScriptBlock {
            param($f)
            & docker compose -f $f up -d --wait 2>&1
            $LASTEXITCODE
        } -ArgumentList $composeFile

        $elapsed = 0
        while ($elapsed -lt 120 -and $startJob.State -eq "Running") {
            Start-Sleep -Seconds 5; $elapsed += 5
            Write-Log "  waiting for daemons... ${elapsed}s"
        }
        $jobOutput = Receive-Job $startJob
        $jobExit   = $startJob.ChildJobs[0].Output | Select-Object -Last 1
        Remove-Job $startJob -Force

        if ($jobExit -eq 0) {
            Write-Log "Daemons healthy."
            $DaemonsStarted = $true
        } else {
            Write-Log "WARNING: daemon startup failed or timed out. Falling through to --mock-deps."
            Write-Log "Docker output: $jobOutput"
            $UseMock = $true
        }
    } else {
        Write-Log "WARNING: docker compose not available. Falling through to --mock-deps."
        $UseMock = $true
    }
}

# ─── Build + run eval ─────────────────────────────────────────────────────────

$OutputFile = "$OutputDir\w1-longmemeval-v0.5.1.json"
$Dataset    = "$env:LOCALAPPDATA\Amore\datasets\longmemeval\test.jsonl"

$RunArgs = @(
    "run", "--release", "--bin", "amore-eval-longmemeval", "--"
    "--dataset", $Dataset
    "--output",  $OutputFile
    "--subset",  $Subset
)
if ($UseMock) { $RunArgs += "--mock-deps" }

Write-Log "Building + running eval (subset=$Subset mock=$UseMock)..."
Write-Log "Command: cargo $($RunArgs -join ' ')"

Push-Location $RepoRoot
try {
    $proc = Start-Process -FilePath "cargo" -ArgumentList $RunArgs `
        -NoNewWindow -Wait -PassThru `
        -RedirectStandardOutput "$LogFile.stdout" `
        -RedirectStandardError  "$LogFile.stderr"
    $exitCode = $proc.ExitCode
    Get-Content "$LogFile.stdout" | ForEach-Object { Write-Log "  STDOUT: $_" }
    Get-Content "$LogFile.stderr" | ForEach-Object { Write-Log "  STDERR: $_" }
    if ($exitCode -ne 0) {
        Write-Log "ERROR: cargo run exited $exitCode"
        exit $exitCode
    }
} finally {
    Pop-Location
}

Write-Log "Eval complete. Output: $OutputFile"

if (Test-Path $OutputFile) {
    $report = Get-Content $OutputFile | ConvertFrom-Json
    Write-Log "GATE: R@5=$($report.overall.r_at_5) R@10=$($report.overall.r_at_10) STATUS=$($report.status)"
}

# ─── Daemon teardown ──────────────────────────────────────────────────────────

if ($DaemonsStarted -and -not $KeepDaemons) {
    Write-Log "Stopping eval daemons..."
    & docker compose -f "$RepoRoot\docker-compose.eval.yml" down 2>&1 | ForEach-Object { Write-Log "  $_" }
}

Write-Log "Done."
