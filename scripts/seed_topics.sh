jq -c '.[]' "$ROOT/contexts/category_seed.json" | while read -r row; do
#!/usr/bin/env bash
set -euo pipefail

# Unified topic/category seeder for Helix. Replaces older seed_* scripts.
# Defaults to contexts/category_seed_general.json; override with --input FILE.
# Flags: --skip-existing (default on) to call SearchTopics before insert;
#        --no-skip-existing to force insert; --no-skip-sentinel to include
#        rows with status=skip or names starting with '_'.

ROOT=$(cd "$(dirname "$0")/.." && pwd)
INPUT="$ROOT/contexts/category_seed_general.json"
SKIP_EXISTING=1
SKIP_SENTINEL=1
BASE_URL="${HELIX_BASE_URL:-http://127.0.0.1:6969}"
AUTH=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --input|-i) INPUT="$2"; shift 2;;
    --skip-existing) SKIP_EXISTING=1; shift;;
    --no-skip-existing) SKIP_EXISTING=0; shift;;
    --skip-sentinel) SKIP_SENTINEL=1; shift;;
    --no-skip-sentinel) SKIP_SENTINEL=0; shift;;
    *) echo "Unknown option: $1" >&2; exit 1;;
  esac
done

if [[ -n "${HELIX_API_TOKEN:-}" ]]; then
  AUTH=(-H "Authorization: Bearer ${HELIX_API_TOKEN}")
fi

if [[ ! -f "$INPUT" ]]; then
  echo "Input file not found: $INPUT" >&2
  exit 1
fi

echo "Seeding topics from $INPUT to $BASE_URL/InsertTopic (skip_existing=$SKIP_EXISTING skip_sentinel=$SKIP_SENTINEL)"

total=0; ok=0; fail=0; skipped=0

while read -r row; do
  name=$(jq -r '.name' <<<"$row")
  status=$(jq -r '.status // ""' <<<"$row")

  if [[ "$SKIP_SENTINEL" -eq 1 && ( "$status" == "skip" || "$name" == _* ) ]]; then
    ((skipped++)); ((total++)); continue
  fi

  meta=$(jq -c '{description,status,parent}' <<<"$row")

  if [[ "$SKIP_EXISTING" -eq 1 ]]; then
    existing=$(curl -s "${AUTH[@]}" -H 'Content-Type: application/json' \
      -d "{\"name\":\"$name\"}" "$BASE_URL/SearchTopics" | jq '.topics | length')
    if [[ "${existing:-0}" -gt 0 ]]; then
      echo "skip $name (exists)"
      ((skipped++)); ((total++)); continue
    fi
  fi

  payload=$(jq -n --arg name "$name" --arg meta "$meta" '{name:$name, metadata:$meta}')
  if curl -sS "${AUTH[@]}" -H 'Content-Type: application/json' -d "$payload" "$BASE_URL/InsertTopic" >/dev/null; then
    echo "inserted $name"
    ((ok++))
  else
    echo "failed $name" >&2
    ((fail++))
  fi
  ((total++))
done < <(jq -c '.[]' "$INPUT")

echo "Done. total=$total ok=$ok skipped=$skipped fail=$fail"
exit $(( fail > 0 ? 1 : 0 ))
