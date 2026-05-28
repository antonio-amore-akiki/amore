#!/usr/bin/env bash
# certify-user-flow.sh — Linux/macOS stranger-install cert (10 gates)
# Usage: bash certify-user-flow.sh [--tag <tag>] [--repo <owner/repo>]
# Runs syntax-check safe (no live OS changes at commit time).
# Idempotent: cleans up all artefacts it creates.
set -euo pipefail

REPO="${CERT_REPO:-antonio-amore-akiki/amore}"
CERT_DIR="${CERT_DIR:-$HOME/.amore-cert-tmp}"
SCHEMA_DIR="$(cd "$(dirname "$0")/../schema" && pwd)"
STATE_DIR="$(cd "$(dirname "$0")/../state/cert" && pwd 2>/dev/null || echo "$HOME/.amore-cert-state")"
OS_TAG=""
TAG=""
GATES=()
OVERALL_PASS=true
START_TS=""
FINISH_TS=""

# ── helpers ────────────────────────────────────────────────────────────────
log()  { echo "[cert] $*"; }
warn() { echo "[cert][WARN] $*" >&2; }

ns_now() {
  if [[ "$(uname)" == "Darwin" ]]; then
    python3 -c "import time; print(int(time.time()*1000))"
  else
    date +%s%3N
  fi
}

detect_os() {
  case "$(uname -s)" in
    Linux*)  OS_TAG="linux";;
    Darwin*) OS_TAG="macos";;
    *)       echo "Unsupported OS: $(uname -s)"; exit 1;;
  esac
}

gate_record() {
  local num="$1" name="$2" pass="$3" dur="$4" err="${5:-}" detail="${6:-}"
  local entry="{\"gate\":$num,\"name\":\"$name\",\"pass\":$pass,\"duration_ms\":$dur"
  [[ -n "$err"    ]] && entry+=",\"error\":$(python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "$err")"
  [[ -n "$detail" ]] && entry+=",\"detail\":$(python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "$detail")"
  entry+="}"
  GATES+=("$entry")
}

run_gate() {
  local num="$1" name="$2"; shift 2
  log "Gate $num: $name"
  local t0; t0=$(ns_now)
  local err="" pass=true detail=""
  if detail=$("$@" 2>&1); then
    pass=true
  else
    pass=false; err="$detail"; OVERALL_PASS=false
  fi
  local t1; t1=$(ns_now)
  local dur=$(( t1 - t0 ))
  gate_record "$num" "$name" "$pass" "$dur" "$err" "$detail"
  [[ "$pass" == true ]] && log "  PASS (${dur}ms)" || warn "  FAIL (${dur}ms): $err"
}

emit_result() {
  mkdir -p "$STATE_DIR"
  local out="$STATE_DIR/local-${OS_TAG}-result.json"
  local gates_json; gates_json=$(IFS=,; echo "[${GATES[*]}]")
  python3 - "$out" "$OS_TAG" "$RUN_ID" "$START_TS" "$FINISH_TS" \
    "$( [[ $OVERALL_PASS == true ]] && echo true || echo false )" \
    "${TAG:-}" "$gates_json" <<'PYEOF'
import json, sys
out, os_tag, run_id, started, finished, overall, tag, gates_json = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6], sys.argv[7], sys.argv[8]
result = {
  "schema_version": 1,
  "os": os_tag,
  "run_id": run_id,
  "started_at": started,
  "finished_at": finished,
  "overall_pass": overall == "true",
  "gates": json.loads(gates_json)
}
if tag:
  result["release_tag"] = tag
with open(out, "w") as f:
  json.dump(result, f, indent=2)
print("Result written to", out)
PYEOF
}

