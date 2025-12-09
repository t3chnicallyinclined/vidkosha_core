#!/usr/bin/env bash
set -euo pipefail

# Legacy wrapper: seeds from contexts/category_seed_general.json via unified seed_topics.sh

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$ROOT_DIR/scripts/seed_topics.sh" --input "$ROOT_DIR/contexts/category_seed_general.json" "$@"
