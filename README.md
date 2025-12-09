# Vidkosha Cortex — powered by Nervos CKB

Sovereign, human-first AI OS: Rust agents that remember, route, and build with you; Helix fabric for memory; Nervos CKB is the default rail for anchoring/provenance, and node operators can host embedding, storage, or inference specialists to earn reputation- and quality-based rewards.
Stack highlights: Rust + Helix AI Fabric (vector graph, provenance-ready), Nervos CKB rail for anchoring/provenance and reputation/quality-based operator rewards across embedding/storage/inference, vLLM/OpenAI API.

Minimal notes for bringing the local stack online and exercising the CLI agent loop.

## Working Methodology

This repo runs on a deliberately ritualized flow so humans and agents stay in lockstep:

1. **Seed immersion** – Read in this order: `contexts/context.md` → `contexts/workstreams/README.md` → `OPEN_BACKLOG.md` → `contexts/vision_board.md` → `agents/agent_readme.md` (workflow + next-step menu) → the active workstream brief. After `agent_readme`, run `cargo run -- helix-smoke` as the default first verification so intent and constraints stay aligned before you touch code or memory.
2. **Plan and log** – Propose, execute, and summarize work in repo docs (`OPEN_BACKLOG.md` and, when present, a changelog entry) with three concrete `Next up` actions. If it is not logged, it did not happen.
3. **Tooling + memory discipline** – Specialists route through the orchestrator/router, call real tools (LLM, RAG, embeddings), and capture structured memories via the RAG writer instead of dumping raw transcripts.
4. **Tight feedback cadence** – Before coding, confirm the workflow against the active workstream and backlog item, then update code and the relevant docs before handing the baton to the next agent.
5. **README as the front desk** – Any time you change setup/config instructions, mirror the actionable bits here so the next human or agent can ramp by skimming this file first.

## Copilot Playbook (fast ramp for humans + agents)

Use this when pairing with GitHub Copilot / GPT on this repo:

1) Load context: follow the Seed Immersion order above (context → workstreams README → backlog/vision → agent_readme → specialist brief), then add `contexts/helix_schema.md` (schema). Front-desk tone and routing live in `agents/agent_readme.md`.
2) Verify services: ensure the local LLM, embeddings, and memory services respond (defaults: 8000, 9000, 6969); run `cargo run -- helix-smoke` after they’re up. Full bring-up commands live in `scripts/node-operator/README.md`.
3) Route by specialist: follow the routing cheatsheet in `agents/agent_readme.md`; default to front-desk Agent when unsure. To target a specialist directly, include a token in your prompt, e.g., `@ctoagent`, `@seniorengineeragent`, `@researcheragent`, `@opschainagent`, or `@ragagent` (aliases like `specialist:researcher` also work).
4) Memory discipline: only the RAG writer persists memories; include metadata (agent_name, topic, project, timestamp, summary, confidence, open_questions, edges/perspectives) on every write.
5) Inline memory ops: you can ask the front desk to `save ...`/`remember ...`/`store ...` to persist via the RAG writer, or `forget <id>` to delete. On save it returns the memory id plus inferred topic/categories and any `tag=`/`tags=` you provide (comma/space separated).
6) Close the loop: update code + tests, then log the change in `OPEN_BACKLOG.md` or a changelog entry with three `Next up` bullets and mirror any setup deltas back into this README.

For contribution expectations and how to propose expansions (what/why, defaults, review), see `CONTRIBUTING.md`.

### New dev quickstart (10 minutes)

1) Clone + env: `cp .env.example .env` and skim `contexts/context.md`, `contexts/workstreams/README.md`, and `OPEN_BACKLOG.md`.
2) Services: start Helix on `:6969` (see section 5 below) plus LLM on `:8000` and embeddings on `:9000` from the repo root.
3) Bootstrap check: `./scripts/node-operator/bootstrap.sh` (verifies LLM, embeddings, Helix `/introspect`, then runs `helix-smoke`).
4) Tests: `cargo test` (fast). For a CLI smoke without bootstrap, use `cargo run -- helix-smoke`.
5) First Copilot prompt: “Give me a fast orientation to this repo and the exact commands to bring up LLM, embeddings, Helix, and run `helix-smoke`.”

### Release & semver policy

