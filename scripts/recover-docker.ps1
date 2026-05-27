# scripts/recover-docker.ps1 — Autonomous Docker Desktop crash recovery.
#
# Prior-art: Adapt from tests/qa/lib/ensure_daemons.ps1 (Docker-start pattern),
# extended with zombie-kill, stale-socket cleanup, EnableDockerAI assertion,
# service-only start path (no GUI). See ~/.claude/state/prior-art-verdict.json
# entry recover-docker.ps1 2026-05-27T20:09:22Z.
#
# Exit codes:
#   0  Docker daemon is ready (docker info succeeds)
#   1  Recovery failed; diagnostic printed to stderr

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$TOTAL_BUDGET_S = 120
$POLL_INTERVAL_S = 5
$StartTime = Get-Date

function Write-Step([string]$Msg) {
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] $Msg"
}

function Get-ElapsedSeconds {
    return [int]((Get-Date) - $StartTime).TotalSeconds
}

# ─── Step 1: Diagnose ─────────────────────────────────────────────────────────
Write-Step "PHASE A: Docker autonomous recovery"
Write-Step "Step 1 — Diagnosing current Docker state..."

$null = & docker version 2>&1
Write-Step "docker version exit=$LASTEXITCODE"

$dockerProcs = Get-Process | Where-Object { $_.Name -match "Docker Desktop|com\.docker|docker-index|qemu-system|vpnkit" }
if ($dockerProcs) {
    Write-Step "Found $($dockerProcs.Count) zombie Docker process(es): $($dockerProcs.Name -join ', ')"
} else {
    Write-Step "No zombie Docker processes found."
}

# ─── Step 2: Force-kill every Docker process ──────────────────────────────────
Write-Step "Step 2 — Force-killing all Docker processes..."
Get-Process | Where-Object {
    $_.Name -match "Docker Desktop|com\.docker\.backend|com\.docker\.cli|com\.docker\.dev-envs|com\.docker\.diagnose|com\.docker\.proxy|com\.docker\.service|com\.docker\.wsl-distro-proxy|docker-index|qemu-system|vpnkit"
} | ForEach-Object {
    try {
        Write-Step "  Killing PID $($_.Id) ($($_.Name))"
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    } catch {
        Write-Step "  (could not kill $($_.Name): $_)"
    }
}

# ─── Step 3: Stop Docker Desktop Service ──────────────────────────────────────
Write-Step "Step 3 — Stopping com.docker.service..."
try {
    Stop-Service com.docker.service -Force -ErrorAction SilentlyContinue
    Write-Step "  Service stop requested."
} catch {
    Write-Step "  Service stop error (may already be stopped): $_"
}

# ─── Step 4: Terminate WSL Docker distros ─────────────────────────────────────
Write-Step "Step 4 — Terminating WSL Docker distros (errors expected if not registered)..."
& wsl --terminate docker-desktop      2>$null
& wsl --terminate docker-desktop-data 2>$null
Write-Step "  WSL terminate done."

# ─── Step 5: Wait for handles to release ──────────────────────────────────────
Write-Step "Step 5 — Waiting 5s for handles to release..."
Start-Sleep -Seconds 5

# ─── Step 6: Remove stale socket/lock files ───────────────────────────────────
Write-Step "Step 6 — Removing stale socket files in $env:LOCALAPPDATA\Docker\run..."
$dockerRunDir = "$env:LOCALAPPDATA\Docker\run"
if (Test-Path $dockerRunDir) {
    Get-ChildItem $dockerRunDir | ForEach-Object {
        Write-Step "  Removing stale: $($_.Name)"
        try {
            Remove-Item $_.FullName -Force -ErrorAction SilentlyContinue
        } catch {
            Write-Step "  (could not remove $($_.Name): $_)"
        }
    }
} else {
    Write-Step "  $dockerRunDir does not exist — nothing to clean."
}

