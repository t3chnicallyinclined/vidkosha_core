# Contributing

Welcome! This repo stays off-chain and Rust-first by default. Please keep changes small and well-explained.

## Workflow
- Read `contexts/vision_board.md` and the relevant workstream context under `contexts/workstreams/` before coding.
- Summarize what you plan to change and why; open/annotate an issue or PR with that context.
- Run `cargo fmt` and `cargo clippy --all-targets --all-features` before/after `cargo build`/`cargo run`.
- Prefer incremental PRs; update `contexts/changelog.md` when landing meaningful changes.
- Follow SemVer: only bump `MAJOR` for breaking CLI/Helix contract changes, `MINOR` for backward-compatible features/query additions, and `PATCH` for fixes/docs. Keep `Cargo.toml` as the source of truth and tag releases on `main` (e.g., `vX.Y.Z`).
- Branches: develop on `feature/*`, merge to `staging` after review/CI, then fast-forward `staging` → `main` for releases. Protect `staging`/`main` with required checks.
- Start every feature from staging: `git switch staging && git pull`, then `git switch -c feature/<slug>`; open PRs into `staging` (owner/Code Owner approval required).

## Expanding Ideas
- You are encouraged to extend or adjust designs in the workstream docs. Document the **what/why**, call out assumptions, and mark anything risky.
- Keep defaults intact (e.g., chain features off unless explicitly enabled). Flag deviations clearly for review/approval.

## Verification
- Use the smokes where relevant: `cargo run -- helix-smoke`, `cargo run -- helix-rich-smoke`, `cargo run -- rag-smoke`.
- If touching routing or RAG behavior, sanity-check with the scenarios in `contexts/testing_semantic_and_neighbors.md`.

## Review Expectations
- Link the workstream/context you followed in the PR description.
- Note commands/tests run and any follow-up tasks.
