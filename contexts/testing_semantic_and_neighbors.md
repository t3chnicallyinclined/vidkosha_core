# Testing Semantic Routing and Helix Neighbor Fetch

Quick guide for humans/agents to validate the new flag-gated semantic router and Helix neighbor hydration. Covers commands, env vars, expected outputs, and code touchpoints.

Related workstreams/runbooks: `contexts/workstreams/routing_semantic_v2.md`, `contexts/workstreams/helix_graph_neighbors.md`, and the umbrella runbook `contexts/plan_routing_graph_evolution.md`.

Related: front-desk prompt/tool/RAG guidance lives in `agents/agent_readme.md`. Only update this guide when routing/neighbor flags, commands, or expected outputs change; prompt/policy tuning alone does not require edits here.

## Ownership & Update Triggers



# Testing Semantic Routing and Helix Neighbor Fetch (Condensed)

Use this as a quick smoke checklist for routing + neighbor hydration. Update only when routing/neighbor flags, commands, or expected outputs change (prompt/policy tuning does not require edits).

Related workstreams/runbooks: `contexts/workstreams/routing_semantic_v2.md`, `contexts/workstreams/helix_graph_neighbors.md`, `contexts/plan_routing_graph_evolution.md`. Front-desk prompt/tool guidance: `agents/agent_readme.md`.

## Ownership

- **Owner:** `RagAgent` (smokes), **Backups:** `HelixFabricAgent`, `SeniorEngineerAgent`.

## Semantic Routing Smoke

- **Flags:** `ROUTING_SEMANTIC_ENABLED=1`, optional `ROUTING_SEMANTIC_THRESHOLD=0.25`.
- **Command:**
  ```bash
  ROUTING_SEMANTIC_ENABLED=1 ROUTING_SEMANTIC_THRESHOLD=0.25 RUST_LOG=debug \
    cargo run -- --prompt "gpu budget modeling for k8s clusters"
  ```
- **Expect:** debug log shows semantic branch and chosen specialist (e.g., OpsChainAgent). If it falls back, lower threshold or check prototypes in `src/orchestrator/routing/semantic.rs`.

## Helix Neighbor Smoke (HelixQL path)

- **Flag:** `RAG_NEIGHBOR_DEPTH=1` (depth>0 enables fetch; default off).
- **Commands:**
  - Chunk: `RAG_NEIGHBOR_DEPTH=1 cargo run -- helix-smoke`
  - Rich: `RAG_NEIGHBOR_DEPTH=1 cargo run -- helix-rich-smoke`
- **Expect:** `Neighbors attached: <n>` (>0 when edges exist). Rich smoke prints `neighbors=<n>` for first record. If always 0, seed edges via rich smoke and ensure Helix env (`HELIX_BASE_URL`, `HELIX_API_TOKEN`, `HELIX_GRAPH_NAMESPACE`).
- **Smoke command:**
  ```bash
  ROUTING_SEMANTIC_ENABLED=1 ROUTING_SEMANTIC_THRESHOLD=0.25 RUST_LOG=debug \
    cargo run -- --prompt "gpu budget modeling for k8s clusters"
  ```
- **What to look for:**
  - Router logs (with `RUST_LOG=debug`) should show `router_intent`/`suggested_agent` from semantic branch (e.g., `OpsChainAgent`).
  - Prompt that avoids explicit keywords should still hit a specialist instead of front desk.
- **Troubleshooting:**
  - If it always falls back to Agent, lower threshold or check prototypes in `semantic.rs`.
  - If build fails, ensure `RoutingDecision::new` remains `pub(crate)` and module exported in `orchestrator::routing`.

## Helix Neighbor Fetch (HelixQL path)

- **Purpose:** Hydrate search results with graph neighbors (topic/perspective/message/artifact) using Helix `/neighbors` API when using `HelixQueryRagClient`.
- **Env flags:**
  - `RAG_NEIGHBOR_DEPTH=1` (depth >0 enables fetch; default off)
- **Code:**
  - `src/rag/helix.rs` (`HelixQueryRagClient` attach_neighbors, env read)
  - `src/main.rs` smokes print neighbor counts
- **Smokes:**
  - Chunk path: `RAG_NEIGHBOR_DEPTH=1 cargo run -- helix-smoke`
  - Rich path (creates messages/artifacts/tool_calls): `RAG_NEIGHBOR_DEPTH=1 cargo run -- helix-rich-smoke`
- **What to look for:**
  - Output line `Neighbors attached: <n>` in `helix-smoke`. `>0` means hydration worked; `0` means no neighbors or namespace lacks linked nodes.
  - In rich smoke, the printed context shows `neighbors=<n>` for the first record.
- **Data pre-req:** Namespace must have edges to return. The rich smoke write includes conversation/messages/artifact/tool_call; run it if your namespace is empty.
- **Troubleshooting:**
  - If errors: ensure Helix is up (`HELIX_BASE_URL`, `HELIX_API_TOKEN`, `HELIX_GRAPH_NAMESPACE` set) and the gateway supports `/nodes/{id}/neighbors`.
  - If always 0 neighbors, inspect the node via `helix namespace export --namespace $HELIX_GRAPH_NAMESPACE | jq 'select(.node_id=="<id>")'` to confirm edges exist.

## Current Defaults / Behavior

- Semantic routing is **off by default** (opt-in via env). Semantic branch runs after explicit mention and keyword rules.
- Neighbor fetch is **off by default**; HelixQL read path remains vector-only unless `RAG_NEIGHBOR_DEPTH` is set.
- Writes still use HelixQL chunk queries; graph-client read default is pending.

## Quick Reference Commands

```
# Semantic routing smoke
ROUTING_SEMANTIC_ENABLED=1 ROUTING_SEMANTIC_THRESHOLD=0.25 cargo run -- --prompt "design an fpv racing plan"

# Helix neighbor smoke (chunk path)
RAG_NEIGHBOR_DEPTH=1 cargo run -- helix-smoke

# Helix neighbor smoke (rich path)
RAG_NEIGHBOR_DEPTH=1 cargo run -- helix-rich-smoke
```
