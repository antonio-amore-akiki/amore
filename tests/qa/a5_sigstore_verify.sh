#!/usr/bin/env bash
# tests/qa/a5_sigstore_verify.sh
#
# QA A5 — Sigstore + cosign verify-blob on a CLEAN debian:12 container.
#
# Proves the v0.2.1 Linux artifact's Sigstore bundle is verifiable end-to-end
# on a host that never built Amore and has nothing pre-installed besides
# what apt + curl bring in this turn.
#
# Auth note: the amore repo is private during MVP, so the host pre-downloads
# the archive + bundle with the authenticated `gh` CLI and bind-mounts them
# into /qa/in/ inside the container. Cosign verification of the blob ↔ bundle
# pair is identity-bound (OIDC SAN regex + issuer) + Rekor-transparency-log
# anchored — NOT source-bound — so the clean-container claim is preserved
# even though the bytes arrived via gh on the host.
#
# Caller (host, Windows or POSIX):
#   gh release download v0.2.1 -p '*linux-gnu.tar.gz*' -D /tmp/a5-in/
#   docker run --rm -i \
#     -v /tmp/a5-in:/qa/in:ro \
#     debian:12 bash -s < tests/qa/a5_sigstore_verify.sh
#
# Exit 0 -> archive is provably signed by the Amore release workflow at
# refs/tags/v0.2.1 under GitHub's OIDC issuer. Sigstore claim defended on
# real footprint.
# Exit non-zero -> verification failed; caller must surface (no silent skip).

set -euo pipefail

TAG="${AMORE_VERIFY_TAG:-v0.2.1}"
TARGET="${AMORE_VERIFY_TARGET:-x86_64-unknown-linux-gnu}"
COSIGN_VERSION="${COSIGN_VERSION:-v2.4.1}"
ARCHIVE="amore-${TAG}-${TARGET}.tar.gz"
BUNDLE="${ARCHIVE}.bundle"
IN_DIR="/qa/in"

echo "[a5] verifying ${ARCHIVE} signed via Sigstore (cosign ${COSIGN_VERSION})"

# --- sanity: blob + bundle must be present from host bind-mount
if [ ! -f "${IN_DIR}/${ARCHIVE}" ] || [ ! -f "${IN_DIR}/${BUNDLE}" ]; then
  echo "[a5] FATAL: ${IN_DIR}/${ARCHIVE} or ${IN_DIR}/${BUNDLE} missing." >&2
  echo "[a5]   Host must run: gh release download ${TAG} -p '*${TARGET}*' -D <host_dir>" >&2
  echo "[a5]   Then: docker run --rm -i -v <host_dir>:${IN_DIR}:ro ..." >&2
  exit 2
fi

# --- minimal install on the empty debian:12 base
apt-get update -qq >/dev/null
apt-get install -y -qq --no-install-recommends curl ca-certificates >/dev/null
echo "[a5] installed: curl + ca-certificates"

# --- fetch cosign upstream (no apt package on debian:12)
curl -sSL -o /usr/local/bin/cosign \
  "https://github.com/sigstore/cosign/releases/download/${COSIGN_VERSION}/cosign-linux-amd64"
chmod +x /usr/local/bin/cosign
echo "[a5] cosign installed: $(cosign version 2>&1 | grep -E 'GitVersion' | head -1)"

# --- copy into writable workdir + record sha
mkdir -p /qa/work
cp "${IN_DIR}/${ARCHIVE}" /qa/work/
cp "${IN_DIR}/${BUNDLE}" /qa/work/
cd /qa/work
ARCHIVE_SIZE=$(stat -c %s "${ARCHIVE}")
BUNDLE_SIZE=$(stat -c %s "${BUNDLE}")
ARCHIVE_SHA=$(sha256sum "${ARCHIVE}" | cut -d' ' -f1)
echo "[a5] archive: ${ARCHIVE_SIZE} bytes, sha256=${ARCHIVE_SHA}"
echo "[a5] bundle:  ${BUNDLE_SIZE} bytes"

# --- the actual verification
# The signing identity is the GitHub Actions workflow that fired release.yml
# for this exact tag. cosign verify-blob requires the identity to match the
# certificate's Subject Alternative Name from Fulcio (the Sigstore CA).
EXPECTED_IDENTITY_REGEX="^https://github\.com/antonio-amore-akiki/amore/\.github/workflows/release\.yml@refs/tags/${TAG}$"
EXPECTED_ISSUER="https://token.actions.githubusercontent.com"

echo "[a5] running: cosign verify-blob --bundle ${BUNDLE} ${ARCHIVE}"
echo "[a5]   --certificate-identity-regexp ${EXPECTED_IDENTITY_REGEX}"
echo "[a5]   --certificate-oidc-issuer ${EXPECTED_ISSUER}"

cosign verify-blob \
  --bundle "${BUNDLE}" \
  --certificate-identity-regexp "${EXPECTED_IDENTITY_REGEX}" \
  --certificate-oidc-issuer "${EXPECTED_ISSUER}" \
  "${ARCHIVE}"

echo "[a5] PASS — ${ARCHIVE} provably signed by the release workflow at ${TAG}"
echo "[a5]   sha256=${ARCHIVE_SHA}"
