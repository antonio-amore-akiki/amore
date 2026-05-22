#!/usr/bin/env pwsh
# scripts/build-installer-windows.ps1 — Windows MSI build pipeline.
#
# Prior-art: Adapt from release-local.ps1 (param block, Write-Log, Invoke-Required,
# cosign block, $RepoRoot resolution) + release-dry-run.ps1 (Set-StrictMode,
# ISO-timestamp log). Build (net-new): cargo wix invocation, bundled dep fetch+SHA256
# verify, Expand-Archive extraction, Get-AuthenticodeSignature check.
#
# Steps:
#   1. Fetch + verify ollama.exe (v0.24.0) + qdrant.exe (v1.18.1) -> target/bundled/
#   2. cargo build --release --bin amore --bin amore-mcp --bin amore-gui
#   3. cargo wix --package amore-cli --include packaging/installer/windows/main.wxs
#      --no-build --output target/wix/amore-windows-x64.msi --bin-path <WiX>
#   4. cosign sign-blob --bundle amore-windows-x64.msi.sigstore amore-windows-x64.msi
#   5. cosign verify-blob (proof)
#   Log:   state/w8.5a-build.log
#   Smoke: state/w8.5a-smoke.log
#
# Usage:
#   pwsh scripts/build-installer-windows.ps1
#   pwsh scripts/build-installer-windows.ps1 -WixBinPath "C:\wix314"
#   pwsh scripts/build-installer-windows.ps1 -SkipBuild     # skip cargo build if built
#   pwsh scripts/build-installer-windows.ps1 -ForceRefetch  # re-download runtime deps