- We use SemVer: `MAJOR` = breaking changes to the CLI/Helix contract; `MINOR` = new features/backward-compatible query additions; `PATCH` = fixes and docs.
- Version source of truth: `Cargo.toml`. Update it on staging, then merge to `main` and tag `vX.Y.Z`.
- Release checklist:
  1) Ensure `cargo build`, `cargo test`, `cargo clippy` all pass.
  2) Update `contexts/changelog.md` with the release entry and three `Next up` bullets.
  3) Set the version (`cargo set-version 0.1.0` if changing), commit, merge staging → main.
  4) Tag on `main`: `git tag -a v0.1.0 -m "Vidkosha Cortex 0.1.0" && git push origin v0.1.0`.
  5) Announce any required env/Helix query changes in README and changelog.

### Branch model (protected)

- `main`: always releasable; tagged releases are cut here.
- `staging`: integration branch for reviewed PRs; fast-forward into `main` when ready.
- `feature/*`: short-lived development branches that merge into `staging`.
- Protections: required checks (build/test/clippy), PR-only merges, and owner approval on `main`/`staging`. Tag releases from `main` only.

New contributor branch routine:
- `git switch staging && git pull` to sync.
- `git switch -c feature/<slug>` to start work from staging.
- Rebase on staging regularly; keep the branch short-lived.
- Push `feature/<slug>` and open a PR into `staging`; CI + owner approval gate the merge.

### Visibility

- This branch is sanitized for open sharing; keep strategy, pricing, and operator economics outside the repo.
- If you maintain an external pack, apply it out-of-repo after pulling this branch; do not reference unpublished endpoints, keys, or internal dates here.

## Node Operator bootstrap kit (fast start)

If you are bringing up a Helix node for the first time, skim `scripts/node-operator/README.md` for a compose file, env template, and smoke/seed scripts tailored to operators. When defaults change (LLM/embeddings model, vector dim, Helix write query, seed corpus), update the kit’s `.env.example`, `docker-compose.yml`, and `ingest_seed.sh`, then log it in `contexts/changelog.md` and mirror the delta here.


## 1. Prerequisites

- Python 3.12 virtual environment with dependencies from `requirements.lock` installed (no bundled wheels required).
- GPU with ~24 GB VRAM (tested on RTX 3090) and CUDA drivers compatible with vLLM 0.12.0.
- Hugging Face CLI logged into an account that can pull `meta-llama/Llama-3.1-8B-Instruct`.
- Rust toolchain installed via `rustup` (`cargo`, `rustfmt`, `clippy`).

## 2. Environment variables

Copy `.env.example` to `.env` and adjust if needed:

```bash
cp .env.example .env
```

These values set the model alias (`llama3-local`), the OpenAI-compatible base URL, and a dummy API key required by the SDK. Source the file (or use a manager such as `direnv`) before running the CLI.

### Helix AI Fabric variables

Add the following once HelixDB is running locally or remotely:

| Variable | Purpose |
| --- | --- |
| `HELIX_BASE_URL` | Base URL for the Helix REST API (e.g. `http://127.0.0.1:6969`). |
| `HELIX_API_TOKEN` | Bearer token for authenticated requests. |
| `HELIX_GRAPH_NAMESPACE` | Namespace/collection that stores Vidkosha Cortex knowledge (defaults to `vidkosha_cortex`). |
| `HELIX_HTTP_TIMEOUT_MS` | Optional timeout override for HTTP calls (defaults to 10 seconds). |

### Start HelixDB (AI Fabric) before the model/embeddings

Install the Helix CLI if you don’t have it yet:

```bash
curl -sSL "https://install.helix-db.com" | bash
```

Bring up Helix first so later steps can write/read memory. From the repo root:

```bash
cd helix-db
sudo apt-get install -y pkg-config libssl-dev   # first time only
helix build dev                                 # prepares .helix/dev
helix start dev                                 # serves Helix on :6969
```

Helix listens on port `6969`; once it is up, export the `HELIX_*` vars above (or source `.env`) and keep this terminal running. Full details and curl smoke examples remain in section 5 below.

Seed Topics (idempotent):

```bash
./scripts/seed_topics.sh               # uses contexts/category_seed_general.json; skips sentinels + existing
./scripts/seed_topics.sh --input contexts/category_seed.json   # legacy seed file
```

The `RAG_*` variables now exclusively configure the dedicated embeddings server that Helix calls until its native embedding service ships.

### Fail-fast startup

