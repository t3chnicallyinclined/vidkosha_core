#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROOT_ENV="$ROOT/.env.example"
KIT_ENV="$ROOT/scripts/node-operator/.env.example"

require() {
  if [[ ! -f "$1" ]]; then
    echo "[parity] Missing file: $1" >&2
    exit 1
  fi
}

require "$ROOT_ENV"
require "$KIT_ENV"

get_val() {
  local file=$1 key=$2
  grep -E "^${key}=" "$file" | head -n1 | cut -d'=' -f2-
}

keys=(
  "VK_CORTEX_LLM_MODEL"
  "OPENAI_BASE_URL"
  "RAG_EMBEDDING_MODEL"
  "RAG_EMBEDDING_BASE_URL"
  "RAG_VECTOR_DIM"
  "HELIX_WRITE_QUERY"
  "HELIX_SEARCH_QUERY"
  "HELIX_DELETE_QUERY"
  "HELIX_RICH_WRITE_QUERY"
)

status=0
for key in "${keys[@]}"; do
  root_val=$(get_val "$ROOT_ENV" "$key" || true)
  kit_val=$(get_val "$KIT_ENV" "$key" || true)
  if [[ "$root_val" != "$kit_val" ]]; then
    echo "[parity] MISMATCH $key: root='$root_val' kit='$kit_val'" >&2
    status=1
  else
    echo "[parity] OK $key=$root_val"
  fi
done

if [[ $status -ne 0 ]]; then
  echo "[parity] Parity check failed" >&2
else
  echo "[parity] Parity check passed"
fi

exit $status