[CmdletBinding()]
param(
    [string]$WixBinPath = "",
    [switch]$SkipBuild,
    [switch]$ForceRefetch
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot   = Split-Path -Parent $PSScriptRoot
$BundledDir = Join-Path $RepoRoot "target\bundled"
$WixOut     = Join-Path $RepoRoot "target\wix"
$StateDir   = Join-Path $RepoRoot "state"
$LogFile    = Join-Path $StateDir "w8.5a-build.log"
$SmokeLog   = Join-Path $StateDir "w8.5a-smoke.log"
$MsiPath    = Join-Path $WixOut "amore-windows-x64.msi"
$BundlePath = "$MsiPath.sigstore"

# ollama v0.24.0 — SHA256 from upstream sha256sum.txt published with each release
$OllamaVersion = "v0.24.0"
$OllamaZipUrl  = "https://github.com/ollama/ollama/releases/download/$OllamaVersion/ollama-windows-amd64.zip"
$OllamaZipSha  = "40c523d3eeba6f4647c5ca58fe47f15b8dee79f7675ebf573458890064f424c7"

# qdrant v1.18.1 — no upstream sha file for Windows zip; sentinel until first build
$QdrantVersion = "v1.18.1"
$QdrantZipUrl  = "https://github.com/qdrant/qdrant/releases/download/$QdrantVersion/qdrant-x86_64-pc-windows-msvc.zip"
$QdrantZipSha  = "fe1eab78c24157b21988b3480ce75709e76ca0168ba644fc5a49017bacfec1c6"

function Write-Log {
    param([string]$Msg, [string]$Level = "INFO")
    $ts   = [datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
    $line = "[$ts][$Level] $Msg"
    Write-Host $line
    Add-Content -Path $LogFile -Value $line -Encoding UTF8
}

function Invoke-Required {
    param([string]$Exe, [string[]]$CmdArgs, [string]$Desc)
    Write-Log "RUN: $Exe $($CmdArgs -join ' ')"
    & $Exe @CmdArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Log "FAIL: $Desc (exit $LASTEXITCODE)" "ERROR"
        exit $LASTEXITCODE
    }
    Write-Log "OK: $Desc"
}

function Get-Sha256File {
    param([string]$Path)
    return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLower()
}

function Assert-Sha256 {
    param([string]$Path, [string]$Expected, [string]$Label)
    $actual = Get-Sha256File $Path
    if ($actual -ne $Expected.ToLower()) {
        Write-Log "SHA256 MISMATCH: $Label expected=$Expected actual=$actual" "ERROR"
        exit 1
    }
    Write-Log "SHA256 OK: $Label = $actual"
}

function Get-BinaryFromZip {
    param([string]$ZipPath, [string]$ExeName, [string]$DestPath)
    $tmp = Join-Path $env:TEMP "amore-extract-$([System.IO.Path]::GetRandomFileName())"
    Expand-Archive -Path $ZipPath -DestinationPath $tmp -Force
    $found = Get-ChildItem -Recurse -Filter $ExeName -Path $tmp | Select-Object -First 1
    if (-not $found) {
        Write-Log "$ExeName not found inside zip $ZipPath" "ERROR"
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
        exit 1
    }
    Copy-Item -Path $found.FullName -Destination $DestPath -Force
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Write-Log "Extracted: $ExeName -> $DestPath"
}

foreach ($d in @($BundledDir, $WixOut, $StateDir)) { New-Item -ItemType Directory -Force -Path $d | Out-Null }
Set-Content -Path $LogFile -Value "" -Encoding UTF8
Write-Log "=== Windows MSI build start ==="

# Step 1: Bundled runtime deps
Write-Log "-- Step 1: Fetch bundled runtime deps --"
$OllamaExe = Join-Path $BundledDir "ollama.exe"
if ($ForceRefetch -or -not (Test-Path $OllamaExe)) {
    $ollamaZip = Join-Path $BundledDir "ollama-windows-amd64.zip"
    Write-Log "Downloading ollama $OllamaVersion ..."
    Invoke-WebRequest -Uri $OllamaZipUrl -OutFile $ollamaZip -UseBasicParsing
    Assert-Sha256 $ollamaZip $OllamaZipSha "ollama-windows-amd64.zip"
    Get-BinaryFromZip $ollamaZip "ollama.exe" $OllamaExe
    Remove-Item $ollamaZip -Force
} else { Write-Log "ollama.exe cached: $OllamaExe" }

$QdrantExe = Join-Path $BundledDir "qdrant.exe"
if ($ForceRefetch -or -not (Test-Path $QdrantExe)) {
    $qdrantZip = Join-Path $BundledDir "qdrant-windows.zip"
    Write-Log "Downloading qdrant $QdrantVersion ..."
    Invoke-WebRequest -Uri $QdrantZipUrl -OutFile $qdrantZip -UseBasicParsing
    $actualQdrantSha = Get-Sha256File $qdrantZip
    Write-Log "qdrant zip SHA256: $actualQdrantSha"
    if ($QdrantZipSha -notin @("FETCH_AND_PIN", "")) {
        Assert-Sha256 $qdrantZip $QdrantZipSha "qdrant-x86_64-pc-windows-msvc.zip"
    } else {
        Write-Log "WARN: QdrantZipSha sentinel — pin '$actualQdrantSha' in script after first run." "WARN"
    }
    Get-BinaryFromZip $qdrantZip "qdrant.exe" $QdrantExe
    Remove-Item $qdrantZip -Force
} else { Write-Log "qdrant.exe cached: $QdrantExe" }

# Step 2: Rust binaries
if (-not $SkipBuild) {
    Write-Log "-- Step 2: cargo build --release --"
    Push-Location $RepoRoot
    Invoke-Required "cargo" @("build","--release","--bin","amore","--bin","amore-mcp","--bin","amore-gui") "cargo build --release"
    Pop-Location
} else { Write-Log "-- Step 2: SKIPPED (-SkipBuild) --" }

# Step 3: WiX Toolset
Write-Log "-- Step 3: Locate WiX Toolset --"
if ($WixBinPath -eq "") {
    if ($env:WIX -and (Test-Path (Join-Path $env:WIX "bin\candle.exe"))) {
        $WixBinPath = Join-Path $env:WIX "bin"
    } elseif (Get-Command "candle.exe" -ErrorAction SilentlyContinue) {
        $WixBinPath = (Get-Command "candle.exe").Source | Split-Path -Parent
    } else {
        $portable = Join-Path $env:TEMP "wix314"
        if (Test-Path (Join-Path $portable "candle.exe")) { $WixBinPath = $portable }
        else { Write-Log "WiX not found. Install: winget install WiXToolset.WiXToolset" "ERROR"; exit 1 }
    }
}
Write-Log "WiX bin: $WixBinPath"

# Step 4: cargo wix
Write-Log "-- Step 4: cargo wix --"
Push-Location $RepoRoot
Invoke-Required "cargo" @(
    "wix","--package","amore-cli",
    "--include","packaging\installer\windows\main.wxs",
    "--no-build","--output",$MsiPath,"--bin-path",$WixBinPath,
    "-L","-sval"   # suppress MSI validation: portable WiX lacks darice.cub
) "cargo wix"
Pop-Location

if (-not (Test-Path $MsiPath)) { Write-Log "MSI not produced at $MsiPath" "ERROR"; exit 1 }
$msiMb     = [math]::Round((Get-Item $MsiPath).Length / 1MB, 2)
$msiSha256 = Get-Sha256File $MsiPath
Write-Log "MSI: $MsiPath | $msiMb MB | SHA256=$msiSha256"

# Step 5: Sigstore sign + verify
Write-Log "-- Step 5: cosign sign-blob + verify-blob --"
$cosignCmd = Get-Command "cosign" -ErrorAction SilentlyContinue
if (-not $cosignCmd) {
    Write-Log "cosign not in PATH — Sigstore signing deferred." "WARN"
    Set-Content -Path $SmokeLog -Value "SIGSTORE: DEFERRED`nMSI: $MsiPath`nSHA256: $msiSha256" -Encoding UTF8
} else {
    Invoke-Required "cosign" @("sign-blob","--bundle",$BundlePath,$MsiPath) "cosign sign-blob"
    Invoke-Required "cosign" @("verify-blob","--bundle",$BundlePath,$MsiPath) "cosign verify-blob"
    Write-Log "Sigstore bundle: $BundlePath"
    Set-Content -Path $SmokeLog -Value "SIGSTORE: PASS`nMSI: $MsiPath`nSHA256: $msiSha256`nBUNDLE: $BundlePath" -Encoding UTF8
}

# Step 6: Authenticode (informational)
Write-Log "-- Step 6: Get-AuthenticodeSignature --"
$authSig = Get-AuthenticodeSignature $MsiPath
Write-Log "AuthenticodeSignature.Status: $($authSig.Status)"
Write-Log "=== DONE | ${msiMb}MB | sha256=$msiSha256 ==="
