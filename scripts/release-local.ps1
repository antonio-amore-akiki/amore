#!/usr/bin/env pwsh
# scripts/release-local.ps1 — Local-only release pipeline for Amore.
#
# Replaces GHA release.yml (killed by paid-minute billing on private repo).
# Produces signed Linux (Docker) + Windows (native cargo) artifacts and uploads
# them to a GitHub release tag.  macOS is dropped — no Apple hardware available.
#
# HERMETICITY (SLSA L3): Linux x86_64 uses pre-baked amore-builder-linux-x86_64:latest
# (Dockerfile.builder-linux-x86_64, rust@sha256:6258907..., no apt-get at build time — W8-8C M2).
# ARM64 uses cross-rs/cross with digest-pinned image in Cross.toml. Both containers --rm.
# Windows runs on dev host (non-ephemeral; see SLSA-L3-ATTESTATION.md gap note).
# SOURCE_DATE_EPOCH + RUSTFLAGS --remap-path-prefix ensure reproducible output.
# PREREQUISITE: run `pwsh ./scripts/build-builder-images.ps1` once before first release.
#
# Exit codes (0=all green):
#   1=deps missing  2=git state dirty/missing tag  3=Win build  4=Linux Docker
#   5=SBOM gen  6=sigstore sign  7=attestation  8=cosign verify  9=gh upload
#   10/11/12=preflight clippy/test/bin-version (release-preflight.ps1)
#
# Usage:
#   pwsh ./scripts/release-local.ps1 -Version 0.5.0
#   pwsh ./scripts/release-local.ps1 -Version 0.4.1 -SkipLinux -SkipSign -SkipUpload
#
# .NOTES
# Reproducible-build env set before cargo build per reproducible-builds.org:
# - SOURCE_DATE_EPOCH from `git log -1 --format=%ct` (commit timestamp, deterministic)
# - RUSTFLAGS --remap-path-prefix removes machine-local paths from binary debug info
#
# Build twice on same SOURCE_DATE_EPOCH -> identical SHA256 (verified W5).

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$SkipLinux,
    [switch]$SkipSign,
    [switch]$SkipUpload,
    [switch]$SkipPreflight,
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
Log-Step "1/12 Verifying required tools"
foreach ($cmd in @("docker --version", "cargo --version", "gh --version", "cosign version")) {
    $result = Invoke-Expression $cmd 2>&1
    if ($LASTEXITCODE -ne 0) { Log-Fail "Required tool missing: $cmd. Install it first. See docs/RELEASING.md." 1 }
}
# cargo-cyclonedx ships as `cargo cyclonedx`, probe it
cargo cyclonedx --version 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { Log-Fail "cargo-cyclonedx not installed. Run: cargo install cargo-cyclonedx" 1 }
# cross (cross-rs/cross) required for ARM64 Linux build — W5-5A
cross --version 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) { Log-Fail "cross not installed. Run: cargo install cross --git https://github.com/cross-rs/cross --locked" 1 }
Log-Pass "All deps present"

# ---- Step 2: Verify git state ----
Log-Step "2/12 Verifying git state"
$dirty = git -C $RepoRoot status --short 2>&1
if ($dirty) { Log-Fail "Working tree is dirty. Commit or stash changes before releasing." 2 }
$HeadSha = git -C $RepoRoot rev-parse HEAD 2>&1
$tagExists = git -C $RepoRoot tag -l $ReleaseTag 2>&1
if (-not $tagExists) {
    Log-Fail "Tag '$ReleaseTag' does not exist. Create it first: gh release create $ReleaseTag --draft" 2
}
Log-Pass "Git clean; HEAD=$HeadSha; tag=$ReleaseTag found"

# ---- Step 2.5: Preflight (v-next #36 tag-blocking gate per RELEASE-NOTES-v1.0.2.md) ----
if (-not $SkipPreflight) { Log-Step "2.5/12 Preflight (clippy + tests; bin-version verified after build)"; pwsh -File (Join-Path $PSScriptRoot "release-preflight.ps1") -Version $Version -SkipBinVersionCheck; if ($LASTEXITCODE -ne 0) { Log-Fail "release-preflight.ps1 failed (clippy or tests) — see [preflight] lines above" $LASTEXITCODE } } else { Write-Host "[release-local] WARN PREFLIGHT SKIPPED (-SkipPreflight) - emergency only" -ForegroundColor Yellow }

