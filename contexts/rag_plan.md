# Helix AI Fabric Plan

Design notes for Phase 2.5, when Vidkosha Cortex replaces the plain vector store with HelixDB. Helix combines graph storage, semantic search, and namespaces so every agent output becomes a navigable knowledge structure.

---

## Ownership & Update Triggers

- **Owner:** `RagAgent` maintains this plan.
- **Backups:** `HelixFabricAgent` (schema/query alignment) and `SeniorEngineerAgent` (client/orchestrator implementation).
- **Update when:** Helix contract changes (nodes/edges/queries), RAG client behavior changes, embedding defaults change, or observability/provenance flows change.
- **Mirror:** Keep `contexts/helix_schema.md`, `agents/agent_readme.md`, and related workstreams in sync when updating.

## 1. Goals

1. **Helix-first memory gateway:** RagAgent writes nodes/edges to Helix through a single client so every memory has consistent metadata, relationships, and audit logs.
2. **Multi-view knowledge:** Specialists store raw artifacts, distilled summaries, reasoning chains, workflows, and policy relationships inside the same namespace. Retrieval pulls both semantic neighbors and graph neighborhoods.
3. **Typed contracts:** Rust structs describe Helix nodes, edges, and perspective layers so agents can compile-time guarantee completeness before a write.
4. **Observability + provenance:** Every operation captures tracing spans and Helix IDs. Hashes of the resulting nodes/edges flow to CKB cells for on-chain provenance.
5. **Self-reinforcing loop:** Agents interpret data → Helix stores nodes/edges/perspectives → exports feed fine-tunes → smarter model improves agents → Helix graph quality rises.

---

## 2. Architecture Overview

```
Agent ─┐
      ├─ (MemoryRequest) → OrchestratorRouter → RagAgent
LLM ──┘                                  │
                                         ▼
                               HelixGraphClient (HTTP)
                                         │
                                         ▼
                           HelixDB namespace (nodes + edges)
```

1. Non-memory agents submit `MemoryRequest::{Write,Retrieve}` via the router.
2. RagAgent validates metadata, transforms artifacts into Helix `NodeDraft`s and `EdgeDraft`s, then sends them through `HelixGraphClient`.
3. Retrieval combines semantic search (Helix vectors) with graph traversals (neighborhood walk, perspective filters) before returning `MemoryRecord` items.

---

## 3. Data Model

```rust
pub struct NodeDraft {
    pub node_type: NodeType,
    pub summary: String,
    pub body: String,
    pub perspectives: Vec<PerspectiveView>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}

pub struct EdgeDraft {
    pub from: HelixId,
    pub to: HelixId,
    pub relation: String,
    pub weight: f32,
    pub metadata: serde_json::Value,
}

pub struct MemoryRecord {
    pub helix_id: HelixId,
    pub agent_name: String,
    pub topic: String,
    pub project: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub summary: String,
    pub full_content: String,
    pub confidence: f32,
    pub open_questions: Vec<String>,
    pub perspectives: Vec<PerspectiveView>,
    pub related: Vec<RelatedArtifact>,
}
```

Key concepts:

- **Node types:** `memory_entry`, `reasoning_chain`, `workflow_run`, `policy`, `agent_definition`, `dataset_chunk`, etc.
- **PerspectiveView:** specialist-specific interpretation (CEO, Engineer, Legal, Ops). Stored as child nodes or structured fields.
- **RelatedArtifact:** minimal representation of adjacent nodes returned during retrieval so downstream agents understand context without re-querying.

---

## 4. Client & Traits

```rust
#[async_trait::async_trait]
pub trait HelixGraphClient {
    async fn write_node(&self, node: NodeDraft) -> anyhow::Result<HelixId>;
    async fn write_edges(&self, edges: Vec<EdgeDraft>) -> anyhow::Result<()>;
    async fn semantic_search(&self, query: HelixQuery) -> anyhow::Result<Vec<MemoryRecord>>;
    async fn neighborhood(&self, id: &HelixId, depth: usize) -> anyhow::Result<Vec<RelatedArtifact>>;
}
```

