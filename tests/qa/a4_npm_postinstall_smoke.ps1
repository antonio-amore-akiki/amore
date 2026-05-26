# tests/qa/a4_npm_postinstall_smoke.ps1
#
# QA A4 (Windows lane) — npm pack + npm install actually runs postinstall,
# downloads obelion-v<VER>-x86_64-pc-windows-msvc.zip from the GitHub Release,
# extracts to bin/obelion.exe + bin/obelion-mcp.exe, and `--version` reports
# the matching tag. Proves the production npm distribution path end-to-end
# against the LIVE release (no mocks, no synthetic asset).
#
# Auth note: while the obelion repo is private, postinstall.js consumes
# GITHUB_TOKEN/GH_TOKEN/OBELION_GITHUB_TOKEN as Bearer credentials. This
# script forwards `gh auth token` so the test reflects the production code
# path for private-repo users; public-repo users post-v1.0 won't need it.
#
# Exit 0 -> npm install succeeded, binaries present, --version matches.
# Exit non-zero -> any step failed; raw stderr surfaced.

[CmdletBinding()]
param(
    [string]$Tag = "v0.2.1"
)

$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$npmDir = Join-Path $repoRoot "npm"
$expectedVersion = $Tag.TrimStart("v")

Write-Output "[a4] repo root: $repoRoot"
Write-Output "[a4] npm dir:   $npmDir"
Write-Output "[a4] tag:       $Tag (version $expectedVersion)"

if (-not (Test-Path (Join-Path $npmDir "package.json"))) {
    Write-Error "[a4] npm/package.json missing — expected $npmDir/package.json"
    exit 2
}

# --- step 1: npm pack inside npm/ to produce the .tgz npm install will see
Push-Location $npmDir
try {
    Write-Output "[a4] running: npm pack"
    $packOut = & npm pack 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "[a4] npm pack failed: $packOut"
        exit 3
    }
    $packFile = ($packOut | Select-Object -Last 1).ToString().Trim()
    $packPath = Join-Path $npmDir $packFile
    if (-not (Test-Path $packPath)) {
        Write-Error "[a4] npm pack output not found at $packPath"
        exit 3
    }
    Write-Output "[a4] packed: $packPath"
} finally {
    Pop-Location
}

# --- step 2: fresh sandbox dir for the install
$sandbox = Join-Path $env:TEMP "obelion-a4-$([guid]::NewGuid().ToString('N').Substring(0,8))"
New-Item -ItemType Directory -Force -Path $sandbox | Out-Null
Write-Output "[a4] sandbox: $sandbox"

Set-Content -Path (Join-Path $sandbox "package.json") -Value '{"name":"a4-smoke","version":"0.0.0","private":true}' -NoNewline

# --- step 3: resolve a GitHub PAT for the private-repo download
$ghToken = ""
try {
    $ghToken = (& gh auth token 2>&1 | Out-String).Trim()
} catch {}
if (-not $ghToken) {
    Write-Error "[a4] no GH token available via 'gh auth token'. Obelion repo is private during MVP — install would 404."
    exit 4
}
Write-Output "[a4] GH token resolved (length $($ghToken.Length))"

# --- step 4: npm install the local .tgz in the sandbox, with GH_TOKEN passthru
Push-Location $sandbox
try {
    $env:GH_TOKEN = $ghToken
    $env:GITHUB_TOKEN = $ghToken
    Write-Output "[a4] running: npm install $packPath"
    $installOut = & npm install --no-fund --no-audit $packPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Output ($installOut | Out-String)
        Write-Error "[a4] npm install failed (exit $LASTEXITCODE)"
        exit 5
    }
    Write-Output "[a4] npm install OK"
    $installOut | ForEach-Object { Write-Output "[a4-npm] $_" }
} finally {
    Pop-Location
    Remove-Item Env:GH_TOKEN -ErrorAction SilentlyContinue
    Remove-Item Env:GITHUB_TOKEN -ErrorAction SilentlyContinue
}

# --- step 5: validate the binaries were actually extracted into bin/
$installedPkg = Join-Path $sandbox "node_modules\@anto\obelion"
$binDir = Join-Path $installedPkg "bin"
$obelionExe = Join-Path $binDir "obelion.exe"
$obelionMcpExe = Join-Path $binDir "obelion-mcp.exe"

foreach ($f in @($obelionExe, $obelionMcpExe)) {
    if (-not (Test-Path $f)) {
        Write-Error "[a4] expected binary missing: $f"
        exit 6
    }
    $size = (Get-Item $f).Length
    Write-Output "[a4] $($f): $size bytes"
    if ($size -lt 1024 * 1024) {
        Write-Error "[a4] $f under 1MB — suspicious. The release artifact should be ~5MB extracted."
        exit 7
    }
}

# --- step 6: run --version on the obelion binary and assert tag match
Write-Output "[a4] running: obelion.exe --version"
$verOut = (& $obelionExe --version 2>&1 | Out-String).Trim()
Write-Output "[a4] obelion --version -> $verOut"

if ($verOut -notmatch [regex]::Escape($expectedVersion)) {
    Write-Error "[a4] obelion --version output did not contain ${expectedVersion}: '$verOut'"
    exit 8
}

# --- cleanup of pack.tgz + sandbox (sandbox optional; keep on demand)
Remove-Item -Path $packPath -Force -ErrorAction SilentlyContinue
if ($env:OBELION_A4_KEEP_SANDBOX -ne "1") {
    Remove-Item -Path $sandbox -Recurse -Force -ErrorAction SilentlyContinue
    Write-Output "[a4] sandbox cleaned"
} else {
    Write-Output "[a4] sandbox kept at $sandbox (OBELION_A4_KEEP_SANDBOX=1)"
}

Write-Output "[a4] PASS — npm install end-to-end against live $Tag release; binaries verified."
exit 0