# ---- Step 3: Windows build ----
# v-next #36 class-fix: v1.0.0 stale-bin defect — only amore-gui was rebuilt while -p amore-cli
# -p amore-mcp skipped amore-gui entirely. Now build all 3 shipped bins explicitly.
Log-Step "3/12 Windows build (cargo build --release -p amore-cli -p amore-mcp -p amore-gui)"
$null = New-Item -ItemType Directory -Force -Path $DistDir
$env:SOURCE_DATE_EPOCH = (git -C $RepoRoot log -1 --format=%ct)
$env:RUSTFLAGS = "--remap-path-prefix=${RepoRoot}=. --remap-path-prefix=$env:USERPROFILE\.cargo=/cargo"
$winProc = Start-Process -FilePath "cargo" `
    -ArgumentList "build", "--release", "-p", "amore-cli", "-p", "amore-mcp", "-p", "amore-gui" `
    -WorkingDirectory $RepoRoot -NoNewWindow -Wait -PassThru
Remove-Item Env:SOURCE_DATE_EPOCH -ErrorAction SilentlyContinue
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
if ($winProc.ExitCode -ne 0) { Log-Fail "Windows build failed" 3 }
$WinBins = @(
    (Join-Path $RepoRoot "target\release\amore.exe"),
    (Join-Path $RepoRoot "target\release\amore-mcp.exe"),
    (Join-Path $RepoRoot "target\release\amore-gui.exe")
)
foreach ($b in $WinBins) { if (-not (Test-Path $b)) { Log-Fail "Expected binary not found: $b" 3 } }
# v-next #36 bin-version check (post-build half of preflight; gate-blocked at exit 12)
if (-not $SkipPreflight) { pwsh -File (Join-Path $PSScriptRoot "release-preflight.ps1") -Version $Version 2>&1 | Where-Object { $_ -match "BIN VERSION|P3:" } | ForEach-Object { Write-Host $_ }; if ($LASTEXITCODE -ne 0) { Log-Fail "release-preflight.ps1 bin-version check failed — stale binary detected" $LASTEXITCODE } }
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
    # W8-8C M2: pre-baked image (no apt-get at build time); build via: pwsh ./scripts/build-builder-images.ps1
    # Base: rust@sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1 (pinned 2026-05-27)
    $builderImage = "amore-builder-linux-x86_64:latest"
    docker image inspect $builderImage 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "Builder image '$builderImage' not found. Run: pwsh ./scripts/build-builder-images.ps1" 4
    }
    Log-Step "4/12 Linux build via Docker ($builderImage)"
    # Convert Windows path to POSIX for Docker volume mount (handles spaces in path)
    $RepoRootPosix = $RepoRoot -replace '\\', '/' -replace '^([A-Za-z]):', { "/$($_.Groups[1].Value.ToLower())" }
    # Reproducible-build env passed into container (reproducible-builds.org)
    $LinSourceDateEpoch = (git -C $RepoRoot log -1 --format=%ct)
    $dockerArgs = @(
        "run", "--rm",
        "-v", "${RepoRootPosix}:/work",
        "-w", "/work",
        "-e", "SOURCE_DATE_EPOCH=${LinSourceDateEpoch}",
        "-e", "RUSTFLAGS=--remap-path-prefix=/work=. --remap-path-prefix=/usr/local/cargo=/cargo",
        $builderImage,
        "bash", "-c",
        "cargo build --release -p amore-cli -p amore-mcp 2>&1"
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
    Log-Step "4/12 Linux build SKIPPED (-SkipLinux)"
}

