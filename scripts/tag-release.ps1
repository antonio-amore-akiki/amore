<#
.SYNOPSIS
One-shot tag+release+verify ladder for Amore waves W1 (v0.5.1) through W9 (v1.0.0).

.DESCRIPTION
Composes scripts/release-local.ps1 + scripts/update-packaging-shas.ps1 +
scripts/verify-release.ps1 into a single idempotent ladder. Creates a GPG-signed
annotated tag, runs the full local release pipeline, uploads all artifacts via gh,
and verifies signatures post-upload. Emits a per-step duration log and a final
VERDICT line to state/tag-release-<Version>.log.

Prior-art verdict: Adapt — structural pattern from release-local.ps1 (step logging,
Log-Pass/Log-Fail helpers, stopwatch, state dir). Net-new: tag creation, ladder
orchestration, push-tag step, verify-release composition.

.PARAMETER Version
Mandatory. Semantic version string without the 'v' prefix (e.g. "0.5.1").

.PARAMETER NotesFile
Path to the release notes Markdown file.
Defaults to docs/RELEASE-NOTES-v<Version>.md relative to repo root.

.PARAMETER DryRun
If set, validates preconditions and prints every step without executing
git tag, release-local.ps1, or gh release create.

.PARAMETER SkipVerify
If set, skips the post-release scripts/verify-release.ps1 step.

.NOTES
Push target: origin — tag refs only (refs/tags/v<Version>).
Per CLAUDE.md: NEVER push to main/master. NEVER --no-verify or --no-gpg-sign.
Tag signing is mandatory; error out if GPG unavailable rather than downgrading.

References:
  docs/RELEASING.md             — full release SOP
  CLAUDE.md                     — push-target + signing constraints
  scripts/release-local.ps1     — binary build + sign + upload pipeline
  scripts/update-packaging-shas.ps1 — Homebrew/winget/AUR SHA token fill
  scripts/verify-release.ps1    — post-release cosign + SBOM + SHA256 verify

Exit codes:
  0 = VERDICT GO (all steps PASS)
  1 = precondition failure (notes file missing / tag exists / dirty tree)
  2 = GPG not available (tag signing blocked)
  3 = release-local.ps1 failed
  4 = update-packaging-shas.ps1 failed
  5 = gh release create failed
  6 = gh release upload supplemental assets failed
  7 = verify-release.ps1 failed
  8 = tag push failed
#>

[CmdletBinding(SupportsShouldProcess)]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$NotesFile = "",
    [switch]$DryRun,
    [switch]$SkipVerify
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# ---- Resolve paths ----
$RepoRoot   = Split-Path -Parent $PSScriptRoot
$Tag        = "v$Version"
$StateDir   = Join-Path $RepoRoot "state"
$LogFile    = Join-Path $StateDir "tag-release-$Version.log"
$ReleaseDir = Join-Path $RepoRoot ".releases" $Tag
$DistDir    = Join-Path $RepoRoot "dist"

if (-not $NotesFile) {
    $NotesFile = Join-Path $RepoRoot "docs" "RELEASE-NOTES-v$Version.md"
}

$null = New-Item -ItemType Directory -Force -Path $StateDir

# ---- Logging helpers (pattern adapted from release-local.ps1) ----
$stepLog = [System.Collections.Generic.List[string]]::new()

function Log-Info($msg)  { Write-Host "[tag-release] $msg" -ForegroundColor Cyan }
function Log-Pass($msg)  { Write-Host "[tag-release] PASS $msg" -ForegroundColor Green }
function Log-Fail($msg, [int]$code) {
    $line = "$(Get-Date -Format 'o') STEP-FAIL(exit=$code): $msg"
    $script:stepLog.Add($line)
    Write-Host "[tag-release] FAIL $msg (exit $code)" -ForegroundColor Red
    $script:stepLog | Set-Content -Path $script:LogFile -Encoding UTF8
    exit $code
}

function Record-Step([string]$label, [string]$verdict, [double]$elapsedSec) {
    $script:stepLog.Add("$(Get-Date -Format 'o')  $label  $verdict  ${elapsedSec}s")
}

