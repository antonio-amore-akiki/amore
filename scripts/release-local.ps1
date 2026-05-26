#!/usr/bin/env pwsh
# scripts/release-local.ps1 — Local-only release pipeline for Amore.
#
# Replaces GHA release.yml (killed by paid-minute billing on private repo).
# Produces signed Linux (Docker) + Windows (native cargo) artifacts and uploads
# them to a GitHub release tag.  macOS is dropped — no Apple hardware available.
#
# Exit codes:
#   0 = all steps green
#   1 = deps missing
#   2 = git state dirty or tag missing
#   3 = Windows build failed
#   4 = Linux Docker build failed
#   5 = SBOM generation failed
#   6 = sigstore sign failed
#   7 = attestation failed
#   8 = cosign verify-blob failed
#   9 = gh release upload failed
#
# Usage:
#   pwsh ./scripts/release-local.ps1 -Version 0.5.0
#   pwsh ./scripts/release-local.ps1 -Version 0.4.1 -SkipLinux -SkipSign -SkipUpload

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$SkipLinux,
    [switch]$SkipSign,
    [switch]$SkipUpload,
    [string]$ReleaseTag
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not $ReleaseTag) { $ReleaseTag = "v$Version" }

$RepoRoot = Split-Path -Parent $PSScriptRoot
$DistDir  = Join-Path $RepoRoot "dist"

function Log-Step($msg) { Write-Host "[release-local] STEP $msg" -ForegroundColor Cyan }
function Log-Pass($msg) { Write-Host "[release-local] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg, [int]$code) {
    Write-Host "[release-local] FAIL $msg (exit $code)" -ForegroundColor Red
    exit $code
}

# ---- Step 1: Verify deps ----
Log-Step "1/11 Verifying required tools"
foreach ($cmd in @("docker --version", "cargo --version", "gh --version", "cosign version")) {
    $result = Invoke-Expression $cmd 2>&1
    if ($LASTEXITCODE -ne 0) { Log-Fail "Required tool missing: $cmd. Install it first. See docs/RELEASING.md." 1 }
}
# cargo-cyclonedx ships as `cargo cyclonedx`, probe it
cargo cyclonedx --version 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { Log-Fail "cargo-cyclonedx not installed. Run: cargo install cargo-cyclonedx" 1 }
Log-Pass "All deps present"

# ---- Step 2: Verify git state ----
Log-Step "2/11 Verifying git state"
$dirty = git -C $RepoRoot status --short 2>&1
if ($dirty) { Log-Fail "Working tree is dirty. Commit or stash changes before releasing." 2 }
$HeadSha = git -C $RepoRoot rev-parse HEAD 2>&1
$tagExists = git -C $RepoRoot tag -l $ReleaseTag 2>&1
if (-not $tagExists) {
    Log-Fail "Tag '$ReleaseTag' does not exist. Create it first: gh release create $ReleaseTag --draft" 2
}
Log-Pass "Git clean; HEAD=$HeadSha; tag=$ReleaseTag found"

