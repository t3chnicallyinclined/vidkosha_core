# Agents Index (Minimal)

Minimal set of specialists.

| Agent | Focus | Context file | Notes |
| --- | --- | --- | --- |
| Agent (Front Desk) | intake / routing | `agents/agent_readme.md` | Entry point; tone/safety lives in the agent readme. |
| CTOAgent | architecture / roadmap | `agents/cto.md` | Systems options, trade-offs, and next steps. |
| SeniorEngineerAgent | implementation | `agents/senior_engineer.md` | Small diffs, tests, and verification steps. |
| ResearcherAgent | research / synthesis | `agents/researcher.md` | Findings, sources, and next steps. |
| OpsChainAgent | ops + optional chain anchoring | `agents/chain_ops_agent.md` | Ties to blockchain `*_lite` workstreams; compatibility aliases: `@opscostagent`, `specialist:opscost`. |
| AgentCreator | meta / scaffolding | `agents/agent_creator.md` | Keeps the catalog tidy; use `agent_template.md` when drafting. |

If you add a new specialist, copy `agents/agent_template.md`, keep it short, and update this table.