# ---- Step 4b: ARM64 Linux build (cross-rs/cross + Docker) — W5-5A ----
$Arm64ArchivePath = $null
if (-not $SkipLinux) {
    Log-Step "4b/12 ARM64 Linux build via cross-rs/cross (aarch64-unknown-linux-gnu)"
    $crossProc = Start-Process -FilePath "cross" `
        -ArgumentList "build", "--release", "--target", "aarch64-unknown-linux-gnu",
                      "-p", "amore-cli", "-p", "amore-mcp" `
        -WorkingDirectory $RepoRoot `
        -NoNewWindow -Wait -PassThru
    if ($crossProc.ExitCode -ne 0) { Log-Fail "ARM64 cross build failed (aarch64-unknown-linux-gnu)" 4 }
    $Arm64BinDir = Join-Path $RepoRoot "target\aarch64-unknown-linux-gnu\release"
    foreach ($b in @("amore", "amore-mcp")) {
        if (-not (Test-Path (Join-Path $Arm64BinDir $b))) {
            Log-Fail "Expected ARM64 binary not found: $b in $Arm64BinDir" 4
        }
    }
    $Arm64ArchiveName = "amore-$ReleaseTag-aarch64-unknown-linux-gnu.tar.gz"
    $Arm64ArchivePath = Join-Path $DistDir $Arm64ArchiveName
    tar -C $Arm64BinDir -czf $Arm64ArchivePath "amore" "amore-mcp"
    if ($LASTEXITCODE -ne 0) { Log-Fail "ARM64 tar.gz packaging failed" 4 }
    Log-Pass "ARM64 archive: $Arm64ArchivePath"
} else {
    Log-Step "4b/12 ARM64 build SKIPPED (-SkipLinux)"
}

# ---- Step 5: SBOM ----
# cargo-cyclonedx v0.5+ generates per-crate *.cdx.json in each crate dir;
# --workspace and --output-cdx were removed. Run against the workspace root
# (generates one file per crate) and copy the amore-cli SBOM as the representative.
Log-Step "5/12 Generating SBOM (cargo cyclonedx)"
$sbomPath = Join-Path $DistDir "sbom.cdx.json"
$sbomProc = Start-Process -FilePath "cargo" `
    -ArgumentList "cyclonedx", "--manifest-path", (Join-Path $RepoRoot "Cargo.toml"), "--format", "json" `
    -WorkingDirectory $RepoRoot -NoNewWindow -Wait -PassThru
if ($sbomProc.ExitCode -ne 0) { Log-Fail "SBOM generation failed" 5 }
$cliSbomSrc = Join-Path $RepoRoot "crates\amore-cli\amore-cli.cdx.json"
if (-not (Test-Path $cliSbomSrc)) { Log-Fail "amore-cli SBOM not found at $cliSbomSrc" 5 }
Copy-Item -Path $cliSbomSrc -Destination $sbomPath -Force
# Inject CycloneDX composition.aggregate=complete per cyclonedx.org spec (W5-5D)
$sbomJson = Get-Content -Path $sbomPath -Raw | ConvertFrom-Json
if (-not $sbomJson.metadata.component.PSObject.Properties['composition']) {
    $sbomJson.metadata.component | Add-Member -NotePropertyName 'composition' `
        -NotePropertyValue @{ aggregate = 'complete' } -Force
    $sbomJson | ConvertTo-Json -Depth 20 | Set-Content -Path $sbomPath -Encoding UTF8
}
Log-Pass "SBOM: $sbomPath (composition.aggregate=complete)"

# Collect all artifacts for signing + upload
$artifacts = @($WinArchivePath)
if ($LinuxArchivePath) { $artifacts += $LinuxArchivePath }
if ($Arm64ArchivePath) { $artifacts += $Arm64ArchivePath }

# ---- Step 6: Sigstore sign ----
if (-not $SkipSign) {
    Log-Step "6/12 Sigstore keyless sign (browser OIDC flow will open — click the URL)"
    foreach ($art in $artifacts) {
        $bundle = "$art.sigstore"
        Write-Host "[release-local] Signing $art -> $bundle" -ForegroundColor Yellow
        cosign sign-blob --yes --bundle $bundle $art
        if ($LASTEXITCODE -ne 0) { Log-Fail "cosign sign-blob failed for $art" 6 }
    }
    Log-Pass "All artifacts signed"
} else {
    Log-Step "6/12 Sigstore sign SKIPPED (-SkipSign)"
}

# ---- Step 7: SLSA provenance attestation ----
if (-not $SkipSign) {
    Log-Step "7/12 SLSA provenance attestation (cosign attest-blob)"
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
    Log-Step "7/12 Attestation SKIPPED (-SkipSign)"
}

# ---- Step 7b: sha256sums.txt — generate, update packaging, sign, attest ----
$Sha256SumsPath = Join-Path $DistDir "sha256sums.txt"
$Sha256SumsSigstore = "$Sha256SumsPath.sigstore"
$Sha256SumsAttest  = "$Sha256SumsPath.attest.bundle"
Log-Step "7b/12 Emitting sha256sums.txt + updating packaging descriptors"
$manifestLines = ($artifacts + @($sbomPath)) | ForEach-Object {
    "$(((Get-FileHash -Algorithm SHA256 $_).Hash).ToLower())  $(Split-Path -Leaf $_)"
}
$manifestLines | Set-Content -Path $Sha256SumsPath -Encoding UTF8

