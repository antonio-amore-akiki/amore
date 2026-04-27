<#
.SYNOPSIS
Verify an Amore release's signature, SBOM, and SHA256 integrity.

.PARAMETER Version
The release tag to verify (e.g., "0.5.0").

.NOTES
Sources:
  - cyclonedx.org/specification
  - slsa.dev/spec/v1.0/requirements
  - sigstore.dev/docs/signing/quickstart
  - scripts/release-local.ps1 (structural pattern, adapted)
#>
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$DownloadDir = "$env:LOCALAPPDATA/Amore/release-verify/v$Version"
)

$ErrorActionPreference = "Stop"

# Dependency check
foreach ($cmd in @("gh", "cosign", "cyclonedx-cli")) {
    if (!(Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "$cmd not installed. Install before running verify-release."
        exit 1
    }
}

# Download release assets
New-Item -ItemType Directory -Force -Path $DownloadDir | Out-Null
Push-Location $DownloadDir
gh release download "v$Version" --repo antonio-amore-akiki/amore --clobber
Pop-Location

$assets  = Get-ChildItem $DownloadDir -File
$bundles = $assets | Where-Object { $_.Name -like "*.sigstore" }
$failures = @()

# Verify each .sigstore bundle against its artifact
foreach ($bundle in $bundles) {
    $artifact     = $bundle.Name -replace "\.sigstore$", ""
    $artifactPath = Join-Path $DownloadDir $artifact
    if (!(Test-Path $artifactPath)) {
        Write-Warning "Artifact $artifact not found for bundle $($bundle.Name)"
        continue
    }
    Write-Host "Verifying $artifact ..."
    # W8-8C M1: cert-identity-regexp must match signing OIDC SAN (antonioakiki15@gmail.com);
    # prior value "antonio-amore-akiki" was a mismatch that caused every consumer verify to fail.
    cosign verify-blob `
        --bundle $bundle.FullName `
        --certificate-identity-regexp "antonioakiki15@gmail.com" `
        --certificate-oidc-issuer "https://accounts.google.com" `
        $artifactPath 2>&1
    if ($LASTEXITCODE -ne 0) { $failures += $artifact }
}

# Validate SBOM
$sbom = $assets | Where-Object { $_.Name -eq "sbom.cdx.json" } | Select-Object -First 1
if ($sbom) {
    Write-Host "Validating SBOM ..."
    cyclonedx-cli validate --input-file $sbom.FullName
    if ($LASTEXITCODE -ne 0) { $failures += "sbom.cdx.json" }
}

# Verify SHA256 sums
$sumsFile = $assets | Where-Object { $_.Name -eq "sha256sums.txt" } | Select-Object -First 1
if ($sumsFile) {
    Write-Host "Verifying SHA256 sums ..."
    Get-Content $sumsFile.FullName | ForEach-Object {
        if ($_ -match '^([0-9a-f]{64})\s+(.+)$') {
            $expected    = $matches[1]
            $file        = $matches[2]
            $actualPath  = Join-Path $DownloadDir $file
            if (Test-Path $actualPath) {
                $actual = (Get-FileHash $actualPath -Algorithm SHA256).Hash.ToLower()
                if ($actual -ne $expected) { $failures += "sha256-mismatch:$file" }
            }
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Error "Verification failures: $($failures -join ', ')"
    exit 2
}
Write-Host "PASS: all signatures + SBOM + SHA256s verified for v$Version"
exit 0
