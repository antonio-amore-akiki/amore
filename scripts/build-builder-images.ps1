#!/usr/bin/env pwsh
# scripts/build-builder-images.ps1 — Build pre-baked local builder images for Amore releases.
#
# W8-8C M2: pre-bakes protobuf-compiler into a digest-pinned rust:1.95-bookworm image so
# release-local.ps1 step 4 never runs apt-get at build time (SLSA L3 hermeticity).
#
# Prior-art: Adapt from scripts/release-local.ps1 structural pattern (Log-*/ErrorAction/RepoRoot).
# See state/prior-art-verdict.json for full Adopt/Adapt/Build audit.
#
# Run once before each release cycle (or when Dockerfile.builder-linux-x86_64 changes).
# Images are LOCAL ONLY — not pushed to any registry.
#
# Usage:
#   pwsh ./scripts/build-builder-images.ps1
#
# Exit codes:
#   0 = image built and verified
#   1 = docker not available
#   2 = build failed

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$RepoRoot = Split-Path -Parent $PSScriptRoot
$ImageTag = "amore-builder-linux-x86_64:latest"
$DockerfileRelPath = "Dockerfile.builder-linux-x86_64"
$DockerfilePath = Join-Path $RepoRoot $DockerfileRelPath

function Log-Step($msg) { Write-Host "[build-builder-images] STEP $msg" -ForegroundColor Cyan }
function Log-Pass($msg) { Write-Host "[build-builder-images] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg, [int]$code) {
    Write-Host "[build-builder-images] FAIL $msg (exit $code)" -ForegroundColor Red
    exit $code
}

# ---- Step 1: Verify docker available ----
Log-Step "1/2 Checking docker availability"
$dockerCheck = docker version 2>&1
if ($LASTEXITCODE -ne 0) { Log-Fail "Docker not available or not running. Start Docker Desktop first." 1 }
Log-Pass "Docker available"

# ---- Step 2: Build the x86_64 builder image ----
Log-Step "2/2 Building $ImageTag from $DockerfileRelPath"
docker build `
    --file $DockerfilePath `
    --tag $ImageTag `
    $RepoRoot
if ($LASTEXITCODE -ne 0) { Log-Fail "docker build failed for $ImageTag" 2 }

# Local-only image has no RepoDigest; emit ImageID for traceability
$imageId = docker inspect --format='{{.Id}}' $ImageTag 2>&1
Log-Pass "Built $ImageTag (ImageID: $imageId)"

Write-Host ""
Write-Host "[build-builder-images] DONE — builder image ready. Run release-local.ps1 next." -ForegroundColor Green
exit 0
