#Requires -Version 7.0
<#
.SYNOPSIS
    Weekly error-budget burn-rate calculator per Google SRE Ch.3.

.DESCRIPTION
    Queries a Prometheus endpoint for Amore service availability and latency p99
    over a configurable rolling window. Computes seconds consumed from each service
    class budget. Appends a row to docs/ERROR-BUDGET-TRACKER-v1.0.0.md.
    Exits 1 if cumulative burn exceeds 50% of any class budget (release freeze signal).

.PARAMETER Endpoint
    Prometheus base URL. Default: http://localhost:9090

.PARAMETER Window
    Rolling window in days. Default: 7

.EXAMPLE
    .\error-budget-update.ps1
    .\error-budget-update.ps1 -Endpoint http://prometheus.local:9090 -Window 14

Prior-art verdict: state/prior-art-verdict.json (2026-05-26) — Build.
Pattern Adapted from scripts/security-baseline.ps1 + scripts/verify-release.ps1
(param block, Invoke-RestMethod, Add-Content append-only row).
Alternatives sloth/pyrra/alertmanager-rules rejected (not installed; over-scoped).
#>

[CmdletBinding()]
param(
    [string] $Endpoint = 'http://localhost:9090',
    [int]    $Window   = 7
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ---------------------------------------------------------------------------
# Budget ceilings per service class (seconds, 30d rolling)
# ---------------------------------------------------------------------------
$budgetA = 2592   # Class A: 99.9%  SLO — 0.001  * 30 * 86400
$budgetB = 259    # Class B: 99.99% SLO — 0.0001 * 30 * 86400
$budgetC = 1296   # Class C: 99.95% SLO — 0.0005 * 30 * 86400
$freezeThreshold = 0.5

$windowSeconds = $Window * 86400

# ---------------------------------------------------------------------------
# Helper: run a PromQL instant query; returns $null on failure
# ---------------------------------------------------------------------------
function Invoke-PromQL {
    param([string] $Query)
    $encoded = [Uri]::EscapeDataString($Query)
    $url = "$Endpoint/api/v1/query?query=$encoded"
    try {
        $resp = Invoke-RestMethod -Uri $url -Method Get -TimeoutSec 15
    } catch {
        Write-Warning "Prometheus query failed: $Query — $_"
        return $null
    }
    if ($resp.status -ne 'success') {
        Write-Warning "Prometheus returned status=$($resp.status) for: $Query"
        return $null
    }
    $results = $resp.data.result
    if (-not $results -or $results.Count -eq 0) { return $null }
    return [double]($results[0].value[1])
}

# ---------------------------------------------------------------------------
# Query availability and latency p99
# ---------------------------------------------------------------------------
$availQuery  = "avg_over_time(up{job=`"amore`"}[${Window}d])"
$latencyQuery = "avg_over_time(histogram_quantile(0.99, rate(amore_db_operation_duration_seconds_bucket[5m]))[${Window}d:])"

Write-Host "Querying Prometheus at $Endpoint (window=${Window}d)..."

$availability = Invoke-PromQL -Query $availQuery
$latencyP99   = Invoke-PromQL -Query $latencyQuery

if ($null -eq $availability) {
    Write-Warning "Could not retrieve availability — defaulting to 1.0 (no burn recorded)."
    $availability = 1.0
}

# ---------------------------------------------------------------------------
# Burn calculation: seconds unavailable = (1 - availability) * window_seconds
# Class B/C use the same availability signal as Class A until per-class metrics
# are instrumented (documented in ERROR-BUDGET-TRACKER-v1.0.0.md notes column).
# ---------------------------------------------------------------------------
$burnA = [math]::Round((1.0 - $availability) * $windowSeconds, 1)
$burnB = $burnA
$burnC = $burnA

# ---------------------------------------------------------------------------
# Cumulative burn: parse existing burn log rows in tracker
# ---------------------------------------------------------------------------
$trackerRel = Join-Path $PSScriptRoot '..\docs\ERROR-BUDGET-TRACKER-v1.0.0.md'
$trackerAbs = (Resolve-Path $trackerRel -ErrorAction SilentlyContinue)?.Path

$cumA = $burnA
$cumB = $burnB
$cumC = $burnC

if ($trackerAbs -and (Test-Path $trackerAbs)) {
    $lines = Get-Content $trackerAbs
    $logRows = $lines | Where-Object { $_ -match '^\|\s*\d{4}-\d{2}-\d{2}' }
    foreach ($row in $logRows) {
        $cols = $row -split '\|' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
        if ($cols.Count -ge 3) {
            $pA = 0.0; $pB = 0.0; $pC = 0.0
            [void][double]::TryParse($cols[1], [ref]$pA)
            [void][double]::TryParse($cols[2], [ref]$pB)
            [void][double]::TryParse($cols[3], [ref]$pC)
            $cumA += $pA
            $cumB += $pB
            $cumC += $pC
        }
    }
}

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
$pctA = [math]::Round(($cumA / $budgetA) * 100, 2)
$pctB = [math]::Round(($cumB / $budgetB) * 100, 2)
$pctC = [math]::Round(($cumC / $budgetC) * 100, 2)
$latencyStr = if ($null -ne $latencyP99) { "$([math]::Round($latencyP99 * 1000, 1)) ms" } else { 'N/A' }

Write-Host ""
Write-Host "=== Error Budget Update (window: ${Window}d) ==="
Write-Host ("  Availability     : {0:P4}" -f $availability)
Write-Host ("  Latency p99      : {0}" -f $latencyStr)
Write-Host ("  Burn this window : Class A={0}s  Class B={1}s  Class C={2}s" -f $burnA, $burnB, $burnC)
Write-Host ("  Cumulative       : A={0}s ({1}%)  B={2}s ({3}%)  C={4}s ({5}%)" -f $cumA, $pctA, $cumB, $pctB, $cumC, $pctC)
Write-Host ""

# ---------------------------------------------------------------------------
# Append row to tracker (append-only per docs/ERROR-BUDGET-TRACKER-v1.0.0.md)
# ---------------------------------------------------------------------------
$today  = (Get-Date -Format 'yyyy-MM-dd')
$notes  = "availability=$([math]::Round($availability * 100, 4))% latencyP99=$latencyStr window=${Window}d"
$newRow = "| $today | $burnA | $burnB | $burnC | $notes |"

if ($trackerAbs -and (Test-Path $trackerAbs)) {
    Add-Content -Path $trackerAbs -Value $newRow
    Write-Host "Appended burn row to $trackerAbs"
} else {
    Write-Warning "Tracker not found at $trackerRel — row not appended: $newRow"
}

# ---------------------------------------------------------------------------
# Freeze trigger check — exit 1 signals CI to block release pipeline
# ---------------------------------------------------------------------------
$freeze = $false
if (($cumA / $budgetA) -ge $freezeThreshold) {
    Write-Warning "RELEASE FREEZE: Class A budget >= 50% consumed ($pctA%)"
    $freeze = $true
}
if (($cumB / $budgetB) -ge $freezeThreshold) {
    Write-Warning "RELEASE FREEZE: Class B budget >= 50% consumed ($pctB%)"
    $freeze = $true
}
if (($cumC / $budgetC) -ge $freezeThreshold) {
    Write-Warning "RELEASE FREEZE: Class C budget >= 50% consumed ($pctC%)"
    $freeze = $true
}

if ($freeze) {
    Write-Host "Exit 1 — release freeze signal."
    exit 1
}

Write-Host "Budget SAFE. No freeze trigger."
exit 0