# ─── Step 7: Assert EnableDockerAI=false ──────────────────────────────────────
Write-Step "Step 7 — Verifying EnableDockerAI=false..."
$settingsPath = "$env:APPDATA\Docker\settings-store.json"
if (-not (Test-Path $settingsPath)) {
    Write-Error "FATAL: Docker settings not found at $settingsPath"
    exit 1
}
$settings = Get-Content $settingsPath | ConvertFrom-Json
if ($settings.EnableDockerAI -ne $false) {
    Write-Error "FATAL: EnableDockerAI is not false in $settingsPath (value=$($settings.EnableDockerAI)). Aborting to prevent Inference Manager crash."
    exit 1
}
Write-Step "  EnableDockerAI=false confirmed."

# ─── Step 8: Start Docker Desktop Service (daemon only — no GUI) ──────────────
Write-Step "Step 8 — Starting com.docker.service (daemon only, no GUI)..."
try {
    Start-Service com.docker.service -ErrorAction Stop
    Write-Step "  Service started."
} catch {
    Write-Step "  Start-Service failed: $_"
    Write-Step "  Fallback: launching Docker Desktop.exe hidden (daemon may require GUI process on this install)..."
    $desktopExe = "C:\Program Files\Docker\Docker\Docker Desktop.exe"
    if (-not (Test-Path $desktopExe)) {
        Write-Error "FATAL: Docker Desktop.exe not found at $desktopExe"
        exit 1
    }
    Start-Process -FilePath $desktopExe -WindowStyle Hidden -ErrorAction SilentlyContinue
    Write-Step "  Docker Desktop.exe launched in background (hidden)."
}

# ─── Step 9: Poll until daemon ready (initial budget) ─────────────────────────
Write-Step "Step 9 — Polling docker version every ${POLL_INTERVAL_S}s (budget: ${TOTAL_BUDGET_S}s)..."
$ready = $false
while (-not $ready) {
    $elapsed = Get-ElapsedSeconds
    if ($elapsed -gt $TOTAL_BUDGET_S) {
        Write-Step "  Initial budget exhausted after ${elapsed}s."
        break
    }
    $ver = & docker version --format "{{.Server.Version}}" 2>$null
    if ($LASTEXITCODE -eq 0 -and $ver -and $ver.Trim() -ne "") {
        Write-Step "  Docker server version: $($ver.Trim()) — daemon ready. (${elapsed}s)"
        $ready = $true
    } else {
        Write-Step "  Waiting... ${elapsed}s elapsed"
        Start-Sleep -Seconds $POLL_INTERVAL_S
    }
}

# ─── Step 10: Fallback — full GUI relaunch ────────────────────────────────────
if (-not $ready) {
    Write-Step "Step 10 — Service-only start didn't come up within ${TOTAL_BUDGET_S}s. Full Docker Desktop GUI relaunch..."
    $desktopExe = "C:\Program Files\Docker\Docker\Docker Desktop.exe"
    Get-Process | Where-Object { $_.Name -match "Docker Desktop" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 3
    Start-Process -FilePath $desktopExe -WindowStyle Hidden -ErrorAction SilentlyContinue
    Write-Step "  Docker Desktop relaunched. Polling for extended budget (240s total)..."
    while (-not $ready) {
        $elapsed = Get-ElapsedSeconds
        if ($elapsed -gt 240) {
            Write-Step "  Extended budget (240s) exhausted."
            break
        }
        $ver = & docker version --format "{{.Server.Version}}" 2>$null
        if ($LASTEXITCODE -eq 0 -and $ver -and $ver.Trim() -ne "") {
            Write-Step "  Docker server version: $($ver.Trim()) — daemon ready after full relaunch. (${elapsed}s)"
            $ready = $true
        } else {
            Write-Step "  Waiting... ${elapsed}s"
            Start-Sleep -Seconds $POLL_INTERVAL_S
        }
    }
}

# ─── Step 11: Final verification ──────────────────────────────────────────────
if (-not $ready) {
    Write-Error "FATAL: Docker daemon did not become ready within budget. Run 'docker info' manually; check Windows Event Log for com.docker.service."
    exit 1
}

Write-Step "Step 11 — Final: docker info..."
$infoOutput = & docker info 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Error "FATAL: docker info failed after daemon appeared ready. Output: $infoOutput"
    exit 1
}

$totalElapsed = Get-ElapsedSeconds
Write-Step "PHASE A COMPLETE: Docker daemon ready in ${totalElapsed}s."
Write-Host "TIME_TO_DOCKER_READY_S=$totalElapsed"
exit 0