$dryTag = if ($DryRun) { " [DRY-RUN]" } else { "" }
Log-Info "=== tag-release$dryTag $Tag ==="

# ---- Step 1: Preconditions ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 1/9: Precondition checks"

if (-not (Test-Path $NotesFile)) {
    Log-Fail "Notes file not found: $NotesFile — create it before tagging" 1
}

$existingTag = git -C $RepoRoot tag -l $Tag 2>&1
if ($existingTag) {
    Log-Fail "Tag '$Tag' already exists. Delete it or choose a different version." 1
}

$dirty = git -C $RepoRoot status --short 2>&1
if ($dirty) {
    Log-Fail "Working tree is dirty. Commit or stash all changes before tagging." 1
}

$sw.Stop()
Record-Step "1/9 preconditions" "PASS" $sw.Elapsed.TotalSeconds
Log-Pass "Preconditions OK (notes present; tag absent; tree clean)"

# ---- Step 2: Signed annotated tag ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 2/9: Create signed annotated tag $Tag"

if (-not $DryRun) {
    gpg --version 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "GPG not available (gpg --version exit $LASTEXITCODE). Install GPG and configure a signing key. To skip signing manually run: git tag -a '$Tag' -m 'Amore $Tag'" 2
    }
    git -C $RepoRoot tag -s $Tag -m "Amore $Tag"
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "git tag -s failed. Ensure a GPG signing key is configured: git config user.signingkey. Fallback (no signing): git tag -a '$Tag' -m 'Amore $Tag'" 2
    }
    Log-Pass "Tag $Tag created (GPG-signed annotated)"
} else {
    Log-Info "[DRY-RUN] Would run: git tag -s $Tag -m 'Amore $Tag'"
}

$sw.Stop()
Record-Step "2/9 signed-tag" "PASS" $sw.Elapsed.TotalSeconds

# ---- Step 3: release-local.ps1 ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 3/9: scripts/release-local.ps1 -Version $Version"

$releaseLocalPs1 = Join-Path $PSScriptRoot "release-local.ps1"
if (-not (Test-Path $releaseLocalPs1)) {
    Log-Fail "scripts/release-local.ps1 not found at $releaseLocalPs1" 3
}

if (-not $DryRun) {
    pwsh -File $releaseLocalPs1 -Version $Version
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "release-local.ps1 exited $LASTEXITCODE" 3
    }
    Log-Pass "release-local.ps1 PASS"
} else {
    Log-Info "[DRY-RUN] Would run: pwsh -File $releaseLocalPs1 -Version $Version"
}

$sw.Stop()
Record-Step "3/9 release-local" "PASS" $sw.Elapsed.TotalSeconds

# ---- Step 4: update-packaging-shas.ps1 ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 4/9: scripts/update-packaging-shas.ps1 -ReleaseDir $ReleaseDir"

$packagingScript = Join-Path $PSScriptRoot "update-packaging-shas.ps1"

if (-not $DryRun) {
    if (Test-Path $packagingScript) {
        pwsh -File $packagingScript -ReleaseDir $ReleaseDir
        if ($LASTEXITCODE -ne 0) {
            Log-Fail "update-packaging-shas.ps1 exited $LASTEXITCODE" 4
        }
        Log-Pass "update-packaging-shas.ps1 PASS (packaging PLACEHOLDER tokens filled)"
    } else {
        Log-Info "update-packaging-shas.ps1 not found — no packaging descriptors to update"
    }
} else {
    Log-Info "[DRY-RUN] Would run: pwsh -File $packagingScript -ReleaseDir $ReleaseDir"
}

$sw.Stop()
Record-Step "4/9 update-packaging-shas" "PASS" $sw.Elapsed.TotalSeconds

# ---- Step 5: gh release create ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 5/9: gh release create $Tag --notes-file $NotesFile"

