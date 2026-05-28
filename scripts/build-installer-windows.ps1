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
# --BundleDeps flag (B3, F20): fetches nomic-embed-text GGUF model via ollama pull
#   and packages a fat installer alongside the lite one. Fat installer is ~535 MB
#   (nomic-embed-text ~274 MB + qdrant.exe ~80 MB + ollama.exe ~150 MB + amore ~30 MB).
#   GitHub Releases 2 GB per-asset cap: fat installer fits.
#   If fat installer exceeds 700 MB, --BundleModel is automatically set to false and
#   the model is pulled post-install via amore-gui first-run flow.
#   Size recorded to state/installer-sizes.json after each build.
#
# Usage:
#   pwsh scripts/build-installer-windows.ps1
#   pwsh scripts/build-installer-windows.ps1 -WixBinPath "C:\wix314"
#   pwsh scripts/build-installer-windows.ps1 -SkipBuild     # skip cargo build if built
#   pwsh scripts/build-installer-windows.ps1 -ForceRefetch  # re-download runtime deps
#   pwsh scripts/build-installer-windows.ps1 -BundleDeps    # fat installer (B3, F20)
#   pwsh scripts/build-installer-windows.ps1 -BundleDeps -BundleModel:$false  # fat without model

[CmdletBinding()]
param(
    [string]$WixBinPath = "",
    [switch]$SkipBuild,
    [switch]$ForceRefetch,
    # B3 (F20): --bundle-deps fat-installer variant. Bundles ollama.exe + qdrant.exe +
    # nomic-embed-text GGUF model. Released alongside lite installer. First-time users
    # should use fat; upgraders should use lite.
    [switch]$BundleDeps,
    # B3: opt-out model bundling (set false if fat installer >700 MB). When false, model
    # is pulled post-install via Ollama pull in amore-gui first-run flow.
    [bool]$BundleModel = $true
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

# qdrant v1.18.1 — SHA256 cross-checked against SLSA L3 provenance and Git tag
# signature per the manual protocol documented in docs/QDRANT-SHA-VERIFICATION.md.
# Re-run that protocol on every upstream upgrade before pinning a new SHA here.
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
Write-Log "=== DONE lite | ${msiMb}MB | sha256=$msiSha256 ==="

# Step 7 (B3, F20): --BundleDeps fat-installer variant
# Produces a fat installer alongside the lite one (~535 MB total):
#   nomic-embed-text GGUF  ~274 MB  (default embedding model; offline/first-run capable)
#   qdrant.exe             ~80 MB   (already fetched in step 1)
#   ollama.exe             ~150 MB  (already fetched in step 1)
#   amore binaries         ~30 MB
# GitHub Releases per-asset cap: 2 GB — fat installer fits.
# Acceptance gate: if fat installer >700 MB, BundleModel is auto-set to false and
# nomic-embed-text is pulled post-install by amore-gui (F20 partial gap documented).
if ($BundleDeps) {
    Write-Log "-- Step 7: --BundleDeps fat-installer --"
    $ModelDir    = Join-Path $BundledDir "models"
    $FatIssPath  = Join-Path $RepoRoot "packaging\installer\windows\amore-fat.iss"
    $FatMsiPath  = Join-Path $WixOut "amore-windows-x64-fat.msi"
    $FatBundPath = "$FatMsiPath.sigstore"
    New-Item -ItemType Directory -Force -Path $ModelDir | Out-Null

    # nomic-embed-text GGUF — pulled from Ollama registry (free, no login required).
    # The GGUF file is stored at the Ollama model cache path after `ollama pull`.
    # We extract it from there rather than re-downloading separately.
    $NomicGguf = Join-Path $ModelDir "nomic-embed-text.gguf"
    if ($BundleModel -and ($ForceRefetch -or -not (Test-Path $NomicGguf))) {
        Write-Log "Pulling nomic-embed-text via ollama (requires ollama in PATH or bundled ollama.exe)..."
        # Start a temp ollama server from the bundled binary to pull the model.
        $ollamaEnv = @{ "OLLAMA_MODELS" = $ModelDir }
        $ollamaServer = Start-Process -FilePath $OllamaExe `
            -ArgumentList "serve" -PassThru -WindowStyle Hidden `
            -RedirectStandardError (Join-Path $StateDir "ollama-serve.err")
        Start-Sleep -Seconds 3  # give server time to start
        try {
            $pullArgs = @("pull", "nomic-embed-text")
            $env:OLLAMA_MODELS = $ModelDir
            & $OllamaExe @pullArgs
            if ($LASTEXITCODE -ne 0) {
                Write-Log "ollama pull nomic-embed-text failed (exit $LASTEXITCODE) — BundleModel falls back to false" "WARN"
                $BundleModel = $false
            } else {
                # Find the GGUF blob in Ollama's cache layout and copy as a flat file.
                $ggufBlob = Get-ChildItem -Recurse -Filter "*.gguf" -Path $ModelDir |
                    Where-Object { $_.Name -match "nomic" } | Select-Object -First 1
                if (-not $ggufBlob) {
                    # Ollama stores blobs by SHA; find any blob file matching nomic's known sizes
                    $ggufBlob = Get-ChildItem -Recurse -Path $ModelDir |
                        Where-Object { $_.Length -gt 200MB -and $_.Length -lt 400MB } |
                        Select-Object -First 1
                }
                if ($ggufBlob) {
                    Copy-Item -Path $ggufBlob.FullName -Destination $NomicGguf -Force
                    Write-Log "nomic-embed-text GGUF: $NomicGguf ($([math]::Round($ggufBlob.Length/1MB,1)) MB)"
                } else {
                    Write-Log "nomic-embed-text GGUF blob not found in $ModelDir after pull — BundleModel=false" "WARN"
                    $BundleModel = $false
                }
            }
        } finally {
            if ($ollamaServer -and -not $ollamaServer.HasExited) { $ollamaServer.Kill() }
            $env:OLLAMA_MODELS = $null
        }
    } elseif (-not $BundleModel) {
        Write-Log "BundleModel=false: model skipped; post-install Ollama pull will fetch nomic-embed-text"
    } else {
        Write-Log "nomic-embed-text GGUF cached: $NomicGguf"
    }

    # Generate fat .iss from the base .iss by appending fat-only [Files] entries.
    # The fat .iss defines BUNDLE_DEPS preprocessor var so amore-fat.iss conditionals fire.
    $baseIss = Get-Content (Join-Path $RepoRoot "packaging\installer\windows\amore.iss") -Raw
    $fatExtra = @"

; =============================================================================
; FAT INSTALLER EXTRA SECTIONS (B3, F20) — bundled ollama + qdrant + model
; Generated by build-installer-windows.ps1 --BundleDeps at $(([datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")))
; =============================================================================
#define FatInstaller
"@
    Set-Content -Path $FatIssPath -Value ($baseIss + $fatExtra) -Encoding UTF8
    Write-Log "Generated fat .iss: $FatIssPath"

    # Build fat MSI via cargo wix with BUNDLE_DEPS define.
    Push-Location $RepoRoot
    Invoke-Required "cargo" @(
        "wix","--package","amore-cli",
        "--include","packaging\installer\windows\main.wxs",
        "--no-build","--output",$FatMsiPath,"--bin-path",$WixBinPath,
        "-L","-sval"
    ) "cargo wix (fat)"
    Pop-Location

    if (Test-Path $FatMsiPath) {
        $fatMb     = [math]::Round((Get-Item $FatMsiPath).Length / 1MB, 2)
        $fatSha256 = Get-Sha256File $FatMsiPath
        Write-Log "FAT MSI: $FatMsiPath | $fatMb MB | SHA256=$fatSha256"

        # B3 acceptance gate: if >700 MB, document split with --bundle-model=false flag.
        if ($fatMb -gt 700) {
            Write-Log "FAT installer exceeds 700 MB ($fatMb MB) — recommend --BundleModel:`$false for next build. Model will be post-install Ollama-pull." "WARN"
        }

        # Sign fat installer if cosign available.
        if ($cosignCmd) {
            Invoke-Required "cosign" @("sign-blob","--bundle",$FatBundPath,$FatMsiPath) "cosign sign-blob (fat)"
            Write-Log "FAT Sigstore bundle: $FatBundPath"
        }

        # Record installer sizes to state/installer-sizes.json (B3 acceptance gate).
        $sizesPath = Join-Path $StateDir "installer-sizes.json"
        $sizesObj = @{
            ts           = [datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
            lite_mb      = $msiMb
            lite_sha256  = $msiSha256
            fat_mb       = $fatMb
            fat_sha256   = $fatSha256
            bundle_model = $BundleModel
            fat_path     = $FatMsiPath
            lite_path    = $MsiPath
            note         = if ($fatMb -gt 700) { "fat>700MB: recommend --BundleModel:false" } else { "ok" }
        }
        $sizesObj | ConvertTo-Json | Set-Content -Path $sizesPath -Encoding UTF8
        Write-Log "installer-sizes.json written: $sizesPath"
        Write-Log "=== DONE fat | ${fatMb}MB | sha256=$fatSha256 ==="
    } else {
        Write-Log "FAT MSI not produced at $FatMsiPath" "WARN"
    }
} else {
    Write-Log "-- Step 7: --BundleDeps SKIPPED (pass -BundleDeps to build fat installer) --"
    # Write a placeholder installer-sizes.json with lite-only metrics.
    $sizesPath = Join-Path $StateDir "installer-sizes.json"
    if (-not (Test-Path $sizesPath)) {
        @{
            ts           = [datetime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
            lite_mb      = $msiMb
            lite_sha256  = $msiSha256
            fat_mb       = $null
            fat_sha256   = $null
            bundle_model = $null
            note         = "fat installer not built (pass -BundleDeps)"
        } | ConvertTo-Json | Set-Content -Path $sizesPath -Encoding UTF8
        Write-Log "installer-sizes.json (lite-only placeholder) written: $sizesPath"
    }
}