- The CLI now refuses to boot if the LLM client cannot be constructed (missing `OPENAI_*` / `VK_CORTEX_*` vars, unreachable vLLM server, etc.).
 Memory integration now expects **HelixDB** env vars. If they are not present the orchestrator logs a warning and runs without persistence. Use `cargo run -- helix-smoke` to confirm connectivity before attempting writes (this now drives insert/search/delete through the memory path so the vector flow is exercised and cleaned).

## 3. Start local services (summary)

- Default ports: LLM 8000, embeddings 9000, memory 6969. Full bring-up commands and docker-compose profiles live in `scripts/node-operator/README.md`.
- After services are up and `.env` is sourced, run `cargo run -- helix-smoke` to sanity check the memory path; use `helix-rich-smoke` only if you are touching rich payloads.

## 4. Start HelixDB (AI Fabric)

- HelixDB is vendored in `./helix-db`. Use the CLI/binary as documented in `scripts/node-operator/README.md`. Keep `HELIX_BASE_URL`, `HELIX_API_TOKEN`, `HELIX_GRAPH_NAMESPACE`, and `HELIX_HTTP_TIMEOUT_MS` in `.env`.
- Schema references stay in `contexts/helix_schema.md`. Query names and versions are managed in the Helix repo; refer to the node-operator doc for how to push/verify them without listing exact routes here.

## 5. Verify the memory pipeline

- Run `cargo run -- helix-smoke` after services start; it covers insert/search/delete against the memory backend. Use `cargo run -- rag-smoke` only when you need to exercise the richer RAG path.

## 7. Backup the Helix namespace

Use the helper script to snapshot the current namespace (defaults to incremental backups covering the last 24 hours):

```bash
./scripts/helix_backup.sh            # incremental since 24h ago
./scripts/helix_backup.sh full       # full export
./scripts/helix_backup.sh incremental 2025-12-01T00:00:00Z  # custom since timestamp
```

Backups land under `backups/helix/<UTC timestamp>/` and contain `export.ndjson.gz`, a SHA-256 checksum, and the `helix namespace verify` log. Override destinations with `HELIX_BACKUP_DIR`, and be sure the `helix` CLI is on your `PATH` before running the script.

## 8. Run the CLI against the local endpoint

Use a second terminal for development work:

```bash
source ~/.cargo/env           # adds cargo to PATH for the current shell
source .env                   # loads VK_CORTEX_LLM_MODEL / OPENAI_* vars
cargo run -- --prompt "Status check"
```

The CLI loads env vars via `dotenvy`, routes the prompt through the orchestrator/router, and issues the request to the local vLLM server using `async-openai`.

## 9. Linting

```bash
cargo fmt
cargo clippy --all-targets --all-features
```

Both commands are also available via the `Makefile` targets (`make fmt`, `make clippy`, `make lint`). Keep the workspace clean before committing.

## Repo ingest (index-repo)

- Run `cargo run -- index-repo --chunk-bytes 1200 --overlap-bytes 200 [--changed-since HEAD~1] [--no-llm-labels] [--binary-threshold 0.33] [--allow-binary]` to ingest git-tracked files with tree-sitter symbol chunking for Rust/TS/JS/TSX/Python.
- Respects `.gitignore` via `git ls-files` and the optional `.nervos_index_config.json` allow/deny lists (defaults allow code/docs, deny common binaries) plus `max_file_bytes` (flag overrides when unset).
- New `--changed-since <git ref>` filters candidates to `git diff --name-only <ref>`, and the manifest `.vidkosha_index_manifest.json` uses file hash + mtime to skip unchanged chunks while deduping identical chunk bodies by hash.
- Binary guard: files with NUL bytes or a non-printable ratio above `--binary-threshold` (default 0.33, overridable via `.nervos_index_config.json`) are skipped before UTF-8 decode; use `--allow-binary`/config to ingest anyway or extend the deny list if you store archives nearby.
- Handlers: code (symbol-first), markdown (heading-aware), data (CSV/JSON/JSONL row windows), plain text, and optional binary. Enable/disable or tune per-handler (`chunk_bytes`, `overlap_bytes`, `heading_depth`, `max_rows_per_chunk`) via `.nervos_index_config.json` (`handlers_disabled`, `handler_overrides`, `force_handlers`).

Example `.nervos_index_config.json`:

