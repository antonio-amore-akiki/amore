#!/usr/bin/env bash
# build-installer-linux.sh - Linux installer triple via Docker
# Prior-art: Adapt from scripts/build-builder-images.ps1; cargo-deb/generate-rpm/appimage Adopt.
# See docs/prior-art-w8.5.md and state/prior-art-verdict.json.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SKIP_SIGN=false
SKIP_SMOKE=false
for arg in "$@"; do
    case "$arg" in
        --skip-sign)  SKIP_SIGN=true ;;
        --skip-smoke) SKIP_SMOKE=true ;;
    esac
done

if ! docker ps > /dev/null 2>&1; then
    echo "ERROR: Docker daemon is not reachable. Start Docker Desktop and retry." >&2
    exit 1
fi

if [[ ! -f "packaging/installer/linux/amore-icon.png" ]]; then
    echo "WARN: packaging/installer/linux/amore-icon.png not found." >&2
    echo "      AppImage build will fail. See packaging/installer/linux/ICON-REQUIRED.md" >&2
fi

echo "==> Building amore-linux-builder image..."
docker build -t amore-linux-builder -f Dockerfile.builder-linux-x86_64 . 2>&1 | tee state/w8.5c-docker-build.log
echo "==> Builder image ready."

echo "==> Building AppImage..."
# cargo-appimage v2.4.0 has two issues in this workspace:
#   1. It adds --release internally; user must not pass it again.
#   2. complete_from_path_and_workspace fails for workspace-inherited manifests.
# Fix: build the binary with cargo, then assemble AppDir + run appimagetool manually.
# APPIMAGE_EXTRACT_AND_RUN=1 bypasses the FUSE requirement for appimagetool inside Docker.
MSYS_NO_PATHCONV=1 docker run --rm -v "${PWD}:/work" amore-linux-builder \
    bash -c "
set -euo pipefail
cd /work
# Step 1: build binary in release mode
cargo build --release -p amore-gui

# Step 2: assemble AppDir
APPDIR=/work/target/amore-gui.AppDir
mkdir -p \${APPDIR}/usr/bin
cp /work/target/release/amore-gui \${APPDIR}/usr/bin/amore-gui
chmod +x \${APPDIR}/usr/bin/amore-gui
cp /work/packaging/installer/linux/amore-icon.png \${APPDIR}/amore-gui.png
cat > \${APPDIR}/cargo-appimage.desktop <<'DESKTOP'
[Desktop Entry]
Name=Amore
Exec=amore-gui
Icon=amore-gui
Type=Application
Categories=Utility;
DESKTOP
cp /usr/local/cargo/bin/cargo-appimage-runner \${APPDIR}/AppRun
chmod +x \${APPDIR}/AppRun

# Step 3: package with appimagetool (APPIMAGE_EXTRACT_AND_RUN avoids FUSE inside Docker)
mkdir -p /work/target/appimage
ARCH=x86_64 VERSION=0.5.0 APPIMAGE_EXTRACT_AND_RUN=1 appimagetool \${APPDIR} /work/target/appimage/amore-gui-x86_64.AppImage
" 2>&1 | tee state/w8.5c-appimage.log
APPIMAGE_PATH="$(find target/appimage -name "*.AppImage" | head -1)"
[[ -z "$APPIMAGE_PATH" ]] && { echo "ERROR: AppImage not found" >&2; exit 1; }
chmod +x "$APPIMAGE_PATH"
APPIMAGE_SIZE=$(du -h "$APPIMAGE_PATH" | cut -f1)
APPIMAGE_SHA=$(sha256sum "$APPIMAGE_PATH" | awk '{print $1}')
echo "AppImage: $APPIMAGE_PATH  size=$APPIMAGE_SIZE  sha256=$APPIMAGE_SHA"

echo "==> Building .deb..."
# cargo-deb uses --profile, not --release.
MSYS_NO_PATHCONV=1 docker run --rm -v "${PWD}:/work" amore-linux-builder \
    bash -c "cd /work && cargo deb --profile release -p amore-cli" 2>&1 | tee state/w8.5c-deb.log
