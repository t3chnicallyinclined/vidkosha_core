# Operator Prompt — Copilot/GPT Quickstart

Use this prompt (or skim it) before coding with Copilot/GPT so node operators and agents stay aligned.

## Ownership & Update Triggers

- **Owner:** `AgentCreator` maintains this prompt.
- **Backups:** `CTOAgent` (routing/architecture changes) and `SeniorEngineerAgent` (workflow/tooling changes).
- **Update when:** routing rules change, required checks/envs change, Helix write shapes change, or onboarding prompts change.
- **Mirror:** Keep `agents/agent_readme.md` and `README.md` consistent when updating.

## Workflow
- Read: `agents/agent_readme.md`, `contexts/context.md`, `contexts/roadmap.md`, latest `contexts/changelog.md`, and the relevant specialist brief.
- Also skim: `contexts/helix_schema.md` (node/edge expectations, embeddings) to stay schema- and prompt-aligned.
- Verify services: LLM `:8000`, embeddings `:9000`, Helix `:6969`. Then run `cargo run -- helix-smoke` (and `helix-rich-smoke` if editing rich writes).
- Route: follow the routing cheatsheet in `agent_readme.md`; default to front-desk Agent when unsure.
- Memory discipline: only RagAgent writes to Helix; include metadata (agent_name, topic, project, timestamp, summary, confidence, open_questions) and typed edges/perspectives on every write.
- Close the loop: update code/tests, then add a `contexts/changelog.md` entry with three `Next up` bullets; mirror any setup deltas into `README.md`.

## Quick checks
- `curl -s http://127.0.0.1:8000/v1/models | jq '.data[].id'` (LLM)
- `curl -s http://127.0.0.1:9000/v1/models | jq '.data[].id'` (embeddings)
- `curl -s http://127.0.0.1:6969/health` (Helix)
- `cargo run -- helix-smoke` (fast path) / `cargo run -- helix-rich-smoke` (rich path)

## Routing cheatsheet
- Architecture/roadmap → CTOAgent
- Rust/tests → SeniorEngineerAgent
- Research/synthesis → ResearcherAgent
- Infra/ops/anchoring → OpsChainAgent
- Memory/schema → RagAgent
- Unsure → front-desk Agent

## Helix write shape
- Required metadata: `agent_name`, `topic`, `project`, `timestamp`, `summary`, `confidence`, `open_questions`
- Recommended: perspectives (child nodes) + typed edges for relationships (e.g., `agent->produced->memory_entry`, `memory_entry->relates_to->project`)
- Include messages/artifacts/tool_calls when available; prefer versioned queries (e.g., `write_memory_v2`) once deployed

## Change hygiene
- No undocumented edits: every change gets a changelog entry with `What changed / Why / Next up (x3)`
- Keep `contexts/helix_schema.md` in sync with HelixQL updates and export scripts
- When adding agents, ship: Rust scaffolding + context file + registration/Helix metadata template
