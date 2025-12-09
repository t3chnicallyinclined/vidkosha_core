#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="${ENV_FILE:-$ROOT/.env}"
RUN_RICH="${RUN_RICH:-0}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "[bootstrap] Missing $ENV_FILE. Copy scripts/node-operator/.env.example to .env and adjust." >&2
  exit 1
fi

# shellcheck source=/dev/null
source "$ENV_FILE"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "[bootstrap] Missing dependency: $1" >&2; exit 1; }
}

require_cmd cargo
require_cmd curl
require_cmd jq

check_http() {
  local name=$1 url=$2
  if curl -fsS -m 5 "$url" >/dev/null; then
    echo "[bootstrap] $name OK: $url"
  else
    echo "[bootstrap] $name UNREACHABLE: $url" >&2
    exit 1
  fi
}

if [[ -n "${OPENAI_BASE_URL:-}" ]]; then
  check_http "LLM" "$OPENAI_BASE_URL/models"
else
  echo "[bootstrap] Skipping LLM check (OPENAI_BASE_URL unset)"
fi

if [[ -n "${RAG_EMBEDDING_BASE_URL:-}" ]]; then
  check_http "Embeddings" "$RAG_EMBEDDING_BASE_URL/models"
else
  echo "[bootstrap] Skipping embeddings check (RAG_EMBEDDING_BASE_URL unset)"
fi

if [[ -n "${HELIX_BASE_URL:-}" ]]; then
  # Helix gateway does not expose a GET /health; /introspect is a stable read path.
  check_http "Helix" "$HELIX_BASE_URL/introspect"
else
  echo "[bootstrap] Skipping Helix check (HELIX_BASE_URL unset)"
fi

echo "[bootstrap] Running helix-smoke..."
( cd "$ROOT" && cargo run -- helix-smoke )

after="$?"
if [[ "$after" -ne 0 ]]; then
  echo "[bootstrap] helix-smoke failed" >&2
  exit "$after"
fi

if [[ "$RUN_RICH" == "1" ]]; then
  echo "[bootstrap] Running helix-rich-smoke..."
  ( cd "$ROOT" && cargo run -- helix-rich-smoke )
fi

echo "[bootstrap] Done. Services reachable and smoke tests passed."
