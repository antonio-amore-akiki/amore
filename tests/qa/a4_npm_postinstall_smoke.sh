#!/usr/bin/env bash
# tests/qa/a4_npm_postinstall_smoke.sh
#
# QA A4 (Linux lane) — npm install runs postinstall, downloads
# obelion-v<VER>-x86_64-unknown-linux-gnu.tar.gz from the GitHub Release,
# extracts to bin/obelion + bin/obelion-mcp, and `--version` reports the
# matching tag. Proves the npm distribution path against the LIVE release.
#
# Caller (host):
#   1. Pre-pack on host: (cd npm && npm pack) -> npm/anto-obelion-<VER>.tgz
#   2. cp npm/anto-obelion-<VER>.tgz /tmp/a4-in/
#   3. docker run --rm -i -e GH_TOKEN="$(gh auth token)" \
#        -v "C:\Users\anto\AppData\Local\Temp\a4-in:/qa/in:ro" \
#        node:20-bookworm bash -s < tests/qa/a4_npm_postinstall_smoke.sh
#
# Exit 0 -> npm install succeeded, binaries extracted, --version OK.
# Exit non-zero -> any step failed; raw stderr surfaced.

set -euo pipefail

TAG="${OBELION_VERIFY_TAG:-v0.2.1}"
EXPECTED_VERSION="${TAG#v}"
PACK_FILE="anto-obelion-${EXPECTED_VERSION}.tgz"
IN_DIR="/qa/in"
SANDBOX="/tmp/a4-sandbox-$$"

echo "[a4] tag=${TAG}, version=${EXPECTED_VERSION}"
echo "[a4] container: $(uname -a)"
echo "[a4] node: $(node --version), npm: $(npm --version)"

if [ ! -f "${IN_DIR}/${PACK_FILE}" ]; then
  echo "[a4] FATAL: ${IN_DIR}/${PACK_FILE} not bind-mounted." >&2
  echo "[a4]   Host must (cd npm && npm pack) and bind-mount the output dir to /qa/in." >&2
  exit 2
fi
if [ -z "${GH_TOKEN:-${GITHUB_TOKEN:-}}" ]; then
  echo "[a4] FATAL: GH_TOKEN/GITHUB_TOKEN not set (obelion is private during MVP)." >&2
  exit 3
fi
export GH_TOKEN="${GH_TOKEN:-${GITHUB_TOKEN}}"
export GITHUB_TOKEN="${GH_TOKEN}"
echo "[a4] GH token resolved (length ${#GH_TOKEN})"

mkdir -p "${SANDBOX}"
cd "${SANDBOX}"
echo '{"name":"a4-smoke","version":"0.0.0","private":true}' > package.json

echo "[a4] running: npm install ${IN_DIR}/${PACK_FILE}"
npm install --no-fund --no-audit "${IN_DIR}/${PACK_FILE}"
echo "[a4] npm install OK"

OBELION_BIN="${SANDBOX}/node_modules/@anto/obelion/bin/obelion"
OBELION_MCP_BIN="${SANDBOX}/node_modules/@anto/obelion/bin/obelion-mcp"
for f in "${OBELION_BIN}" "${OBELION_MCP_BIN}"; do
  if [ ! -f "${f}" ]; then
    echo "[a4] FATAL: ${f} missing after install" >&2
    ls -laR "${SANDBOX}/node_modules/@anto/obelion/" 2>&1 | head -40
    exit 4
  fi
  size=$(stat -c %s "${f}")
  echo "[a4] ${f}: ${size} bytes"
  if [ "${size}" -lt 1048576 ]; then
    echo "[a4] FATAL: ${f} smaller than 1 MB — corrupt extract?" >&2
    exit 5
  fi
done

echo "[a4] running: ${OBELION_BIN} --version"
VER_OUT=$("${OBELION_BIN}" --version 2>&1)
echo "[a4] obelion --version -> ${VER_OUT}"
if ! echo "${VER_OUT}" | grep -q "${EXPECTED_VERSION}"; then
  echo "[a4] FATAL: --version output does not contain ${EXPECTED_VERSION}: ${VER_OUT}" >&2
  exit 6
fi

# Cleanup
rm -rf "${SANDBOX}"
echo "[a4] PASS — npm install end-to-end against live ${TAG} release on Linux; binaries verified."
