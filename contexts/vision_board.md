# Vidkosha Cortex Vision Board

A contributor-facing map of active and planned workstreams. Each row links to a context file that agents and humans should read before touching code.
Nervos CKB anchoring/indexing/operator registry are core rails; the `*_lite` briefs are entry points, but assume CKB as the default path.

Strategy/architecture live in `contexts/context.md`; this board tracks execution workstreams.

## Ownership & Update Triggers

- **Owner:** `CTOAgent` maintains this board.
- **Backups:** `AgentCreator` (planning graph linkage) and `SeniorEngineerAgent` (execution status changes).
- **Update when:** workstream status/owners change, new workstreams are added, or context links/agents shift.
- **Mirror:** Keep `contexts/context.md`, `contexts/roadmap.md`, and workstream files in sync when updating.

## Workstreams Overview

| Workstream | Status | Difficulty | Owner | Context | Suggested Agents |
| --- | --- | --- | --- | --- | --- |
| CKB Indexing Lite | ready | M | core team | `contexts/workstreams/ckb_indexing_lite.md` | SeniorEngineerAgent, ResearcherAgent |
| CKB Node Operator Lite | ready | M | seeking owner | `contexts/workstreams/node_operator_lite.md` | SeniorEngineerAgent, OpsChainAgent |
| CKB Operator Registry Lite | ready | M | seeking owner | `contexts/workstreams/operator_registry_lite.md` | SeniorEngineerAgent, OpsChainAgent |
| CKB Anchoring Lite | planned | M | core team | `contexts/workstreams/ckb_anchor_lite.md` | SeniorEngineerAgent, OpsChainAgent |
| Treasury Policy Lite | planned | M | seeking owner | `contexts/workstreams/treasury_policy_lite.md` | OpsChainAgent, SeniorEngineerAgent |
| Dev Onboarding Guided Session (`cargo run -- --dev`) | backlog | S | seeking owner | `contexts/improvements.md` (backlog idea) | CTOAgent, AgentCreator, SeniorEngineerAgent |

**Contributing:** see `CONTRIBUTING.md` for expectations, how to propose expansions (what/why), and review requirements.

**Status legend:** `idea` (rough concept), `planned` (scoped but not yet staffed), `active` (in progress), `paused` (on hold), `done` (landed).

---

## How to Use This Board

1. Pick a workstream from the table above.
2. Open its context file and read Sections 1–3 fully.
3. For agents: summarize the context, list relevant code files and Helix schema sections, then propose a short implementation plan before editing code.
4. For contributors: open or pick a GitHub issue labeled with the corresponding `workstream:<name>` tag and follow the implementation plan.

Each context file follows a common template so humans and agents can align quickly.
