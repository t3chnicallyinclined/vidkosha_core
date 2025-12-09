# Routing + Reflection + Graph Retrieval + Evolution — Implementation Plan (Dec 2025)

Scope: executable plan for four improvements (semantic routing, self-reflection loop, Helix neighborhood retrieval, adaptive evolution instrumentation). Keeps changes additive, flag-gated, and testable.

**Where this fits:**
- Semantic routing → `contexts/workstreams/routing_semantic_v2.md`
- Graph neighborhoods → `contexts/workstreams/helix_graph_neighbors.md`
- Reflection loop → stays here + `contexts/improvements.md` (§2) until promoted to a workstream
- Evolution instrumentation → `contexts/improvements.md` (§21) until promoted

Treat this file as the cross-workstream runbook; mirror implementation details back into the linked workstreams when they change.

## Ownership & Update Triggers

- **Owner:** `CTOAgent` owns this plan.
- **Backups:** `SeniorEngineerAgent` (implementation changes) and `RagAgent`/`HelixFabricAgent` (retrieval/neighbors implications).
- **Update when:** routing/reflection/neighborhood/evolution plans change, flags change, or test expectations shift.
- **Mirror:** Align updates with `contexts/workstreams/routing_semantic_v2.md`, `contexts/workstreams/helix_graph_neighbors.md`, and relevant code docs/changelog.

## 1) Semantic + Hybrid Routing
- Interfaces/locations:
  - `src/orchestrator/router.rs`: add semantic branch after explicit tokens and keyword rules.
  - New module `src/orchestrator/routing/semantic.rs` for prototype loading + scoring.
  - Optional `CapabilityRegistry` stub in `src/orchestrator/routing/capability_registry.rs`.
- Prototypes: collect per-specialist seed texts (agent system prompt snippet + 3–5 intent examples). Embed once at startup via existing embeddings client (`OpenAiEmbeddingsClient`). Cache vectors.
- Control flags (env):
  - `ROUTING_SEMANTIC_ENABLED=true|false`
  - `ROUTING_SEMANTIC_THRESHOLD=0.35`
  - `ROUTING_SEMANTIC_TOPK=3`
- Behavior:
  1. Explicit token match → return.
  2. Keyword rules → return.
  3. Semantic: cosine similarity vs prototypes; pick top scoring > threshold; else fall back to front desk.
- Telemetry: log path taken (explicit/rule/semantic), score, selected agent. Attach to `RoutingDecision` metadata.
- Tests: unit tests for threshold behavior, explicit override precedence, and deterministic scoring with fixed vectors (inject mock embedder).

## 2) Agent Self-Reflection Loop
- Interfaces/locations:
  - Add wrapper in `OrchestratorRouter::dispatch` or decorator around `AgentBehavior`.
  - Critic prompt lives in `src/agents/prompts.rs` (new) or inline constant.
- Triggering heuristic: input length > N (e.g., 500 chars) OR keywords (architecture, roadmap, research). Env: `REFLECTION_ENABLED`, `REFLECTION_MAX_SECS`, `REFLECTION_MIN_LENGTH`.
- Flow:
  1. Pass 1: normal specialist response.
  2. Pass 2: send Pass 1 to critic prompt; return refined output + include short critique in response metadata.
- Safeguards: timeout fallback to Pass 1; cap tokens; skip for front-desk Agent unless explicitly requested.
- Tests: snapshot test for critic prompt; unit test heuristic gating; latency budget mock.

## 3) Helix Graph Neighborhood Retrieval
- Interfaces/locations:
  - `src/rag/helix.rs`: feature flag to choose `HelixGraphClient` for reads; keep writes on HelixQL until parity proven.
  - Add neighbor fetch fallback for HelixQL results (call `/nodes/{id}/neighbors?depth=1` when `id` present).
- Control flags: `RAG_GRAPH_READS_ENABLED=true|false`, `RAG_NEIGHBOR_DEPTH=1`.
- Smokes: update `helix-smoke` and `rag-smoke` to assert neighbors (topic/perspective) are attached in metadata (`helix_neighbors`). Keep DROP-based delete.
- Error handling: if neighbor fetch fails, log once per session and return vector-only results.

## 4) Adaptive Memory Evolution (Instrumentation First)
- Interfaces/locations:
  - `src/rag/types.rs`: add optional access stats fields and `MemoryEvolutionConfig` (decay rates, prune thresholds, reporting toggle).
  - `src/rag/helix.rs`: log read access per topic/project/agent; attach counters/last_accessed into metadata when enabled.
  - Background task stub in `src/rag/evolution.rs` (new) to compute decay and emit recommendations (no auto-mutation initially).
- Control flags: `EVOLUTION_ENABLED`, `EVOLUTION_DECAY_HALFLIFE_SECS`, `EVOLUTION_REPORT_INTERVAL_SECS`, `EVOLUTION_PRUNE_THRESHOLD`.
- Phase 1 behavior: collect counters and emit reports; no graph mutations. Future: feed weights into scoring and edge reweighting.
- Tests: evolution smoke that simulates repeated queries and checks counter increments/decay math (feature-flagged), unit tests for config defaults and serialization.

## Milestones / Order
1. Land semantic routing with flags + tests (small blast radius).
2. Add reflection loop (flagged) and critic prompt; measure latency.
3. Enable graph reads + neighbor attachment; update smokes.
4. Add evolution instrumentation + report-only job; later hook into scoring.

## Verification Commands
- Routing: `cargo test routing::semantic` (new module) + existing router tests.
- Reflection: `cargo test reflection` + manual prompt check via `cargo run -- --prompt "architecture plan"` with `REFLECTION_ENABLED=1`.
- Graph neighbors: `cargo run -- helix-smoke` and `cargo run -- rag-smoke` with `RAG_GRAPH_READS_ENABLED=1`.
- Evolution: `cargo test evolution` (feature-flagged) once added.
