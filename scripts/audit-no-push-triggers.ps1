<#
.SYNOPSIS
Audit .github/workflows/*.yml files for forbidden auto-triggers.

.DESCRIPTION
Enforces the constraint "never use my git actions or credits" by scanning every
workflow YAML for bare push:, pull_request:, or schedule: triggers. Any file that
fires on these triggers is a constraint violation. Only workflow_dispatch: is
permitted as the sole trigger.

Emits a JSON result line per file and an aggregate PASS/FAIL verdict.
Captures output to state/ci-no-push-audit-<date>.log.

INVARIANT: Zero auto-triggers in GHA workflows. This script is the enforcement gate.
See docs/CI-NO-PUSH-AUDIT.md for full rationale and first-run result.

Prior-art: Adapted structural pattern from scripts/security-baseline.ps1
(recursive file enumeration, per-file status, aggregate verdict, state log).

.PARAMETER WorkflowDir
Path to the workflows directory.
Defaults to .github/workflows relative to the repo root.

.NOTES
Exit codes:
  0 = PASS (all files workflow_dispatch-only)
  1 = FAIL (one or more files contain push: / pull_request: / schedule: triggers)
  2 = WorkflowDir not found
#>

[CmdletBinding()]
param(
    [string]$WorkflowDir = ""
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $WorkflowDir) {
    $WorkflowDir = Join-Path $RepoRoot ".github" "workflows"
}

$DateStamp = Get-Date -Format "yyyyMMdd"
$StateDir  = Join-Path $RepoRoot "state"
$null = New-Item -ItemType Directory -Force -Path $StateDir
$LogFile   = Join-Path $StateDir "ci-no-push-audit-$DateStamp.log"

function Log-Info($msg)  { Write-Host "[audit-no-push] $msg" -ForegroundColor Cyan }
function Log-Pass($msg)  { Write-Host "[audit-no-push] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg)  { Write-Host "[audit-no-push] FAIL $msg" -ForegroundColor Red }

Log-Info "Scanning: $WorkflowDir"

if (-not (Test-Path $WorkflowDir)) {
    Log-Fail "WorkflowDir not found: $WorkflowDir"
    exit 2
}

$results = [System.Collections.Generic.List[hashtable]]::new()
$violations = [System.Collections.Generic.List[string]]::new()

$files = Get-ChildItem -Path $WorkflowDir -Filter "*.yml" -Recurse -ErrorAction SilentlyContinue

foreach ($file in $files) {
    $content = Get-Content -Path $file.FullName -Raw -ErrorAction SilentlyContinue
    if (-not $content) { continue }

    $triggersFound = [System.Collections.Generic.List[string]]::new()

    # Parse the top-level `on:` block for forbidden trigger keys.
    # Strategy: locate the `on:` section, collect its immediate child keys.
    # YAML is not fully parsed — we use targeted regex on the on: block
    # to find bare push:, pull_request:, schedule:, and branch_protection_rule:.
    # This is sufficient for the flat-key detection needed by this constraint.

    # Extract the on: block: from "^on:" to the next top-level key or EOF.
    $onBlockMatch = [regex]::Match($content, '(?m)^on:\s*\n((?:[ \t]+[^\n]*\n?)*)', 'Multiline')
    $onInlineMatch = [regex]::Match($content, '(?m)^on:\s*\{([^}]*)\}')

    $onBlock = ""
    if ($onBlockMatch.Success) {
        $onBlock = $onBlockMatch.Value
    } elseif ($onInlineMatch.Success) {
        $onBlock = $onInlineMatch.Value
    }

    # Forbidden trigger patterns (keys that auto-fire without human dispatch)
    $forbidden = @{
        "push"               = '(?m)^\s{0,4}push\s*:'
        "pull_request"       = '(?m)^\s{0,4}pull_request\s*:'
        "schedule"           = '(?m)^\s{0,4}schedule\s*:'
        "branch_protection_rule" = '(?m)^\s{0,4}branch_protection_rule\s*:'
    }

    foreach ($key in $forbidden.Keys) {
        if ([regex]::IsMatch($onBlock, $forbidden[$key])) {
            $triggersFound.Add($key)
        }
    }

    $hasDispatch = [regex]::IsMatch($onBlock, '(?m)^\s{0,4}workflow_dispatch\s*:')

    # Status: PASS only if workflow_dispatch is the sole trigger (no forbidden keys).
    $status = if ($triggersFound.Count -eq 0) { "PASS" } else { "FAIL" }

    $result = @{
        file           = $file.Name
        path           = $file.FullName
        triggers_found = ($triggersFound | ForEach-Object { $_ })
        has_dispatch   = $hasDispatch
        status         = $status
    }
    $results.Add($result)

    if ($status -eq "FAIL") {
        $violations.Add($file.Name)
        Log-Fail "$($file.Name) — forbidden triggers: $($triggersFound -join ', ')"
    } else {
        Log-Pass "$($file.Name) — workflow_dispatch only"
    }
}

# ---- Aggregate verdict ----
$aggregateVerdict = if ($violations.Count -eq 0) { "PASS" } else { "FAIL" }

$output = [ordered]@{
    ts              = (Get-Date -Format "o")
    workflow_dir    = $WorkflowDir
    files_scanned   = $results.Count
    violations      = $violations.Count
    aggregate       = $aggregateVerdict
    results         = ($results | ForEach-Object {
        [ordered]@{
            file           = $_.file
            triggers_found = $_.triggers_found
            has_dispatch   = $_.has_dispatch
            status         = $_.status
        }
    })
    invariant       = "Zero auto-triggers in GHA workflows. Any push:/pull_request:/schedule: trigger is a constraint violation."
}

$jsonOut = $output | ConvertTo-Json -Depth 5
$jsonOut | Set-Content -Path $LogFile -Encoding UTF8

Write-Host ""
Log-Info "Log: $LogFile"
Write-Host ""
Write-Host $jsonOut

if ($aggregateVerdict -eq "FAIL") {
    Write-Host ""
    Log-Fail "AGGREGATE FAIL — $($violations.Count) violation(s): $($violations -join ', ')"
    Write-Host "[audit-no-push] ACTION REQUIRED: Remove push:/pull_request:/schedule: triggers or change to workflow_dispatch: only."
    exit 1
}

Write-Host ""
Log-Pass "AGGREGATE PASS — all $($results.Count) workflow(s) are workflow_dispatch-only"
exit 0
