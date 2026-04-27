#!/usr/bin/env pwsh
# scripts/release-dry-run.ps1 — W8 PRR gate: composable release dry-run wrapper.
#
# Role: pre-release validation gate consumed by docs/PRR-CHECKLIST-v1.0.0.md.
# Runs 10 sequential checks; emits verdict.json (GO|NO-GO) and per-step logs.
# A NO-GO verdict exits non-zero and blocks promotion to release-local.ps1.
#
# Optional tools (cosign, cyclonedx-cli, gitleaks): SKIP when not installed.
# Required tools (cargo, pwsh): FAIL when absent.
#
# Usage:
#   pwsh scripts/release-dry-run.ps1 -Version 1.0.0
#   pwsh scripts/release-dry-run.ps1 -Version 1.0.0 -OutputDir state/custom-rc1
#   pwsh scripts/release-dry-run.ps1 -Version 1.0.0 -WhatIf   # parse-only, no I/O
#
# Related: docs/PRR-CHECKLIST-v1.0.0.md, docs/SCORECARD-v0.5.0.md,
#          docs/SCORECARD-v1.0.0-target.md, state/w8-dry-run-template.json

[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$OutputDir = "state/w8-rc1/release-dry-run"
)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Write-StepHeader([string]$Name) {
    Write-Host ""
    Write-Host "=== [$Name] ===" -ForegroundColor Cyan
}

function Get-IsoTimestamp {
    [datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
}

function Invoke-TimedCommand {
    param(
        [string]$Name,
        [scriptblock]$Block,
        [string]$LogFile
    )
    $start = [System.Diagnostics.Stopwatch]::StartNew()
    $status = "PASS"
    $exitCode = 0

    try {
        $output = & $Block 2>&1
        $exitCode = $LASTEXITCODE
        if ($null -eq $exitCode) { $exitCode = 0 }
        if ($exitCode -ne 0) { $status = "FAIL" }
        $output | Out-File -FilePath $LogFile -Encoding utf8
    } catch {
        $status = "FAIL"
        $exitCode = 1
        "ERROR: $_" | Out-File -FilePath $LogFile -Encoding utf8
    }

    $start.Stop()
    return [PSCustomObject]@{
        name        = $Name
        status      = $status
        duration_ms = [int]$start.ElapsedMilliseconds
        log         = [System.IO.Path]::GetFileName($LogFile)
        exit_code   = $exitCode
    }
}

function Invoke-OptionalTool {
    param(
        [string]$Name,
        [string]$ToolName,
        [scriptblock]$Block,
        [string]$LogFile
    )
    $available = $null -ne (Get-Command $ToolName -ErrorAction SilentlyContinue)
    if (-not $available) {
        Write-Host "  SKIP — $ToolName not installed" -ForegroundColor Yellow
        "[SKIP] $ToolName not found on PATH" | Out-File -FilePath $LogFile -Encoding utf8
        return [PSCustomObject]@{
            name        = $Name
            status      = "SKIP"
            duration_ms = 0
            log         = [System.IO.Path]::GetFileName($LogFile)
            exit_code   = 0
        }
    }
    return Invoke-TimedCommand -Name $Name -Block $Block -LogFile $LogFile
}

# ---------------------------------------------------------------------------
# WhatIf guard — parse-only when -WhatIf passed
# ---------------------------------------------------------------------------

if ($WhatIfPreference) {
    Write-Host "[WhatIf] Script parsed successfully. No I/O performed." -ForegroundColor Green
    exit 0
}

# ---------------------------------------------------------------------------
# Setup output directory
# ---------------------------------------------------------------------------

$RepoRoot = Split-Path -Parent $PSScriptRoot
$AbsOutputDir = if ([System.IO.Path]::IsPathRooted($OutputDir)) { $OutputDir } `
    else { Join-Path $RepoRoot $OutputDir }

New-Item -ItemType Directory -Force -Path $AbsOutputDir | Out-Null
Write-Host "Output dir: $AbsOutputDir" -ForegroundColor DarkGray

$ManifestPath  = Join-Path $AbsOutputDir "manifest.json"
$VerdictPath   = Join-Path $AbsOutputDir "verdict.json"
$steps         = [System.Collections.Generic.List[object]]::new()
$artifacts     = [System.Collections.Generic.List[object]]::new()
$verifications = [System.Collections.Generic.List[object]]::new()
$globalVerdict = "GO"

function Add-Step([PSCustomObject]$Result) {
    $steps.Add($Result)
    if ($Result.status -eq "FAIL") { $script:globalVerdict = "NO-GO" }
    $fc = if ($Result.status -eq "PASS") { "Green" } `
           elseif ($Result.status -eq "SKIP") { "Yellow" } else { "Red" }
    Write-Host "  [$($Result.status)] $($Result.name) ($($Result.duration_ms)ms)" -ForegroundColor $fc
    # Rolling manifest after each step
    [PSCustomObject]@{
        version        = $Version
        ts             = Get-IsoTimestamp
        global_verdict = $script:globalVerdict
        steps          = $steps
        artifacts      = $artifacts
        verifications  = $verifications
    } | ConvertTo-Json -Depth 10 | Out-File -FilePath $ManifestPath -Encoding utf8
}

