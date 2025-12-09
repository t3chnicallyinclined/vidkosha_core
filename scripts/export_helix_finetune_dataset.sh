#!/usr/bin/env bash
set -eo pipefail

# Exports Helix namespace memories into JSONL training samples.
# Requires: helix CLI, jq.

NS="${HELIX_GRAPH_NAMESPACE:-vidkosha_cortex}"
OUT="${OUT:-datasets/helix_finetune_export.jsonl}"

mkdir -p "$(dirname "$OUT")"

echo "Exporting Helix namespace '$NS' to $OUT"
helix namespace export --namespace "$NS" \
  | jq -c '
    .nodes[]? 
    | select(.node_type=="MemoryChunk")
    | select((.properties.summary // "") != "")
    | select((.properties.full_content // .properties.text // "") != "")
    | {
        task_type: "expert_summary",
        source_nodes: [ .id ],
        input: (.properties.full_content // .properties.text),
        output: .properties.summary,
        metadata: {
          topic: .properties.topic,
          agent_name: .properties.agent_name,
          project: .properties.project
        }
      }
  ' > "$OUT"

count=$(wc -l < "$OUT" | tr -d ' ')
echo "Wrote ${count} examples to $OUT"