DEB_PATH="$(find target/debian -name "*.deb" | head -1)"
[[ -z "$DEB_PATH" ]] && { echo "ERROR: .deb not found" >&2; exit 1; }
DEB_SIZE=$(du -h "$DEB_PATH" | cut -f1)
DEB_SHA=$(sha256sum "$DEB_PATH" | awk '{print $1}')
echo ".deb:     $DEB_PATH  size=$DEB_SIZE  sha256=$DEB_SHA"

echo "==> Building .rpm..."
# cargo-generate-rpm v0.21.0: must run from the crate dir; -p flag expects a path not a name.
# Output goes to crate-local target/generate-rpm; we copy to workspace target/generate-rpm/.
MSYS_NO_PATHCONV=1 docker run --rm -v "${PWD}:/work" amore-linux-builder \
    bash -c "
mkdir -p /work/target/generate-rpm
cd /work/crates/amore-cli && cargo generate-rpm
# Copy RPM to workspace target for consistent output path
find /work/crates/amore-cli/target/generate-rpm -name '*.rpm' -exec cp {} /work/target/generate-rpm/ \;
" 2>&1 | tee state/w8.5c-rpm.log
RPM_PATH="$(find target/generate-rpm -name "*.rpm" | head -1)"
[[ -z "$RPM_PATH" ]] && { echo "ERROR: .rpm not found" >&2; exit 1; }
RPM_SIZE=$(du -h "$RPM_PATH" | cut -f1)
RPM_SHA=$(sha256sum "$RPM_PATH" | awk '{print $1}')
echo ".rpm:     $RPM_PATH  size=$RPM_SIZE  sha256=$RPM_SHA"

if [[ "$SKIP_SIGN" == "false" ]]; then
    if ! command -v cosign > /dev/null 2>&1; then
        echo "WARN: cosign not found. Install: winget install sigstore.cosign" >&2
    else
        echo "==> Signing artifacts with cosign..."
        for artifact in "$APPIMAGE_PATH" "$DEB_PATH" "$RPM_PATH"; do
            cosign sign-blob --bundle "${artifact}.sigstore" "$artifact"
            cosign verify-blob --bundle "${artifact}.sigstore" "$artifact"
            echo "  Sigstore verified: $artifact"
        done
    fi
else
    echo "==> Sigstore signing skipped (--skip-sign)."
fi

if [[ "$SKIP_SMOKE" == "false" ]]; then
    echo "==> Smoke test: .deb on Ubuntu 22.04..."
    MSYS_NO_PATHCONV=1 docker run --rm -v "${PWD}/target/debian:/tmp/deb" ubuntu:22.04 \
        bash -c "apt-get update -qq && apt-get install -y /tmp/deb/amore_*.deb && amore --version" \
        2>&1 | tee state/w8.5c-smoke.log
    echo "==> Smoke test: .rpm on Fedora 39..."
    MSYS_NO_PATHCONV=1 docker run --rm -v "${PWD}/target/generate-rpm:/tmp/rpm" fedora:39 \
        bash -c "dnf install -y /tmp/rpm/amore-*.rpm && amore --version" \
        2>&1 | tee -a state/w8.5c-smoke.log
else
    echo "==> Smoke tests skipped (--skip-smoke)."
fi

echo ""
echo "=== Linux installer triple complete ==="
echo "AppImage : $APPIMAGE_PATH  ($APPIMAGE_SIZE)  sha256=$APPIMAGE_SHA"
echo ".deb     : $DEB_PATH  ($DEB_SIZE)  sha256=$DEB_SHA"
echo ".rpm     : $RPM_PATH  ($RPM_SIZE)  sha256=$RPM_SHA"
echo ""
echo "Next: gh release upload <tag> \$APPIMAGE_PATH \$DEB_PATH \$RPM_PATH"
