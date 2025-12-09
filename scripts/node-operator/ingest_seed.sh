#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="${ENV_FILE:-$ROOT/.env}"
SEED_FILE="${SEED_FILE:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/seed_corpus.ndjson}"
WRITE_QUERY="${HELIX_WRITE_QUERY:-write_memory_v2}"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "[seed] Missing $ENV_FILE. Copy scripts/node-operator/.env.example to .env and adjust." >&2
  exit 1
fi

# shellcheck source=/dev/null
source "$ENV_FILE"

require() {
  local name=$1
  if [[ -z "${!name:-}" ]]; then
    echo "[seed] Missing required env var: $name" >&2
    exit 1
  fi
}

require HELIX_BASE_URL
require HELIX_GRAPH_NAMESPACE
require HELIX_API_TOKEN
require RAG_EMBEDDING_BASE_URL
require RAG_EMBEDDING_API_KEY
require RAG_EMBEDDING_MODEL

if [[ ! -f "$SEED_FILE" ]]; then
  echo "[seed] Seed file not found: $SEED_FILE" >&2
  require_cmd jq
  require_cmd curl

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "[seed] Missing dependency: $1" >&2; exit 1; }
}

require_cmd jq

  embed() {
    local text=$1
    curl -sS -X POST "$RAG_EMBEDDING_BASE_URL/embeddings" \
      -H "Authorization: Bearer $RAG_EMBEDDING_API_KEY" \
      -H 'Content-Type: application/json' \
      -d "{\"model\":\"$RAG_EMBEDDING_MODEL\",\"input\":$text}" |
      jq -c '.data[0].embedding'
  }
require_cmd curl

topic_default="${NODE_OPERATOR_SEED_TOPIC:-node-operator.seed}"
project_default="${NODE_OPERATOR_SEED_PROJECT:-node-operator}"
    summary=$(echo "$line" | jq -r '.summary')
    full_content=$(echo "$line" | jq -r '.full_content // .summary')
    embed_input=$(printf '%s' "$full_content" | jq -Rs .)
    vector=$(embed "$embed_input")

conf_default="${NODE_OPERATOR_SEED_CONFIDENCE:-0.72}"

endpoint="$HELIX_BASE_URL/$WRITE_QUERY"

      summary: .summary,
      full_content: (.full_content // .summary),
      timestamp: (.timestamp // now | todateiso8601),
while IFS= read -r line || [[ -n "$line" ]]; do
  payload=$(echo "$line" | jq -c --arg topic "$topic_default" --arg project "$project_default" --arg conf "$conf_default" '{
      metadata: (.metadata_json // null),
      payload_hash: null,
      chunk_id: (.chunk_id // null),
      artifact_id: (.artifact_id // null),
    topic: (.topic // $topic),
    project: (.project // $project),
    summary: .summary,
    }' | jq --argjson v "$vector" '. + {vector:$v}')
    confidence: (.confidence // ($conf|tonumber)),
    open_questions: (.open_questions // []),
    metadata: (.metadata_json // null),
    messages: (.messages // []),
    artifacts: (.artifacts // []),
    echo "[seed] wrote: $(echo "$payload" | jq -r '.summary')"

  curl -sS -X POST "$endpoint" \
    -H "Authorization: Bearer $HELIX_API_TOKEN" \
    -H 'Content-Type: application/json' \
    -d "$payload" >/dev/null

echo "[seed] wrote: $(echo "$payload" | jq -r '.summary')"
done < "$SEED_FILE"

echo "[seed] Seed ingest complete."
