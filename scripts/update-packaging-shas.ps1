#!/usr/bin/env pwsh
# scripts/update-packaging-shas.ps1 — Replace PLACEHOLDER_<filename>_SHA256 tokens in
# packaging descriptors using a sha256sums.txt produced by release-local.ps1.
#
# Usage:
#   pwsh ./scripts/update-packaging-shas.ps1 -ReleaseDir <path>
#
# sha256sums.txt format (one entry per line, GNU sha256sum convention):
#   <sha256_hex>  <filename>
#
# Targets (relative to repo root):
#   packaging/homebrew/amore.rb
#   packaging/winget/manifests/**/*.yaml
#   packaging/aur/PKGBUILD
#
# Exit codes:
#   0 = all tokens replaced
#   1 = sha256sums.txt missing or empty
#   2 = one or more PLACEHOLDER tokens still present after substitution pass

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ReleaseDir
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot

function Log-Info($msg)  { Write-Host "[update-packaging-shas] $msg" -ForegroundColor Cyan }
function Log-Pass($msg)  { Write-Host "[update-packaging-shas] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg, [int]$code) {
    Write-Host "[update-packaging-shas] FAIL $msg (exit $code)" -ForegroundColor Red
    exit $code
}

# ---- Parse sha256sums.txt ----
$sumsFile = Join-Path $ReleaseDir "sha256sums.txt"
if (-not (Test-Path $sumsFile)) {
    Log-Fail "sha256sums.txt not found in ReleaseDir: $ReleaseDir" 1
}

$shaMap = @{}
foreach ($line in (Get-Content $sumsFile)) {
    $line = $line.Trim()
    if (-not $line -or $line.StartsWith('#')) { continue }
    # Format: "<sha256>  <filename>" (one or two spaces, GNU sha256sum output)
    if ($line -match '^([0-9a-fA-F]{64})\s{1,2}(.+)$') {
        $sha  = $Matches[1].ToLower()
        $file = [System.IO.Path]::GetFileName($Matches[2].Trim())
        $shaMap[$file] = $sha
        Log-Info "Parsed: $file -> $sha"
    } else {
        Log-Info "Skipping unrecognised line: $line"
    }
}
if ($shaMap.Count -eq 0) { Log-Fail "sha256sums.txt parsed but contains no valid entries" 1 }

# ---- Collect descriptor files ----
$descriptors = [System.Collections.Generic.List[string]]::new()

$homebrewFile = Join-Path $RepoRoot "packaging\homebrew\amore.rb"
if (Test-Path $homebrewFile) { $descriptors.Add($homebrewFile) }

$wingetRoot = Join-Path $RepoRoot "packaging\winget\manifests"
if (Test-Path $wingetRoot) {
    Get-ChildItem -Path $wingetRoot -Recurse -Filter "*.yaml" -ErrorAction SilentlyContinue |
        ForEach-Object { $descriptors.Add($_.FullName) }
}

$pkgbuildFile = Join-Path $RepoRoot "packaging\aur\PKGBUILD"
if (Test-Path $pkgbuildFile) { $descriptors.Add($pkgbuildFile) }

if ($descriptors.Count -eq 0) { Log-Fail "No descriptor files found under packaging/" 1 }
Log-Info "Descriptors to update: $($descriptors.Count)"

# ---- Replace tokens ----
foreach ($filename in $shaMap.Keys) {
    $searchFor = "PLACEHOLDER_${filename}_SHA256"
    $sha       = $shaMap[$filename]
    $replaced  = 0
    foreach ($desc in $descriptors) {
        $content = [System.IO.File]::ReadAllText($desc, [System.Text.Encoding]::UTF8)
        if ($content.Contains($searchFor)) {
            $content = $content.Replace($searchFor, $sha)
            [System.IO.File]::WriteAllText($desc, $content, [System.Text.Encoding]::UTF8)
            Log-Pass "  $([System.IO.Path]::GetFileName($desc)): replaced $searchFor"
            $replaced++
        }
    }
    if ($replaced -eq 0) {
        Log-Info "  Token not found in any descriptor (platform not in this release?): $token"
    }
}

# ---- Verify no PLACEHOLDER tokens remain ----
$remaining = 0
foreach ($desc in $descriptors) {
    $content = [System.IO.File]::ReadAllText($desc, [System.Text.Encoding]::UTF8)
    $hits = [regex]::Matches($content, 'PLACEHOLDER_[^\s"'']+_SHA256')
    foreach ($hit in $hits) {
        Write-Host "[update-packaging-shas] RESIDUAL TOKEN in $([System.IO.Path]::GetFileName($desc)): $($hit.Value)" -ForegroundColor Red
        $remaining++
    }
}
if ($remaining -gt 0) {
    Log-Fail "$remaining PLACEHOLDER marker(s) still present — ensure those filenames are in sha256sums.txt" 2
}

Log-Pass "All PLACEHOLDER tokens resolved. Descriptors are release-ready."
exit 0