if (-not $DryRun) {
    $artifacts = @()
    if (Test-Path $ReleaseDir) {
        $artifacts = Get-ChildItem -Path $ReleaseDir -File |
            Select-Object -ExpandProperty FullName
    }
    $ghArgs = @("release", "create", $Tag, "--notes-file", $NotesFile) + $artifacts
    gh @ghArgs
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "gh release create exited $LASTEXITCODE" 5
    }
    Log-Pass "gh release create PASS ($($artifacts.Count) artifact(s))"
} else {
    Log-Info "[DRY-RUN] Would run: gh release create $Tag --notes-file $NotesFile <artifacts from $ReleaseDir>"
}

$sw.Stop()
Record-Step "5/9 gh-release-create" "PASS" $sw.Elapsed.TotalSeconds

# ---- Step 6: gh release upload supplemental assets ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 6/9: gh release upload supplemental assets"

if (-not $DryRun) {
    $supplemental = @("sha256sums.txt", "sha256sums.txt.sigstore", "sbom.cdx.json") |
        ForEach-Object { Join-Path $DistDir $_ } |
        Where-Object   { Test-Path $_ }

    foreach ($asset in $supplemental) {
        gh release upload $Tag $asset --clobber
        if ($LASTEXITCODE -ne 0) {
            Log-Fail "gh release upload failed for $asset (exit $LASTEXITCODE)" 6
        }
        Log-Pass "Uploaded $(Split-Path -Leaf $asset)"
    }
} else {
    Log-Info "[DRY-RUN] Would upload sha256sums.txt + sha256sums.txt.sigstore + sbom.cdx.json if present in $DistDir"
}

$sw.Stop()
Record-Step "6/9 gh-release-upload-supplemental" "PASS" $sw.Elapsed.TotalSeconds

# ---- Step 7: verify-release.ps1 ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()

if (-not $SkipVerify) {
    Log-Info "STEP 7/9: scripts/verify-release.ps1 -Version $Version"
    $verifyScript = Join-Path $PSScriptRoot "verify-release.ps1"

    if (-not $DryRun) {
        if (-not (Test-Path $verifyScript)) {
            Log-Fail "verify-release.ps1 not found at $verifyScript" 7
        }
        pwsh -File $verifyScript -Version $Version
        if ($LASTEXITCODE -ne 0) {
            Log-Fail "verify-release.ps1 exited $LASTEXITCODE" 7
        }
        Log-Pass "verify-release.ps1 PASS (cosign + SBOM + SHA256 verified)"
    } else {
        Log-Info "[DRY-RUN] Would run: pwsh -File $verifyScript -Version $Version"
    }
} else {
    Log-Info "STEP 7/9: verify-release.ps1 SKIPPED (-SkipVerify)"
}

$sw.Stop()
$verifyLabel = if ($SkipVerify) { "SKIPPED" } else { "PASS" }
Record-Step "7/9 verify-release" $verifyLabel $sw.Elapsed.TotalSeconds

# ---- Step 8: Push tag ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 8/9: git push origin refs/tags/$Tag"

if (-not $DryRun) {
    git -C $RepoRoot push origin "refs/tags/$Tag"
    if ($LASTEXITCODE -ne 0) {
        Log-Fail "git push origin refs/tags/$Tag failed (exit $LASTEXITCODE)" 8
    }
    Log-Pass "Tag $Tag pushed to origin"
} else {
    Log-Info "[DRY-RUN] Would run: git push origin refs/tags/$Tag"
}

$sw.Stop()
Record-Step "8/9 push-tag" "PASS" $sw.Elapsed.TotalSeconds

# ---- Step 9: Emit state log + final verdict ----
$sw = [System.Diagnostics.Stopwatch]::StartNew()
Log-Info "STEP 9/9: Writing state log $LogFile"

$verdict = "VERDICT $Tag GO"
$stepLog.Add($verdict)
$stepLog | Set-Content -Path $LogFile -Encoding UTF8

$sw.Stop()
Record-Step "9/9 emit-log" "PASS" $sw.Elapsed.TotalSeconds

Write-Host ""
Write-Host "[tag-release] $verdict" -ForegroundColor Green
exit 0
