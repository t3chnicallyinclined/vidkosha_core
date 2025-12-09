# Vidkosha Cortex – Dev Onboarding & Next Steps

Plan to make the repo contributor-ready and keep onboarding lightweight. Token/marketplace/economics topics are omitted here.

## Ownership & Update Triggers
- **Owner:** `AgentCreator` maintains this onboarding plan.
- **Backups:** `CTOAgent` (architecture/routing impacts) and `SeniorEngineerAgent` (developer workflow changes).
- **Update when:** onboarding steps change, required commands/envs change, Vision Board/workstream navigation changes, or contributor templates/processes change.
- **Mirror:** Reflect updates in `README.md`, `CONTRIBUTING.md`, `contexts/vision_board.md`, and relevant workstreams.

## 1) Immediate Repo Hygiene & Onboarding
**Goals**
- New developer can clone, understand architecture, and contribute in 5–10 minutes.
- Docs point to Vision Board + workstream context files as the primary navigation.

**Planned Steps**
1. Keep `README.md` aligned with seed order and quickstart commands:
   - `cargo build`
   - `cargo run -- helix-smoke`
   - `cargo run -- rag-smoke`
   - Note that any future chain/DAO features are off by default.
2. Keep `CONTRIBUTING.md` minimal:
   - Flow: pick a workstream from `contexts/vision_board.md` → read its context file → pick/open an issue → implement → open PR.
   - Agents (CTO, SeniorEngineer, etc.) should read the relevant context file before proposing code.
3. Add/maintain `.github/PULL_REQUEST_TEMPLATE.md`:
   - Fields: `Workstream`, `Context file(s) read`, `Tests/commands run`, `Schema/docs updated`.

## 2) Workstreams to Start With
- **Semantic Routing v2** – `contexts/workstreams/routing_semantic_v2.md` (set status to active when coding starts).
- **Node Operator Bootstrap Kit** – `contexts/workstreams/node_operator_kit.md` (ops kit).

Dogfooding: pick a workstream, re-read its context (Sections 1–3), implement small PR-sized chunks, and update both the context file and `contexts/changelog.md`.

## 3) Planning Graph & planning-sync (Helix-backed Vision Board)
Reference: `contexts/planning_graph.md`.

**Phase 1 (Stub)**
- Add `planning-sync` CLI subcommand to parse `contexts/vision_board.md` and print JSON summary (no Helix writes yet).
- Create `src/planning/mod.rs` with `WorkstreamSpec` and `load_vision_board` helper.

**Phase 2 (Helix integration)**
- Extend `contexts/helix_schema.md` with `vision_board`, `workstream`, `context_doc` nodes/edges (documented fields only).
- Implement `sync_to_helix` to upsert those nodes; keep values simple.
- Update `planning-sync` to read git HEAD, load workstreams/context docs, and call `sync_to_helix`.

Usage: after editing Vision Board or a workstream context, run `cargo run -- planning-sync` to mirror into Helix.

## 4) Node Operator Bootstrap Kit
Reference: `contexts/workstreams/node_operator_kit.md`.

- Provide one-shot script under `scripts/node-operator/` to check Helix (`:6969`), LLM (`:8000`), embeddings (`:9000`), then run `helix-smoke`.
- Document usage in the workstream and `README.md`.

## 5) How to Resume a Fresh Session
1. Read `contexts/vision_board.md`, `contexts/planning_graph.md`, and this file.
2. Ask which workstream is most important, or pick Semantic Routing v2 or Node Operator Kit.
3. For the chosen workstream: summarize Sections 1–3, propose a 3–5 step implementation tied to specific files/commands, then implement in small PR-sized increments.
4. Update the workstream context and `contexts/changelog.md` after each chunk.

Keep all economics/marketplace details out of this file; it should remain token-agnostic.
