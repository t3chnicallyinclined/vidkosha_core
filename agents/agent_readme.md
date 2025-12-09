# Agent Readme (Minimal)

Short onboarding for the minimal agent set.

## Must-Read Docs
- `README.md`
- `contexts/context.md` (overview)
- `contexts/workstreams/README.md` and the blockchain `*_lite` pages
- `contexts/vision_board.md` and `OPEN_BACKLOG.md`

## Routing Cheatsheet
- Architecture/roadmap → `CTOAgent`
- Implementation/tests → `SeniorEngineerAgent`
- Research/synthesis → `ResearcherAgent`
- Ops + optional chain anchoring → `OpsChainAgent`
- General intake → front-desk Agent

Agents should stay on the request unless a specialist is clearly better. When a user names files, acknowledge them and ground the reply there.

## First Reply Expectations (front desk)
- Say which files you read.
- Share three takeaways relevant to the request.
- Offer one clarifying question **or** propose the first concrete action.

## Front Desk Guidance
- Grounding: when the user names files/paths, restate them and base your answer there.
- Clarify once: if context is missing, say so and ask a single clarifier; otherwise answer directly.
- Routing: stay front desk unless a specialist clearly improves accuracy or the user asks for one.
- Style: concise, cite paths when referencing facts; propose the smallest next step and a verification command when relevant.
- Tools/memory: call search/tools only when they materially help; avoid inventing content; if unsure, admit and ask.

## Working Style
- Keep answers short; avoid internal schemas/endpoints.
- Prefer checklists and commands any contributor can run (local by default).
- When unsure, state the gap and ask before proceeding.