# ---- Step 3: Windows build ----
Log-Step "3/11 Windows build (cargo build --release -p amore-cli -p amore-mcp)"
$null = New-Item -ItemType Directory -Force -Path $DistDir
$winProc = Start-Process -FilePath "cargo" `
    -ArgumentList "build", "--release", "-p", "amore-cli", "-p", "amore-mcp" `
    -WorkingDirectory $RepoRoot `
    -NoNewWindow -Wait -PassThru
if ($winProc.ExitCode -ne 0) { Log-Fail "Windows build failed" 3 }
$WinBins = @(
    (Join-Path $RepoRoot "target\release\amore.exe"),
    (Join-Path $RepoRoot "target\release\amore-mcp.exe")
)
foreach ($b in $WinBins) {
    if (-not (Test-Path $b)) { Log-Fail "Expected binary not found: $b" 3 }
}
$WinArchiveName = "amore-$ReleaseTag-x86_64-pc-windows-msvc.zip"
$WinArchivePath = Join-Path $DistDir $WinArchiveName
$zipItems = $WinBins + @(
    (Join-Path $RepoRoot "LICENSE"),
    (Join-Path $RepoRoot "NOTICE"),
    (Join-Path $RepoRoot "README.md")
)
Compress-Archive -Path $zipItems -DestinationPath $WinArchivePath -Force
Log-Pass "Windows archive: $WinArchivePath"

# ---- Step 4: Linux build (Docker) ----
$LinuxArchivePath = $null
if (-not $SkipLinux) {
    Log-Step "4/11 Linux build via Docker (rust:1.95-bookworm)"
    $dockerArgs = @(
        "run", "--rm",
        "-v", "${RepoRoot}:/work",
        "-w", "/work",
        "rust:1.95-bookworm",
        "bash", "-c",
        "apt-get update -qq && apt-get install -y -q protobuf-compiler && cargo build --release -p amore-cli -p amore-mcp 2>&1"
    )
    $linProc = Start-Process -FilePath "docker" -ArgumentList $dockerArgs `
        -NoNewWindow -Wait -PassThru
    if ($linProc.ExitCode -ne 0) { Log-Fail "Linux Docker build failed" 4 }
    $LinBins = @(
        (Join-Path $RepoRoot "target\release\amore"),
        (Join-Path $RepoRoot "target\release\amore-mcp")
    )
    foreach ($b in $LinBins) {
        if (-not (Test-Path $b)) { Log-Fail "Expected Linux binary not found: $b" 4 }
    }
    $LinuxArchiveName = "amore-$ReleaseTag-x86_64-unknown-linux-gnu.tar.gz"
    $LinuxArchivePath = Join-Path $DistDir $LinuxArchiveName
    # tar from Git-for-Windows/MSYS2 handles this on Windows
    tar -C (Join-Path $RepoRoot "target\release") `
        -czf $LinuxArchivePath "amore" "amore-mcp"
    if ($LASTEXITCODE -ne 0) { Log-Fail "Linux tar.gz packaging failed" 4 }
    Log-Pass "Linux archive: $LinuxArchivePath"
} else {
    Log-Step "4/11 Linux build SKIPPED (-SkipLinux)"
}

# ---- Step 5: SBOM ----
Log-Step "5/11 Generating SBOM (cargo cyclonedx)"
$sbomPath = Join-Path $DistDir "sbom.cdx.json"
$sbomProc = Start-Process -FilePath "cargo" `
    -ArgumentList "cyclonedx", "--workspace", "--format", "json", "--output-cdx", $sbomPath `
    -WorkingDirectory $RepoRoot -NoNewWindow -Wait -PassThru
if ($sbomProc.ExitCode -ne 0) { Log-Fail "SBOM generation failed" 5 }
Log-Pass "SBOM: $sbomPath"

# Collect all artifacts for signing + upload
$artifacts = @($WinArchivePath)
if ($LinuxArchivePath) { $artifacts += $LinuxArchivePath }

# ---- Step 6: Sigstore sign ----
if (-not $SkipSign) {
    Log-Step "6/11 Sigstore keyless sign (browser OIDC flow will open — click the URL)"
    foreach ($art in $artifacts) {
        $bundle = "$art.sigstore"
        Write-Host "[release-local] Signing $art -> $bundle" -ForegroundColor Yellow
        cosign sign-blob --yes --bundle $bundle $art
        if ($LASTEXITCODE -ne 0) { Log-Fail "cosign sign-blob failed for $art" 6 }
    }
    Log-Pass "All artifacts signed"
} else {
    Log-Step "6/11 Sigstore sign SKIPPED (-SkipSign)"
}

# ---- Step 7: SLSA provenance attestation ----
if (-not $SkipSign) {
    Log-Step "7/11 SLSA provenance attestation (cosign attest-blob)"
    $buildDate = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    foreach ($art in $artifacts) {
        $hash = (Get-FileHash -Algorithm SHA256 $art).Hash.ToLower()
        $predicate = @{
            buildType = "https://github.com/antonio-amore-akiki/amore/release-local"
            builder   = @{ id = "local-windows-$env:COMPUTERNAME" }
            recipe    = @{ entryPoint = "scripts/release-local.ps1" }
            metadata  = @{
                buildStartedOn    = $buildDate
                completeness      = @{ environment = $true }
            }
            materials = @(@{ uri = "git+https://github.com/antonio-amore-akiki/amore"; digest = @{ sha1 = $HeadSha } })
        }
        $predicateFile = "$art.provenance.json"
        $predicate | ConvertTo-Json -Depth 10 | Set-Content -Path $predicateFile -Encoding UTF8
        $attestBundle = "$art.attest.bundle"
        cosign attest-blob --predicate $predicateFile --type slsaprovenance --bundle $attestBundle $art
        if ($LASTEXITCODE -ne 0) { Log-Fail "cosign attest-blob failed for $art" 7 }
        Remove-Item $predicateFile -ErrorAction SilentlyContinue
    }
    Log-Pass "Attestations written"
} else {
    Log-Step "7/11 Attestation SKIPPED (-SkipSign)"
}

# ---- Step 8: Round-trip verify ----
if (-not $SkipSign) {
    Log-Step "8/11 cosign verify-blob (round-trip check)"
    $userEmail = "antonioakiki15@gmail.com"
    foreach ($art in $artifacts) {
        $bundle = "$art.sigstore"
        cosign verify-blob `
            --bundle $bundle `
            --certificate-identity-regexp $userEmail `
            --certificate-oidc-issuer "https://accounts.google.com" `
            $art
        if ($LASTEXITCODE -ne 0) { Log-Fail "cosign verify-blob failed for $art" 8 }
    }
    Log-Pass "All round-trip verifications passed"
} else {
    Log-Step "8/11 Verify SKIPPED (-SkipSign)"
}

# ---- Step 9: Upload ----
if (-not $SkipUpload) {
    Log-Step "9/11 Uploading to GitHub release $ReleaseTag"
    foreach ($art in $artifacts) {
        $uploads = @($art, $sbomPath)
        if (-not $SkipSign) {
            $uploads += "$art.sigstore"
            $uploads += "$art.attest.bundle"
        }
        foreach ($f in $uploads) {
            if (Test-Path $f) {
                gh release upload $ReleaseTag $f --clobber --repo "antonio-amore-akiki/amore"
                if ($LASTEXITCODE -ne 0) { Log-Fail "gh release upload failed for $f" 9 }
            }
        }
    }
    Log-Pass "All assets uploaded to $ReleaseTag"
} else {
    Log-Step "9/11 Upload SKIPPED (-SkipUpload)"
}

# ---- Step 10: Write proof JSON ----
Log-Step "10/11 Writing proof JSON"
$proofDir = Join-Path $env:LOCALAPPDATA "Amore"
$null = New-Item -ItemType Directory -Force -Path $proofDir
$ts = Get-Date -Format "yyyyMMddTHHmmssZ"
$proofPath = Join-Path $proofDir "release-local-$ReleaseTag-$ts.json"
$proof = @{
    version     = $Version
    tag         = $ReleaseTag
    head_sha    = $HeadSha
    artifacts   = $artifacts
    sbom        = $sbomPath
    skip_sign   = [bool]$SkipSign
    skip_upload = [bool]$SkipUpload
    skip_linux  = [bool]$SkipLinux
    timestamp   = (Get-Date -Format "o")
}
$proof | ConvertTo-Json -Depth 10 | Set-Content -Path $proofPath -Encoding UTF8
Log-Pass "Proof JSON: $proofPath"

# ---- Step 11: Append docs/results.tsv ----
Log-Step "11/11 Appending docs/results.tsv"
$tsvPath = Join-Path $RepoRoot "docs\results.tsv"
$skipFlags = @()
if ($SkipLinux)  { $skipFlags += "SkipLinux" }
if ($SkipSign)   { $skipFlags += "SkipSign" }
if ($SkipUpload) { $skipFlags += "SkipUpload" }
$flags = if ($skipFlags) { $skipFlags -join "+" } else { "full" }
$tsvRow = "$(Get-Date -Format 'yyyy-MM-ddTHH:mmZ')`tpipeline-replace`tpwsh ./scripts/release-local.ps1 -Version $Version -$($skipFlags -join ' -')`tPASS`tpipeline_dry_run`tdeps-ok+builds-ok+sbom-ok+flags=$flags`tscripts/release-local.ps1`t$HeadSha"
Add-Content -Path $tsvPath -Value $tsvRow -Encoding UTF8
Log-Pass "results.tsv row appended"

Write-Host ""
Write-Host "[release-local] DONE — release $ReleaseTag complete" -ForegroundColor Green
exit 0
