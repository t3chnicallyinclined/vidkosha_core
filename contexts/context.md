# Vidkosha Cortex (Overview)

A lightweight summary of the project. This repo anchors on Nervos CKB for provenance/settlement, with lite workstreams as starter slices for contributors.

## What this is
- Rust orchestrator with a front-desk Agent and a few specialists (architecture, implementation, research, ops/CKB).
- Nervos CKB is the default rail for anchoring/provenance/payouts; local-first development is supported without posting on-chain.
- Memory layer details are intentionally minimal here; contributors should not assume any non-Helix backend.

## Principles
- Keep it simple for new contributors; grow capabilities as contributions arrive.
- Default to Nervos CKB for provenance and settlement; allow offline/local dev runs without chain writes.
- Stay factual and lean; omit internal schemas, tokens, or operator economics.
- Prefer small, verifiable steps over grand designs.

## Current surface
- Workstreams: see `contexts/workstreams/README.md` and the `*_lite` pages (e.g., `ckb_anchor_lite.md`, `operator_registry_lite.md`, `node_operator_lite.md`, `ckb_indexing_lite.md` as the CKB indexer, `treasury_policy_lite.md`) as entry points to the default CKB rail.
- Backlog & vision: `OPEN_BACKLOG.md`, `contexts/vision_board.md`.
- Agents: minimal set in `agents/` (front desk, CTO, SeniorEngineer, Researcher, Ops+Chain, AgentCreator template).

## What is intentionally excluded here
- Detailed memory/Helix schemas and ops guides.
- DAO/treasury/operator marketplace specifics beyond the lite workstreams.
- Proprietary evaluation, routing, or prompt scaffolding details.

## How to contribute
- Pick a `*_lite` workstream issue or a backlog item marked good-first.
- Keep changes small and cite file paths in PR descriptions.
- If you need deeper system details, open an issue and we can decide what to expose next.

## Safe defaults
- Assume local development can run without posting to chain, but align schema and flows to the CKB rail.
- Do not hard-code or publish secrets, endpoints, or internal schemas.
- When in doubt, ask before adding new surface area.