- `HelixClient` (HTTP) implements the trait using the `/api/v1/namespaces/{ns}` endpoints.
- `MockHelixClient` mirrors behavior for unit tests.
- `RagAgent` orchestrates validation + translation:
  - `MemoryRequest::Write`: enforce metadata, call interpreter helpers to produce `NodeDraft` + `EdgeDraft`, persist via client, return Helix IDs.
  - `MemoryRequest::Retrieve`: call `semantic_search`, then optionally `neighborhood` to pull adjacent nodes (context files, workflows, policy nodes).

---

## 5. Interpretation & Embeddings Pipeline

1. **Agent interpretation:** whichever specialist produced the artifact emits:
   - canonical summary
   - role-specific perspectives
   - structured metadata (topic, project, risks, decisions)
   - suggested edges (e.g., `relates_to:Project`, `derives_from:ContextFile`).
2. **Chunking + embeddings:** long artifacts are chunked (300–500 tokens). Each chunk receives embeddings through the dedicated vLLM embeddings server until Helix-native embeddings are available.
3. **Graph writes:** RagAgent creates nodes for the chunk(s), reasoning chains, and workflows; edges tie them together so retrieval can walk the structure.
4. **Compression:** Helix stores raw reasoning plus distilled “Reasoning Compression” nodes so future fine-tunes can pull either level of detail.

---

## 6. Integration Steps

1. **Config plumbing** – Add `HelixConfig` (env: `HELIX_BASE_URL`, `HELIX_API_TOKEN`, `HELIX_GRAPH_NAMESPACE`, `HELIX_HTTP_TIMEOUT_MS`). `RAG_*` vars now purely describe the dedicated embeddings service Helix relies on until native embeddings land.
2. **Helix client module** – Implement `HelixClient` (health, namespace metadata, node/edge writes, search). Use `reqwest` with rustls, capture tracing spans.
3. **RagAgent bridge** – Introduce translation layer that converts `MemoryWriteRequest` into Helix drafts so every write immediately lands in Helix (no dual-write shims).
4. **CLI smoke tests** – `cargo run -- helix-smoke` verifies `/health` + namespace metadata before any writes. Future iteration: insert a disposable node + relationship, then delete it.
5. **Orchestrator wiring** – Router prefers Helix-backed RagAgent; if Helix env missing it warns and falls back to the in-memory mock.
6. **Docs + onboarding** – README, `context.md`, and `roadmap.md` describe Helix as the canonical AI Fabric, including install instructions and operator responsibilities.

---

## 7. Open Questions

1. What Helix schema best represents multi-perspective views (separate nodes vs. structured JSON field)?
2. How should we chunk enormous artifacts (codebases, transcripts) without overwhelming Helix? Need ingestion rules.
3. Do we require transactional guarantees for node + edge writes, or is best-effort with retries sufficient?
4. Should RagAgent keep a short-lived cache of recently-read nodes to reduce duplicate queries during a single orchestrator run?
5. How will we map Helix IDs to on-chain commitments (hashing format, batch cadence)?

---

## 8. Next Actions

1. Add regression tests (mock Helix HTTP) that verify perspective nodes + edges are emitted for every `MemoryWriteRequest`.
2. Surface the retrieved neighbor metadata to downstream specialists (router prompts, evaluations) so graph context changes behavior, not just storage.
3. Schedule the new `scripts/helix_backup.sh` helper inside CI/cron and document retention/restore SOPs for the ops team.

---

## 9. Memory Enrichment Pipeline (Helix Edition)

1. **Specialist summary** – Each specialist outputs canonical summary + `PerspectiveView`s (CEO/Eng/Ops/etc.).
2. **Chunk & label** – Break artifacts into chunks with inline headers (`## Risks`, `## Actions`). Each chunk inherits metadata + perspective labels.
3. **Edge synthesis** – Router infers standard edges (produced_by, relates_to, depends_on) while specialists can suggest domain-specific relations.
4. **Embeddings** – Use the dedicated embeddings server until Helix exposes built-in models, then switch to Helix-native vectors.
5. **Validation** – RagAgent ensures required metadata + edge count before writes; rejects secrets or incomplete payloads.
6. **Audit + provenance** – Every Helix write returns IDs + hashes. These flow into future CKB audit cells for the Cognitive Mesh.
