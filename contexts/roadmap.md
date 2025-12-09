# Vidkosha Cortex Roadmap

An outline of the Vidkosha Cortex journey. Economic, token, and DAO details are intentionally minimal here; stay focused on the Nervos CKB rail for anchoring/provenance while keeping token/DAO specifics gated.

## Ownership & Update Triggers
- **Owner:** `CTOAgent` maintains this roadmap.
- **Backups:** `AgentCreator` (planning/agent scaffolding) and `SeniorEngineerAgent` (delivery sequencing impacts).
- **Update when:** strategy or sequencing changes, phase definitions shift, feature flag posture changes, or ownership/status changes.
- **Mirror:** Keep `contexts/context.md`, `contexts/vision_board.md`, and relevant workstreams aligned when updating.

## Phase Overview
| Phase | Focus | Core Deliverables | Target Signal |
| --- | --- | --- | --- |
| **Phase 1 — Foundation** | Rust scaffolding, CLI, single-entry Agent. | `AgentBehavior` trait, front-desk Agent, LLM client, CLI flow (`You → Agent → LLM`). | One interactive CLI session routed through Agent that returns a model response. |
| **Phase 2 — Specialists + Helix Memory** | Specialist agents and Helix-backed memory. | CTO, Senior Engineer, Researcher, OpsChain, Rag agents; Helix client + namespace schema; per-agent context files. | Cross-agent workflow that reads/writes Helix nodes/edges via RagAgent without manual intervention. |
| **Phase 3 — AgentCreator** | Automated agent genesis. | AgentCreator prompt + Rust scaffolding generator, registry updates, RAG metadata tagging; controlled A/B tests per `contexts/test_evaluation.md`. | A new specialist created end-to-end by AgentCreator plus documented evaluation vs. baseline. |
| **Phase 4 — Multi-Agent Workflows** | Showcase production-style pipelines. | Documentary Studio pipeline, orchestration scripts, optional dashboards. | Ten-minute mini-doc generated via automated pipeline with minimal human edits. |
| **Future** | Any token/DAO mechanics. | Scoped and gated until opened. | N/A in current scope. |

**Posture:** Ship Phases 1–4 with Nervos CKB as the canonical/default rail for anchoring/provenance/payouts; token/DAO topics remain gated and opened explicitly.

## Current Milestone (December 2025)
- ✅ Phase 1 shipped (CLI, LLM client, baseline specialists).
- 🚧 Phase 2 Helix migration in progress (docs, env wiring, Helix client scaffolding).
- 🧭 Chain/DAO ideas are deferred and not part of current milestones.

### Immediate Priorities
1. Enrich Helix writes with perspective nodes + typed edges so memories are navigable.
2. Layer graph neighborhood traversal atop Helix semantic search so retrieval returns vectors plus adjacent artifacts.
3. Automate Helix namespace export + backup scripts for the fine-tune dataset pipeline (`contexts/helix_schema.md`).
4. Add a deep reasoning mode ("Mega Brain") that favors quality over latency (plan → branch → vote → critique → verify → synthesize) with citations.
5. Ship an operator bootstrap kit (`scripts/node-operator/*`) so new nodes verify Helix + embeddings + LLM via `helix-smoke`/`helix-rich-smoke`.
6. Add semantic routing as default (flagged) for specialist selection.
7. Evolve `index-repo` into a universal ingest tool (allow/deny lists, incremental hashing, symbol-aware parsing, modality handlers, metadata alignment).

### Blocking Questions
- What schema best represents multi-perspective memories (child nodes vs. structured fields)?
- How do we batch large ingests without overwhelming Helix namespaces?

## Near-Term Backlog
- Define per-agent context file templates before implementing each specialist.
- Update AgentCreator to emit scaffolding + context + Helix metadata template in one pass.
- Evaluate crates for async HTTP + streaming responses.
- Decide on serialization format for RAG payloads ahead of Phase 2.
- Map out testing approach (integration vs. snapshot) for multi-agent flows.
- Compare single-node Docker/Compose vs. managed Kubernetes for orchestrator + vLLM + HelixDB.
- Prototype an event bus for agent pub/sub signals; document which events matter.
- Keep `contexts/helix_schema.md` in lockstep with actual Helix namespaces.
- Mirror Vision Board into Helix (planning graph sync): implement `planning-sync` to upsert `vision_board`/`workstream`/`context_doc` nodes per `contexts/planning_graph.md`.
- Developer onboarding guided session (`cargo run -- --dev`): front-desk flow that checks env/ports, points to must-reads, and offers a starter workstream choice.

## Operating Rhythm
- **Weekly checkpoint:** Update this roadmap plus the changelog with progress, blockers, and confidence level.
- **Milestone exit criteria:** Each phase requires a demo or artifact that proves the target signal in the table above.
- **Source of truth:** `contexts/context.md` for philosophy, this file for execution trace.

## How to Update
1. Edit the phase table when scope or sequencing shifts.
2. Add/remove items in “Immediate Priorities” and “Near-Term Backlog” as work progresses.
3. Mirror key completions in `contexts/changelog.md` so agents stay current.

A living plan for turning the Vidkosha Cortex vision into a fully functioning, community-powered ecosystem. Update this document whenever strategy, sequencing, or ownership changes.