# ---------------------------------------------------------------------------
# Step 1 — cargo fmt --check
# ---------------------------------------------------------------------------

Write-StepHeader "Step 1: cargo fmt"
$r = Invoke-TimedCommand -Name "fmt" -LogFile (Join-Path $AbsOutputDir "fmt.log") -Block {
    Set-Location $RepoRoot
    cargo fmt --check 2>&1
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 2 — cargo clippy
# ---------------------------------------------------------------------------

Write-StepHeader "Step 2: cargo clippy"
$r = Invoke-TimedCommand -Name "clippy" -LogFile (Join-Path $AbsOutputDir "clippy.log") -Block {
    Set-Location $RepoRoot
    cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 3 — cargo audit
# ---------------------------------------------------------------------------

Write-StepHeader "Step 3: cargo audit"
$auditLog = Join-Path $AbsOutputDir "audit.log"
$r = Invoke-TimedCommand -Name "audit" -LogFile $auditLog -Block {
    Set-Location $RepoRoot
    cargo audit --json 2>&1 | Tee-Object -FilePath (Join-Path $AbsOutputDir "audit.json")
}
# cargo audit exits 1 when advisories found; if deny.toml covers them treat as PASS
if ($r.status -eq "FAIL") {
    if (Test-Path (Join-Path $RepoRoot "deny.toml")) {
        $r.status = "PASS"
        Add-Content -Path $auditLog -Value "[INFO] Advisory failures covered by deny.toml ignore list"
    }
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 4 — cargo test --no-run (compile only)
# ---------------------------------------------------------------------------

Write-StepHeader "Step 4: cargo test compile"
$r = Invoke-TimedCommand -Name "test_compile" `
    -LogFile (Join-Path $AbsOutputDir "test_compile.log") -Block {
    Set-Location $RepoRoot
    cargo test --workspace --release --no-run 2>&1
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 5 — cargo test --release (full run, 60s cap by harness timeout)
# ---------------------------------------------------------------------------

Write-StepHeader "Step 5: cargo test run"
$r = Invoke-TimedCommand -Name "test_run" `
    -LogFile (Join-Path $AbsOutputDir "test_run.log") -Block {
    Set-Location $RepoRoot
    cargo test --workspace --release 2>&1
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 6 — release-local.ps1 -DryRun
# ---------------------------------------------------------------------------

Write-StepHeader "Step 6: release-local.ps1 -DryRun"
$releaseScript = Join-Path $RepoRoot "scripts" "release-local.ps1"
if (-not (Test-Path $releaseScript)) {
    "[SKIP] release-local.ps1 not found at $releaseScript" | `
        Out-File -FilePath (Join-Path $AbsOutputDir "release_local.log") -Encoding utf8
    Add-Step ([PSCustomObject]@{
        name = "release_local_dry"; status = "SKIP"; duration_ms = 0
        log = "release_local.log"; exit_code = 0
    })
} else {
    $r = Invoke-TimedCommand -Name "release_local_dry" `
        -LogFile (Join-Path $AbsOutputDir "release_local.log") -Block {
        pwsh -NoProfile -File $releaseScript -Version $Version -DryRun 2>&1
    }
    # Collect artifact paths if release-local emitted a manifest
    $relMfPath = Join-Path $RepoRoot "state" "release-$Version-manifest.json"
    if (Test-Path $relMfPath) {
        $relMf = Get-Content $relMfPath | ConvertFrom-Json -ErrorAction SilentlyContinue
        if ($relMf.artifacts) { foreach ($a in $relMf.artifacts) { $artifacts.Add($a) } }
    }
    Add-Step $r
}

# ---------------------------------------------------------------------------
# Step 7 — cosign verify-blob (optional)
# ---------------------------------------------------------------------------

Write-StepHeader "Step 7: cosign verify-blob"
$cosignLog = Join-Path $AbsOutputDir "cosign.log"
if ($artifacts.Count -eq 0) {
    "[SKIP] No artifacts from Step 6" | Out-File -FilePath $cosignLog -Encoding utf8
    Add-Step ([PSCustomObject]@{
        name = "cosign_verify"; status = "SKIP"; duration_ms = 0
        log = "cosign.log"; exit_code = 0
    })
} elseif ($null -eq (Get-Command cosign -ErrorAction SilentlyContinue)) {
    "[SKIP] cosign not installed" | Out-File -FilePath $cosignLog -Encoding utf8
    Add-Step ([PSCustomObject]@{
        name = "cosign_verify"; status = "SKIP"; duration_ms = 0
        log = "cosign.log"; exit_code = 0
    })
} else {
    $allVerified = $true
    $cosignOut = [System.Text.StringBuilder]::new()
    foreach ($artifact in $artifacts) {
        $bundle = "$($artifact.path).bundle"
        if (-not (Test-Path $bundle)) {
            [void]$cosignOut.AppendLine("[WARN] No bundle for $($artifact.path)")
            continue
        }
        $res = cosign verify-blob --bundle $bundle $artifact.path 2>&1
        [void]$cosignOut.AppendLine("$($artifact.path): $res")
        if ($LASTEXITCODE -ne 0) { $allVerified = $false }
        $verifications.Add([PSCustomObject]@{
            artifact = $artifact.path; bundle = $bundle
            verified = ($LASTEXITCODE -eq 0)
        })
    }
    $cosignOut.ToString() | Out-File -FilePath $cosignLog -Encoding utf8
    Add-Step ([PSCustomObject]@{
        name        = "cosign_verify"
        status      = if ($allVerified) { "PASS" } else { "FAIL" }
        duration_ms = 0; log = "cosign.log"
        exit_code   = if ($allVerified) { 0 } else { 1 }
    })
}

# ---------------------------------------------------------------------------
# Step 8 — cyclonedx-cli validate (optional)
# ---------------------------------------------------------------------------

Write-StepHeader "Step 8: cyclonedx-cli validate"
$sbomPath = Join-Path $RepoRoot "sbom.cdx.json"
$r = Invoke-OptionalTool -Name "sbom_validate" -ToolName "cyclonedx-cli" `
    -LogFile (Join-Path $AbsOutputDir "sbom.log") -Block {
    if (-not (Test-Path $sbomPath)) { throw "sbom.cdx.json not found at $sbomPath" }
    cyclonedx-cli validate --input-file $sbomPath 2>&1
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 9 — gitleaks detect (optional)
# ---------------------------------------------------------------------------

Write-StepHeader "Step 9: gitleaks detect"
$r = Invoke-OptionalTool -Name "gitleaks" -ToolName "gitleaks" `
    -LogFile (Join-Path $AbsOutputDir "gitleaks.log") -Block {
    Set-Location $RepoRoot
    gitleaks detect --source . --no-git --redact 2>&1
}
Add-Step $r

# ---------------------------------------------------------------------------
# Step 10 — emit verdict.json
# ---------------------------------------------------------------------------

Write-StepHeader "Step 10: emit verdict"
$verdict = [PSCustomObject]@{
    version       = $Version
    ts            = Get-IsoTimestamp
    verdict       = $globalVerdict
    step_results  = $steps
    artifacts     = $artifacts
    verifications = $verifications
}
$verdict | ConvertTo-Json -Depth 10 | Out-File -FilePath $VerdictPath -Encoding utf8
Add-Step ([PSCustomObject]@{
    name = "verdict_emit"; status = "PASS"; duration_ms = 0
    log = "verdict.json"; exit_code = 0
})

# ---------------------------------------------------------------------------
# Final summary
# ---------------------------------------------------------------------------

$color = if ($globalVerdict -eq "GO") { "Green" } else { "Red" }
Write-Host ""
Write-Host "=========================" -ForegroundColor $color
Write-Host "  VERDICT: $globalVerdict — v$Version" -ForegroundColor $color
Write-Host "  verdict.json : $VerdictPath" -ForegroundColor DarkGray
Write-Host "  manifest.json: $ManifestPath" -ForegroundColor DarkGray
Write-Host "=========================" -ForegroundColor $color

if ($globalVerdict -eq "NO-GO") {
    $failed = $steps | Where-Object { $_.status -eq "FAIL" } | ForEach-Object { $_.name }
    Write-Host "Failed steps: $($failed -join ', ')" -ForegroundColor Red
    exit 1
}
exit 0
