#!/usr/bin/env pwsh
# scripts/release-preflight.ps1 — v-next #36 tag-blocking preflight (class-fix per
# docs/RELEASE-NOTES-v1.0.2.md). Called by release-local.ps1 BEFORE any tag/upload.
#
# Class observed across v1.0.0/v1.0.1/v1.0.2: each release shipped a defect that a clean
# preflight would have caught — stale CLI/MCP binaries reporting 0.5.0 (v1.0.0), latent
# clippy errors hidden by cargo incremental cache (v1.0.1), and 2 pre-existing integration
# test failures (v1.0.2). This preflight blocks the tag pipeline until ALL pass.
#
# Steps:
#   P1. cargo clippy --workspace --release --all-targets -- -D warnings
#   P2. cargo test --workspace --release --no-fail-fast
#   P3. per-bin --version match $Version (= CARGO_PKG_VERSION) — catches stale-bin defect
#
# Exit codes:
#   0 = all green
#  10 = clippy -D warnings failed
#  11 = workspace tests failed
#  12 = at least one .exe --version did not match $Version (stale-bin defect)
#
# Usage:
#   pwsh ./scripts/release-preflight.ps1 -Version 1.0.2
#   pwsh ./scripts/release-preflight.ps1 -Version 1.0.2 -SkipBinVersionCheck   # if pre-build

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$SkipBinVersionCheck
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot

function PF-Step($msg) { Write-Host "[preflight] STEP $msg" -ForegroundColor Cyan }
function PF-Pass($msg) { Write-Host "[preflight] PASS $msg" -ForegroundColor Green }
function PF-Fail($msg, [int]$code) {
    Write-Host "[preflight] FAIL $msg (exit $code)" -ForegroundColor Red
    exit $code
}

# ---- P1: cargo clippy -D warnings ----
PF-Step "P1: cargo clippy --workspace --release --all-targets -- -D warnings"
$clippyProc = Start-Process -FilePath "cargo" `
    -ArgumentList "clippy", "--workspace", "--release", "--all-targets", "--", "-D", "warnings" `
    -WorkingDirectory $RepoRoot -NoNewWindow -Wait -PassThru
if ($clippyProc.ExitCode -ne 0) { PF-Fail "clippy -D warnings failed — fix lints before tagging" 10 }
PF-Pass "clippy clean (0 warnings, 0 errors)"

# ---- P2: cargo test --workspace --release ----
PF-Step "P2: cargo test --workspace --release --no-fail-fast"
$testProc = Start-Process -FilePath "cargo" `
    -ArgumentList "test", "--workspace", "--release", "--no-fail-fast" `
    -WorkingDirectory $RepoRoot -NoNewWindow -Wait -PassThru
if ($testProc.ExitCode -ne 0) { PF-Fail "cargo test --workspace --release failed — fix failing tests before tagging" 11 }
PF-Pass "workspace tests green"

# ---- P3: per-bin --version match-CARGO_PKG_VERSION ----
# Class-fix per docs/RELEASE-NOTES-v1.0.1.md: v1.0.0 shipped MSI with amore.exe + amore-mcp.exe
# stamped 0.5.0 because only amore-gui was rebuilt. Verify every shipped .exe reports $Version.
if (-not $SkipBinVersionCheck) {
    PF-Step "P3: per-bin --version match $Version"
    $verChecks = @(
        @{ exe = "amore.exe";     args = "--version"; pattern = "amore $([regex]::Escape($Version))" },
        @{ exe = "amore-gui.exe"; args = "--no-gui";  pattern = "`"version`":`"$([regex]::Escape($Version))`"" },
        @{ exe = "amore-mcp.exe"; args = "--no-gui";  pattern = "`"version`":`"$([regex]::Escape($Version))`"" }
    )
    foreach ($check in $verChecks) {
        $exePath = Join-Path $RepoRoot "target\release\$($check.exe)"
        if (-not (Test-Path $exePath)) {
            PF-Fail "Expected binary not built: $exePath. Run: cargo build --release -p amore-cli -p amore-mcp -p amore-gui (or pass -SkipBinVersionCheck for pre-build)" 12
        }
        $verOut = & $exePath $check.args 2>&1 | Out-String
        if ($verOut -notmatch $check.pattern) {
            PF-Fail "BIN VERSION MISMATCH: $($check.exe) $($check.args) -> '$($verOut.Trim())' does NOT contain pattern '$($check.pattern)'. Expected $Version. Run: cargo clean && cargo build --release -p amore-cli -p amore-mcp -p amore-gui" 12
        }
        PF-Pass "$($check.exe) reports $Version"
    }
} else {
    PF-Step "P3: BIN VERSION CHECK SKIPPED (-SkipBinVersionCheck)"
}

Write-Host "[preflight] ALL GREEN — tag pipeline may proceed" -ForegroundColor Green
exit 0