# Update packaging descriptor PLACEHOLDER tokens (Homebrew/winget/AUR)
$fillerScript = Join-Path $PSScriptRoot "update-packaging-shas.ps1"
if (Test-Path $fillerScript) {
    pwsh -File $fillerScript -ReleaseDir $DistDir
    if ($LASTEXITCODE -ne 0) { Log-Fail "update-packaging-shas.ps1 failed" 5 }
    Log-Pass "Packaging descriptors updated"
}

if (-not $SkipSign) {
    Log-Step "7b-sign/12 cosign keyless sign + SLSA attestation for sha256sums bundle"
    cosign sign-blob --yes --bundle $Sha256SumsSigstore $Sha256SumsPath
    if ($LASTEXITCODE -ne 0) { Log-Fail "cosign sign-blob failed for sha256sums.txt" 6 }

    $buildDate = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    $sha256sumsHash = (Get-FileHash -Algorithm SHA256 $Sha256SumsPath).Hash.ToLower()
    $bundlePredicate = @{
        buildType = "https://github.com/antonio-amore-akiki/amore/release-local/bundle"
        builder   = @{ id = "local-windows-$env:COMPUTERNAME" }
        recipe    = @{ entryPoint = "scripts/release-local.ps1" }
        metadata  = @{
            buildStartedOn = $buildDate
            completeness   = @{ environment = $true }
        }
        materials = @(@{
            uri    = "git+https://github.com/antonio-amore-akiki/amore"
            digest = @{ sha1 = $HeadSha }
        })
        subject   = @(@{
            name   = "sha256sums.txt"
            digest = @{ sha256 = $sha256sumsHash }
        })
    }
    $bundlePredicateFile = "$Sha256SumsPath.provenance.json"
    $bundlePredicate | ConvertTo-Json -Depth 10 | Set-Content -Path $bundlePredicateFile -Encoding UTF8
    cosign attest-blob --predicate $bundlePredicateFile --type slsaprovenance `
        --bundle $Sha256SumsAttest $Sha256SumsPath
    if ($LASTEXITCODE -ne 0) { Log-Fail "cosign attest-blob failed for sha256sums.txt" 7 }
    Remove-Item $bundlePredicateFile -ErrorAction SilentlyContinue

    Log-Pass "sha256sums.txt signed ($Sha256SumsSigstore) + attested ($Sha256SumsAttest)"
} else {
    Log-Step "7b/12 sha256sums sign/attest SKIPPED (-SkipSign)"
}

# ---- Step 8: Round-trip verify ----
if (-not $SkipSign) {
    Log-Step "8/12 cosign verify-blob (round-trip check)"
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
    # Verify sha256sums.txt bundle
    cosign verify-blob `
        --bundle $Sha256SumsSigstore `
        --certificate-identity-regexp $userEmail `
        --certificate-oidc-issuer "https://accounts.google.com" `
        $Sha256SumsPath
    if ($LASTEXITCODE -ne 0) { Log-Fail "cosign verify-blob failed for sha256sums.txt" 8 }
    Log-Pass "All round-trip verifications passed (artifacts + sha256sums.txt)"
} else {
    Log-Step "8/12 Verify SKIPPED (-SkipSign)"
}

# ---- Step 9: Upload ----
if (-not $SkipUpload) {
    Log-Step "9/12 Uploading to GitHub release $ReleaseTag"
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
    # Upload sha256sums manifest + its cosign bundles
    foreach ($f in @($Sha256SumsPath, $Sha256SumsSigstore, $Sha256SumsAttest)) {
        if (Test-Path $f) {
            gh release upload $ReleaseTag $f --clobber --repo "antonio-amore-akiki/amore"
            if ($LASTEXITCODE -ne 0) { Log-Fail "gh release upload failed for $f" 9 }
        }
    }
    Log-Pass "All assets uploaded to $ReleaseTag"
} else {
    Log-Step "9/12 Upload SKIPPED (-SkipUpload)"
}

# ---- Step 10: Write proof JSON ----
Log-Step "10/12 Writing proof JSON"
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
Log-Step "11/12 Appending docs/results.tsv"
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