validate_schema() {
  local schema="$1" doc="$2"
  python3 - "$schema" "$doc" <<'PYEOF'
import sys, json
schema_path, doc_path = sys.argv[1], sys.argv[2]
try:
  import jsonschema
  with open(schema_path) as f: schema = json.load(f)
  with open(doc_path) as f: doc = json.load(f)
  jsonschema.validate(doc, schema)
  print("Schema valid")
except ImportError:
  print("jsonschema not installed; JSON parse only")
  with open(doc_path) as f: json.load(f)
  print("JSON parse OK")
PYEOF
}

# ── gate functions ──────────────────────────────────────────────────────────
gate1_download() {
  mkdir -p "$CERT_DIR"
  if [[ -z "$TAG" ]]; then
    TAG=$(gh release list --repo "$REPO" --limit 1 --json tagName --jq '.[0].tagName')
  fi
  log "  Tag: $TAG"
  if [[ "$OS_TAG" == "linux" ]]; then
    gh release download "$TAG" --repo "$REPO" \
      --pattern "amore-*.AppImage" \
      --pattern "amore-*.AppImage.sha256" \
      --pattern "amore-*.AppImage.sigstore" \
      --dir "$CERT_DIR"
    ARTIFACT=$(ls "$CERT_DIR"/amore-*.AppImage | head -1)
  else
    gh release download "$TAG" --repo "$REPO" \
      --pattern "amore-*-macos.tar.gz" \
      --pattern "amore-*-macos.tar.gz.sha256" \
      --pattern "amore-*-macos.tar.gz.sigstore" \
      --dir "$CERT_DIR"
    ARTIFACT=$(ls "$CERT_DIR"/amore-*-macos.tar.gz | head -1)
  fi
  ARTIFACT_BASE=$(basename "$ARTIFACT")
  log "  Downloaded: $ARTIFACT_BASE"
}

gate2_sha256() {
  local sha_file="$CERT_DIR/${ARTIFACT_BASE}.sha256"
  pushd "$CERT_DIR" >/dev/null
  sha256sum -c "$sha_file"
  popd >/dev/null
}

gate3_sigstore() {
  cosign verify-blob \
    --bundle "$CERT_DIR/${ARTIFACT_BASE}.sigstore" \
    --certificate-identity-regexp "antonioakiki15@gmail.com" \
    --certificate-oidc-issuer "https://accounts.google.com" \
    "$ARTIFACT"
}

gate4_install() {
  if [[ "$OS_TAG" == "linux" ]]; then
    chmod +x "$ARTIFACT"
    local link="/usr/local/bin/amore"
    if [[ -w /usr/local/bin ]]; then
      ln -sf "$ARTIFACT" "$link"
    else
      sudo ln -sf "$ARTIFACT" "$link"
    fi
    INSTALL_BIN="$link"
  else
    tar -xzf "$ARTIFACT" -C "$CERT_DIR"
    local bin; bin=$(find "$CERT_DIR" -name "amore" -type f | head -1)
    chmod +x "$bin"
    INSTALL_BIN="$bin"
  fi
  log "  Installed: $INSTALL_BIN"
}

gate5_sideeffects() {
  "$INSTALL_BIN" --version
  local data_dir="$HOME/.local/share/amore"
  [[ "$OS_TAG" == "macos" ]] && data_dir="$HOME/Library/Application Support/amore"
  [[ -d "$data_dir/qdrant" ]] || (mkdir -p "$data_dir/qdrant" && log "  Created qdrant dir (first-run)")
}

gate6_mcp_self_contained() {
  local out_file="$CERT_DIR/auto-wire-gate6.json"
  "$INSTALL_BIN" mcp --register-claude-code --self-contained --json > "$out_file"
  validate_schema "$SCHEMA_DIR/auto-wire-contract.schema.json" "$out_file"
  cat "$out_file"
}

