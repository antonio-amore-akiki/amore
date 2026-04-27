#Requires -Version 7
# security-baseline.ps1 — Nightly supply-chain audit for the Amore workspace.
# Prior-art: Adapt from ~/.claude/scripts/backup-restore-canary.ps1 (PS7 + schtasks + ntfy pattern).
# Registered via Task Scheduler (Amore-Security-Baseline-Nightly) at 02:30 local.
# Outputs: %LOCALAPPDATA%\Amore\security-baselines\<YYYYMMDD>.json
# Exit: 1 if cargo audit or cargo deny report errors; 0 otherwise.

param(
    [string]$WorkspaceRoot = (Split-Path $PSScriptRoot -Parent),
    [switch]$SkipInstall
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── C-3: Self-register the nightly Task Scheduler task if absent ───────────────
# Ensures the task survives a Task Scheduler wipe or re-deployment without
# requiring a manual orchestrator step. Uses $PSCommandPath so the registered
# action path is always the script's real on-disk location.
# Skip with: $env:AMORE_SKIP_SELF_REGISTER = '1'  (CI / test escape hatch)
if ($env:AMORE_SKIP_SELF_REGISTER -ne '1') {
    $TaskName   = 'Amore-Security-Baseline-Nightly'
    $ScriptPath = $PSCommandPath
    $existing   = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    if ($null -eq $existing) {
        $action  = New-ScheduledTaskAction -Execute 'pwsh.exe' `
                       -Argument "-NonInteractive -File `"$ScriptPath`""
        $trigger = New-ScheduledTaskTrigger -Daily -At '02:30'
        $settings = New-ScheduledTaskSettingsSet `
                       -StartWhenAvailable `
                       -AllowStartIfOnBatteries `
                       -DontStopIfGoingOnBatteries
        Register-ScheduledTask -TaskName $TaskName `
            -Action $action -Trigger $trigger -Settings $settings `
            -RunLevel Limited -Force | Out-Null
        $nextRun = (Get-ScheduledTask -TaskName $TaskName |
                        Get-ScheduledTaskInfo).NextRunTime
        Write-Host "[security-baseline] Registered task $TaskName (next run: $nextRun)"
    } else {
        $nextRun = (Get-ScheduledTaskInfo -TaskName $TaskName).NextRunTime
        Write-Host "[security-baseline] Task already registered (status: $($existing.State)) (next run: $nextRun)"
    }
}

# ── Paths ──────────────────────────────────────────────────────────────────────
$OutputDir = Join-Path $env:LOCALAPPDATA 'Amore\security-baselines'
$Today     = (Get-Date -Format 'yyyyMMdd')
$OutFile   = Join-Path $OutputDir "$Today.json"
$NtfyLog   = Join-Path $env:USERPROFILE '.claude\state\ntfy.log'

New-Item -ItemType Directory -Force $OutputDir | Out-Null

# ── Helpers ────────────────────────────────────────────────────────────────────
function Invoke-CargoInstallIfMissing {
    param([string]$Crate, [string]$SubCmd)
    $ver = cargo $SubCmd --version 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Installing $Crate ..."
        cargo install --locked $Crate
        if ($LASTEXITCODE -ne 0) { throw "Failed to install $Crate (exit $LASTEXITCODE)" }
    } else {
        Write-Host "${Crate}: $ver (already installed)"
    }
}

function Send-NtfyAlert {
    param([string]$Title, [string]$Body)
    if (-not (Test-Path $NtfyLog)) { return }    # ntfy not configured — skip silently
    try {
        $NtfyUrl = (Get-Content $NtfyLog -TotalCount 1).Trim()
        if ([string]::IsNullOrWhiteSpace($NtfyUrl)) { return }
        Invoke-RestMethod -Method Post -Uri $NtfyUrl `
            -Headers @{ Title = $Title; Priority = 'high'; Tags = 'warning,rust,amore' } `
            -Body $Body -ContentType 'text/plain' | Out-Null
    } catch {
        Write-Warning "ntfy alert failed (non-fatal): $_"
    }
}

# ── Tool installation (idempotent) ─────────────────────────────────────────────
Set-Location $WorkspaceRoot
if (-not $SkipInstall) {
    Invoke-CargoInstallIfMissing 'cargo-audit'  'audit'
    Invoke-CargoInstallIfMissing 'cargo-deny'   'deny'
    Invoke-CargoInstallIfMissing 'cargo-geiger' 'geiger'
}

# ── Phase 1: cargo audit ───────────────────────────────────────────────────────
Write-Host "`n=== cargo audit ==="
$AuditRaw  = cargo audit --json 2>&1
$AuditExit = $LASTEXITCODE
$Vulnerabilities = @()
try {
    $AuditJson = ($AuditRaw -join "`n") | ConvertFrom-Json -ErrorAction Stop
    $VulnList  = $AuditJson.vulnerabilities.list
    if ($VulnList) {
        foreach ($v in $VulnList) {
            $Vulnerabilities += [PSCustomObject]@{
                id       = $v.advisory.id
                package  = $v.advisory.package
                severity = if ($v.advisory.cvss) { $v.advisory.cvss } else { 'unknown' }
                title    = $v.advisory.title
            }
        }
    }
} catch {
    Write-Warning "cargo audit JSON parse partial — raw exit code: $AuditExit"
}

# ── Phase 2: cargo deny ────────────────────────────────────────────────────────
Write-Host "`n=== cargo deny ==="
$DenyRaw  = cargo deny --workspace check 2>&1
$DenyExit = $LASTEXITCODE
$DenyErrors = @($DenyRaw | Where-Object { $_ -match '\berror\b' })

# ── Phase 3: cargo geiger ──────────────────────────────────────────────────────
Write-Host "`n=== cargo geiger ==="
$GeigerRaw  = cargo geiger --workspace --output-format Json 2>&1
$GeigerExit = $LASTEXITCODE
$UnsafeCount = 0
try {
    $GeigerJson  = ($GeigerRaw -join "`n") | ConvertFrom-Json -ErrorAction Stop
    $UnsafeCount = ($GeigerJson.packages | ForEach-Object {
        $u = $_.unsafety.used.exprs.unsafe
        if ($null -ne $u) { $u } else { 0 }
    } | Measure-Object -Sum).Sum
} catch {
    Write-Warning "cargo geiger JSON parse skipped (non-fatal): $_"
}

# ── Aggregate ─────────────────────────────────────────────────────────────────
$GateFail = ($AuditExit -ne 0) -or ($DenyExit -ne 0)

$Result = [PSCustomObject]@{
    date            = (Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ')
    workspace       = $WorkspaceRoot
    audit_exit      = $AuditExit
    deny_exit       = $DenyExit
    geiger_exit     = $GeigerExit
    vulnerabilities = $Vulnerabilities
    vuln_count      = $Vulnerabilities.Count
    deny_errors     = $DenyErrors
    unsafe_exprs    = $UnsafeCount
    gate_fail       = $GateFail
}

$Result | ConvertTo-Json -Depth 10 | Set-Content -Encoding UTF8 $OutFile

# ── Summary ────────────────────────────────────────────────────────────────────
$AuditStatus  = if ($AuditExit -eq 0) { 'OK' } else { "FAIL($($Vulnerabilities.Count) vulns)" }
$DenyStatus   = if ($DenyExit  -eq 0) { 'OK' } else { 'FAIL' }
$GeigerStatus = "unsafe-exprs=$UnsafeCount"
$GateStatus   = if ($GateFail) { 'GATE:FAIL' } else { 'GATE:OK' }
$Summary = "audit=$AuditStatus | deny=$DenyStatus | geiger=$GeigerStatus | $GateStatus"

Write-Host "`nSUMMARY: $Summary"
Write-Host "Output : $OutFile"

if ($GateFail) {
    Send-NtfyAlert -Title 'Amore security-baseline FAILED' -Body $Summary
    exit 1
}

exit 0
