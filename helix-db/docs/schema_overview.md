# Helix Schema Quick Reference

## MemoryChunk (Vector)
- `agent_name` (String): Authoring agent name for filtering/audit.
- `topic` (String): Coarse label for retrieval scopes.
- `project` (String): Project/initiative grouping.
- `summary` (String): Short description of the chunk.
- `timestamp` (Date): When this chunk was written.
- `open_questions` ([String]): Unresolved follow-ups tied to this chunk.
- `metadata` (String): JSON string for structured extras (source path, section, role, etc.).
- `payload_hash` (String): Hash of payload for dedupe/version detection.
- `chunk_id` (String): Client-generated chunk identifier.
- `artifact_id` (String): FK to parent `Artifact` node.

## Nodes
- `Artifact`: Logical parent for one or more chunks; fields: `agent_name`, `topic`, `project`, `summary`, `status`, `payload_hash`, `metadata`, `created_at`.
- `Agent`: Registry entry; fields: `name`, `role`, `version`, `routing_intent`, `metadata`.
- `Topic`: Taxonomy entry; fields: `name`, `metadata`.
- `Project`: Grouping entry; fields: `name`, `metadata`.

## Edges
- `Produced_by`: From `Artifact` → `Agent`.
- `Belongs_to_topic`: From `Artifact` → `Topic`.
- `Belongs_to_project`: From `Artifact` → `Project`.
- `Supersedes`: From `Artifact` → `Artifact`; properties: `reason`.
- `Relates_to`: From `Artifact` → `Artifact`; properties: `label`, `weight`.

## Query (vector-first insert)
- `InsertMemoryChunk(vector: [F64], agent_name: String, topic: String, project: String, summary: String, timestamp: Date, open_questions: [String], metadata: String, payload_hash: String, chunk_id: String, artifact_id: String)`
  - Writes one `MemoryChunk` row with supplied vector and properties; returns `{ chunk_id }`.
