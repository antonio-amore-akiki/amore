#!/usr/bin/env bash
# tests/qa/lib/ensure_daemons.sh
#
# CLASS FIX (2026-05-26): never skip a daemon-gated QA gate. If Ollama or
# Qdrant is down, start it. Per user directive: "u start them not me. and fix
# the class so u always start if smthg not running."
#
# Probes:
#   Ollama  http://127.0.0.1:11434/api/version    -> spawns `ollama serve`
#   Qdrant  http://127.0.0.1:6333/readyz          -> docker run qdrant/qdrant:v1.15.4
#
# Exit 0 -> both up + reachable. Caller proceeds with QA gate.
# Exit non-zero -> daemon failed to come up; caller MUST surface, never skip.

set -euo pipefail

OLLAMA_TIMEOUT=${OLLAMA_TIMEOUT:-15}
QDRANT_TIMEOUT=${QDRANT_TIMEOUT:-30}
QDRANT_IMAGE=${QDRANT_IMAGE:-qdrant/qdrant:v1.15.4}
QDRANT_CONTAINER=${QDRANT_CONTAINER:-qdrant}
QDRANT_VOLUME=${QDRANT_VOLUME:-qdrant_storage}

probe() {
  curl -s -o /dev/null -w "%{http_code}" --max-time 2 "$1" 2>/dev/null || echo "000"
}

poll_until_ready() {
  local url=$1 label=$2 timeout=$3 i=0
  while [ $i -lt $timeout ]; do
    if [ "$(probe "$url")" = "200" ]; then
      echo "[ensure_daemons] $label up after ${i}s"
      return 0
    fi
    sleep 1
    i=$((i + 1))
  done
  return 1
}

start_ollama() {
  if ! command -v ollama >/dev/null; then
    echo "[ensure_daemons] ERROR: ollama not on PATH. Install via https://ollama.com/download" >&2
    return 1
  fi
  echo "[ensure_daemons] starting ollama serve (background)..."
  nohup ollama serve >/tmp/ollama-qa.log 2>&1 &
  echo "[ensure_daemons] ollama PID: $!"
}

start_qdrant() {
  if ! command -v docker >/dev/null; then
    echo "[ensure_daemons] ERROR: docker not on PATH" >&2
    return 1
  fi
  if ! docker info >/dev/null 2>&1; then
    echo "[ensure_daemons] ERROR: docker daemon not running. Start Docker Desktop / systemctl start docker" >&2
    return 1
  fi
  if docker ps -a --format '{{.Names}}' | grep -q "^${QDRANT_CONTAINER}\$"; then
    if ! docker ps --format '{{.Names}}' | grep -q "^${QDRANT_CONTAINER}\$"; then
      echo "[ensure_daemons] starting existing qdrant container..."
      docker start "$QDRANT_CONTAINER" >/dev/null
    else
      echo "[ensure_daemons] qdrant container already running"
    fi
  else
    echo "[ensure_daemons] running new qdrant container..."
    docker run -d --name "$QDRANT_CONTAINER" \
      -p 6333:6333 -p 6334:6334 \
      -v "${QDRANT_VOLUME}:/qdrant/storage" \
      "$QDRANT_IMAGE" >/dev/null
  fi
}

# --- main ---
echo "[ensure_daemons] probing Ollama..."
if [ "$(probe http://127.0.0.1:11434/api/version)" != "200" ]; then
  start_ollama
  if ! poll_until_ready "http://127.0.0.1:11434/api/version" "ollama" "$OLLAMA_TIMEOUT"; then
    echo "[ensure_daemons] ERROR: ollama failed to come up after ${OLLAMA_TIMEOUT}s" >&2
    tail -n 20 /tmp/ollama-qa.log 2>/dev/null || true
    exit 3
  fi
else
  echo "[ensure_daemons] Ollama already up"
fi

echo "[ensure_daemons] probing Qdrant..."
if [ "$(probe http://127.0.0.1:6333/readyz)" != "200" ]; then
  start_qdrant
  if ! poll_until_ready "http://127.0.0.1:6333/readyz" "qdrant" "$QDRANT_TIMEOUT"; then
    echo "[ensure_daemons] ERROR: qdrant failed to come up after ${QDRANT_TIMEOUT}s" >&2
    docker logs "$QDRANT_CONTAINER" 2>&1 | tail -n 20 || true
    exit 5
  fi
else
  echo "[ensure_daemons] Qdrant already up"
fi

# gRPC reachability — qdrant-client crate uses :6334
if ! (echo > /dev/tcp/127.0.0.1/6334) 2>/dev/null; then
  echo "[ensure_daemons] ERROR: Qdrant gRPC :6334 not reachable" >&2
  exit 6
fi

echo "[ensure_daemons] both daemons ready (ollama:11434, qdrant:6333+6334)"