gate7_cli_wire() {
  # Back up and remove existing amore entry so we test fresh add
  if [[ -f "$HOME/.claude.json" ]]; then
    cp "$HOME/.claude.json" "$HOME/.claude.json.cert-bak"
    python3 -c "
import json, sys
with open('$HOME/.claude.json') as f: cfg = json.load(f)
cfg.setdefault('mcpServers', {}).pop('amore', None)
with open('$HOME/.claude.json', 'w') as f: json.dump(cfg, f, indent=2)
"
  fi
  claude mcp add amore "$INSTALL_BIN" mcp --stdio
}

gate8_mcp_list() {
  local listing
  listing=$(claude mcp list 2>&1)
  echo "$listing" | grep -i "amore" | grep -i "CONNECTED"
}

gate9_stdio_drive() {
  local req_init='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cert","version":"0"}}}'
  local req_list='{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'
  local req_observe='{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"observe","arguments":{"query":"cert smoke test"}}}'
  local req_recall='{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"recall","arguments":{"query":"cert smoke test"}}}'
  local resp
  resp=$(printf '%s\n%s\n%s\n%s\n' \
    "$req_init" "$req_list" "$req_observe" "$req_recall" \
    | "$INSTALL_BIN" mcp --stdio 2>/dev/null)
  echo "$resp" | python3 -c "
import sys, json
lines = [l for l in sys.stdin.read().splitlines() if l.strip()]
ids = set()
for l in lines:
    try:
        obj = json.loads(l)
        if 'id' in obj: ids.add(obj['id'])
    except: pass
assert {1,2,3,4}.issubset(ids), f'Missing response IDs; got {ids}'
print('Stdio drive OK; response IDs:', sorted(ids))
"
}

gate10_cleanup() {
  # Restore claude.json from backup
  if [[ -f "$HOME/.claude.json.cert-bak" ]]; then
    mv "$HOME/.claude.json.cert-bak" "$HOME/.claude.json"
  fi
  # npm cleanup: remove claude-code if it was installed by this cert run
  if [[ "${CERT_INSTALLED_NPM:-}" == "1" ]]; then
    npm uninstall -g @anthropic-ai/claude-code 2>/dev/null || true
  fi
  # Remove installed symlink (linux)
  if [[ "$OS_TAG" == "linux" && -L "/usr/local/bin/amore" ]]; then
    sudo rm -f "/usr/local/bin/amore" 2>/dev/null || rm -f "/usr/local/bin/amore" 2>/dev/null || true
  fi
  # Remove cert work dir
  rm -rf "$CERT_DIR"
  log "  Cleanup complete"
  # Emit + validate result
  FINISH_TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  emit_result
  validate_schema "$SCHEMA_DIR/cert-result.schema.json" "$STATE_DIR/local-${OS_TAG}-result.json"
}

# ── main ───────────────────────────────────────────────────────────────────
main() {
  detect_os
  RUN_ID=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
  START_TS="$RUN_ID"

  # Parse args
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --tag) TAG="$2"; shift 2;;
      --repo) REPO="$2"; shift 2;;
      *) warn "Unknown arg: $1"; shift;;
    esac
  done

  log "=== Amore cert: $OS_TAG | repo=$REPO ==="

  run_gate 1 "gh-download"          gate1_download
  run_gate 2 "sha256-verify"        gate2_sha256
  run_gate 3 "sigstore-verify"      gate3_sigstore
  run_gate 4 "install"              gate4_install
  run_gate 5 "side-effects"         gate5_sideeffects
  run_gate 6 "mcp-self-contained"   gate6_mcp_self_contained
  run_gate 7 "cli-wire"             gate7_cli_wire
  run_gate 8 "mcp-list-connected"   gate8_mcp_list
  run_gate 9 "stdio-drive"          gate9_stdio_drive
  run_gate 10 "cleanup"             gate10_cleanup

  if [[ "$OVERALL_PASS" == true ]]; then
    log "=== CERT PASS ==="
    exit 0
  else
    warn "=== CERT FAIL — see $STATE_DIR/local-${OS_TAG}-result.json ==="
    exit 1
  fi
}

main "$@"
