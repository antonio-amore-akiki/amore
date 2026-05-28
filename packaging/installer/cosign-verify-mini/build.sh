#!/usr/bin/env bash
# packaging/installer/cosign-verify-mini/build.sh
#
# Prior-art: Adopt — cosign upstream OSS (github.com/sigstore/cosign, Apache-2.0).
# cosign v2.x ships static-linked binaries per release:
#   cosign-linux-amd64, cosign-windows-amd64.exe
# No custom build is needed. This script fetches + SHA256-verifies the upstream binary.
# "cosign-verify-mini" is a naming convention; the actual binary is the full cosign static
# binary (~20 MB; plan's ~3 MB estimate was for a stripped subset). Already used in
# scripts/build-installer-windows.ps1 for sign-blob/verify-blob.
#
# Purpose (B4, F21): bundled into both lite and fat Amore installers so that
# InitializeSetup (Windows/Inno) and preinst (Linux/.deb) can verify the
# release SHA256 + Sigstore signature BEFORE extracting payload.
# Fail-loud on mismatch — installer aborts with clear error pointing to release-page checksums.
#
# Usage (CI / first-time setup):
#   bash packaging/installer/cosign-verify-mini/build.sh
#   # Produces:
#   #   packaging/installer/cosign-verify-mini/cosign-verify-mini-linux-amd64
#   #   packaging/installer/cosign-verify-mini/cosign-verify-mini-windows-amd64.exe
#
# After running:
#   1. Pin real SHA256 values from cosign_checksums.txt into this script.
#   2. Copy to staging dirs before installer build:
#      cosign-verify-mini-windows-amd64.exe -> packaging/installer/windows/staging/cosign-verify-mini.exe
#      cosign-verify-mini-linux-amd64 -> packaging/installer/linux/staging/cosign-verify-mini
#
# SWAP NOTE: orchestrator should run this script once, pin real SHAs, and commit
# resulting binaries or reference them from a CI release asset download step.

set -euo pipefail

COSIGN_VERSION="${COSIGN_VERSION:-v2.4.1}"
# SHA256 placeholders — pin real values from:
#   https://github.com/sigstore/cosign/releases/download/${COSIGN_VERSION}/cosign_checksums.txt
COSIGN_LINUX_AMD64_SHA256="FETCH_AND_PIN"
COSIGN_WINDOWS_AMD64_SHA256="FETCH_AND_PIN"

OUTDIR="$(cd "$(dirname "$0")" && pwd)"

fetch_and_verify() {
    local url="$1" dest="$2" expected_sha="$3" label="$4"
    echo "[cosign-verify-mini] Fetching $label ..."
    curl -sSL --retry 3 -o "$dest" "$url"
    local actual
    actual="$(sha256sum "$dest" | awk '{print $1}')"
    echo "[cosign-verify-mini] $label SHA256: $actual"
    if [ "$expected_sha" != "FETCH_AND_PIN" ]; then
        if [ "$actual" != "$expected_sha" ]; then
            echo "ERROR: SHA256 mismatch for $label. expected=$expected_sha actual=$actual" >&2
            rm -f "$dest"
            exit 1
        fi
        echo "[cosign-verify-mini] SHA256 verified OK"
    else
        echo "WARN: SHA not pinned yet — pin '$actual' in this script before production use"
    fi
    chmod +x "$dest" 2>/dev/null || true
}

mkdir -p "$OUTDIR"

# Linux amd64
fetch_and_verify \
    "https://github.com/sigstore/cosign/releases/download/${COSIGN_VERSION}/cosign-linux-amd64" \
    "$OUTDIR/cosign-verify-mini-linux-amd64" \
    "$COSIGN_LINUX_AMD64_SHA256" \
    "cosign-linux-amd64 ${COSIGN_VERSION}"

# Windows amd64
fetch_and_verify \
    "https://github.com/sigstore/cosign/releases/download/${COSIGN_VERSION}/cosign-windows-amd64.exe" \
    "$OUTDIR/cosign-verify-mini-windows-amd64.exe" \
    "$COSIGN_WINDOWS_AMD64_SHA256" \
    "cosign-windows-amd64.exe ${COSIGN_VERSION}"

echo ""
echo "[cosign-verify-mini] Done."
echo "Next steps:"
echo "  1. Pin SHA256 from cosign_checksums.txt and re-run to verify"
echo "  2. cp $OUTDIR/cosign-verify-mini-windows-amd64.exe packaging/installer/windows/staging/cosign-verify-mini.exe"
echo "  3. cp $OUTDIR/cosign-verify-mini-linux-amd64 packaging/installer/linux/staging/cosign-verify-mini"