```json
{
  "allow_extensions": ["rs", "md", "toml", "json", "csv", "jsonl"],
  "deny_extensions": ["bin", "exe"],
  "max_file_bytes": 800000,
  "binary_threshold": 0.25,
  "allow_binary": false,
  "handlers_disabled": ["binary"],
  "handler_overrides": {
    "code": {"chunk_bytes": 1600, "overlap_bytes": 200},
    "markdown": {"heading_depth": 4},
    "data": {"max_rows_per_chunk": 100}
  },
  "force_handlers": {"sql": "text"}
}
```

## 10. Troubleshooting

- **404 on `/v1/models`** – make sure you launched `vllm.entrypoints.openai.api_server` (not the generic `api_server`).
- **`cargo` not found** – install Rust via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y`, then `source ~/.cargo/env`.
- **LLM errors mentioning `llama-3-8b-instruct`** – verify `VK_CORTEX_LLM_MODEL=llama3-local` matches the `--served-model-name` flag.
- **Docker compose errors (`unknown flag --project-name` or compose missing)** – install Docker Compose v2 (`docker compose version` should work) or upgrade by placing the v2 binary under `~/.docker/cli-plugins/docker-compose`. If the Helix CLI claims Docker isn’t running while `docker ps` works under `sudo`, add your user to the docker group (`sudo usermod -aG docker $USER && newgrp docker`) or run the Helix commands with `sudo -E` as a fallback.

## 12. Node Operator Bootstrap (ready for operators)

For operators who clone this repo to join the fabric:

- Copy `scripts/node-operator/.env.example` to `.env` and set `HELIX_API_TOKEN`, `HELIX_BASE_URL`, `RAG_EMBEDDING_BASE_URL`, and LLM values. Skim `contexts/helix_schema.md` (node/edge/embedding expectations) so your node aligns with the schema and prompt discipline.
- Optional services: `docker compose -f scripts/node-operator/docker-compose.yml up -d embeddings` (CPU-friendly). Enable `--profile helix` or `--profile llm` only if you have a trusted Helix image and GPU LLM weights.
- Run `./scripts/node-operator/bootstrap.sh` (set `RUN_RICH=1` to also exercise rich writes). This checks endpoints and runs `helix-smoke`.
- Seed the fabric with starter memories: `./scripts/node-operator/ingest_seed.sh`.
- For Copilot/GPT usage, hand operators `contexts/operator_prompt.md` and the Copilot Playbook above.

## 11. Decentralized Node Operator Vision (Phase 5)

Vidkosha Cortex is designed to evolve into a decentralized intelligence network where DAO members and token holders can operate nodes that contribute to the ecosystem. This transforms participants from passive users into **intelligence providers**.

### Node Classes

| Node Type | Purpose | Hardware |
| --- | --- | --- |
| **Embedding Nodes** | Text → vector processing | Mid-tier GPU (8GB+ VRAM) |
| **Agent Hosting Nodes** | Run specialist agents | GPU (24GB+ VRAM) |
| **Data Nodes** | Store RAG shards | 100GB+ SSD |
| **Evaluator Nodes** | Quality scoring | Standard compute |
| **Training Nodes** | LoRA/fine-tuning | High-end GPU (3090/4090+) |
| **Workflow Nodes** | Multi-agent pipelines | Variable by pipeline |
| **Governance Nodes** | On-chain operations | Standard compute |

### Key Concepts

- **Operator-Owned Agents:** Node operators can host their own specialist agents with their own context files, creating personal RAG shards ("micro-brains") that attach to the Mega Brain.
- **Reputation System:** Operators earn reputation (not money) based on response quality, uptime, RAG contributions, and community feedback. Higher reputation unlocks priority routing and governance influence.
- **Query Routing:** The orchestrator routes specialized queries to the highest-reputation matching operator nodes, creating natural expertise zones across the network.

### Documentation

- Full vision: `contexts/context.md` Section 6 (Node Operator Classes)
- Roadmap highlights: use the workstream lite pages in `contexts/workstreams/`
- Participation roles: summarized in `contexts/context.md`; keep evaluation details out-of-repo

This architecture transforms Vidkosha Cortex into:
- A **decentralized, multi-specialist intelligence environment** where intelligence grows from many contributors
- A **knowledge commons** curated by people, maintained by nodes, accessed by agents
- A **leveling-up mechanism** where participants specialize where they are strongest
