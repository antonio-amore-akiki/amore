# tests/qa/c3_resource_footprint.ps1
#
# QA C3 — Resource footprint measurement (Windows lane).
#
# Measures: binary size on disk; cold-start `amore --version` timing;
# cold-start `amore doctor` timing (with all live deps probed);
# idle working-set + private-memory of amore-mcp 1 second after start.
# Appends one row per metric to docs/perf-baseline.tsv.
#
# Gates (per QA + HARDENING ROADMAP, C3 row):
#   - amore / amore-mcp binary <= 80 MB
#   - amore --version cold start <= 500 ms
#   - amore-mcp idle RSS <= 80 MB
#
# Run:
#   pwsh -File tests/qa/c3_resource_footprint.ps1
#
# Exit 0 -> all measurements under their gate; Exit non-zero -> over.

[CmdletBinding()]
param(
    [string]$Repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

$ErrorActionPreference = "Stop"

$exe = Join-Path $Repo "target\release\amore.exe"
$exeMcp = Join-Path $Repo "target\release\amore-mcp.exe"
if (-not (Test-Path $exe) -or -not (Test-Path $exeMcp)) {
    Write-Output "[c3] release binaries missing — building..."
    & cargo build --release --manifest-path (Join-Path $Repo "Cargo.toml") -p amore-cli -p amore-mcp
    if ($LASTEXITCODE -ne 0) { Write-Error "cargo build failed"; exit 1 }
}

$sizeKb = [math]::Round((Get-Item $exe).Length / 1KB, 1)
$sizeMcpKb = [math]::Round((Get-Item $exeMcp).Length / 1KB, 1)
$gateKb = 81920  # 80 MB

# Warm-up
& $exe --version | Out-Null

# Cold-start: 5 runs of --version
$runs = @()
for ($i = 0; $i -lt 5; $i++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $exe --version | Out-Null
    $sw.Stop()
    $runs += $sw.ElapsedMilliseconds
}
$verMin = ($runs | Measure-Object -Minimum).Minimum
$verAvg = [math]::Round(($runs | Measure-Object -Average).Average, 1)

# Doctor full probe: 3 runs
$dr = @()
for ($i = 0; $i -lt 3; $i++) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    & $exe doctor | Out-Null
    $sw.Stop()
    $dr += $sw.ElapsedMilliseconds
}
$drMin = ($dr | Measure-Object -Minimum).Minimum

# amore-mcp idle RSS
$proc = Start-Process -FilePath $exeMcp -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput "$env:TEMP\amore-mcp-c3.out" `
    -RedirectStandardError "$env:TEMP\amore-mcp-c3.err"
Start-Sleep -Seconds 1
$wsMb = [math]::Round((Get-Process -Id $proc.Id).WorkingSet64 / 1MB, 2)
$pmMb = [math]::Round((Get-Process -Id $proc.Id).PrivateMemorySize64 / 1MB, 2)
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue

# Verdicts
function Verdict($value, $gate, $direction = "le") {
    if ($direction -eq "le") {
        if ($value -le $gate) { return "PASS" } else { return "FAIL" }
    }
    if ($value -ge $gate) { return "PASS" } else { return "FAIL" }
}

Write-Output "[c3] amore.exe          $sizeKb KB     gate <= $gateKb KB    $(Verdict $sizeKb $gateKb)"
Write-Output "[c3] amore-mcp.exe      $sizeMcpKb KB   gate <= $gateKb KB    $(Verdict $sizeMcpKb $gateKb)"
Write-Output "[c3] --version cold      min=$($verMin)ms avg=$($verAvg)ms     gate <= 500ms       $(Verdict $verMin 500)"
Write-Output "[c3] doctor full probe   min=$($drMin)ms                          (informal)"
Write-Output "[c3] amore-mcp RSS       ws=$($wsMb) MB priv=$($pmMb) MB           gate <= 80 MB        $(Verdict $wsMb 80)"

# Append to docs/perf-baseline.tsv
$ts = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mmZ")
$sha = (& git -C $Repo rev-parse --short HEAD 2>&1).Trim()
$tsv = Join-Path $Repo "docs\perf-baseline.tsv"
$rows = @(
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_binary_size_kb`t$sizeKb`t<=81920`t$(Verdict $sizeKb $gateKb)",
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_mcp_binary_size_kb`t$sizeMcpKb`t<=81920`t$(Verdict $sizeMcpKb $gateKb)",
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_version_cold_min_ms`t$verMin`t<=500`t$(Verdict $verMin 500)",
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_version_cold_avg_ms`t$verAvg`t<=500`t$(Verdict $verAvg 500)",
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_doctor_full_probe_min_ms`t$drMin`t<=2000`t$(Verdict $drMin 2000)",
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_mcp_idle_working_set_mb`t$wsMb`t<=80`t$(Verdict $wsMb 80)",
    "$ts`t$sha`twindows-msvc-x86_64-laptop`tamore_mcp_idle_private_mem_mb`t$pmMb`t<=80`t$(Verdict $pmMb 80)"
)
Add-Content -Path $tsv -Value $rows

# Hard gate: any FAIL exits non-zero
$results = @($sizeKb, $sizeMcpKb, $verMin, $wsMb)
$gates = @($gateKb, $gateKb, 500, 80)
$failures = 0
for ($i = 0; $i -lt $results.Count; $i++) {
    if ((Verdict $results[$i] $gates[$i]) -eq "FAIL") { $failures++ }
}
if ($failures -gt 0) {
    Write-Error "[c3] $failures gate(s) failed"
    exit 2
}
Write-Output "[c3] PASS — all gates met"
